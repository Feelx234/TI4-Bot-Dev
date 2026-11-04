//! `SeenView`: the typed core's `Seen` as a [`View`] / [`SeatView`] (docs/todo/D3-typed-bot.md section 2.2, lane V0).
//!
//! The rule bot's functions take `&dyn View`. Behind it is either the string `Observed` (the oracle) or this adapter, which
//! answers every accessor from a [`Seen`] and nothing else: no `GameState` is held or read. The answers have the shape of the
//! string side (`PlayerId`, `SystemId`, `SystemState`, `Movable`, `CardProgress`, ...), so the same bot code runs on both.
//!
//! * The seat table [`Owners`] maps seats to the player ids of the game (the core numbers the players, the bot names them).
//! * Borrowed answers (`players`, `seat`, `board`, `laws`, ...) are built on first use and kept for the life of the view, in
//!   `OnceCell`s: the position cannot change while a view is borrowed (the same argument as `Seen`'s own memo).
//! * Where the typed side can not answer (`scoreable_public` and the objective progress of an ocean card: `None` from `Seen`;
//!   the string engine answers there), the adapter answers as the thin crate's `ObjView` does (empty / zero) and counts the
//!   call in [`SeenView::bails`], so a differential run can list them.
//!
//! Not here: `ship_routes_into` (the bot only mentions it in a comment; `Seen` has no routes yet) and the text of an agenda
//! prediction (the bot asks only whether the seat predicted).

use std::cell::{Cell, OnceCell};
use std::collections::{BTreeMap, BTreeSet};

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::PublicSeat;
use ti4_engine::generated::IdStr;
use ti4_engine::objectives::{CardProgress, Imagined};
use ti4_engine::production::Spend;
use ti4_engine::tactical::Movable;
use ti4_engine::transit::{Cargo, CargoSource};
use ti4_engine_core::generated::{Agenda, ExploreType, Planet, StrategyCard, System, Technology, Unit as CoreUnit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::keys::Outcome;
use ti4_engine_core::query::economy::Spend as CoreSpend;
use ti4_engine_core::seen::{ProgressRow, RowFamily, Seen};
use ti4_engine_core::state::ObjectiveRef;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{
    ActionCardId, BreakthroughId, FactionId, LeaderId, ObjectiveId, PlanetId, PlayerId, RelicId, SecretObjectiveId, StrategyCardId, SystemId,
    TechnologyId,
};
use ti4_model::state::{LeaderStatus, Phase, SystemState};
use ti4_model::units::Unit;
use ti4_rule_bot::view::{InvasionFacts, SeatView, View};

/// The players of a game by seat: the table that names the core's numbered seats.
#[derive(Debug, Clone, Default)]
pub struct Owners {
    ids: Vec<PlayerId>,
}

impl Owners {
    /// The table for the given ids, index = seat number.
    #[must_use]
    pub fn new(ids: Vec<PlayerId>) -> Self {
        Self { ids }
    }

    /// The seat of a player id.
    #[must_use]
    pub fn seat(&self, player: &PlayerId) -> Option<Seat> {
        self.ids.iter().position(|p| p == player).and_then(|i| u8::try_from(i).ok()).map(Seat)
    }

    /// The id of a seat; the empty id for a seat the table does not know (a position of a different game).
    #[must_use]
    pub fn id(&self, seat: Seat) -> &PlayerId {
        static NONE: std::sync::OnceLock<PlayerId> = std::sync::OnceLock::new();
        self.ids.get(usize::from(seat.0)).unwrap_or_else(|| NONE.get_or_init(|| PlayerId::new("")))
    }
}

/// The lists of a seat's public sheet in the string model, for [`PublicSeat`] to borrow.
struct Sheet {
    faction: FactionId,
    fleet_tokens: i32,
    strategy_cards: Vec<StrategyCardId>,
    exhausted_strategy_cards: BTreeSet<StrategyCardId>,
    technologies: BTreeSet<TechnologyId>,
    exhausted_technologies: BTreeSet<TechnologyId>,
    relics: Vec<RelicId>,
    exhausted_relics: BTreeSet<RelicId>,
    exploration_cards: Vec<String>,
    relic_fragments: BTreeMap<String, i32>,
    breakthrough: Option<BreakthroughId>,
    leaders: BTreeMap<LeaderId, LeaderStatus>,
}

/// The fragment keys of the string state (`CULTURAL`, ...), by trait.
const FRAGMENT_KEYS: [(ExploreType, &str); 4] = [
    (ExploreType::Cultural, "CULTURAL"),
    (ExploreType::Frontier, "FRONTIER"),
    (ExploreType::Hazardous, "HAZARDOUS"),
    (ExploreType::Industrial, "INDUSTRIAL"),
];

/// A [`Seen`] answering as the string `Observed` / `SeatObservation` do.
pub struct SeenView<'a> {
    seen: &'a Seen<'a>,
    owners: &'a Owners,
    content: &'a ContentStore,
    sources: SourceSet,
    galaxy: Option<&'a Galaxy>,
    me: Seat,
    sys_ids: Vec<OnceCell<SystemId>>,
    planet_ids: Vec<OnceCell<PlanetId>>,
    sheets: Vec<OnceCell<Option<Sheet>>>,
    laws: OnceCell<BTreeMap<String, String>>,
    board: OnceCell<BTreeMap<SystemId, SystemState>>,
    bails: Cell<u32>,
}

impl<'a> SeenView<'a> {
    /// The view of `seen` for its bound seat. `content`, `sources` and `galaxy` are the game's tables (they are not part of the
    /// position).
    #[must_use]
    pub fn new(seen: &'a Seen<'a>, owners: &'a Owners, content: &'a ContentStore, sources: SourceSet, galaxy: Option<&'a Galaxy>) -> Self {
        Self {
            seen,
            owners,
            content,
            sources,
            galaxy,
            me: seen.me(),
            sys_ids: (0..System::COUNT).map(|_| OnceCell::new()).collect(),
            planet_ids: (0..Planet::COUNT).map(|_| OnceCell::new()).collect(),
            sheets: (0..16).map(|_| OnceCell::new()).collect(),
            laws: OnceCell::new(),
            board: OnceCell::new(),
            bails: Cell::new(0),
        }
    }

    /// How many accessor calls the typed side could not answer (and the adapter answered with the empty value).
    #[must_use]
    pub fn bails(&self) -> u32 {
        self.bails.get()
    }

    fn bail(&self) {
        self.bails.set(self.bails.get() + 1);
    }

    fn pid(&self, seat: Seat) -> &PlayerId {
        self.owners.id(seat)
    }

    fn seat_of(&self, player: &PlayerId) -> Option<Seat> {
        self.owners.seat(player)
    }

    fn sys_id(&self, s: System) -> &SystemId {
        self.sys_ids[s.index()].get_or_init(|| SystemId::new(s.as_str()))
    }

    fn planet_id(&self, p: Planet) -> &PlanetId {
        self.planet_ids[p.index()].get_or_init(|| PlanetId::new(p.as_str()))
    }

    fn outcome_text(&self, o: Outcome) -> String {
        match o {
            Outcome::Empty => String::new(),
            Outcome::For => "for".to_owned(),
            Outcome::Against => "against".to_owned(),
            Outcome::Seat(s) => self.pid(s).as_str().to_owned(),
            Outcome::Planet(p) => p.as_str().to_owned(),
            Outcome::StrategyCard(c) => c.as_str().to_owned(),
            Outcome::Law(a) => a.as_str().to_owned(),
            Outcome::Secret(s) => s.as_str().to_owned(),
        }
    }

    fn model_unit(&self, p: &ti4_engine_core::state::Piece) -> Unit {
        Unit {
            type_id: ti4_model::id::UnitTypeId::new(p.kind().as_str()),
            owner: self.pid(p.owner()).clone(),
            sustained_damage: p.sustained(),
            galvanized: p.galvanized(),
        }
    }

    /// The string model of one system (units, tokens, control, units on planets), as `SeenOwned::to_model_system`.
    fn model_system(&self, system: System) -> SystemState {
        let view = self.seen.system(system);
        let mut out = SystemState::default();
        out.units = view.units().iter().map(|p| self.model_unit(p)).collect();
        out.command_tokens = view.tokens().iter().map(|s| self.pid(s).clone()).collect();
        for planet in view.planets().as_slice() {
            let cell = view.planet(*planet);
            if let Some(seat) = cell.control {
                out.planet_control.insert(PlanetId::new(planet.as_str()), self.pid(seat).clone());
            }
            if !cell.units.is_empty() {
                out.planet_units.insert(PlanetId::new(planet.as_str()), cell.units.iter().map(|p| self.model_unit(p)).collect());
            }
        }
        out
    }

    fn sheet(&self, seat: Seat) -> Option<&Sheet> {
        self.sheets.get(usize::from(seat.0))?.get_or_init(|| {
            let v = self.seen.seat(seat)?;
            Some(Sheet {
                faction: FactionId::new(v.faction.as_str()),
                fleet_tokens: v.fleet_tokens,
                strategy_cards: v.strategy_cards.iter().map(|c| StrategyCardId::new(c.as_str())).collect(),
                exhausted_strategy_cards: v.exhausted_strategy_cards.iter().map(|c| StrategyCardId::new(c.as_str())).collect(),
                technologies: v.technologies.iter().map(|t| TechnologyId::new(t.as_str())).collect(),
                exhausted_technologies: v.exhausted_technologies.iter().map(|t| TechnologyId::new(t.as_str())).collect(),
                relics: v.relics.iter().map(|r| RelicId::new(r.as_str())).collect(),
                exhausted_relics: v.exhausted_relics.iter().map(|r| RelicId::new(r.as_str())).collect(),
                exploration_cards: v.exploration_cards.iter().map(|c| c.as_str().to_owned()).collect(),
                relic_fragments: FRAGMENT_KEYS
                    .iter()
                    .filter_map(|(kind, key)| v.relic_fragments.get(*kind).map(|n| ((*key).to_owned(), i32::from(n))))
                    .collect(),
                breakthrough: v.breakthrough.map(|b| BreakthroughId::new(b.as_str())),
                leaders: v.leaders.status.iter().map(|(k, st)| (LeaderId::new(k.as_str()), leader_status(*st))).collect(),
            })
        })
        .as_ref()
    }

    fn card_rows(&self, rows: &[ProgressRow]) -> Vec<CardProgress> {
        rows.iter().map(card_of).collect()
    }
}

const fn leader_status(s: ti4_engine_core::state::LeaderStatus) -> LeaderStatus {
    use ti4_engine_core::state::LeaderStatus as C;
    match s {
        C::Locked => LeaderStatus::Locked,
        C::Readied => LeaderStatus::Readied,
        C::Exhausted => LeaderStatus::Exhausted,
        C::Unlocked => LeaderStatus::Unlocked,
        C::Purged => LeaderStatus::Purged,
    }
}

const fn core_spend(kind: Spend) -> CoreSpend {
    match kind {
        Spend::Resources => CoreSpend::Resources,
        Spend::Influence => CoreSpend::Influence,
    }
}

/// The row of a core progress, as the policy feature path (and the bot) consume it.
fn card_of(row: &ProgressRow) -> CardProgress {
    use ti4_engine::objectives as oc;
    let alias = match row.objective {
        ObjectiveRef::Public(o) => o.as_str(),
        ObjectiveRef::Secret(o) => o.as_str(),
    };
    #[expect(clippy::cast_precision_loss, reason = "small integer counts")]
    let (have, threshold) = (row.have as f64, row.threshold as f64);
    let family_token = match row.family {
        RowFamily::Count(f) => ti4_engine::objectives::family_token(&oc::family(f)),
        RowFamily::Cost(f) => {
            let p = oc::cost_progress_of(ti4_engine_core::query::objectives::CostProgress { family: f, have: row.have, target: row.threshold });
            ti4_engine::objectives::cost_family_token(&p.family)
        }
    };
    CardProgress { alias: alias.to_owned(), family_token, have, threshold, satisfied: row.satisfied, stage: row.stage }
}

/// An imagined option in typed form (an id the tables do not know is dropped, as the thin crate's `TypedImagined`).
struct Typed {
    planets: Vec<Planet>,
    systems: Vec<System>,
    technologies: Vec<Technology>,
    structures: Vec<(System, Planet)>,
}

impl Typed {
    fn of(im: &Imagined<'_>) -> Self {
        Self {
            planets: im.planets.iter().filter_map(|p| <Planet as IdStr>::from_id(p.as_str())).collect(),
            systems: im.systems.iter().filter_map(|s| <System as IdStr>::from_id(s)).collect(),
            technologies: im.technologies.iter().filter_map(|t| <Technology as IdStr>::from_id(t)).collect(),
            structures: im.structures.iter().filter_map(|(s, p)| Some((<System as IdStr>::from_id(s)?, <Planet as IdStr>::from_id(p)?))).collect(),
        }
    }

    fn get(&self) -> ti4_engine_core::query::objectives::Imagined<'_> {
        ti4_engine_core::query::objectives::Imagined {
            planets: &self.planets,
            systems: &self.systems,
            technologies: &self.technologies,
            structures: &self.structures,
        }
    }
}

impl View for SeenView<'_> {
    fn content(&self) -> &ContentStore {
        self.content
    }
    fn sources(&self) -> SourceSet {
        self.sources
    }
    fn galaxy(&self) -> Option<&Galaxy> {
        self.galaxy
    }
    fn round(&self) -> u32 {
        self.seen.round()
    }
    fn phase(&self) -> Phase {
        use ti4_engine_core::state::Phase as C;
        match self.seen.phase() {
            C::Strategy => Phase::Strategy,
            C::Action => Phase::Action,
            C::Status => Phase::Status,
            C::Agenda => Phase::Agenda,
        }
    }
    fn active_player(&self) -> Option<&PlayerId> {
        self.seen.active().map(|s| self.pid(s))
    }
    fn speaker(&self) -> &PlayerId {
        self.pid(self.seen.speaker())
    }
    fn custodians_removed(&self) -> bool {
        self.seen.custodians_removed()
    }
    fn laws(&self) -> &BTreeMap<String, String> {
        self.laws.get_or_init(|| {
            Agenda::ALL
                .iter()
                .filter_map(|law| self.seen.law_outcome(*law).map(|o| (law.as_str().to_owned(), self.outcome_text(o))))
                .collect()
        })
    }
    fn law_outcome(&self, alias: &str) -> Option<&str> {
        self.laws().get(alias).map(String::as_str)
    }
    fn players(&self) -> Vec<&PlayerId> {
        self.seen.players().iter().map(|s| self.pid(*s)).collect()
    }
    fn seat(&self, player: &PlayerId) -> Option<PublicSeat<'_>> {
        let seat = self.seat_of(player)?;
        let v = self.seen.seat(seat)?;
        let sheet = self.sheet(seat)?;
        Some(PublicSeat {
            faction: &sheet.faction,
            victory_points: v.victory_points,
            trade_goods: v.trade_goods,
            commodities: v.commodities,
            tactic_tokens: v.tactic_tokens,
            fleet_tokens: sheet.fleet_tokens,
            strategic_tokens: v.strategic_tokens,
            strategy_cards: &sheet.strategy_cards,
            exhausted_strategy_cards: &sheet.exhausted_strategy_cards,
            technologies: &sheet.technologies,
            exhausted_technologies: &sheet.exhausted_technologies,
            action_cards_held: v.action_cards_held,
            secret_objectives_held: v.secret_objectives_held,
            passed: v.passed,
            relics: &sheet.relics,
            exhausted_relics: &sheet.exhausted_relics,
            exploration_cards: &sheet.exploration_cards,
            relic_fragments: &sheet.relic_fragments,
            breakthrough: sheet.breakthrough.as_ref(),
            leaders: &sheet.leaders,
        })
    }
    fn board(&self) -> &BTreeMap<SystemId, SystemState> {
        self.board.get_or_init(|| {
            System::ALL
                .iter()
                .filter_map(|s| {
                    let m = self.model_system(*s);
                    let touched = !m.units.is_empty() || !m.command_tokens.is_empty() || !m.planet_control.is_empty() || !m.planet_units.is_empty();
                    touched.then(|| (self.sys_id(*s).clone(), m))
                })
                .collect()
        })
    }
    fn system(&self, system: &SystemId) -> SystemState {
        <System as IdStr>::from_id(system.as_str()).map_or_else(SystemState::default, |s| self.model_system(s))
    }
    fn active_system(&self) -> Option<&SystemId> {
        self.seen.active_system().map(|s| self.sys_id(s))
    }
    fn controlled_planets(&self, player: &PlayerId) -> Vec<(&SystemId, &PlanetId)> {
        let Some(seat) = self.seat_of(player) else { return Vec::new() };
        self.seen.controlled_planets(seat).as_slice().iter().map(|(s, p)| (self.sys_id(*s), self.planet_id(*p))).collect()
    }
    fn planet_is_ready(&self, planet: &PlanetId) -> bool {
        <Planet as IdStr>::from_id(planet.as_str()).is_some_and(|p| self.seen.planet_is_ready(p))
    }
    fn systems_with_token(&self, player: &PlayerId) -> BTreeSet<&SystemId> {
        let Some(seat) = self.seat_of(player) else { return BTreeSet::new() };
        self.seen.systems_with_token(seat).iter().map(|i| self.sys_id(ti4_engine_core::ids::system_at(i))).collect()
    }
    fn can_reach(&self, player: &PlayerId, origin: &SystemId, destination: &SystemId, move_value: i32) -> bool {
        let (Some(seat), Some(from), Some(to)) =
            (self.seat_of(player), <System as IdStr>::from_id(origin.as_str()), <System as IdStr>::from_id(destination.as_str()))
        else {
            return false;
        };
        self.seen.can_reach(seat, from, to, move_value)
    }
    fn movable_into(&self, player: &PlayerId, destination: &SystemId) -> Vec<Movable> {
        let (Some(seat), Some(to)) = (self.seat_of(player), <System as IdStr>::from_id(destination.as_str())) else { return Vec::new() };
        let found = self.seen.movable_into(seat, to);
        found
            .iter()
            .filter_map(|m| {
                let origin = SystemId::new(m.origin.as_str());
                // The hull is the `index`th unit of the player in the origin system (`GameState::ships_of`).
                let unit = self.model_system(m.origin).units.into_iter().filter(|u| &u.owner == player).nth(usize::from(m.index))?;
                Some(Movable { origin, index: usize::from(m.index), unit, capacity: m.capacity, gravity_drive: m.gravity_drive, ionian: m.ionian })
            })
            .collect()
    }
    fn rift_exits_into(&self, player: &PlayerId, origin: &SystemId, unit_type: &str, boosts: (bool, bool), destination: &SystemId) -> usize {
        let (Some(seat), Some(from), Some(to), Some(kind)) = (
            self.seat_of(player),
            <System as IdStr>::from_id(origin.as_str()),
            <System as IdStr>::from_id(destination.as_str()),
            <CoreUnit as IdStr>::from_id(unit_type),
        ) else {
            return 0;
        };
        self.seen.rift_exits_into(seat, from, kind, boosts, to)
    }
    fn loadable(&self, player: &PlayerId, origin: &SystemId) -> Vec<Cargo> {
        let (Some(seat), Some(from)) = (self.seat_of(player), <System as IdStr>::from_id(origin.as_str())) else { return Vec::new() };
        self.seen
            .loadable(seat, from)
            .iter()
            .map(|c| Cargo {
                unit: self.model_unit(&c.piece),
                source: c.source.map_or(CargoSource::Space, |p| CargoSource::Planet(PlanetId::new(p.as_str()))),
                system: SystemId::new(c.system.as_str()),
            })
            .collect()
    }
    fn available_spend(&self, player: &PlayerId, kind: Spend) -> i64 {
        self.seat_of(player).map_or(0, |seat| self.seen.available_spend(seat, core_spend(kind)))
    }
    fn agenda_in_play(&self) -> Option<&str> {
        self.seen.agenda_in_play().map(Agenda::as_str)
    }
    fn agendas_resolved_this_round(&self) -> usize {
        self.seen.agendas_resolved_this_round()
    }
    fn has_agenda_prediction(&self, player: &PlayerId) -> bool {
        self.seat_of(player).is_some_and(|seat| self.seen.agenda_prediction_of(seat).is_some())
    }
    fn invasion(&self) -> Option<InvasionFacts> {
        let i = self.seen.invasion()?;
        Some(InvasionFacts { system: SystemId::new(i.system.as_str()), invader: self.pid(i.invader).clone(), defender: i.defender.map(|d| self.pid(d).clone()) })
    }
    fn space_battle(&self) -> Option<(SystemId, PlayerId, PlayerId)> {
        let (s, a, d) = self.seen.space_battle()?;
        Some((SystemId::new(s.as_str()), self.pid(a).clone(), self.pid(d).clone()))
    }
    fn promissory_notes(&self) -> BTreeMap<String, PlayerId> {
        self.seen
            .promissory_notes_in_play()
            .iter()
            .map(|(k, holder)| {
                let id = k.note.as_str();
                (format!("{}:{}", id.strip_prefix("<color>_").unwrap_or(id), k.faction.as_str()), self.pid(*holder).clone())
            })
            .collect()
    }
    fn strategy_card_goods(&self, card: &StrategyCardId) -> i32 {
        <StrategyCard as IdStr>::from_id(card.as_str()).map_or(0, |c| self.seen.strategy_card_goods(c))
    }
    fn scored_by(&self, player: &PlayerId) -> BTreeSet<ObjectiveId> {
        let Some(seat) = self.seat_of(player) else { return BTreeSet::new() };
        let scored = self.seen.scored_by(seat);
        scored
            .public
            .iter()
            .map(|o| ObjectiveId::new(o.as_str()))
            .chain(scored.secret.iter().map(|o| ObjectiveId::new(o.as_str())))
            .collect()
    }
    fn scoreable_public(&self, player: &PlayerId) -> usize {
        let Some(seat) = self.seat_of(player) else { return 0 };
        self.seen.scoreable_public(seat).unwrap_or_else(|| {
            self.bail();
            0
        })
    }
    fn revealed_objective_progress_imagining(&self, player: &PlayerId, imagined: &Imagined<'_>) -> Vec<CardProgress> {
        let Some(seat) = self.seat_of(player) else { return Vec::new() };
        let typed = Typed::of(imagined);
        self.seen.revealed_objective_progress(seat, typed.get()).map_or_else(
            || {
                self.bail();
                Vec::new()
            },
            |rows| self.card_rows(&rows),
        )
    }
    fn objectives_implicating_system(&self, player: &PlayerId, system: &SystemId) -> Vec<ObjectiveId> {
        let (Some(seat), Some(sys)) = (self.seat_of(player), <System as IdStr>::from_id(system.as_str())) else { return Vec::new() };
        self.seen.objectives_implicating_system(seat, sys).as_slice().iter().map(|o| ObjectiveId::new(o.as_str())).collect()
    }
}

impl SeatView for SeenView<'_> {
    fn observed(&self) -> &dyn View {
        self
    }
    fn bound_seat(&self) -> &PlayerId {
        self.pid(self.me)
    }
    fn held_secrets(&self) -> Vec<SecretObjectiveId> {
        self.seen.my_secrets().iter().map(|s| SecretObjectiveId::new(s.as_str())).collect()
    }
    fn held_action_cards(&self) -> Vec<ActionCardId> {
        self.seen.my_action_cards().iter().map(|c| ActionCardId::new(c.as_str())).collect()
    }
    fn held_secret_progress(&self) -> Vec<CardProgress> {
        self.held_secret_progress_imagining(&Imagined::NONE)
    }
    fn held_secret_progress_imagining(&self, imagined: &Imagined<'_>) -> Vec<CardProgress> {
        let typed = Typed::of(imagined);
        self.seen.my_secret_progress(typed.get()).map_or_else(
            || {
                self.bail();
                Vec::new()
            },
            |rows| self.card_rows(&rows),
        )
    }
}
