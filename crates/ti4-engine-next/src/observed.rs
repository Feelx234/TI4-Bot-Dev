//! What a decider may see (8b3 of docs/todo/D2-8-wrapper.md): the public position beside a choice, and the view bound to one seat.
//!
//! **Only public facts are reachable through [`Observed`].** The state is held privately and every accessor answers something any player
//! at the table may read: the board, who controls what, how many cards somebody holds. A hand's *contents* are not reachable here at all:
//! they live on the engine-bound [`SeatObservation`], which answers for its bound seat only; no method here takes a caller-chosen seat and
//! returns private data, and no method on either type produces a `GameState`.
//!
//! The type is a copy of the legacy `choice::Observed` (the string rule bot reads it until D3). The accessors that only read the
//! `GameState` are copied verbatim; the ones that ask a rule module (movement, objectives, production, fleet, the faction hooks) are
//! **delegated to a legacy `Observed` over the same position** (`Self::legacy`, built with the value) until lane B's native queries land
//! (docs/D28A_RESUME.md lists them; each is a one-line delegate marked in this file). The delegates return the legacy result types.

use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{BreakthroughId, LeaderId, ObjectiveId, PlanetId, PlayerId, RelicId, SecretObjectiveId, SystemId};
use ti4_model::state::{GameState, LeaderStatus, SystemState};

use crate::wire::choice::{Choice, ChoiceOption, Decider, IllegalChoice, validate};

/// The public position, offered to a decider alongside the choice.
///
/// A choice on its own is not enough to play well. "Activate a system" lists ids; whether one of them is worth a command token depends on
/// what is in it, what defends it, and whether anything of yours can reach it: facts about the board, not about the choice.
///
/// **Only public facts are reachable through this type** (see the module docs).
pub struct Observed<'a> {
    pub(crate) state: &'a GameState,
    pub(crate) content: &'a ContentStore,
    pub(crate) sources: SourceSet,
    pub(crate) galaxy: Option<&'a Galaxy>,
    /// The legacy view of the same position, for the accessors that still ask a legacy rule module (see the module docs). Built with the
    /// value (a few empty caches; the legacy type has no interior-mutable field that names `'a`, so `Observed` stays covariant, which
    /// `SeatObservation::bind` needs).
    legacy: ti4_engine_legacy::choice::Observed<'a>,
    /// The typed position and the answers memoised for it (movement questions), built on the first typed question.
    seats: std::cell::RefCell<crate::queries::adapter::SeatCache>,
}

impl<'a> Observed<'a> {
    /// Wrap a position. Public so tests and sibling crates can build one.
    #[must_use]
    pub fn new(state: &'a GameState, content: &'a ContentStore, sources: SourceSet, galaxy: Option<&'a Galaxy>) -> Self {
        Self { state, content, sources, galaxy, legacy: ti4_engine_legacy::choice::Observed::new(state, content, sources, galaxy), seats: std::cell::RefCell::new(crate::queries::adapter::SeatCache::new()) }
    }

    /// The legacy view of this position (its memoised answers are shared by the delegates).
    const fn legacy(&self) -> &ti4_engine_legacy::choice::Observed<'a> {
        &self.legacy
    }

    /// The seat's public sheet. (Native; the Mahact's fleet pool also holds other players' tokens (Edict), in public view, which the
    /// legacy faction module counts.)
    #[must_use]
    pub fn seat(&self, player: &PlayerId) -> Option<PublicSeat<'a>> {
        ti4_model::count::bump("Observed::seat");
        self.state.player(player).map(|seat| PublicSeat {
            faction: &seat.faction,
            victory_points: seat.victory_points,
            trade_goods: seat.trade_goods,
            commodities: seat.commodities,
            tactic_tokens: seat.tactic_tokens,
            fleet_tokens: seat.fleet_tokens + crate::queries::factions::mahact_foreign_tokens(self.state, player),
            strategic_tokens: seat.strategic_tokens,
            strategy_cards: &seat.strategy_cards,
            exhausted_strategy_cards: &seat.exhausted_strategy_cards,
            technologies: &seat.technologies,
            exhausted_technologies: &seat.exhausted_technologies,
            action_cards_held: seat.action_cards.len(),
            secret_objectives_held: seat.secret_objectives.len(),
            passed: seat.passed,
            relics: &seat.relics,
            exhausted_relics: &seat.exhausted_relics,
            exploration_cards: &seat.exploration_cards,
            relic_fragments: &seat.relic_fragments,
            breakthrough: seat.breakthrough.as_ref(),
            leaders: &seat.leaders,
        })
    }

    /// The position, content, source scope and map for the crate's own loaders (`seen_load`, the parity tools).
    /// Not public: the doc of [`Observed`] promises that no method hands a decider the state.
    pub(crate) const fn parts(&self) -> (&'a GameState, &'a ContentStore, SourceSet, Option<&'a Galaxy>) {
        (self.state, self.content, self.sources, self.galaxy)
    }

    // ---- copied verbatim: the accessors that only read the public state -------------------------------------------------------------

    /// The public battle boundary at this exact decision, including the victory window.
    #[must_use]
    pub fn space_battle(&self) -> Option<(SystemId, PlayerId, PlayerId)> {
        ti4_model::count::bump("Observed::space_battle");
        self.state.active_space_combat.clone()
    }

    /// The public invasion boundary at this exact decision, including nested reactions.
    pub fn invasion(&self) -> Option<ti4_model::state::ActiveInvasion> {
        ti4_model::count::bump("Observed::invasion");
        self.state.active_invasion.clone()
    }

    /// The content corpus this game is played from.
    #[must_use]
    pub const fn content(&self) -> &'a ContentStore {
        self.content
    }

    /// The source scope in play.
    #[must_use]
    pub const fn sources(&self) -> SourceSet {
        self.sources
    }

    /// The map, when the game has one.
    #[must_use]
    pub const fn galaxy(&self) -> Option<&'a Galaxy> {
        self.galaxy
    }

    /// Public directional relationship from `observer` toward `subject`.
    #[must_use]
    pub fn diplomacy_relationship(
        &self,
        observer: &PlayerId,
        subject: &PlayerId,
    ) -> ti4_model::Relationship {
        ti4_model::count::bump("Observed::diplomacy_relationship");
        self.state.diplomacy.relationship(observer, subject)
    }

    /// Active public structured deals involving a seat.
    #[must_use]
    pub fn active_diplomacy_deals(&self, player: &PlayerId) -> Vec<&'a ti4_model::Deal> {
        ti4_model::count::bump("Observed::active_diplomacy_deals");
        self.state
            .diplomacy
            .active_deals
            .values()
            .filter(|deal| &deal.proposer == player || &deal.recipient == player)
            .collect()
    }

    /// Every active structured deal. Deals are public in diplomacy v1.
    pub fn public_diplomacy_deals(&self) -> impl Iterator<Item = &'a ti4_model::Deal> + '_ {
        ti4_model::count::bump("Observed::public_diplomacy_deals");
        self.state.diplomacy.active_deals.values()
    }

    /// Unexpired public signals involving a seat.
    #[must_use]
    pub fn recent_diplomacy_signals(&self, player: &PlayerId) -> Vec<&'a ti4_model::Signal> {
        ti4_model::count::bump("Observed::recent_diplomacy_signals");
        self.state
            .diplomacy
            .recent_signals
            .iter()
            .filter(|signal| &signal.speaker == player || &signal.target == player)
            .collect()
    }

    /// Every unexpired signal. Signals are public in diplomacy v1.
    pub fn public_diplomacy_signals(&self) -> impl Iterator<Item = &'a ti4_model::Signal> + '_ {
        ti4_model::count::bump("Observed::public_diplomacy_signals");
        self.state.diplomacy.recent_signals.iter()
    }

    /// The round number.
    #[must_use]
    pub const fn round(&self) -> u32 {
        self.state.round
    }

    /// Which phase the round is in.
    ///
    /// Public: every player at the table knows whether the game is in strategy, action, status or
    /// agenda. The critic extractor (MLP plan §4.1) needs it as part of its option-free base.
    #[must_use]
    pub const fn phase(&self) -> ti4_model::state::Phase {
        self.state.phase
    }

    /// The player whose action or turn is currently resolving, when the phase has one.
    #[must_use]
    pub const fn active_player(&self) -> Option<&'a PlayerId> {
        self.state.active.as_ref()
    }

    /// The speaker, which is public and breaks several ordering ties.
    #[must_use]
    pub const fn speaker(&self) -> &'a PlayerId {
        &self.state.speaker
    }

    /// Players in the public strategy-card initiative order.
    #[must_use]
    pub fn initiative_order(&self) -> Vec<PlayerId> {
        ti4_model::count::bump("Observed::initiative_order");
        self.state.initiative_order()
    }

    /// The named step of the tactical action currently awaiting resolution.
    #[must_use]
    pub fn pending_step(&self) -> Option<&'a str> {
        ti4_model::count::bump("Observed::pending_step");
        self.state.pending.as_deref()
    }

    /// Whether Mecatol's custodians token has been removed.
    #[must_use]
    pub const fn custodians_removed(&self) -> bool {
        self.state.custodians_removed
    }

    /// Laws in play, by alias to the outcome elected for them (LRR 8.20).
    ///
    /// Public: a law is enacted face up in front of the table, and its standing effect binds every
    /// seat. Fourteen decision producers read this state for legality or application and no
    /// feature carried any of it, which is the gap recorded in the OBS-002b matrix.
    #[must_use]
    pub const fn laws(&self) -> &'a BTreeMap<String, String> {
        &self.state.laws
    }

    /// The outcome elected for one law, if that law is in play.
    #[must_use]
    pub fn law_outcome(&self, alias: &str) -> Option<&'a str> {
        ti4_model::count::bump("Observed::law_outcome");
        self.state.laws.get(alias).map(String::as_str)
    }

    /// Every system holding anything. Absent systems are empty.
    #[must_use]
    pub const fn board(&self) -> &'a BTreeMap<SystemId, SystemState> {
        &self.state.board
    }

    /// One system's contents.
    #[must_use]
    pub fn system(&self, system: &SystemId) -> SystemState {
        ti4_model::count::bump("Observed::system");
        self.state.system_state(system)
    }

    /// The system currently activated for a tactical action, when there is one.
    ///
    /// An activation token is public; exposing the active system lets a policy value the legal
    /// movement options it was offered without handing it the game state that owns the choice.
    #[must_use]
    pub fn active_system(&self) -> Option<&'a SystemId> {
        ti4_model::count::bump("Observed::active_system");
        self.state.active_system.as_ref()
    }

    /// `(system, planet)` for every planet a player controls.
    #[must_use]
    pub fn controlled_planets(&self, player: &PlayerId) -> Vec<(&'a SystemId, &'a PlanetId)> {
        ti4_model::count::bump("Observed::controlled_planets");
        self.state.controlled_planets(player)
    }

    /// Whether a public planet card is ready and can contribute its printed values.
    #[must_use]
    pub fn planet_is_ready(&self, planet: &PlanetId) -> bool {
        ti4_model::count::bump("Observed::planet_is_ready");
        !self.state.exhausted_planets.contains(planet)
    }

    /// How many current systems contain at least one of this player's production units.
    ///
    /// The producer predicate is intentional: temporary bonuses such as War Machine increase a
    /// real producer's capacity but do not turn every empty system into a production opportunity.
    /// The catalogue is built once for the whole board scan rather than once per system.
    #[must_use]
    pub fn production_system_count(&self, player: &PlayerId) -> usize {
        ti4_model::count::bump("Observed::production_system_count");
        let types = ti4_content::units::catalogue(self.content, self.sources);
        self.state
            .board
            .values()
            .filter(|system| {
                system
                    .units
                    .iter()
                    .chain(system.planet_units.values().flatten())
                    .any(|unit| {
                        &unit.owner == player
                            && types
                                .get(unit.type_id.as_str())
                                .is_some_and(ti4_content::units::UnitType::has_production)
                    })
            })
            .count()
    }

    /// Systems holding any of a player's units.
    #[must_use]
    pub fn systems_with_units_of(&self, player: &PlayerId) -> BTreeSet<&'a SystemId> {
        ti4_model::count::bump("Observed::systems_with_units_of");
        self.state.systems_with_units_of(player)
    }

    /// Systems already holding a player's command token, which 89.1 forbids activating.
    #[must_use]
    pub fn systems_with_token(&self, player: &PlayerId) -> BTreeSet<&'a SystemId> {
        ti4_model::count::bump("Observed::systems_with_token");
        self.state.systems_with_token(player)
    }

    /// The agenda being voted on (public: it is revealed), while its vote is open.
    #[must_use]
    pub fn agenda_in_play(&self) -> Option<&str> {
        ti4_model::count::bump("Observed::agenda_in_play");
        self.state.agenda_in_play.as_deref()
    }

    /// How many agendas have already been resolved in the current round (public history).
    #[must_use]
    pub fn agendas_resolved_this_round(&self) -> usize {
        ti4_model::count::bump("Observed::agendas_resolved_this_round");
        self.state
            .agenda_log
            .iter()
            .filter(|record| record.round == self.state.round)
            // A card discarded without a vote is listed, but it was not resolved.
            .filter(|record| record.discard_reason.is_none())
            .count()
    }

    /// The outcome a seat predicted on the agenda in play with a rider card, if it did (public:
    /// the card is played face up and the prediction is aloud).
    #[must_use]
    pub fn agenda_prediction_of(&self, player: &PlayerId) -> Option<&str> {
        ti4_model::count::bump("Observed::agenda_prediction_of");
        self.state
            .agenda_predictions
            .get(player)
            .map(String::as_str)
    }

    /// The seats, in seating order.
    #[must_use]
    pub fn players(&self) -> Vec<&'a PlayerId> {
        ti4_model::count::bump("Observed::players");
        self.state.players.iter().map(|seat| &seat.id).collect()
    }

    /// One opponent's public relationship to `player` (OBS-005).
    ///
    /// Ranked by strategic salience, most urgent first, since the contract names the three kinds
    /// without fixing a tie-break order among them: a live shared-system presence outranks a
    /// standing Support tie, which outranks mere adjacency.
    #[must_use]
    pub fn opponent_relationship(
        &self,
        player: &PlayerId,
        other: &PlayerId,
    ) -> OpponentRelationship {
        ti4_model::count::bump("Observed::opponent_relationship");
        if self
            .state
            .board
            .values()
            .any(|system| system.has_units_of(player) && system.has_units_of(other))
        {
            return OpponentRelationship::CombatCounterpart;
        }
        if self.state.support_holders.get(other) == Some(player)
            || self.state.support_holders.get(player) == Some(other)
        {
            return OpponentRelationship::Support;
        }
        if let Some(galaxy) = self.galaxy {
            let mine = self.state.systems_with_units_of(player);
            let theirs = self.state.systems_with_units_of(other);
            let neighbors = mine.iter().any(|system| {
                galaxy
                    .adjacent(system.as_str())
                    .into_iter()
                    .any(|adjacent| theirs.iter().any(|their| their.as_str() == adjacent))
            });
            if neighbors {
                return OpponentRelationship::Neighbor;
            }
        }
        OpponentRelationship::None
    }

    /// Every opponent, assigned to a deterministic actor-relative slot (OBS-005).
    ///
    /// Sorted by `(relationship, initiative rank, seating offset)` ascending — the contract's
    /// exact ordering. The **position** in the returned list is the slot index a caller may name in
    /// a feature (`opponent-slot:0`, `opponent-slot:1`, ...), never the player id: relabeling every
    /// seat in a game that has the same relative structure produces the same slot assignment.
    #[must_use]
    pub fn opponent_slots(&self, player: &PlayerId) -> Vec<&'a PlayerId> {
        ti4_model::count::bump("Observed::opponent_slots");
        let seating = self.players();
        let Some(seat_index) = seating.iter().position(|seat| *seat == player) else {
            return Vec::new();
        };
        let initiative = self.initiative_order();
        let mut opponents: Vec<&'a PlayerId> = seating
            .iter()
            .copied()
            .filter(|seat| *seat != player)
            .collect();
        opponents.sort_by_key(|other| {
            let relationship = self.opponent_relationship(player, other);
            let initiative_rank = initiative
                .iter()
                .position(|seat| seat == *other)
                .unwrap_or(initiative.len());
            let seat_offset = seating
                .iter()
                .position(|seat| *seat == *other)
                .map_or(usize::MAX, |index| {
                    (index + seating.len() - seat_index) % seating.len()
                });
            (relationship, initiative_rank, seat_offset)
        });
        opponents
    }

    /// Objectives revealed so far, which are faceup and public.
    #[must_use]
    pub const fn revealed_objectives(&self) -> &'a [ObjectiveId] {
        self.state.revealed_objectives.as_slice()
    }

    /// Promissory notes faceup in a play area (LRR 69.3): note id (`alias:owner`) to holder.
    ///
    /// Only the faceup subset is public information. Notes held in hand are private — their
    /// positions do not appear here; an offline diagnostic that needs them reads full state
    /// explicitly, at visible cost (F-M09-021-1 AA1).
    #[must_use]
    pub fn promissory_notes(&self) -> BTreeMap<String, PlayerId> {
        ti4_model::count::bump("Observed::promissory_notes");
        self.state
            .promissory_notes
            .iter()
            .filter(|(id, _)| self.state.promissory_faceup.contains(*id))
            .map(|(id, holder)| (id.clone(), holder.clone()))
            .collect()
    }

    /// Support for the Throne: owner to the player holding it faceup — the one note whose
    /// position scores.
    #[must_use]
    pub const fn support_holders(&self) -> &'a BTreeMap<PlayerId, PlayerId> {
        &self.state.support_holders
    }

    /// The initiative number printed on a strategy card.
    ///
    /// Public: it is printed on the card, and it decides the whole action phase's turn order.
    /// The draft head could not see it, so a card was chosen from its identity alone.
    #[must_use]
    pub fn card_initiative(&self, card: &ti4_model::id::StrategyCardId) -> Option<i32> {
        ti4_model::count::bump("Observed::card_initiative");
        self.state.card_initiative.get(card).copied()
    }

    /// Trade goods sitting on an unpicked strategy card (LRR 83.2).
    ///
    /// Public, on the table, and one of the two reasons to take a card you do not otherwise want.
    #[must_use]
    pub fn strategy_card_goods(&self, card: &ti4_model::id::StrategyCardId) -> i32 {
        ti4_model::count::bump("Observed::strategy_card_goods");
        self.state
            .strategy_card_goods
            .get(card)
            .copied()
            .unwrap_or(0)
    }

    /// What a player has already scored, which is public once scored (61.18).
    #[must_use]
    pub fn scored_by(&self, player: &PlayerId) -> BTreeSet<ObjectiveId> {
        ti4_model::count::bump("Observed::scored_by");
        self.state.scored_by(player)
    }

    /// Every unit this player owns, in space **and** on planets.
    ///
    /// Ground forces and structures live on planets rather than in the space area, and counting
    /// only the space area makes "the seat built something" mean "the seat built a ship".
    #[must_use]
    pub fn units_held(&self, player: &PlayerId) -> usize {
        ti4_model::count::bump("Observed::units_held");
        self.state
            .board
            .values()
            .map(|system| {
                system.units_of(player).len()
                    + system
                        .planet_units
                        .values()
                        .flatten()
                        .filter(|unit| &unit.owner == player)
                        .count()
            })
            .sum()
    }

    /// The value of this player's fleet in per-mille resource units.
    ///
    /// Ships, infantry and mechs, including ground forces on planets. Infantry and mechs use
    /// their normal printed resource cost, with no upgrade premium. Fighters count as 1.0 resource each (1000),
    /// other ships at their printed cost times 1000, and an upgraded ship at 1300 times its base
    /// unit's cost: the upgrade's own printed price is deliberately ignored, so a dreadnought II
    /// counts as 5.2 (5200) against the dreadnought's four rather than whatever the corpus prints
    /// for the card itself. A unit whose type or base type is missing from the active catalogue
    /// contributes nothing rather than guessing.
    #[must_use]
    pub fn fleet_value_permille(&self, player: &PlayerId) -> i64 {
        ti4_model::count::bump("Observed::fleet_value_permille");
        let types = ti4_content::units::catalogue(self.content, self.sources);
        self.state
            .board
            .values()
            .flat_map(|system| {
                system
                    .units
                    .iter()
                    .chain(system.planet_units.values().flatten())
            })
            .filter(|unit| &unit.owner == player)
            .filter_map(|unit| types.get(unit.type_id.as_str()))
            .map(|stats| Self::unit_value_permille(*stats, &types))
            .sum()
    }

    /// One ship or ground force's share of [`Self::fleet_value_permille`].
    fn unit_value_permille(
        stats: ti4_content::units::UnitType<'_>,
        types: &BTreeMap<&str, ti4_content::units::UnitType<'_>>,
    ) -> i64 {
        let ground_force = matches!(stats.base_type(), "infantry" | "mech");
        if !stats.is_ship() && !ground_force {
            return 0;
        }
        // Fighters are valued at a flat 1.0 resource each (0.75 until 2026-09-15, raised at the
        // user's direction); an upgraded ship counts as 1.3x its base unit's cost, ignoring the
        // upgrade's own printed price (dreadnought II = 5.2); everything else pays its printed
        // cost, whole resources for every non-fighter ship.
        let resources = if ground_force {
            stats.cost()
        } else if stats.is_fighter() {
            1.0
        } else if let Some(base_id) = stats.upgrades_from()
            && let Some(base) = types.get(base_id)
        {
            base.cost() * 1.3
        } else {
            stats.cost()
        };
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            reason = "fleet values stay far below f64's exact-integer range"
        )]
        let permille = (resources * 1000.0).round() as i64;
        permille
    }

    /// The typed position (the loaded slices movement reads), built on the first typed question.
    fn typed_position(&self) -> Option<std::rc::Rc<crate::queries::adapter::Position>> {
        let galaxy = self.galaxy?;
        self.seats.borrow_mut().position(self.state, self.sources, galaxy)
    }

    // ---- still delegated to the legacy view: the objectives group (lane B, next) ---------------------------------------------------------

    /// [`Self::rift_exits_into`] on the typed core (`tactical::route_rift_exits`); `None` when
    /// something is not mapped. The ship type is read from the generated rows, not the content store.
    #[doc(hidden)]
    #[must_use]
    pub fn typed_rift_exits(
        &self,
        player: &PlayerId,
        origin: &SystemId,
        unit_type: &str,
        boosts: (bool, bool),
        destination: &SystemId,
    ) -> Option<usize> {
        let pos = self.typed_position()?;
        crate::queries::tactical::typed_rift_exits(&mut self.seats.borrow_mut(), &pos, player, origin, unit_type, boosts, destination)
    }

    /// [`Self::movable_into`] on the typed core; `None` when something is not mapped.
    #[doc(hidden)]
    #[must_use]
    pub fn typed_movable_into(
        &self,
        player: &PlayerId,
        destination: &SystemId,
    ) -> Option<Vec<crate::queries::tactical::Movable>> {
        let pos = self.typed_position()?;
        crate::queries::tactical::typed_movable_into(&mut self.seats.borrow_mut(), &pos, self.state, player, destination)
    }

    #[must_use]
    pub fn recent_diplomacy_attack(&self, observer: &PlayerId, subject: &PlayerId) -> bool {
        crate::queries::diplomacy::recent_attack(self.state, observer, subject)
    }

    #[must_use]
    pub fn recent_diplomacy_breach(&self, observer: &PlayerId, subject: &PlayerId) -> bool {
        crate::queries::diplomacy::recent_breach(self.state, observer, subject)
    }

    /// Revealed public objectives whose fixed map-shaped requirement includes `system`.
    ///
    /// This is derived by the objective engine from the same map geometry used for scoring, so a
    /// decision model need not infer the connection from card text or reimplement adjacency.
    #[must_use]
    pub fn objectives_implicating_system(
        &self,
        player: &PlayerId,
        system: &SystemId,
    ) -> Vec<ti4_model::id::ObjectiveId> {
        self.legacy().objectives_implicating_system(player, system)
    }

    /// Whether a ship with this printed movement can reach a public destination.
    ///
    /// This is an observation query, not a second legality entry point. It reads only the public
    /// map, board occupancy, command tokens, and laws, then reuses the engine's movement search
    /// so a policy does not approximate a route with geometric distance. Effects from a card that
    /// has not been played are deliberately absent: a bot with no hand must not infer them.
    #[must_use]
    pub fn can_reach(
        &self,
        player: &PlayerId,
        origin: &SystemId,
        destination: &SystemId,
        move_value: i32,
    ) -> bool {
        let Some(galaxy) = self.galaxy else { return false };
        let _ = galaxy;
        self.typed_position().and_then(|pos| crate::queries::tactical::typed_can_reach(&mut self.seats.borrow_mut(), &pos, player, origin, destination, move_value)).unwrap_or(false)
    }

    /// The routes (rules version 29, [`crate::tactical::ship_routes`]) the ship at `index` of
    /// `player`'s ships in `origin` may take to `destination` if it were activated, the default
    /// first; with the boosts (Gravity Drive, Ionian Fuel Refinery) of the move option they belong
    /// to. Empty without a map or without a route. A bot that names no route gets the default, so
    /// reading this is for a bot that weighs the rift on the way or the units it could pick up.
    #[must_use]
    pub fn ship_routes_into(
        &self,
        player: &PlayerId,
        origin: &SystemId,
        index: usize,
        boosts: (bool, bool),
        destination: &SystemId,
    ) -> Vec<crate::queries::tactical::MoveRoute> {
        let Some(galaxy) = self.galaxy else { return Vec::new() };
        crate::queries::tactical::routes_into(self.state, self.content, self.sources, galaxy, player, destination, origin, index, boosts)
    }

    /// Every ship of this player that could reach `destination` if it were activated.
    ///
    /// [`Self::can_reach`] answers this one hull at a time and needs the caller to know the hull's
    /// effective movement; this answers it for the whole fleet, through the engine's own
    /// `movable_into`, so gravity drive, action-card effects and laws are all accounted for
    /// exactly as they would be during the real movement step.
    ///
    /// Public by the same argument as [`Self::can_reach`]: it reads only this seat's own units and
    /// the public map.
    #[must_use]
    pub fn movable_into(
        &self,
        player: &PlayerId,
        destination: &SystemId,
    ) -> Vec<crate::queries::tactical::Movable> {
        self.movable_into_shared(player, destination).to_vec()
    }

    /// [`Self::movable_into`] without the copy: the memoised list, shared.
    #[must_use]
    pub fn movable_into_shared(
        &self,
        player: &PlayerId,
        destination: &SystemId,
    ) -> std::rc::Rc<[crate::queries::tactical::Movable]> {
        if self.galaxy.is_none() {
            return std::rc::Rc::from(Vec::new());
        }
        // The memo slot: the player's place at the table and the destination's system index.
        let slot = self.state.players.iter().position(|p| &p.id == player).zip(crate::ties::core_adapter::system_ix(destination.as_str()));
        if let Some((row, system)) = slot
            && let Some(known) = self.seats.borrow().known_movable(row, system)
        {
            return known;
        }
        let found: std::rc::Rc<[crate::queries::tactical::Movable]> = self.typed_movable_into(player, destination).unwrap_or_default().into();
        if let Some((row, system)) = slot {
            self.seats.borrow_mut().remember_movable(row, system, found.clone());
        }
        found
    }

    /// Gravity rifts a ship of `player` sailing from `origin` to `destination` would exit (each
    /// rolls a die: 1-3 destroys it and its cargo, 41.2); 0 without a map. Reads the public map
    /// and the seat's own effects, like [`Self::movable_into`].
    #[must_use]
    pub fn rift_exits_into(
        &self,
        player: &PlayerId,
        origin: &SystemId,
        unit_type: &str,
        boosts: (bool, bool),
        destination: &SystemId,
    ) -> usize {
        if self.galaxy.is_none() {
            return 0;
        }
        let ix = crate::ties::core_adapter::system_ix;
        let slot = self
            .state
            .players
            .iter()
            .position(|p| &p.id == player)
            .zip(ix(destination.as_str()))
            .zip(ix(origin.as_str()))
            .zip(crate::queries::adapter::unit_ix(unit_type))
            .map(|(((row, to), from), kind)| crate::queries::adapter::rift_key(row, to, from, kind, boosts));
        if let Some(key) = slot
            && let Some(known) = self.seats.borrow().known_rift_exits(key)
        {
            return known;
        }
        let found = self.typed_rift_exits(player, origin, unit_type, boosts, destination).unwrap_or(0);
        if let Some(key) = slot {
            self.seats.borrow_mut().remember_rift_exits(key, found);
        }
        found
    }

    /// Every unit of this player a ship leaving `origin` could carry: fighters and ground forces
    /// in its space area and on its planets, in the engine's own order. Reads only the seat's own
    /// units.
    #[must_use]
    pub fn loadable(&self, player: &PlayerId, origin: &SystemId) -> Vec<crate::queries::transit::Cargo> {
        crate::queries::transit::loadable(self.state, self.content, self.sources, player, origin)
    }

    /// Public resources or influence a player can currently spend.
    ///
    /// This deliberately returns an aggregate rather than the exhausted-card set. Ready planet
    /// cards and trade goods are visible at the table; the engine remains the single owner of
    /// the exact payment accounting used by policy and later factual feature capture.
    #[must_use]
    pub fn available_spend(&self, player: &PlayerId, kind: crate::queries::production::Spend) -> i64 {
        crate::queries::production::available(self.state, self.content, self.sources, player, kind)
    }

    /// The public fleet-supply limit after laws and standing faction abilities.
    #[must_use]
    pub fn fleet_supply_limit(&self, player: &PlayerId) -> i32 {
        crate::queries::factions::fleet_limit(self.state, self.content, player)
    }

    /// Whose command tokens sit in `mahact`'s fleet pool (Edict), one entry per token. Public: the
    /// tokens are on the table in the owner's colour.
    #[must_use]
    pub fn mahact_fleet_pool_owners(&self, mahact: &PlayerId) -> Vec<PlayerId> {
        crate::queries::factions::mahact_fleet_pool_owners(self.state, mahact)
    }

    /// Players with a control token on one of `owner`'s faceup plots (the Firmament's puppets),
    /// which the Heaven's Eye reads. Public: the tokens sit on faceup cards.
    #[must_use]
    pub fn firmament_puppets(&self, owner: &PlayerId) -> BTreeSet<PlayerId> {
        crate::queries::factions::firmament_puppeted(self.state, owner)
    }

    /// Whether `system` holds an active breach (the Crimson Rebellion). Public: the token's face
    /// shows.
    #[must_use]
    pub fn active_breach_in(&self, system: &SystemId) -> bool {
        crate::queries::factions::crimson_active_breaches(self.state).contains(system)
    }

    /// Non-home systems containing a planet `player` controls, the count the Bastion flagship
    /// reads. Public: control is on the board.
    #[must_use]
    pub fn non_home_systems_with_planets(&self, player: &PlayerId) -> usize {
        crate::queries::factions::non_home_systems_with_planets(self.state, self.content, self.sources, player)
    }

    /// Capacity ships and ground forces owned by this player, using the opening gate's exact
    /// catalogue-aware classification.
    ///
    /// This deliberately delegates to the authoritative opening measurement. Training reward
    /// shaping and final clearance must not drift into two different definitions of the same
    /// fleet-composition bar.
    #[must_use]
    pub fn opening_fleet(&self, player: &PlayerId) -> (usize, usize) {
        crate::queries::fleet::opening_fleet(self.state, player, self.content, self.sources)
    }

    /// Whether `system` is a Fracture system of a Fracture that is in play.
    /// Used only by training progress; it does not add a policy observation feature.
    #[must_use]
    pub fn is_fracture_system(&self, system: &SystemId) -> bool {
        self.state.fracture_in_play && crate::queries::fracture::is_fracture_system(self.content, self.sources, system)
    }

    /// Whether this seat currently has a ship, infantry or mech in a Fracture system.
    /// Used only by training progress; it does not add a policy observation feature.
    #[must_use]
    pub fn has_units_in_fracture(&self, player: &PlayerId) -> bool {
        if !self.state.fracture_in_play {
            return false;
        }
        self.state.board.iter().any(|(id, system)| {
            crate::queries::fracture::is_fracture_system(self.content, self.sources, id)
                && system.units.iter().chain(system.planet_units.values().flatten()).any(|unit| {
                    &unit.owner == player
                        && ti4_content::units::unit_type(self.content, unit.type_id.as_str(), self.sources)
                            .is_some_and(|stats| stats.is_ship() || matches!(stats.base_type(), "infantry" | "mech"))
                })
        })
    }

    /// How many revealed public objectives this seat could score right now.
    ///
    /// A rules predicate, not an opinion about which objective is worth chasing. It exists so a
    /// policy has something to climb before it ever scores: a four-round game yields about 1.49
    /// victory points per faction, which is far too sparse to learn from on its own.
    #[must_use]
    pub fn scoreable_public(&self, player: &PlayerId) -> usize {
        self.legacy().scoreable_public(player)
    }

    /// The same for the secrets this seat holds.
    ///
    /// Private to its holder, and answered only for the seat asking — which is the one case where
    /// reading a hand is not reading somebody else's.
    #[must_use]
    pub fn scoreable_secret(&self, player: &PlayerId) -> usize {
        self.legacy().scoreable_secret(player)
    }

    /// Exact progress for every revealed public objective, from the seat's point of view.
    ///
    /// A rules predicate built on the scoring sources of truth — [`crate::objectives::
    /// counting_progress`], [`crate::objectives::remaining_position_progress`] and [`crate::
    /// objectives::bought_progress`] — so a feature that says "three of five" is the same number
    /// the scorer would use. Cards whose progress cannot be resolved (unknown alias, missing
    /// galaxy context) are omitted rather than emitted as factual zero.
    #[must_use]
    pub fn revealed_objective_progress(
        &self,
        player: &PlayerId,
    ) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        self.legacy().revealed_objective_progress(player)
    }

    /// The same, for the position this player would hold if it also controlled `gained`.
    ///
    /// This is what makes "would this action help an objective?" answerable without guessing: the
    /// caller names the planets an action would take, and the engine's own requirement functions
    /// report the progress that would result. Compared against the current progress it gives an
    /// exact per-card delta.
    ///
    /// Planet control only. For everything else an option changes -- unit presence, technologies,
    /// structures -- use [`Self::revealed_objective_progress_imagining`], which this delegates to.
    #[must_use]
    pub fn revealed_objective_progress_gaining(
        &self,
        player: &PlayerId,
        gained: &[PlanetId],
    ) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        self.legacy().revealed_objective_progress_gaining(player, gained)
    }

    /// The same, for everything an option would change, not planet control alone.
    ///
    /// A planet-only counterfactual cancels for the 20 public objectives counted in units,
    /// technologies or structures, so no option ever carried a gain toward them: the requirement
    /// was in view with nothing linking any action to it. The caller names what its option kind
    /// actually does -- an invasion names planets, an activation the system it would occupy, a
    /// research the technology -- and the engine's own requirement functions report the result.
    #[must_use]
    pub fn revealed_objective_progress_imagining(
        &self,
        player: &PlayerId,
        imagined: &ti4_engine_legacy::objectives::Imagined<'_>,
    ) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        self.legacy().revealed_objective_progress_imagining(player, imagined)
    }

    /// [`Self::revealed_objective_progress_imagining`] on the string state: the oracle of the dual
    /// run.
    #[doc(hidden)]
    #[must_use]
    pub fn revealed_objective_progress_text(
        &self,
        player: &PlayerId,
        imagined: &ti4_engine_legacy::objectives::Imagined<'_>,
    ) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        self.legacy().revealed_objective_progress_text(player, imagined)
    }
}

/// An [`Observed`] bound to one seat (the seat of the choice being asked).
///
/// Built only by the engine's ask path (`Table::ask_seeing`) and by [`ask_private`]: no API takes caller-controlled identity data and
/// produces this type. The bound view answers only for its own seat; everything else on the position stays reachable through the deref
/// to [`Observed`], which exposes public facts only.
pub struct SeatObservation<'a> {
    observed: &'a Observed<'a>,
    acting_seat: PlayerId,
}

impl<'a> SeatObservation<'a> {
    /// Engine-internal binding. The seat is authenticated by the caller's context (the table's per-seat decider lookup, or an explicit
    /// ask), never by data a policy-side caller supplies.
    pub(crate) fn bind(observed: &'a Observed<'a>, acting_seat: PlayerId) -> Self {
        Self { observed, acting_seat }
    }

    /// The public position this view is bound to. Public facts only.
    #[must_use]
    pub const fn observed(&self) -> &'a Observed<'a> {
        self.observed
    }

    /// The seat this private observation is bound to. Named `bound_seat` rather than `seat` so it does not shadow the deref'd public
    /// accessor [`Observed::seat`] that takes a player argument.
    #[must_use]
    pub const fn bound_seat(&self) -> &PlayerId {
        &self.acting_seat
    }

    /// The raw secret objectives the bound seat holds, and only that seat's.
    #[must_use]
    pub fn held_secrets(&self) -> Vec<SecretObjectiveId> {
        ti4_model::count::bump("SeatObservation::held_secrets");
        self.observed.state.player(&self.acting_seat).map_or_else(Vec::new, |seat| seat.secret_objectives.clone())
    }

    /// The action cards the bound seat holds.
    #[must_use]
    pub fn held_action_cards(&self) -> Vec<ti4_model::id::ActionCardId> {
        ti4_model::count::bump("SeatObservation::held_action_cards");
        self.observed.state.player(&self.acting_seat).map_or_else(Vec::new, |seat| seat.action_cards.clone())
    }

    /// The facedown promissory notes the bound seat holds.
    #[must_use]
    pub fn held_promissory_notes(&self) -> Vec<String> {
        ti4_model::count::bump("SeatObservation::held_promissory_notes");
        self.observed
            .state
            .promissory_notes
            .iter()
            .filter(|(id, holder)| **holder == self.acting_seat && !self.observed.state.promissory_faceup.contains(*id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    // ---- delegated to the legacy modules until the native queries land (lane B; see the module docs) -------------------------------

    /// The hands shown to the bound seat (Mahact's Edict, Keleres' Tribunii, Nekro's Valefar ...), and the facedown promissory notes of the
    /// owners whose hand a standing permission lets it read. (Delegate: `factions::hooks_cards`.)
    #[must_use]
    pub fn revealed_cards(&self) -> Vec<ti4_engine_legacy::factions::hooks_cards::Revealed> {
        ti4_model::count::bump("SeatObservation::revealed_cards");
        use ti4_engine_legacy::factions::hooks_cards::{Revealed, RevealKind, may_view_hand, revealed_to};
        let state = self.observed.state;
        let mut revealed = revealed_to(state, &self.acting_seat);
        // Standing hand permissions are read from the current board and current leader state. They are deliberately not persisted in
        // faction_marks, which would go stale on movement or a change to the owner's hand.
        for owner in &state.seating_order {
            if may_view_hand(state, self.observed.content, self.observed.sources, self.observed.galaxy, &self.acting_seat, owner, RevealKind::PromissoryNotes) {
                let ids: Vec<String> = state
                    .promissory_notes
                    .iter()
                    .filter(|(id, holder)| *holder == owner && !state.promissory_faceup.contains(*id))
                    .map(|(id, _)| id.clone())
                    .collect();
                if !ids.is_empty() {
                    if let Some(existing) = revealed.iter_mut().find(|row| row.owner == *owner && row.kind == RevealKind::PromissoryNotes) {
                        for id in ids {
                            if !existing.ids.contains(&id) {
                                existing.ids.push(id);
                            }
                        }
                    } else {
                        revealed.push(Revealed { owner: owner.clone(), kind: RevealKind::PromissoryNotes, ids });
                    }
                }
            }
        }
        revealed
    }

    /// The first and last card of the agenda deck, for a Naalu seat whose M'aban commander is unlocked. (Delegate: `promissory`.)
    #[must_use]
    pub fn agenda_deck_ends(&self) -> Option<(String, String)> {
        ti4_model::count::bump("SeatObservation::agenda_deck_ends");
        let state = self.observed.state;
        if !ti4_engine_legacy::promissory::has_commander_ability(state, &self.acting_seat, "naalucommander") {
            return None;
        }
        Some((state.agenda_deck.first()?.clone(), state.agenda_deck.last()?.clone()))
    }

    /// The action cards other seats showed the bound seat, by owner.
    #[must_use]
    pub fn revealed_action_cards(&self) -> Vec<(PlayerId, Vec<ti4_model::id::ActionCardId>)> {
        ti4_model::count::bump("SeatObservation::revealed_action_cards");
        self.revealed_of(ti4_engine_legacy::factions::hooks_cards::RevealKind::ActionCards)
            .into_iter()
            .map(|(owner, ids)| (owner, ids.into_iter().map(ti4_model::id::ActionCardId::new).collect()))
            .collect()
    }

    /// The facedown promissory notes other seats showed the bound seat, by owner.
    #[must_use]
    pub fn revealed_promissory_notes(&self) -> Vec<(PlayerId, Vec<String>)> {
        ti4_model::count::bump("SeatObservation::revealed_promissory_notes");
        self.revealed_of(ti4_engine_legacy::factions::hooks_cards::RevealKind::PromissoryNotes)
    }

    /// The secret objectives other seats showed the bound seat, by owner.
    #[must_use]
    pub fn revealed_secret_objectives(&self) -> Vec<(PlayerId, Vec<SecretObjectiveId>)> {
        ti4_model::count::bump("SeatObservation::revealed_secret_objectives");
        self.revealed_of(ti4_engine_legacy::factions::hooks_cards::RevealKind::SecretObjectives)
            .into_iter()
            .map(|(owner, ids)| (owner, ids.into_iter().map(SecretObjectiveId::new).collect()))
            .collect()
    }

    fn revealed_of(&self, kind: ti4_engine_legacy::factions::hooks_cards::RevealKind) -> Vec<(PlayerId, Vec<String>)> {
        self.revealed_cards().into_iter().filter(|shown| shown.kind == kind).map(|shown| (shown.owner, shown.ids)).collect()
    }

    /// Progress toward the bound seat's held secret objectives. (Delegate: `secrets`/`objectives`.)
    #[must_use]
    pub fn held_secret_progress(&self) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        ti4_model::count::bump("SeatObservation::held_secret_progress");
        held_secret_progress(self.observed.state, self.observed.content, self.observed.sources, self.observed.galaxy, &self.acting_seat)
    }

    /// [`Self::held_secret_progress`] as it would stand if `imagined` happened first.
    #[must_use]
    pub fn held_secret_progress_imagining(&self, imagined: &ti4_engine_legacy::objectives::Imagined<'_>) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
        ti4_model::count::bump("SeatObservation::held_secret_progress_imagining");
        held_secret_progress_imagining(self.observed.state, self.observed.content, self.observed.sources, self.observed.galaxy, &self.acting_seat, imagined)
    }
}

impl<'a> std::ops::Deref for SeatObservation<'a> {
    type Target = Observed<'a>;

    fn deref(&self) -> &Self::Target {
        self.observed
    }
}

/// Held-secret progress for `viewer`, computed from the complete game state. For offline analysis and training contexts, which hold the
/// full state by design. (Delegate: the legacy free function.)
#[must_use]
pub fn held_secret_progress(state: &GameState, content: &ContentStore, sources: SourceSet, galaxy: Option<&Galaxy>, viewer: &PlayerId) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
    ti4_engine_legacy::choice::held_secret_progress(state, content, sources, galaxy, viewer)
}

/// [`held_secret_progress`] as it would stand if `imagined` happened first. (Delegate: the legacy free function.)
#[must_use]
pub fn held_secret_progress_imagining(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    viewer: &PlayerId,
    imagined: &ti4_engine_legacy::objectives::Imagined<'_>,
) -> Vec<ti4_engine_legacy::objectives::CardProgress> {
    ti4_engine_legacy::choice::held_secret_progress_imagining(state, content, sources, galaxy, viewer, imagined)
}

/// The whole state with every other seat's hand replaced by markers. For offline contexts; live play binds a [`SeatObservation`].
#[must_use]
pub fn redacted_full_state(state: &GameState, viewer: &PlayerId) -> GameState {
    let mut view = state.clone();
    redact_others(&mut view, viewer);
    view
}

fn redact_others(view: &mut GameState, keep: &PlayerId) {
    ti4_model::view::redact_marks(view, keep);
    for seat in &mut view.players {
        if &seat.id != keep {
            seat.action_cards = seat.action_cards.iter().map(|_| ti4_model::id::ActionCardId::new(HIDDEN)).collect();
            seat.secret_objectives = seat.secret_objectives.iter().map(|_| ti4_model::id::SecretObjectiveId::new(HIDDEN)).collect();
        }
    }
}

/// Ask `decider` the way the table does, for tests and offline drivers: bind the view to the choice's owner, hand it over, validate the
/// answer.
///
/// # Errors
/// As [`Decider::choose_seeing`], or [`IllegalChoice::NotOffered`].
pub fn ask_private(
    choice: &Choice,
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    decider: &mut dyn Decider,
) -> Result<ChoiceOption, IllegalChoice> {
    let seen = Observed::new(state, content, sources, galaxy);
    let seat_view = SeatObservation::bind(&seen, choice.player.clone());
    let answer = decider.choose_seeing(choice, &seat_view)?;
    validate(choice, answer)
}

/// Stands in for a card whose identity is private.
///
/// Not a valid alias anywhere, so a lookup against real content fails rather than quietly matching
/// something.
pub const HIDDEN: &str = "?";

/// One opponent's public relationship to the acting seat (OBS-005).
///
/// Ordered by declared strategic salience, most urgent first: a live shared-system presence, then
/// a standing Support tie, then mere adjacency, then nothing in particular. The derive order below
/// **is** the ranking `Observed::opponent_slots` sorts by — reordering these variants changes slot
/// assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpponentRelationship {
    /// Some system holds space units of both seats right now.
    CombatCounterpart,
    /// A Support for the Throne note is held between the two seats, in either direction.
    Support,
    /// With a galaxy known, some system holding either seat's units is adjacent to a system
    /// holding the other's.
    Neighbor,
    /// None of the above, or no galaxy to test adjacency against.
    None,
}

/// A seat as the rest of the table sees it: counts, never identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicSeat<'a> {
    /// Which faction sits here.
    pub faction: &'a ti4_model::id::FactionId,
    /// Points scored so far.
    pub victory_points: i32,
    /// Trade goods, which sit faceup in the play area.
    pub trade_goods: i32,
    /// Commodities, likewise faceup.
    pub commodities: i32,
    /// Tokens in the tactic pool.
    pub tactic_tokens: i32,
    /// Tokens in the fleet pool.
    pub fleet_tokens: i32,
    /// Tokens in the strategy pool.
    pub strategic_tokens: i32,
    /// Strategy cards held this round. Their identities and initiative are public.
    pub strategy_cards: &'a [ti4_model::id::StrategyCardId],
    /// Held strategy cards whose primary has already been used.
    pub exhausted_strategy_cards: &'a BTreeSet<ti4_model::id::StrategyCardId>,
    /// Technologies owned, which are faceup.
    pub technologies: &'a BTreeSet<ti4_model::id::TechnologyId>,
    /// Owned technologies currently exhausted, also faceup.
    pub exhausted_technologies: &'a BTreeSet<ti4_model::id::TechnologyId>,
    /// How many cards, never which. At a table you can see a hand's size.
    pub action_cards_held: usize,
    /// The same for unscored secrets (61.17).
    pub secret_objectives_held: usize,
    /// Whether this seat has passed for the round.
    pub passed: bool,
    /// Relics held faceup in the play area (73.4: cannot be traded, so identity is public).
    pub relics: &'a [RelicId],
    /// Held relics currently exhausted — visible on the same faceup card.
    pub exhausted_relics: &'a BTreeSet<RelicId>,
    /// Exploration cards placed faceup in the play area (e.g. Enigmatic Device).
    pub exploration_cards: &'a [String],
    /// Relic fragments by trait, kept faceup in the play area until purged (35.9).
    pub relic_fragments: &'a BTreeMap<String, i32>,
    /// The faction breakthrough, once earned — a passive-ability card, not a hand card.
    pub breakthrough: Option<&'a BreakthroughId>,
    /// Leader lifecycle states. A leader sheet's identity and lock/ready/exhaust/purge status are
    /// never hidden information in TI4, unlike a hand of cards.
    pub leaders: &'a BTreeMap<LeaderId, LeaderStatus>,
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::choice::{FirstOption, Table};
    use ti4_content::ContentStore;
    use ti4_model::id::{BreakthroughId, LeaderId, ObjectiveId, RelicId, SystemId};
    use ti4_model::state::LeaderStatus;

    fn pid(id: &str) -> PlayerId {
        PlayerId::new(id)
    }

    fn choice(options: Vec<ChoiceOption>) -> Choice {
        Choice::new(pid("a"), "pick one", options)
    }

    fn three() -> Choice {
        choice(vec![
            ChoiceOption::labelled("x", "action", "Do X"),
            ChoiceOption::labelled("y", "action", "Do Y"),
            ChoiceOption::decline(),
        ])
    }

    // --- what a decider may see ---------------------------------------------------------------

    use ti4_model::content_types::{ContentType, POK};

    fn watched() -> ti4_model::state::GameState {
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        let seat = state.player_mut(&pid("b")).unwrap();
        seat.action_cards = vec![
            ti4_model::id::ActionCardId::new("sabotage"),
            ti4_model::id::ActionCardId::new("direct_hit"),
        ];
        seat.secret_objectives = vec![ti4_model::id::SecretObjectiveId::new("become_a_legend")];
        seat.victory_points = 4;
        state
    }

    #[test]
    fn a_seat_is_seen_as_counts_never_as_identities() {
        // The whole point of the type. At a table you can count somebody's cards without reading
        // them, and `PublicSeat` has no field that could carry one.
        let state = watched();
        let seen = Observed::new(&state, ContentStore::embedded(), POK, None);
        let rival = seen.seat(&pid("b")).expect("b is seated");

        assert_eq!(rival.action_cards_held, 2);
        assert_eq!(rival.secret_objectives_held, 1);
        assert_eq!(rival.victory_points, 4, "and public facts survive");
    }

    #[test]
    fn public_timing_and_readiness_are_available_without_a_state_handle() {
        let mut state = watched();
        let imperial = ti4_model::id::StrategyCardId::new("imperial");
        state.phase = ti4_model::state::Phase::Action;
        state.active = Some(pid("b"));
        state.speaker = pid("a");
        state.pending = Some("move".to_owned());
        state.custodians_removed = true;
        state.card_initiative.insert(imperial.clone(), 8);
        {
            let rival = state.player_mut(&pid("b")).unwrap();
            rival.strategy_cards.push(imperial.clone());
            rival.exhausted_strategy_cards.insert(imperial.clone());
        }
        let (_, planet) = ti4_engine_legacy::fixtures::a_placed_planet();
        state.exhausted_planets.insert(planet.clone());

        let seen = Observed::new(&state, ContentStore::embedded(), POK, None);
        assert_eq!(seen.phase(), ti4_model::state::Phase::Action);
        assert_eq!(seen.active_player(), Some(&pid("b")));
        assert_eq!(seen.speaker(), &pid("a"));
        assert_eq!(seen.pending_step(), Some("move"));
        assert!(seen.custodians_removed());
        assert!(!seen.planet_is_ready(&planet));
        assert_eq!(seen.initiative_order().first(), Some(&pid("b")));
        let rival = seen.seat(&pid("b")).unwrap();
        assert_eq!(rival.strategy_cards, std::slice::from_ref(&imperial));
        assert!(rival.exhausted_strategy_cards.contains(&imperial));
    }

    #[test]
    fn obs004a_public_seat_carries_faceup_inventory() {
        // OBS-004a: relics, exhaustion, exploration cards, fragments, breakthrough, and leaders
        // are all faceup under current LRR rules, so any seat can read another's -- the same
        // standing `technologies`/`strategy_cards` already have. This is not a `SeatObservation`
        // capability: `seen.seat(&pid("b"))` is called by the "a" side of the table.
        let mut state = watched();
        let leader = LeaderId::new("hackerleader");
        {
            let rival = state.player_mut(&pid("b")).unwrap();
            rival.relics = vec![RelicId::new("codex")];
            rival.exhausted_relics.insert(RelicId::new("codex"));
            rival.exploration_cards = vec!["ed1".to_owned()];
            rival.relic_fragments.insert("CULTURAL".to_owned(), 2);
            rival.breakthrough = Some(BreakthroughId::new("letnevbt"));
            rival.leaders.insert(leader.clone(), LeaderStatus::Unlocked);
        }

        let seen = Observed::new(&state, ContentStore::embedded(), POK, None);
        let rival = seen.seat(&pid("b")).expect("b is seated");
        assert_eq!(rival.relics, [RelicId::new("codex")]);
        assert!(
            rival
                .exhausted_relics
                .contains(ti4_model::id::text(&"codex"))
        );
        assert_eq!(rival.exploration_cards, ["ed1".to_owned()]);
        assert_eq!(rival.relic_fragments.get("CULTURAL"), Some(&2));
        assert_eq!(rival.breakthrough, Some(&BreakthroughId::new("letnevbt")));
        assert_eq!(rival.leaders.get(&leader), Some(&LeaderStatus::Unlocked));

        // The private capability is unaffected: it still answers only for its own bound seat, and
        // nothing about the new public fields changes what it exposes.
        let mine = SeatObservation::bind(&seen, pid("a"));
        assert!(
            !mine
                .held_secrets()
                .contains(&ti4_model::id::SecretObjectiveId::new("become_a_legend")),
            "b's secret must not reach a's bound view"
        );
    }

    #[test]
    fn obs005_opponent_slots_are_relationship_then_initiative_then_seating() {
        let hub = ti4_engine_legacy::fixtures::plain_hub();
        let centre = SystemId::new(&hub.centre);
        let outer0 = SystemId::new(&hub.outer[0]);
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b", "c", "d", "e", "f"]);
        for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
            state.board.entry(SystemId::new(id)).or_default();
        }
        let (a, b, c, d, e, f) = (pid("a"), pid("b"), pid("c"), pid("d"), pid("e"), pid("f"));
        ti4_engine_legacy::fixtures::put(&mut state, &centre, "fighter", &a, 1);
        ti4_engine_legacy::fixtures::put(&mut state, &centre, "fighter", &b, 1); // shares a's system: combat
        ti4_engine_legacy::fixtures::put(&mut state, &outer0, "fighter", &d, 1); // adjacent to a: neighbor
        state.support_holders.insert(c.clone(), a.clone()); // c's note held by a: support

        let imperial = ti4_model::id::StrategyCardId::new("imperial");
        let diplomacy = ti4_model::id::StrategyCardId::new("diplomacy");
        state.card_initiative.insert(imperial.clone(), 1);
        state.card_initiative.insert(diplomacy.clone(), 2);
        state.player_mut(&e).unwrap().strategy_cards.push(imperial);
        state.player_mut(&f).unwrap().strategy_cards.push(diplomacy);

        let seen = Observed::new(&state, ContentStore::embedded(), POK, Some(&hub.galaxy));
        assert_eq!(
            seen.opponent_slots(&a),
            vec![&b, &c, &d, &e, &f],
            "combat, then support, then neighbor, then the two unrelated seats by initiative"
        );
    }

    #[test]
    fn obs005_opponent_slots_are_invariant_under_player_id_relabeling() {
        // The contract's permutation-equivariance requirement: the same relative structure built
        // from a completely different set of player ids produces the same *shape* of relationship
        // classifications, never a coincidence about which literal id landed where.
        fn build(ids: [&str; 6]) -> (GameState, ti4_content::galaxy::Galaxy, PlayerId) {
            let hub = ti4_engine_legacy::fixtures::plain_hub();
            let centre = SystemId::new(&hub.centre);
            let outer0 = SystemId::new(&hub.outer[0]);
            let mut state = ti4_engine_legacy::fixtures::game(&ids);
            for id in std::iter::once(&hub.centre).chain(hub.outer.iter()) {
                state.board.entry(SystemId::new(id)).or_default();
            }
            let actor = pid(ids[0]);
            ti4_engine_legacy::fixtures::put(&mut state, &centre, "fighter", &actor, 1);
            ti4_engine_legacy::fixtures::put(&mut state, &centre, "fighter", &pid(ids[1]), 1);
            ti4_engine_legacy::fixtures::put(&mut state, &outer0, "fighter", &pid(ids[3]), 1);
            state.support_holders.insert(pid(ids[2]), actor.clone());
            (state, hub.galaxy, actor)
        }

        let (state_x, galaxy_x, actor_x) = build(["a", "b", "c", "d", "e", "f"]);
        let (state_y, galaxy_y, actor_y) = build(["p", "q", "r", "s", "t", "u"]);
        let seen_x = Observed::new(&state_x, ContentStore::embedded(), POK, Some(&galaxy_x));
        let seen_y = Observed::new(&state_y, ContentStore::embedded(), POK, Some(&galaxy_y));

        let shape = |seen: &Observed<'_>, actor: &PlayerId| -> Vec<OpponentRelationship> {
            seen.opponent_slots(actor)
                .into_iter()
                .map(|other| seen.opponent_relationship(actor, other))
                .collect()
        };
        assert_eq!(
            shape(&seen_x, &actor_x),
            shape(&seen_y, &actor_y),
            "relabeling every seat must not change the relationship shape"
        );
    }

    #[test]
    fn reading_a_hand_costs_a_copy_and_returns_markers() {
        // The offline form takes explicit records: this test holds the fixture state, so it may
        // name its viewer. Live play has no such call — the capability carries no state source at
        // all (F-M09-021-1 round 4).
        let state = watched();
        let view = redacted_full_state(&state, &pid("a"));

        let rival = view.player(&pid("b")).unwrap();
        assert_eq!(rival.action_cards.len(), 2, "the count is public");
        assert!(
            rival
                .action_cards
                .iter()
                .all(|card| card.as_str() == HIDDEN),
            "the names are not: {:?}",
            rival.action_cards
        );
        assert_eq!(rival.secret_objectives[0].as_str(), HIDDEN);

        let own = view.player(&pid("a")).unwrap();
        assert_eq!(own.id, pid("a"), "your own seat is untouched");
    }

    #[test]
    fn you_can_read_your_own_hand() {
        let state = watched();
        let view = redacted_full_state(&state, &pid("b"));

        assert_eq!(
            view.player(&pid("b")).unwrap().action_cards[0].as_str(),
            "sabotage"
        );
    }

    #[test]
    fn the_marker_matches_no_real_card() {
        // A redacted hand must not resolve against content, or a bot reading it would find a card
        // nobody holds rather than failing.
        assert!(
            ContentStore::embedded()
                .get(ti4_model::content_types::ContentType::ActionCards, HIDDEN)
                .is_none()
        );
    }

    #[test]
    fn public_spend_capacity_counts_only_ready_planets_and_faceup_goods() {
        // A seated opening on a real map (the native query answers for what the core holds, and the `watched()` fixture seats no faction).
        let content = ContentStore::embedded();
        let ids = [pid("a"), pid("b"), pid("c")];
        let (mut state, _) = ti4_host::map::create_game_with_template(content, &ids, 3, None).expect("a map");
        state.player_mut(&pid("a")).unwrap().trade_goods = 2;
        let planets: Vec<_> = state.controlled_planets(&pid("a")).into_iter().map(|(_, planet)| planet.clone()).collect();
        assert!(!planets.is_empty(), "a seat controls its home planets at the opening");
        let worth = |spend| -> i64 { planets.iter().map(|planet| crate::production::planet_value(content, POK, planet, spend)).sum() };
        let resources = worth(ti4_engine_legacy::production::Spend::Resources);
        let influence = worth(ti4_engine_legacy::production::Spend::Influence);

        let seen = Observed::new(&state, content, POK, None);
        assert_eq!(seen.available_spend(&pid("a"), crate::queries::production::Spend::Resources), resources + 2);
        assert_eq!(seen.available_spend(&pid("a"), crate::queries::production::Spend::Influence), influence + 2);

        for planet in planets {
            state.exhaust_planet(planet);
        }
        let seen = Observed::new(&state, content, POK, None);
        assert_eq!(seen.available_spend(&pid("a"), crate::queries::production::Spend::Resources), 2);
        assert_eq!(seen.available_spend(&pid("a"), crate::queries::production::Spend::Influence), 2);
    }

    #[test]
    fn a_decider_that_does_not_look_gets_the_same_answer_either_way() {
        // The default on `choose_seeing` is what lets every scripted test and random smoke run
        // stay ignorant of the board. If it ever stopped delegating, those would silently change.
        let state = watched();
        let asked = three();

        let mut blind = FirstOption;
        assert_eq!(
            blind.choose(&asked).unwrap(),
            ask_private(
                &asked,
                &state,
                ContentStore::embedded(),
                POK,
                None,
                &mut blind,
            )
            .unwrap()
        );
    }

    #[test]
    fn both_ask_paths_validate_and_record_alike() {
        // A game driven through `ask_seeing` must replay through `ask`, which needs the log and
        // the validation to be the same on both. They share `settle` for exactly that reason.
        let state = watched();
        let seen = Observed::new(&state, ContentStore::embedded(), POK, None);

        let mut blind = Table::new();
        blind.ask(&three()).unwrap();
        let mut looking = Table::new();
        looking.ask_seeing(&three(), &seen).unwrap();

        assert_eq!(blind.log.records, looking.log.records);
    }

    #[test]
    fn an_answer_that_was_not_offered_is_refused_on_the_seeing_path_too() {
        struct Inventing;
        impl Decider for Inventing {
            fn choose(&mut self, _choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Ok(ChoiceOption::new("not_offered", "invented"))
            }
        }
        let state = watched();
        let seen = Observed::new(&state, ContentStore::embedded(), POK, None);

        let mut table = Table::with_default(Box::new(Inventing));
        assert!(
            table.ask_seeing(&three(), &seen).is_err(),
            "the boundary holds on both paths"
        );
    }

    #[test]
    fn held_secret_progress_is_bound_to_the_private_view_not_the_observed() {
        // F-M09-021-1: named secret progress lives on `SeatObservation`, a capability with no
        // public constructor. One public `Observed` value, two engine-bound views — each answers
        // for exactly its own bound seat, and there is no method on either type that takes an
        // arbitrary seat (the former `held_secret_progress(player)` / `_for_choice(choice)`
        // signatures are gone; `Choice` is freely constructible, so binding to it authenticated
        // nothing).
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state.revealed_objectives = vec![ObjectiveId::new("outer_rim")];
        state.player_mut(&pid("a")).unwrap().secret_objectives =
            vec![ti4_model::id::SecretObjectiveId::new("otf")];
        state.player_mut(&pid("b")).unwrap().secret_objectives =
            vec![ti4_model::id::SecretObjectiveId::new("mlp")];

        let hub = ti4_engine_legacy::fixtures::hub_with_centre(crate::seating::MECATOL);
        let seen = Observed::new(&state, content, POK, Some(&hub.galaxy));

        // Publics: the public view still answers for any seat — revealed cards are public.
        for seat in [&pid("a"), &pid("b")] {
            let cards = seen.revealed_objective_progress(seat);
            assert_eq!(cards.len(), 1);
            assert_eq!(cards[0].alias, "outer_rim");
        }

        // Secrets: engine-bound views (as `Table::ask_seeing` produces them) answer for their own
        // bound seat only.
        let view_a = SeatObservation::bind(&seen, pid("a"));
        let view_b = SeatObservation::bind(&seen, pid("b"));

        let a_cards = view_a.held_secret_progress();
        assert_eq!(a_cards.len(), 1);
        assert_eq!(a_cards[0].alias, "otf");

        let b_cards = view_b.held_secret_progress();
        assert_eq!(b_cards.len(), 1);
        assert_eq!(b_cards[0].alias, "mlp");

        // Negative boundary: neither bound view names the other seat's card. `held_secret_
        // progress()` takes no arguments — even holding both views, there is no call that could
        // ask for an opponent, and a public `Observed` value has no secret-data method at all.
        assert!(
            !a_cards.iter().any(|card| card.alias == "mlp"),
            "an opponent's secret alias reached the acting seat's view"
        );
        assert!(
            !b_cards.iter().any(|card| card.alias == "otf"),
            "an opponent's secret alias reached the acting seat's view"
        );

        // The offline full-state form obeys the same binding when given explicit records: each
        // copy shows its viewer's cards and hides the other seat's. (The capability itself no
        // longer produces a state at all — round 4 removed `held_state()`; this free function is
        // authority-gated by full-state possession, which these tests hold.)
        let state_a = redacted_full_state(&state, &pid("a"));
        let state_b = redacted_full_state(&state, &pid("b"));
        assert_eq!(
            state_a.player(&pid("a")).unwrap().secret_objectives,
            vec![ti4_model::id::SecretObjectiveId::new("otf")]
        );
        assert_eq!(
            state_a.player(&pid("b")).unwrap().secret_objectives,
            vec![ti4_model::id::SecretObjectiveId::new(HIDDEN)],
            "a's view must hide b's cards"
        );
        assert_eq!(
            state_b.player(&pid("b")).unwrap().secret_objectives,
            vec![ti4_model::id::SecretObjectiveId::new("mlp")]
        );
        assert_eq!(
            state_b.player(&pid("a")).unwrap().secret_objectives,
            vec![ti4_model::id::SecretObjectiveId::new(HIDDEN)],
            "b's view must hide a's cards"
        );
    }

    #[test]
    fn ask_private_binds_the_view_to_the_choice_owner() {
        // The offline/test seam performs the identical binding: the decider sees only its own
        // seat's secrets, and the answer goes through the same validation as the table path.
        struct Seeing;
        impl Decider for Seeing {
            fn choose(&mut self, _choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Ok(ChoiceOption::decline())
            }
            fn choose_seeing(
                &mut self,
                choice: &Choice,
                seen: &SeatObservation<'_>,
            ) -> Result<ChoiceOption, IllegalChoice> {
                // The decider can read the bound seat's own secrets — and nothing else exists to
                // call for another seat's.
                let cards = seen.held_secret_progress();
                assert_eq!(cards.len(), 1);
                assert_eq!(
                    cards[0].alias, "otf",
                    "the view is bound to the choice owner"
                );
                Ok(choice.options[0].clone())
            }
        }

        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state.player_mut(&pid("a")).unwrap().secret_objectives =
            vec![ti4_model::id::SecretObjectiveId::new("otf")];
        state.player_mut(&pid("b")).unwrap().secret_objectives =
            vec![ti4_model::id::SecretObjectiveId::new("mlp")];

        let choice = Choice::new(
            pid("a"),
            "decide",
            vec![ChoiceOption::labelled("x", "kind", "x")],
        );

        let mut decider = Seeing;
        // Offline context: the test holds the full state, so it may bind any owner — and the
        // bound view still answers for that owner only.
        let answer = ask_private(&choice, &state, content, POK, None, &mut decider).unwrap();
        assert_eq!(answer.id, "x");
    }

    #[test]
    fn promissory_notes_expose_only_the_faceup_subset() {
        // F-M09-021-1 AA1 — in-hand notes are private. The public view carries only the faceup
        // subset, and this pins that projection against the engine's own receipt path.
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);

        // At setup every note is held in hand: the position map is populated, the public view empty.
        assert!(
            !state.promissory_notes.is_empty(),
            "setup deals every seat its notes"
        );
        let seen = Observed::new(&state, content, POK, None);
        assert!(
            seen.promissory_notes().is_empty(),
            "no note is faceup at setup"
        );

        // The engine's own receipt path: a play-area note goes to the table (public), an in-hand
        // note stays private.
        crate::promissory::take(&mut state, content, &pid("a"), "an:generic");
        crate::promissory::take(&mut state, content, &pid("b"), "cf:generic");

        let seen = Observed::new(&state, content, POK, None);
        let public_notes = seen.promissory_notes();
        assert_eq!(public_notes.len(), 1, "only the play-area note is public");
        assert_eq!(public_notes.get("an:generic"), Some(&pid("a")));
        assert!(
            !public_notes.contains_key("cf:generic"),
            "in-hand notes are private"
        );
    }

    #[test]
    fn a_bound_view_cannot_mint_an_opponent_capability() {
        // F-M09-021-1 round 4 regression — the attacker is a live decider implementation, and its
        // asset set is exactly what `choose_seeing` hands it: one bound view for seat "a" (and the
        // public `Observed` it derefs to). Every reachable read is attempted below; anything that
        // names opponent "b's private data is recorded, and the test asserts nothing was.
        struct Attacker {
            /// The alias being hunted — b's actual card. Known only because this test holds the
            /// table side; a live attacker would hunt every alias in turn.
            target: String,
            leaked: Vec<String>,
        }

        impl Decider for Attacker {
            fn choose(&mut self, _choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
                Ok(ChoiceOption::decline())
            }

            fn choose_seeing(
                &mut self,
                choice: &Choice,
                seen: &SeatObservation<'_>,
            ) -> Result<ChoiceOption, IllegalChoice> {
                // Attempt 1 — the bound progress records.
                for card in seen.held_secret_progress() {
                    if card.alias == self.target {
                        self.leaked.push(format!("progress:{card:?}"));
                    }
                }

                // Attempt 2 — the raw held secrets (the round-4 accessor).
                for secret in seen.held_secrets() {
                    if secret.as_str() == self.target {
                        self.leaked.push(format!("secrets:{secret}"));
                    }
                }

                // Attempt 3 — every public fact on the deref'd Observed that names b. Counts are
                // legitimately visible; what must not exist is any named private data.
                let rival = seen.seat(&pid("b")).expect("b is seated");
                assert_eq!(rival.secret_objectives_held, 1, "counts stay public");

                // Attempt 4 — minting an opponent capability. `ask_private` requires `&GameState`,
                // and no method on SeatObservation or Observed produces one (round 4 removed
                // held_state()), so this call cannot be written with these assets: the decider's
                // signature has nothing to pass. If a state source is ever re-added, extend this
                // attempt to use it — and watch this test fail.

                Ok(choice.options[0].clone())
            }
        }

        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        let originally_held: Vec<String> = state
            .players
            .iter()
            .flat_map(|seat| {
                seat.secret_objectives
                    .iter()
                    .map(|id| id.as_str().to_owned())
            })
            .collect();

        // The complement attack is real, and this proves it from the table side: catalogue − deck
        // names every secret that was dealt — exactly the two cards above. This is the danger the
        // gate prevents; it is unreachable from inside `choose_seeing` because no bound asset
        // exposes a deck.
        let catalogue: BTreeSet<String> = content
            .from_sources(ContentType::SecretObjectives, POK)
            .filter_map(|record| record.id().map(ToOwned::to_owned))
            .collect();
        let deck: BTreeSet<String> = state
            .secret_deck
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let recovered: BTreeSet<String> =
            catalogue.difference(&deck).map(ToOwned::to_owned).collect();
        assert_eq!(
            recovered.len(),
            2,
            "two secrets were dealt to the two seats"
        );
        for held in &originally_held {
            assert!(
                recovered.contains(held),
                "the complement names every dealt card: {held}"
            );
        }

        // Now set known cards and run the attack through the offline seam, which is driven with
        // full state (this test holds it); the decider inside gets only its bound view.
        state.player_mut(&pid("a")).unwrap().secret_objectives =
            vec![SecretObjectiveId::new("otf")];
        state.player_mut(&pid("b")).unwrap().secret_objectives =
            vec![SecretObjectiveId::new("mlp")];

        let choice = Choice::new(
            pid("a"),
            "decide",
            vec![ChoiceOption::labelled("x", "kind", "x")],
        );
        let mut attacker = Attacker {
            target: "mlp".to_owned(),
            leaked: Vec::new(),
        };
        ask_private(&choice, &state, content, POK, None, &mut attacker).unwrap();
        assert!(
            attacker.leaked.is_empty(),
            "the bound view named opponent data: {:?}",
            attacker.leaked
        );
    }
}

#[cfg(test)]
mod obs004_actor_owned_inventory {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;

    use super::*;

    fn pid(name: &str) -> PlayerId {
        PlayerId::new(name)
    }

    /// The bound seat reads its own hand; an opponent's mutations never reach it.
    ///
    /// The interesting half is the mutation. A view that merely *happens* to hold the right cards
    /// proves nothing — it could be reading a snapshot taken before the opponent acted. Changing
    /// the opponent's holdings and re-reading proves the accessor is bound to a seat rather than
    /// to a moment.
    #[test]
    fn an_opponents_hand_never_reaches_the_bound_seat() {
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state.player_mut(&pid("a")).unwrap().action_cards =
            vec![ti4_model::id::ActionCardId::new("sabotage")];
        state.player_mut(&pid("b")).unwrap().action_cards = vec![
            ti4_model::id::ActionCardId::new("direct_hit"),
            ti4_model::id::ActionCardId::new("upgrade"),
        ];

        {
            let seen = Observed::new(&state, content, POK, None);
            let mine = SeatObservation::bind(&seen, pid("a"));
            assert_eq!(mine.held_action_cards().len(), 1);
            assert_eq!(mine.held_action_cards()[0].as_str(), "sabotage");
            // The count is public; the contents are not.
            assert_eq!(seen.seat(&pid("b")).map(|s| s.action_cards_held), Some(2));
        }

        // Give the opponent three more. The bound seat's own hand is unmoved.
        state.player_mut(&pid("b")).unwrap().action_cards.extend([
            ti4_model::id::ActionCardId::new("x"),
            ti4_model::id::ActionCardId::new("y"),
            ti4_model::id::ActionCardId::new("z"),
        ]);
        let seen = Observed::new(&state, content, POK, None);
        let mine = SeatObservation::bind(&seen, pid("a"));
        assert_eq!(
            mine.held_action_cards(),
            vec![ti4_model::id::ActionCardId::new("sabotage")],
            "an opponent drawing cards changes nothing about what this seat holds"
        );
        assert_eq!(
            seen.seat(&pid("b")).map(|s| s.action_cards_held),
            Some(5),
            "the public count does move, because a hand size is public"
        );
    }

    /// A note in hand is private; the same note faceup is public.
    #[test]
    fn a_promissory_note_becomes_public_only_when_it_is_played() {
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state
            .promissory_notes
            .insert("ceasefire:b".to_owned(), pid("a"));

        {
            let seen = Observed::new(&state, content, POK, None);
            let mine = SeatObservation::bind(&seen, pid("a"));
            assert_eq!(
                mine.held_promissory_notes(),
                vec!["ceasefire:b".to_owned()],
                "held unplayed, so its holder can see it"
            );
            assert!(
                seen.promissory_notes().is_empty(),
                "and the table cannot: only the faceup subset is public"
            );
        }

        state.promissory_faceup.insert("ceasefire:b".to_owned());
        let seen = Observed::new(&state, content, POK, None);
        let mine = SeatObservation::bind(&seen, pid("a"));
        assert!(
            mine.held_promissory_notes().is_empty(),
            "played, so no longer in hand"
        );
        assert_eq!(
            seen.promissory_notes().get("ceasefire:b"),
            Some(&pid("a")),
            "and now public"
        );
    }

    /// A seat cannot read another seat's unplayed notes.
    #[test]
    fn an_opponents_unplayed_notes_are_not_listed() {
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state
            .promissory_notes
            .insert("support:b".to_owned(), pid("b"));
        let seen = Observed::new(&state, content, POK, None);
        let mine = SeatObservation::bind(&seen, pid("a"));
        assert!(
            mine.held_promissory_notes().is_empty(),
            "the note is B's and unplayed; A holds nothing"
        );
    }

    #[test]
    fn naalu_commander_reads_neighbor_hands_and_agenda_ends_live() {
        use ti4_engine_legacy::fixtures::{hub_with_centre, put};
        use ti4_model::id::SystemId;

        let content = ContentStore::embedded();
        let hub = hub_with_centre(crate::seating::MECATOL);
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b", "c"]);
        state.player_mut(&pid("a")).unwrap().faction = ti4_model::id::FactionId::new("naalu");
        state
            .player_mut(&pid("a"))
            .unwrap()
            .leaders
            .insert(LeaderId::new("naalucommander"), LeaderStatus::Unlocked);
        put(
            &mut state,
            &SystemId::new(hub.outer[0].clone()),
            "infantry",
            &pid("a"),
            1,
        );
        put(
            &mut state,
            &SystemId::new(crate::seating::MECATOL),
            "cruiser",
            &pid("b"),
            1,
        );
        let far = hub.across(&hub.outer[0]);
        put(&mut state, &SystemId::new(far), "cruiser", &pid("c"), 1);
        state.promissory_notes.insert("b:note".to_owned(), pid("b"));
        state.promissory_notes.insert("c:note".to_owned(), pid("c"));
        state.agenda_deck = vec![
            "agenda_top".to_owned(),
            "middle".to_owned(),
            "agenda_bottom".to_owned(),
        ];

        let seen = Observed::new(&state, content, POK, Some(&hub.galaxy));
        let a = SeatObservation::bind(&seen, pid("a"));
        assert_eq!(
            a.revealed_promissory_notes(),
            vec![(pid("b"), vec!["b:note".to_owned()])]
        );
        assert!(a.revealed_action_cards().is_empty());
        assert!(a.revealed_secret_objectives().is_empty());
        assert!(
            state
                .faction_marks
                .keys()
                .all(|key| !key.starts_with("cards:reveal:")),
            "M'aban's live permission creates no stored reveal"
        );
        assert_eq!(
            a.agenda_deck_ends(),
            Some(("agenda_top".to_owned(), "agenda_bottom".to_owned()))
        );
        assert!(
            SeatObservation::bind(&seen, pid("b"))
                .revealed_promissory_notes()
                .is_empty()
        );
        assert!(
            SeatObservation::bind(&seen, pid("c"))
                .agenda_deck_ends()
                .is_none()
        );
        let without_map = Observed::new(&state, content, POK, None);
        let a_without_map = SeatObservation::bind(&without_map, pid("a"));
        assert!(a_without_map.revealed_promissory_notes().is_empty());
        assert_eq!(
            a_without_map.agenda_deck_ends(),
            Some(("agenda_top".to_owned(), "agenda_bottom".to_owned()))
        );

        // Hand changes, faceup play, and commander lock state are all observed immediately.
        state.promissory_notes.insert("b:new".to_owned(), pid("b"));
        state.promissory_faceup.insert("b:note".to_owned());
        state.agenda_deck = vec!["new_top".to_owned(), "new_bottom".to_owned()];
        let seen = Observed::new(&state, content, POK, Some(&hub.galaxy));
        let a = SeatObservation::bind(&seen, pid("a"));
        assert_eq!(
            a.revealed_promissory_notes(),
            vec![(pid("b"), vec!["b:new".to_owned()])]
        );
        assert_eq!(
            a.agenda_deck_ends(),
            Some(("new_top".to_owned(), "new_bottom".to_owned()))
        );

        state
            .player_mut(&pid("a"))
            .unwrap()
            .leaders
            .insert(LeaderId::new("naalucommander"), LeaderStatus::Locked);
        let seen = Observed::new(&state, content, POK, Some(&hub.galaxy));
        let a = SeatObservation::bind(&seen, pid("a"));
        assert!(a.revealed_promissory_notes().is_empty());
        assert_eq!(a.agenda_deck_ends(), None);
    }

    /// Laws are public: every seat reads the same standing effects.
    ///
    /// Fourteen decision producers read this state for legality or application and no feature
    /// carried any of it. The accessor is the first half of closing that; wiring it into features
    /// is later work.
    #[test]
    fn laws_are_visible_to_every_seat_alike() {
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b"]);
        state
            .laws
            .insert("fleet_regulations".to_owned(), "for".to_owned());
        state.laws.insert("censure".to_owned(), "a".to_owned());

        let seen = Observed::new(&state, content, POK, None);
        assert_eq!(seen.laws().len(), 2);
        assert_eq!(seen.law_outcome("fleet_regulations"), Some("for"));
        assert_eq!(seen.law_outcome("censure"), Some("a"));
        assert_eq!(seen.law_outcome("not_in_play"), None);

        for seat in [pid("a"), pid("b")] {
            let bound = SeatObservation::bind(&seen, seat);
            assert_eq!(
                bound.laws().len(),
                2,
                "a standing law binds the table, so it reads the same from either seat"
            );
        }
    }

    #[test]
    fn a_reveal_is_visible_to_its_viewer_alone_and_never_through_the_public_position() {
        // BF-00h-cards: three seats, a shows nothing, b's hand is shown to a. Only a's bound view
        // reports it, in each of the three kinds; b and c get nothing, and the public `Observed`
        // carries a count for b, as it did before.
        use crate::factions::hooks_cards::{RevealKind, RevealScope, reveal, reveal_hand};
        let content = ContentStore::embedded();
        let mut state = ti4_engine_legacy::fixtures::game(&["a", "b", "c"]);
        state.player_mut(&pid("b")).unwrap().action_cards = vec![
            ti4_model::id::ActionCardId::new("bribery"),
            ti4_model::id::ActionCardId::new("dh1"),
        ];
        state.player_mut(&pid("b")).unwrap().secret_objectives =
            vec![SecretObjectiveId::new("otf")];
        let note = "spynet:yssaril".to_owned();
        state.promissory_notes.insert(note.clone(), pid("b"));
        let before = Observed::new(&state, content, POK, None);
        for seat in ["a", "b", "c"] {
            assert!(
                SeatObservation::bind(&before, pid(seat))
                    .revealed_cards()
                    .is_empty()
            );
        }
        reveal_hand(
            &mut state,
            &pid("a"),
            &pid("b"),
            RevealScope::Choice,
            "yssarilcommander",
        );
        for (kind, ids) in [
            (RevealKind::SecretObjectives, vec!["otf".to_owned()]),
            (RevealKind::PromissoryNotes, vec![note.clone()]),
        ] {
            assert!(reveal(
                &mut state,
                &pid("a"),
                &pid("b"),
                kind,
                &ids,
                RevealScope::Choice,
                "yssarilcommander"
            ));
        }
        let seen = Observed::new(&state, content, POK, None);
        let a = SeatObservation::bind(&seen, pid("a"));
        assert_eq!(a.revealed_action_cards()[0].1.len(), 2);
        assert_eq!(a.revealed_promissory_notes(), vec![(pid("b"), vec![note])]);
        assert_eq!(
            a.revealed_secret_objectives(),
            vec![(pid("b"), vec![SecretObjectiveId::new("otf")])]
        );
        for seat in ["b", "c"] {
            assert!(
                SeatObservation::bind(&seen, pid(seat))
                    .revealed_cards()
                    .is_empty(),
                "{seat} was shown nothing"
            );
        }
        assert_eq!(seen.seat(&pid("b")).unwrap().action_cards_held, 2);
    }
}

