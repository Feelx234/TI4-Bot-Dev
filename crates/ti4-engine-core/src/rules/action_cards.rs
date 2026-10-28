//! Action cards (package B6; LRR 2, 22): the reads of `action_cards::{hand_limit, is_component_action,
//! is_playable}` that the core can answer from its own state. This is slice 1 of the package: reads only.
//! What needs another area's string rules (payment, research, the unit catalogue, neighbours) is not
//! asked here and the string function stands; each such place is a documented bail (see
//! [`has_legal_target`]). No `Env` trait: the faction answers are plain arguments.

use crate::generated::{ActionCard, System, Unit, UnitBaseType};
use crate::query::board;
use crate::state::Piece;
use crate::tables::Scope;
use crate::ids::Seat;
use crate::query::laws;
use crate::state::CoreState;

/// `action_cards::HAND_LIMIT`: the printed limit (LRR 2.4).
pub const HAND_LIMIT: usize = 7;

/// `action_cards::is_component_action`: the printed window is "Action".
#[must_use]
pub fn is_component_action(card: ActionCard) -> bool {
    card.data().component_action
}

/// `action_cards::hand_limit`: the law cap of Sanctions, then the faction override (`faction`, the
/// string module's hook, applied last as it overrides the law).
#[must_use]
pub fn hand_limit(core: &CoreState, blanked: bool, faction: impl FnOnce(usize) -> usize) -> usize {
    faction(laws::action_card_limit(core, blanked, HAND_LIMIT))
}

/// The seat-independent gates at the head of `action_cards::is_playable`: Political Censure's owner, the
/// faction bar and Political Secret's target cannot play action cards. `false` bars the play.
#[must_use]
pub fn may_play(core: &CoreState, blanked: bool, who: Seat, faction_forbids: bool, political_secret_bars: bool) -> bool {
    !(laws::censure_bars(core, blanked, who) || faction_forbids || political_secret_bars)
}

/// `action_cards::has_legal_target` for the cards whose target list is a plain read of the core:
/// `Some(answer)`, or `None` for the arms that need string-only rules (the bail: the string function
/// answers). A card without a requirement answers `Some(true)`.
///
/// Bails (`None`): Meltdown, Cripple Defenses, Lucky Shot, Plague, Uprising, Seize Artifact, Messiah and
/// Frontline Deployment, War Effort, Tactical Bombardment, Unstable Planet, Scuttle (planet catalogue,
/// unit catalogue, neighbours or the controlled-spot rules).
#[must_use]
pub fn has_legal_target(core: &CoreState, blanked: bool, who: Seat, card: ActionCard) -> Option<bool> {
    let rival = |keep: &dyn Fn(&crate::state::Player) -> bool| core.players().iter().any(|p| p.seat != who && keep(p));
    use ActionCard as C;
    match card {
        C::Insubordination => Some(rival(&|p| p.econ.tactic_tokens > 0)),
        C::Spy => Some(rival(&|p| !p.cards.hand.is_empty())),
        C::RepealLaw => Some(!laws::in_play(core, blanked).is_empty()),
        C::UnexpectedAction => Some(!crate::query::board::systems_with_token(core, who).is_empty()),
        C::ReactorMeltdown
        | C::CrippleDefenses
        | C::LuckyShot
        | C::Plague
        | C::Uprising
        | C::SeizeArtifact
        | C::RiseOfAMessiah
        | C::FrontlineDeployment
        | C::WarEffort
        | C::TacticalBombardment
        | C::UnstablePlanet
        | C::Scuttle => None,
        _ => Some(true),
    }
}

/// What `reaction_can_matter` reads of the window's event: who acted and in which system.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub system: System,
    /// The event's `player` as a seat: `None` when the event names nobody (or nobody seated).
    pub actor: Option<Seat>,
}

fn pieces_in(core: &CoreState, system: System) -> impl Iterator<Item = Piece> + '_ {
    let space = board::space_units(core, system).iter().copied();
    let ground = board::planets_of(core, system).flat_map(move |p| board::planet_units(core, p).iter().copied());
    space.chain(ground)
}

/// `action_cards::reaction_can_matter` for a window that names a system, for the cards whose extra
/// requirement is a read of that system's pieces: `Some(answer)`, or `None` (bail: Harness Energy
/// needs the tile catalogue and the rift hook, Infiltrate needs the swap rules, and a card without a
/// requirement is `Some(true)`). The printed-cost check comes first at the call site.
#[must_use]
pub fn reaction_can_matter(core: &CoreState, scope: Scope, who: Seat, card: ActionCard, w: Window) -> Option<bool> {
    use ActionCard as C;
    let is_actor = w.actor == Some(who);
    let base = |u: Unit| scope.unit(u).map(|d| d.base_type);
    let ship = |u: Unit| scope.unit(u).is_some_and(|d| d.ship);
    let fighter = |u: Unit| base(u) == Some(UnitBaseType::Fighter);
    let space = board::space_units(core, w.system);
    let own = || space.iter().filter(|p| p.owner() == who);
    let own_all = || pieces_in(core, w.system).filter(|p| p.owner() == who);
    let has_own_ship = own().any(|p| ship(p.kind()));
    Some(match card {
        C::Bunker => w.actor.is_some() && !is_actor && board::controls_a_planet_in(core, w.system, who),
        C::Blitz => {
            is_actor
                && own().any(|p| {
                    let u = p.kind();
                    ship(u) && !fighter(u) && scope.unit(u).is_some_and(|d| d.bombardment.is_none())
                })
        }
        C::Disable => {
            is_actor
                && pieces_in(core, w.system).any(|p| p.owner() != who && base(p.kind()) == Some(UnitBaseType::Pds))
        }
        C::FighterPrototype => own_all().any(|p| fighter(p.kind())),
        C::Upgrade => own_all().any(|p| base(p.kind()) == Some(UnitBaseType::Cruiser)),
        C::Rally => space.iter().any(|p| p.owner() != who && ship(p.kind())),
        C::Rescue | C::Intercept => !is_actor && has_own_ship,
        C::EmergencyRepairs => pieces_in(core, w.system).any(|p| p.owner() == who && p.sustained()),
        C::HarnessEnergy | C::Infiltrate => return None,
        _ => true,
    })
}

/// `action_cards::cost_is_payable` (rules version 23) for the cards whose bill the core can judge. `goods`
/// is the seat's spendable trade goods, the Keleres agent's commodities included (the string payment
/// rules supply it until they land). `Some(true)` for a card without a printed cost; `None` (bail) for
/// the cards whose bill needs a payment or research rule not yet in the core: Focused Research,
/// Impersonation, Plagiarize, Mercenary Contract, Pirate Fleet and Reveal Prototype.
#[must_use]
pub fn cost_is_payable(core: &CoreState, who: Seat, card: ActionCard, goods: i64) -> Option<bool> {
    use ActionCard as C;
    match card {
        C::FocusedResearch
        | C::Impersonation
        | C::Plagiarize
        | C::MercenaryContract
        | C::PirateFleet
        | C::RevealPrototype => None,
        C::DivertFunding => Some(
            crate::query::tech::owned(core, who)
                .iter()
                .any(|t| !crate::query::tech::is_unit_upgrade(t) && t.data().faction.is_none()),
        ),
        C::Bribery => Some(goods >= 1),
        _ => Some(true),
    }
}

/// The played effects that only set a stamp or a flag (the mutation half of the package, first batch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Simple {
    /// "+1 to each combat roll during this combat round": `combat_bonus_round`.
    MoraleBoost,
    /// "+1 to move value during this tactical action": `move_bonus_activation`.
    FlankSpeed,
    /// "other players cannot use SPACE CANNON against your ships": the activation joins the `SolarFlare` marks.
    SolarFlare,
    /// `STRATEGIC_CANCELLED`.
    Coup,
    /// `SKIP_NEXT_TURN`.
    Crisis,
    /// `ADDITIONAL_ACTION`.
    MasterPlan,
}

impl Simple {
    /// The effect a card plays, `None` for a card whose effect is not one of these (the bail: every other
    /// card keeps its string effect).
    #[must_use]
    pub const fn of(card: ActionCard) -> Option<Self> {
        use ActionCard as C;
        Some(match card {
            C::MoraleBoost1 | C::MoraleBoost2 | C::MoraleBoost3 | C::MoraleBoost4 => Self::MoraleBoost,
            C::FlankSpeed1 | C::FlankSpeed2 | C::FlankSpeed3 | C::FlankSpeed4 => Self::FlankSpeed,
            C::SolarFlare => Self::SolarFlare,
            C::CoupDEtat => Self::Coup,
            C::Crisis => Self::Crisis,
            C::MasterPlan => Self::MasterPlan,
            _ => return None,
        })
    }
}

/// Play a [`Simple`] effect for `who`.
pub fn play_simple(core: &mut CoreState, who: Seat, effect: Simple) {
    use crate::state::{RoundKind, TransientFlags};
    use crate::containers::Opt32;
    let (round, activation) = (core.meta().combat_round_seq, core.meta().activation_seq);
    match effect {
        Simple::Coup => core.transient_mut().misc.flags.set(TransientFlags::STRATEGIC_CANCELLED),
        Simple::Crisis => core.transient_mut().misc.flags.set(TransientFlags::SKIP_NEXT_TURN),
        Simple::MasterPlan => core.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION),
        Simple::MoraleBoost => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.combat_timers.combat_bonus_round = Opt32::some(round);
            }
        }
        Simple::FlankSpeed => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.timers.move_bonus_activation = Opt32::some(activation);
            }
        }
        Simple::SolarFlare => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.round_marks.push(RoundKind::SolarFlare, activation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Idx;

    #[test]
    fn component_action_is_the_printed_window() {
        assert!(is_component_action(ActionCard::Spy));
        assert!(!is_component_action(ActionCard::Blitz));
    }

    /// One test for every documented bail: the core declines, the string function answers.
    #[test]
    fn every_bail_declines_and_the_rest_answers() {
        let core = CoreState::default();
        let who = Seat::from_index(0).unwrap();
        for card in [
            ActionCard::ReactorMeltdown,
            ActionCard::CrippleDefenses,
            ActionCard::LuckyShot,
            ActionCard::Plague,
            ActionCard::Uprising,
            ActionCard::SeizeArtifact,
            ActionCard::RiseOfAMessiah,
            ActionCard::FrontlineDeployment,
            ActionCard::WarEffort,
            ActionCard::TacticalBombardment,
            ActionCard::UnstablePlanet,
            ActionCard::Scuttle,
        ] {
            assert_eq!(has_legal_target(&core, false, who, card), None, "{card:?}");
        }
        for card in [
            ActionCard::FocusedResearch,
            ActionCard::Impersonation,
            ActionCard::Plagiarize,
            ActionCard::MercenaryContract,
            ActionCard::PirateFleet,
            ActionCard::RevealPrototype,
        ] {
            assert_eq!(cost_is_payable(&core, who, card, 9), None, "{card:?}");
        }
        assert_eq!(cost_is_payable(&core, who, ActionCard::Bribery, 0), Some(false));
        assert_eq!(cost_is_payable(&core, who, ActionCard::Bribery, 1), Some(true));
        assert_eq!(cost_is_payable(&core, who, ActionCard::Blitz, 0), Some(true));
        let scope = Scope(u32::MAX);
        let w = Window { system: System::ALL[0], actor: None };
        for card in [ActionCard::HarnessEnergy, ActionCard::Infiltrate] {
            assert_eq!(reaction_can_matter(&core, scope, who, card, w), None, "{card:?}");
        }
        assert_eq!(has_legal_target(&core, false, who, ActionCard::Blitz), Some(true));
        assert_eq!(has_legal_target(&core, false, who, ActionCard::Spy), Some(false), "no rival");
    }
}
