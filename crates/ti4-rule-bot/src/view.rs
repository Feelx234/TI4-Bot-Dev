//! `View`: the position as the rule bot reads it (docs/todo/D3-typed-bot.md section 2.2, lane V0).
//!
//! Both traits are object safe and the bot takes `&dyn View` / `&dyn SeatView`: one copy of the code for every source (no
//! monomorphisation, which matters for the wasm budget), and registries of planners stay `Box<dyn ..>`.
//!
//! The bot reads a position through about forty accessors of [`Observed`] (public facts) and [`SeatObservation`] (the bound seat's
//! hand). The two traits here list exactly those, with the signatures of the string side, so that one function body can run against
//! more than one source of the same position: the string `Observed` (this crate, the oracle) and the typed core's `Seen` (the
//! adapter `SeenView` of `ti4-rule-bot-typed`). A function that is generic over [`View`] returns what it returns for every source
//! that answers the accessors alike; the differential tests of `ti4-rule-bot-typed` check that, accessor by accessor and decision
//! by decision.
//!
//! Rules of this module:
//! * The string side is a pure delegation (`Observed::round(self)` and so on, the inherent method wins over the trait method of the
//!   same name), so the generic code monomorphised for [`Observed`] is the code it was before.
//! * Borrowed answers are tied to `&self`, not to the position's `'a`: another source may own the data it lends.
//! * No accessor is added that the bot does not call. A method the bot starts to call is added here, to both impls and to the
//!   differential test in the same commit.

use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::{Observed, PublicSeat, SeatObservation};
use ti4_engine::objectives::{CardProgress, Imagined};
use ti4_engine::production::Spend;
use ti4_engine::tactical::Movable;
use ti4_engine::transit::Cargo;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{ActionCardId, ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, StrategyCardId, SystemId};
use ti4_model::state::{Phase, SystemState};

/// The invasion in progress, as far as the bot reads it: where, who invades and who defends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvasionFacts {
    /// The system being invaded.
    pub system: SystemId,
    /// The invading player.
    pub invader: PlayerId,
    /// The defending player, once there is one.
    pub defender: Option<PlayerId>,
}

/// The public facts of a position: what [`Observed`] answers.
pub trait View {
    /// The content tables of the game.
    fn content(&self) -> &ContentStore;
    /// The expansions in play.
    fn sources(&self) -> SourceSet;
    /// The map, when the caller has one.
    fn galaxy(&self) -> Option<&Galaxy>;
    /// The round number.
    fn round(&self) -> u32;
    /// The phase.
    fn phase(&self) -> Phase;
    /// The active player, in the action phase.
    fn active_player(&self) -> Option<&PlayerId>;
    /// The speaker.
    fn speaker(&self) -> &PlayerId;
    /// Whether the Custodians token has left Mecatol Rex.
    fn custodians_removed(&self) -> bool;
    /// The laws in play: agenda alias to outcome text.
    fn laws(&self) -> &BTreeMap<String, String>;
    /// The outcome text of one law in play.
    fn law_outcome(&self, alias: &str) -> Option<&str>;
    /// The seats, in seating order.
    fn players(&self) -> Vec<&PlayerId>;
    /// A seat's public sheet.
    fn seat(&self, player: &PlayerId) -> Option<PublicSeat<'_>>;
    /// The touched systems of the board.
    fn board(&self) -> &BTreeMap<SystemId, SystemState>;
    /// One system (an untouched one is empty).
    fn system(&self, system: &SystemId) -> SystemState;
    /// The system activated in the current tactical action.
    fn active_system(&self) -> Option<&SystemId>;
    /// The planets a player controls, with their systems.
    fn controlled_planets(&self, player: &PlayerId) -> Vec<(&SystemId, &PlanetId)>;
    /// Whether a planet card is ready.
    fn planet_is_ready(&self, planet: &PlanetId) -> bool;
    /// Systems that hold a command token of the player.
    fn systems_with_token(&self, player: &PlayerId) -> BTreeSet<&SystemId>;
    /// Whether a ship with this movement can reach the destination.
    fn can_reach(&self, player: &PlayerId, origin: &SystemId, destination: &SystemId, move_value: i32) -> bool;
    /// Every ship of the player that could reach the destination.
    fn movable_into(&self, player: &PlayerId, destination: &SystemId) -> Vec<Movable>;
    /// Gravity rifts a ship would exit on its way.
    fn rift_exits_into(&self, player: &PlayerId, origin: &SystemId, unit_type: &str, boosts: (bool, bool), destination: &SystemId) -> usize;
    /// Units a ship leaving `origin` could carry.
    fn loadable(&self, player: &PlayerId, origin: &SystemId) -> Vec<Cargo>;
    /// Resources or influence the player can spend.
    fn available_spend(&self, player: &PlayerId, kind: Spend) -> i64;
    /// The agenda being voted on.
    fn agenda_in_play(&self) -> Option<&str>;
    /// Agendas resolved in the current round.
    fn agendas_resolved_this_round(&self) -> usize;
    /// Whether the seat predicted an outcome on the agenda in play with a rider (the text of the prediction is not read).
    fn has_agenda_prediction(&self, player: &PlayerId) -> bool;
    /// The invasion in progress (the parts the bot reads).
    fn invasion(&self) -> Option<InvasionFacts>;
    /// The space battle in progress: system, attacker, defender.
    fn space_battle(&self) -> Option<(SystemId, PlayerId, PlayerId)>;
    /// Promissory notes in play: note key to holder.
    fn promissory_notes(&self) -> BTreeMap<String, PlayerId>;
    /// Goods on a strategy card.
    fn strategy_card_goods(&self, card: &StrategyCardId) -> i32;
    /// The objectives a player has scored.
    fn scored_by(&self, player: &PlayerId) -> BTreeSet<ObjectiveId>;
    /// How many revealed public objectives the player could score now.
    fn scoreable_public(&self, player: &PlayerId) -> usize;
    /// Progress on the revealed objectives if `imagined` happened first.
    fn revealed_objective_progress_imagining(&self, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress>;
    /// Revealed objectives whose fixed map requirement includes the system.
    fn objectives_implicating_system(&self, player: &PlayerId, system: &SystemId) -> Vec<ObjectiveId>;
}

/// A position bound to one seat: [`View`] plus that seat's own hand ([`SeatObservation`]).
pub trait SeatView: View {
    /// The public position this view is bound to.
    fn observed(&self) -> &dyn View;
    /// The seat this view answers for.
    fn bound_seat(&self) -> &PlayerId;
    /// The secret objectives the bound seat holds.
    fn held_secrets(&self) -> Vec<SecretObjectiveId>;
    /// The action cards the bound seat holds.
    fn held_action_cards(&self) -> Vec<ActionCardId>;
    /// Progress on the bound seat's secrets.
    fn held_secret_progress(&self) -> Vec<CardProgress>;
    /// Progress on the bound seat's secrets if `imagined` happened first.
    fn held_secret_progress_imagining(&self, imagined: &Imagined<'_>) -> Vec<CardProgress>;
}

/// Forward the accessors of [`View`] to the inherent methods of the string `Observed` (or of a bound view, through the position it holds).
macro_rules! forward_view {
    ($ty:ty, $this:ident => $obs:expr) => {
        impl View for $ty {
            fn content(&self) -> &ContentStore {
                Observed::content({ let $this = self; $obs })
            }
            fn sources(&self) -> SourceSet {
                Observed::sources({ let $this = self; $obs })
            }
            fn galaxy(&self) -> Option<&Galaxy> {
                Observed::galaxy({ let $this = self; $obs })
            }
            fn round(&self) -> u32 {
                Observed::round({ let $this = self; $obs })
            }
            fn phase(&self) -> Phase {
                Observed::phase({ let $this = self; $obs })
            }
            fn active_player(&self) -> Option<&PlayerId> {
                Observed::active_player({ let $this = self; $obs })
            }
            fn speaker(&self) -> &PlayerId {
                Observed::speaker({ let $this = self; $obs })
            }
            fn custodians_removed(&self) -> bool {
                Observed::custodians_removed({ let $this = self; $obs })
            }
            fn laws(&self) -> &BTreeMap<String, String> {
                Observed::laws({ let $this = self; $obs })
            }
            fn law_outcome(&self, alias: &str) -> Option<&str> {
                Observed::law_outcome({ let $this = self; $obs }, alias)
            }
            fn players(&self) -> Vec<&PlayerId> {
                Observed::players({ let $this = self; $obs })
            }
            fn seat(&self, player: &PlayerId) -> Option<PublicSeat<'_>> {
                Observed::seat({ let $this = self; $obs }, player)
            }
            fn board(&self) -> &BTreeMap<SystemId, SystemState> {
                Observed::board({ let $this = self; $obs })
            }
            fn system(&self, system: &SystemId) -> SystemState {
                Observed::system({ let $this = self; $obs }, system)
            }
            fn active_system(&self) -> Option<&SystemId> {
                Observed::active_system({ let $this = self; $obs })
            }
            fn controlled_planets(&self, player: &PlayerId) -> Vec<(&SystemId, &PlanetId)> {
                Observed::controlled_planets({ let $this = self; $obs }, player)
            }
            fn planet_is_ready(&self, planet: &PlanetId) -> bool {
                Observed::planet_is_ready({ let $this = self; $obs }, planet)
            }
            fn systems_with_token(&self, player: &PlayerId) -> BTreeSet<&SystemId> {
                Observed::systems_with_token({ let $this = self; $obs }, player)
            }
            fn can_reach(&self, player: &PlayerId, origin: &SystemId, destination: &SystemId, move_value: i32) -> bool {
                Observed::can_reach({ let $this = self; $obs }, player, origin, destination, move_value)
            }
            fn movable_into(&self, player: &PlayerId, destination: &SystemId) -> Vec<Movable> {
                Observed::movable_into({ let $this = self; $obs }, player, destination)
            }
            fn rift_exits_into(
                &self,
                player: &PlayerId,
                origin: &SystemId,
                unit_type: &str,
                boosts: (bool, bool),
                destination: &SystemId,
            ) -> usize {
                Observed::rift_exits_into({ let $this = self; $obs }, player, origin, unit_type, boosts, destination)
            }
            fn loadable(&self, player: &PlayerId, origin: &SystemId) -> Vec<Cargo> {
                Observed::loadable({ let $this = self; $obs }, player, origin)
            }
            fn available_spend(&self, player: &PlayerId, kind: Spend) -> i64 {
                Observed::available_spend({ let $this = self; $obs }, player, kind)
            }
            fn agenda_in_play(&self) -> Option<&str> {
                Observed::agenda_in_play({ let $this = self; $obs })
            }
            fn agendas_resolved_this_round(&self) -> usize {
                Observed::agendas_resolved_this_round({ let $this = self; $obs })
            }
            fn has_agenda_prediction(&self, player: &PlayerId) -> bool {
                Observed::agenda_prediction_of({ let $this = self; $obs }, player).is_some()
            }
            fn invasion(&self) -> Option<InvasionFacts> {
                Observed::invasion({ let $this = self; $obs }).map(|i| InvasionFacts { system: i.system, invader: i.invader, defender: i.defender })
            }
            fn space_battle(&self) -> Option<(SystemId, PlayerId, PlayerId)> {
                Observed::space_battle({ let $this = self; $obs })
            }
            fn promissory_notes(&self) -> BTreeMap<String, PlayerId> {
                Observed::promissory_notes({ let $this = self; $obs })
            }
            fn strategy_card_goods(&self, card: &StrategyCardId) -> i32 {
                Observed::strategy_card_goods({ let $this = self; $obs }, card)
            }
            fn scored_by(&self, player: &PlayerId) -> BTreeSet<ObjectiveId> {
                Observed::scored_by({ let $this = self; $obs }, player)
            }
            fn scoreable_public(&self, player: &PlayerId) -> usize {
                Observed::scoreable_public({ let $this = self; $obs }, player)
            }
            fn revealed_objective_progress_imagining(&self, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress> {
                Observed::revealed_objective_progress_imagining({ let $this = self; $obs }, player, imagined)
            }
            fn objectives_implicating_system(&self, player: &PlayerId, system: &SystemId) -> Vec<ObjectiveId> {
                Observed::objectives_implicating_system({ let $this = self; $obs }, player, system)
            }
        }
    };
}

forward_view!(Observed<'_>, this => this);
forward_view!(SeatObservation<'_>, this => this.observed());

impl SeatView for SeatObservation<'_> {
    fn observed(&self) -> &dyn View {
        self
    }
    fn bound_seat(&self) -> &PlayerId {
        SeatObservation::bound_seat(self)
    }
    fn held_secrets(&self) -> Vec<SecretObjectiveId> {
        SeatObservation::held_secrets(self)
    }
    fn held_action_cards(&self) -> Vec<ActionCardId> {
        SeatObservation::held_action_cards(self)
    }
    fn held_secret_progress(&self) -> Vec<CardProgress> {
        SeatObservation::held_secret_progress(self)
    }
    fn held_secret_progress_imagining(&self, imagined: &Imagined<'_>) -> Vec<CardProgress> {
        SeatObservation::held_secret_progress_imagining(self, imagined)
    }
}
