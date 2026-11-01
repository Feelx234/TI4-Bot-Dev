//! The Embers of Muaat's component actions (package W11, lent-bodies lane 7; the string bodies of `factions/muaat.rs`): Star Forge and The
//! Nucleus as one [`Window`] (with the Ember Colossus placements that follow a forge), The Inferno and Fires of the Gashlai as functions.
//!
//! The placements are made under the fleet supply and the space capacity (`kernel::fx::max_fit`: the units that leave the fleet pool and
//! the capacity no worse than they are now, then the box). The adapter vouches for what the core does not hold: the unit forms the seat
//! places (upgrades and faction overrides), the fleet limit (pool, laws, abilities) and the systems next to the forge for this seat.

use crate::choice::generic::Content;
use crate::choice::promissory::{Ask as PAsk, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Faction, Planet, System, Technology, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next, fx, fx_strategy};
use crate::query::tech::UnitRoster;
use crate::query::{board, explore};
use crate::state::{Code, CoreState};
use crate::tables::Scope;

/// What the position tells the Star Forge window besides the state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeFacts {
    pub scope: Scope,
    /// The fighter, the destroyer and the infantry the seat places (`action_cards::placed_unit_id`; `None`: the tables have none).
    pub fighter: Option<Unit>,
    pub destroyer: Option<Unit>,
    pub infantry: Option<Unit>,
    /// The fleet limit of the seat (`fleet::limit`, or the unlimited bill).
    pub fleet_limit: i64,
    /// For each system of a war sun: the systems next to it for this seat (`PlayerAdjacency::neighbours`; empty without a galaxy).
    pub near: Vec<(System, Vec<System>)>,
    /// The interned `star_forge`: the source of the question and the reason of the token event.
    pub source: Code,
    /// The interned `muaat_mech`: the source of the Ember Colossus question.
    pub mech: Code,
}

/// Which action the window performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Star Forge: a strategy token is spent once the units stand.
    StarForge,
    /// The Nucleus (Avernus): no token; the planet's ability card is exhausted after the forge.
    Nucleus,
}

/// The suspension points of [`StarForge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FStage {
    Start,
    /// Waiting on "place which units, and where".
    Forge,
    /// Waiting on an Ember Colossus: place 1 infantry with this mech.
    Ember,
    Done,
}

/// Star Forge: "ACTION: Spend 1 token from your strategy pool to place either 2 fighters or 1 destroyer from your reinforcements in a system
/// that contains 1 or more of your war suns", and The Nucleus (Avernus): the same without the token, exhausting the legendary card.
/// The system and the choice are made before the token is spent; the units are placed first (the box and the fleet supply can refuse them,
/// and then nothing is spent and the action did not resolve). Then each mech of the seat in the forge's system or next to it may place one
/// infantry with it (Ember Colossus), asked once per mech. [`Self::done`] is whether the action resolved.
#[derive(Debug, Clone)]
pub struct StarForge {
    pub who: Seat,
    mode: Mode,
    facts: ForgeFacts,
    stage: FStage,
    /// The forge's mechs still to ask, in the string's order (`(system, planet)`, `None`: the space area).
    mechs: Vec<(System, Option<Planet>)>,
    at: usize,
    pub done: bool,
}

impl StarForge {
    #[must_use]
    pub fn new(who: Seat, mode: Mode, facts: ForgeFacts) -> Self {
        Self { who, mode, facts, stage: FStage::Start, mechs: Vec::new(), at: 0, done: false }
    }

    fn muaat(core: &CoreState, who: Seat) -> bool {
        core.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheEmbersOfMuaat)
    }

    /// The systems holding a war sun of the seat, in system order.
    pub fn war_sun_systems(core: &CoreState, who: Seat) -> Vec<System> {
        let mut found: Vec<System> = core
            .board()
            .systems()
            .filter(|(_, cell)| cell.units.as_slice().iter().any(|p| p.owner() == who && p.kind().data().base_type == UnitBaseType::Warsun))
            .map(|(system, _)| system)
            .collect();
        found.sort_unstable();
        found
    }

    /// `forge_options`: `(system, fighters)` for each war sun's system: two fighters, then one destroyer, each when the box holds one and
    /// some of them fit.
    fn options(&self, core: &CoreState, k: &Kernel) -> Vec<(System, bool)> {
        let mut found = Vec::new();
        for system in Self::war_sun_systems(core, self.who) {
            for (fighters, form, wanted) in [(true, self.facts.fighter, 2), (false, self.facts.destroyer, 1)] {
                let Some(unit) = form else { continue };
                if fx::box_has(core, k, self.facts.scope, self.who, unit)
                    && fx::max_fit(core, k, self.facts.scope, self.who, system, true, unit, wanted, self.facts.fleet_limit) > 0
                {
                    found.push((system, fighters));
                }
            }
        }
        found
    }

    fn ctx(&self, source: Code, subtype: PSub) -> Ctx {
        Ctx {
            origin: Origin::FactionAbility(Content::Other(source)),
            subtype: Subtype::Promissory(subtype),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    /// The mechs of the seat in the forge's system or next to it, in system order, a planet's mechs by planet and the space area last.
    fn mechs_near(&self, core: &CoreState, forge: System) -> Vec<(System, Option<Planet>)> {
        let mut near = vec![forge];
        if let Some((_, next_to)) = self.facts.near.iter().find(|(system, _)| *system == forge) {
            near.extend(next_to.iter().copied());
        }
        near.sort_unstable();
        near.dedup();
        let mech = |p: &crate::state::Piece| p.owner() == self.who && p.kind() == Unit::EmberColossus;
        let mut found = Vec::new();
        for system in near {
            let mut planets: Vec<Planet> = board::planets_of(core, system).collect();
            planets.sort_unstable();
            for planet in planets {
                for _ in board::planet_units(core, planet).iter().filter(|p| mech(p)) {
                    found.push((system, Some(planet)));
                }
            }
            for _ in board::space_units(core, system).iter().filter(|p| mech(p)) {
                found.push((system, None));
            }
        }
        found
    }

    /// The next mech that can take an infantry: skipped while the box holds none or none fits there.
    fn next_ember(&mut self, core: &CoreState, k: &Kernel) {
        while let Some((system, planet)) = self.mechs.get(self.at).copied() {
            let fits = self.facts.infantry.is_some_and(|unit| {
                fx::box_has(core, k, self.facts.scope, self.who, unit)
                    && fx::max_fit(core, k, self.facts.scope, self.who, system, planet.is_none(), unit, 1, self.facts.fleet_limit) > 0
            });
            if fits {
                self.stage = FStage::Ember;
                return;
            }
            self.at += 1;
        }
        self.finish(core);
    }

    fn finish(&mut self, core: &CoreState) {
        let _ = core;
        self.stage = FStage::Done;
    }

    /// The window is over: the Nucleus exhausts its card.
    fn close(&mut self, s: &mut CoreState) {
        if self.mode == Mode::Nucleus && self.done {
            if let Some(p) = s.players_mut().by_seat_mut(self.who) {
                p.cards.legendary_exhausted.insert(Planet::Avernus, ());
            }
        }
    }

    fn ember_ask(&self) -> Option<Ask> {
        let (system, planet) = self.mechs.get(self.at).copied()?;
        Some(Ask::Promissory(PAsk::EmberColossus { system, planet }))
    }
}

impl Window for StarForge {
    type Stage = FStage;

    fn stage(&self) -> FStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == FStage::Start {
            let ready = match self.mode {
                Mode::StarForge => {
                    Self::muaat(sx.state, self.who)
                        && sx.state.players().by_seat(self.who).is_some_and(|p| p.econ.strategic_tokens > 0)
                        && !self.options(sx.state, sx.kernel).is_empty()
                }
                Mode::Nucleus => explore::legendary_available(sx.state, self.who, Planet::Avernus),
            };
            self.stage = if ready { FStage::Forge } else { FStage::Done };
        }
        match self.stage {
            FStage::Forge => Next::Continue(Ask::Promissory(PAsk::StarForge)),
            FStage::Ember => self.ember_ask().map_or(Next::Done, Next::Continue),
            FStage::Start | FStage::Done => {
                self.close(sx.state);
                Next::Done
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            FStage::Forge => Some(Choice {
                actor: self.who,
                ask: Ask::Promissory(PAsk::StarForge),
                ctx: self.ctx(self.facts.source, PSub::StarForge),
                opts: self
                    .options(s, k)
                    .into_iter()
                    .map(|(system, fighters)| Opt::Promissory(POpt::Forge { system, fighters }))
                    .chain(std::iter::once(Opt::Decline))
                    .collect::<Opts>(),
            }),
            FStage::Ember => Some(Choice {
                actor: self.who,
                ask: self.ember_ask()?,
                ctx: self.ctx(self.facts.mech, PSub::EmberColossus),
                opts: [Opt::Promissory(POpt::Ember), Opt::Decline].into_iter().collect::<Opts>(),
            }),
            FStage::Start | FStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        match (self.stage, chosen) {
            (FStage::Forge, Opt::Decline) => {
                self.stage = FStage::Done;
                Ok(())
            }
            (FStage::Forge, Opt::Promissory(POpt::Forge { system, fighters })) if self.options(sx.state, sx.kernel).contains(&(system, fighters)) => {
                let (form, wanted) = if fighters { (self.facts.fighter, 2) } else { (self.facts.destroyer, 1) };
                let fit = form.map_or(0, |unit| fx::max_fit(sx.state, sx.kernel, self.facts.scope, self.who, system, true, unit, wanted, self.facts.fleet_limit));
                // Placed first: the box can still refuse, and then nothing has been spent.
                if fx::place_counted(sx.state, sx.kernel, self.facts.scope, self.who, system, None, form, fit) == 0 {
                    self.stage = FStage::Done;
                    return Ok(());
                }
                if self.mode == Mode::StarForge {
                    fx::spend_strategy_token_staged(sx.state, self.who, self.facts.source);
                }
                self.done = true;
                self.mechs = self.mechs_near(sx.state, system);
                self.at = 0;
                self.next_ember(sx.state, sx.kernel);
                Ok(())
            }
            (FStage::Ember, Opt::Decline) => {
                self.at += 1;
                self.next_ember(sx.state, sx.kernel);
                Ok(())
            }
            (FStage::Ember, Opt::Promissory(POpt::Ember)) => {
                let (system, planet) = self.mechs.get(self.at).copied().ok_or(Illegal)?;
                let _ = fx::place_counted(sx.state, sx.kernel, self.facts.scope, self.who, system, planet, self.facts.infantry, 1);
                self.at += 1;
                self.next_ember(sx.state, sx.kernel);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

/// What The Inferno needs of the position besides the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfernoFacts {
    pub scope: Scope,
    /// The cruiser the seat places (`None`: the tables have none).
    pub cruiser: Option<Unit>,
    pub fleet_limit: i64,
    /// The interned `inferno`: the reason of the token event.
    pub source: Code,
}

/// The system holding The Inferno (the Muaat flagship, or a Nekro flagship lent its text by the Z token) of the seat, in system order, if the cruiser can be placed there and the pool
/// holds a token (`inferno_system`).
#[must_use]
pub fn inferno_system(core: &CoreState, k: &Kernel, who: Seat, facts: &InfernoFacts) -> Option<System> {
    let tokens = core.players().by_seat(who).map_or(0, |p| p.econ.strategic_tokens);
    let cruiser = facts.cruiser?;
    if tokens <= 0 || !fx::box_has(core, k, facts.scope, who, cruiser) {
        return None;
    }
    let mut homes: Vec<System> = core
        .board()
        .systems()
        .filter(|(_, cell)| cell.units.as_slice().iter().any(|p| p.owner() == who && crate::query::reach::flagship_text(core, who, p.kind(), Unit::TheInferno)))
        .map(|(system, _)| system)
        .collect();
    homes.sort_unstable();
    let system = *homes.first()?;
    (fx::max_fit(core, k, facts.scope, who, system, true, cruiser, 1, facts.fleet_limit) > 0).then_some(system)
}

/// The Inferno: "ACTION: Spend 1 token from your strategy pool to place 1 cruiser in this system." The cruiser is placed first and the token
/// spent (and announced) after it. Whether the action resolved.
pub fn inferno(core: &mut CoreState, k: &Kernel, who: Seat, facts: &InfernoFacts) -> bool {
    let Some(system) = inferno_system(core, k, who, facts) else { return false };
    if fx::place_counted(core, k, facts.scope, who, system, None, facts.cruiser, 1) == 0 {
        return false;
    }
    fx::spend_strategy_token_staged(core, who, facts.source);
    true
}

/// Fires of the Gashlai (the Muaat promissory note, a component action of its holder): "Remove 1 token from the Muaat player's fleet pool
/// and return it to their reinforcements. Then, gain your war sun unit upgrade technology card. Then, return this card to the Muaat
/// player." `owner` is the Muaat seat (the first in seating order). No question: every condition is read again and nothing is written
/// unless all hold: the holder has the note of a Muaat seat's faction, plays another faction, has not the technology's text (owned, or carried by a
/// Valefar Assimilator), and the owner's
/// fleet pool holds a token. The units re-form to the upgrade (90.8). Whether the action resolved.
pub fn fires_of_the_gashlai(core: &mut CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: Seat, owner: Seat) -> bool {
    use crate::generated::PromissoryNote;
    let Some(p) = core.players().by_seat(who) else { return false };
    let Some(note) = core
        .cards()
        .promissory
        .iter()
        .find(|(key, holder)| **holder == who && key.note == PromissoryNote::FiresOfTheGashlai && key.faction == Faction::TheEmbersOfMuaat)
        .map(|(key, _)| *key)
    else {
        return false;
    };
    if p.faction == Faction::TheEmbersOfMuaat || crate::query::reach::technology_text(core, who, Technology::WarSun) {
        return false;
    }
    if core.players().by_seat(owner).is_none_or(|o| o.faction != Faction::TheEmbersOfMuaat || o.econ.fleet_tokens <= 0) {
        return false;
    }
    if let Some(o) = core.players_mut().by_seat_mut(owner) {
        o.econ.fleet_tokens -= 1;
    }
    fx::grant_technology(core, who, Technology::WarSun);
    fx_strategy::apply_unit_upgrades(core, k, scope, roster, who);
    fx::give_back_note(core, note);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players};

    const A: Seat = Seat(0);
    const B: Seat = Seat(1);

    fn kernel() -> Kernel {
        Kernel::plain()
    }

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(A, Faction::TheEmbersOfMuaat), Player::new(B, Faction::ALL[0])]);
        s.seats = crate::state::Seats::new(&[A, B], &[A, B], 1);
        s
    }

    fn plain(base: UnitBaseType) -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none() && u.data().upgrades_from.is_none()).expect("plain unit")
    }

    fn war_sun() -> Unit {
        Unit::WarSun
    }

    /// Two systems that have a planet each, in system order.
    fn systems() -> [(System, Planet); 3] {
        let mut found: Vec<(System, Planet)> = Vec::new();
        for planet in Planet::ALL {
            if let Some(system) = planet.data().system
                && !found.iter().any(|(s, _)| *s == system)
            {
                found.push((system, planet));
            }
        }
        found.sort_unstable();
        [found[0], found[1], found[2]]
    }

    fn facts() -> ForgeFacts {
        ForgeFacts {
            scope: Scope(u32::MAX),
            fighter: Some(plain(UnitBaseType::Fighter)),
            destroyer: Some(plain(UnitBaseType::Destroyer)),
            infantry: Some(plain(UnitBaseType::Infantry)),
            fleet_limit: 20,
            near: Vec::new(),
            source: Code(1),
            mech: Code(2),
        }
    }

    fn driver(mode: Mode, facts: ForgeFacts) -> Driver<StarForge> {
        Driver::new(StarForge::new(A, mode, facts), Resolver::new(&[A, B], Some(A)), AbilityTable::default(), 1)
    }

    fn with_war_sun(s: &mut CoreState, system: System) {
        fx::place_unit(s, A, system, None, war_sun());
    }

    fn tokens(s: &mut CoreState, n: i16) {
        s.players_mut().by_seat_mut(A).unwrap().econ.strategic_tokens = n;
    }

    fn count(s: &CoreState, system: System, base: UnitBaseType) -> usize {
        board::space_units(s, system).iter().filter(|p| p.owner() == A && p.kind().data().base_type == base).count()
    }

    fn asked(status: Status) -> Choice {
        match status {
            Status::Asking(c) => c,
            Status::Done => panic!("the window is over"),
        }
    }

    fn pick(d: &mut Driver<StarForge>, s: &mut CoreState, opt: Opt) -> Status {
        d.resume(s, &kernel(), Pick::Opt(opt)).unwrap()
    }

    #[test]
    fn star_forge_places_two_fighters_and_spends_the_token_after_the_units_stand() {
        let [(a, _), (b, _), _] = systems();
        let mut s = table();
        tokens(&mut s, 2);
        with_war_sun(&mut s, a);
        let mut d = driver(Mode::StarForge, facts());
        let q = asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(
            q.opts.as_slice(),
            &[Opt::Promissory(POpt::Forge { system: a, fighters: true }), Opt::Promissory(POpt::Forge { system: a, fighters: false }), Opt::Decline],
            "both choices at the war sun, then the decline"
        );
        assert_eq!(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: a, fighters: true })), Status::Done);
        assert!(d.window.done);
        assert_eq!(count(&s, a, UnitBaseType::Fighter), 2);
        assert_eq!(s.players().by_seat(A).unwrap().econ.strategic_tokens, 1);
        assert_eq!(count(&s, b, UnitBaseType::Fighter), 0);
        assert_eq!(s.pending().staged.len(), 1, "the spent token is announced at the game step");
        assert!(s.players().by_seat(A).unwrap().cards.legendary_exhausted.is_empty(), "only the Nucleus spends the planet's card");
    }

    #[test]
    fn the_nucleus_declined_leaves_the_planet_card_ready() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        with_war_sun(&mut s, a);
        s.board_mut().planet_mut(Planet::Avernus).control = Some(A);
        let mut d = driver(Mode::Nucleus, facts());
        asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(pick(&mut d, &mut s, Opt::Decline), Status::Done);
        assert!(!d.window.done);
        assert!(s.players().by_seat(A).unwrap().cards.legendary_exhausted.is_empty());
    }

    #[test]
    fn the_mechs_are_asked_in_system_order_whichever_system_is_the_forges() {
        let [(a, pa), (b, pb), _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, b);
        for (system, planet) in [(a, pa), (b, pb)] {
            s.board_mut().planet_mut(planet).control = Some(A);
            fx::place_unit(&mut s, A, system, Some(planet), Unit::EmberColossus);
        }
        let mut f = facts();
        f.near = vec![(b, vec![a])];
        let mut d = driver(Mode::StarForge, f);
        asked(d.run(&mut s, &kernel()).unwrap());
        let q = asked(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: b, fighters: true })));
        assert_eq!(q.ask, Ask::Promissory(PAsk::EmberColossus { system: a, planet: Some(pa) }), "the neighbour that sorts first is asked first");
        let q = asked(pick(&mut d, &mut s, Opt::Decline));
        assert_eq!(q.ask, Ask::Promissory(PAsk::EmberColossus { system: b, planet: Some(pb) }));
    }

    #[test]
    fn star_forge_needs_a_muaat_seat_a_token_and_a_war_sun() {
        let [(a, _), _, _] = systems();
        let run = |muaat: bool, n: i16, sun: bool| {
            let mut s = table();
            if !muaat {
                s.players_mut().by_seat_mut(A).unwrap().faction = Faction::ALL[0];
            }
            tokens(&mut s, n);
            if sun {
                with_war_sun(&mut s, a);
            }
            let before = s.clone();
            let mut d = driver(Mode::StarForge, facts());
            assert!(matches!(d.run(&mut s, &kernel()).unwrap(), Status::Done), "no question when it cannot be done");
            assert!(!d.window.done);
            assert_eq!(s, before, "nothing written");
        };
        run(false, 1, true);
        run(true, 0, true);
        run(true, 1, false);
    }

    #[test]
    fn star_forge_declined_spends_nothing() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        let before = s.clone();
        let mut d = driver(Mode::StarForge, facts());
        asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(pick(&mut d, &mut s, Opt::Decline), Status::Done);
        assert!(!d.window.done);
        assert_eq!(s, before);
    }

    #[test]
    fn a_choice_that_was_not_offered_is_refused() {
        let [(a, _), (b, _), _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        let mut d = driver(Mode::StarForge, facts());
        asked(d.run(&mut s, &kernel()).unwrap());
        assert!(d.resume(&mut s, &kernel(), Pick::Opt(Opt::Promissory(POpt::Forge { system: b, fighters: true }))).is_err());
    }

    #[test]
    fn the_fleet_supply_cuts_the_forge_and_hides_what_does_not_fit() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        // A limit of one ship: the war sun is not a fleet-supply ship? It is: it fills the pool, so no destroyer fits; the fighters ride
        // in the war sun's capacity.
        let mut f = facts();
        f.fleet_limit = 1;
        let mut d = driver(Mode::StarForge, f);
        let q = asked(d.run(&mut s, &kernel()).unwrap());
        assert!(!q.opts.as_slice().contains(&Opt::Promissory(POpt::Forge { system: a, fighters: false })), "a destroyer would break the pool");
    }

    #[test]
    fn the_forge_places_only_as_many_fighters_as_the_capacity_holds() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        // The war sun carries six: five fighters stand, so one of the two fits.
        for _ in 0..5 {
            fx::place_unit(&mut s, A, a, None, plain(UnitBaseType::Fighter));
        }
        let mut d = driver(Mode::StarForge, facts());
        asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: a, fighters: true })), Status::Done);
        assert_eq!(count(&s, a, UnitBaseType::Fighter), 6, "one more, not two");
        assert!(d.window.done);
        assert_eq!(s.players().by_seat(A).unwrap().econ.strategic_tokens, 0);
    }

    #[test]
    fn a_unit_whose_box_is_empty_is_not_offered() {
        let [(a, _), (b, _), _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        // All eight destroyers stand elsewhere.
        for _ in 0..8 {
            fx::place_unit(&mut s, A, b, None, plain(UnitBaseType::Destroyer));
        }
        let mut f = facts();
        f.fleet_limit = 99;
        let mut d = driver(Mode::StarForge, f);
        let q = asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(q.opts.as_slice(), &[Opt::Promissory(POpt::Forge { system: a, fighters: true }), Opt::Decline], "the fighters only");
    }

    #[test]
    fn a_faction_rule_that_bars_the_form_hides_it_and_places_none() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        let mut barred = Kernel::plain();
        barred.hooks.economy.effect_placement_forbidden.push(|_, _, _, u| u.data().base_type == UnitBaseType::Destroyer);
        assert!(!fx::box_has(&s, &barred, Scope(u32::MAX), A, plain(UnitBaseType::Destroyer)));
        assert!(fx::box_has(&s, &barred, Scope(u32::MAX), A, plain(UnitBaseType::Fighter)));
        let mut d = Driver::new(StarForge::new(A, Mode::StarForge, facts()), Resolver::new(&[A, B], Some(A)), AbilityTable::default(), 1);
        let q = asked(d.run(&mut s, &barred).unwrap());
        assert_eq!(q.opts.as_slice(), &[Opt::Promissory(POpt::Forge { system: a, fighters: true }), Opt::Decline]);
        assert_eq!(fx::place_counted(&mut s, &barred, Scope(u32::MAX), A, a, None, Some(plain(UnitBaseType::Destroyer)), 1), 0);
    }

    #[test]
    fn units_are_placed_only_as_far_as_the_box_holds() {
        let [(a, _), (b, _), _] = systems();
        let mut s = table();
        for _ in 0..7 {
            fx::place_unit(&mut s, A, b, None, plain(UnitBaseType::Destroyer));
        }
        let placed = fx::place_counted(&mut s, &kernel(), Scope(u32::MAX), A, a, None, Some(plain(UnitBaseType::Destroyer)), 3);
        assert_eq!(placed, 1, "one of eight is left");
        assert_eq!(count(&s, a, UnitBaseType::Destroyer), 1);
        assert_eq!(fx::place_counted(&mut s, &kernel(), Scope(u32::MAX), A, a, None, None, 2), 0, "a form the seat does not place");
    }

    #[test]
    fn the_nucleus_costs_no_token_and_exhausts_the_planet_card_after_the_units_stand() {
        let [(a, _), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 0);
        with_war_sun(&mut s, a);
        s.board_mut().planet_mut(Planet::Avernus).control = Some(A);
        let mut d = driver(Mode::Nucleus, facts());
        asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: a, fighters: false })), Status::Done);
        assert!(d.window.done);
        assert_eq!(count(&s, a, UnitBaseType::Destroyer), 1);
        assert!(s.players().by_seat(A).unwrap().cards.legendary_exhausted.contains_key(&Planet::Avernus), "the card is spent");
        assert_eq!(s.pending().staged.len(), 0, "no token, no event");
        // A card already spent is not offered again.
        let mut again = driver(Mode::Nucleus, facts());
        assert!(matches!(again.run(&mut s, &kernel()).unwrap(), Status::Done));
        assert!(!again.window.done);
    }

    #[test]
    fn an_ember_colossus_places_an_infantry_with_the_mech_in_the_forge_system_and_next_to_it() {
        let [(a, pa), (b, pb), (c, pc)] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        // One mech on a planet of the forge's system, one in a system next to it, one in a system that is not.
        for (system, planet) in [(a, pa), (b, pb), (c, pc)] {
            s.board_mut().planet_mut(planet).control = Some(A);
            fx::place_unit(&mut s, A, system, Some(planet), Unit::EmberColossus);
        }
        let mut f = facts();
        f.near = vec![(a, vec![b])];
        let mut d = driver(Mode::StarForge, f);
        asked(d.run(&mut s, &kernel()).unwrap());
        let q = asked(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: a, fighters: true })));
        assert_eq!(q.ask, Ask::Promissory(PAsk::EmberColossus { system: a, planet: Some(pa) }));
        assert_eq!(q.opts.as_slice(), &[Opt::Promissory(POpt::Ember), Opt::Decline]);
        let q = asked(pick(&mut d, &mut s, Opt::Promissory(POpt::Ember)));
        assert_eq!(q.ask, Ask::Promissory(PAsk::EmberColossus { system: b, planet: Some(pb) }), "the neighbour's mech is next, the far one is never asked");
        assert_eq!(pick(&mut d, &mut s, Opt::Decline), Status::Done);
        let infantry = |planet: Planet| board::planet_units(&s, planet).iter().filter(|p| p.owner() == A && p.kind().data().base_type == UnitBaseType::Infantry).count();
        assert_eq!((infantry(pa), infantry(pb), infantry(pc)), (1, 0, 0), "only the one that was accepted");
    }

    #[test]
    fn an_ember_colossus_is_not_asked_when_no_infantry_fits() {
        let [(a, pa), _, _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        with_war_sun(&mut s, a);
        s.board_mut().planet_mut(pa).control = Some(A);
        fx::place_unit(&mut s, A, a, Some(pa), Unit::EmberColossus);
        let mut f = facts();
        f.infantry = None;
        let mut d = driver(Mode::StarForge, f);
        asked(d.run(&mut s, &kernel()).unwrap());
        assert_eq!(pick(&mut d, &mut s, Opt::Promissory(POpt::Forge { system: a, fighters: true })), Status::Done, "the seat places no infantry: nothing to ask");
        assert!(d.window.done);
    }

    fn inferno_facts() -> InfernoFacts {
        InfernoFacts { scope: Scope(u32::MAX), cruiser: Some(plain(UnitBaseType::Cruiser)), fleet_limit: 20, source: Code(3) }
    }

    #[test]
    fn the_inferno_places_the_cruiser_in_the_flagships_system_and_spends_the_token() {
        let [(a, _), (b, _), _] = systems();
        let mut s = table();
        tokens(&mut s, 1);
        fx::place_unit(&mut s, A, b, None, Unit::TheInferno);
        fx::place_unit(&mut s, A, a, None, Unit::TheInferno);
        assert_eq!(inferno_system(&s, &kernel(), A, &inferno_facts()), Some(a), "the first system in system order");
        assert!(inferno(&mut s, &kernel(), A, &inferno_facts()));
        assert_eq!((count(&s, a, UnitBaseType::Cruiser), count(&s, b, UnitBaseType::Cruiser)), (1, 0));
        assert_eq!(s.players().by_seat(A).unwrap().econ.strategic_tokens, 0);
        assert_eq!(s.pending().staged.len(), 1);
        assert!(!inferno(&mut s, &kernel(), A, &inferno_facts()), "no token left");
    }

    #[test]
    fn the_inferno_changes_nothing_without_a_flagship_a_token_a_form_or_room() {
        let [(a, _), _, _] = systems();
        let run = |flagship: bool, n: i16, f: InfernoFacts| {
            let mut s = table();
            tokens(&mut s, n);
            if flagship {
                fx::place_unit(&mut s, A, a, None, Unit::TheInferno);
            }
            let before = s.clone();
            assert!(!inferno(&mut s, &kernel(), A, &f));
            assert_eq!(s, before);
        };
        run(false, 1, inferno_facts());
        run(true, 0, inferno_facts());
        run(true, 1, InfernoFacts { cruiser: None, ..inferno_facts() });
        // A limit that the flagship and the cruiser together break (the flagship counts as a ship against the pool).
        run(true, 1, InfernoFacts { fleet_limit: 1, ..inferno_facts() });
    }

    fn fires_state() -> CoreState {
        use crate::generated::PromissoryNote;
        let mut s = table();
        s.players_mut().by_seat_mut(A).unwrap().econ.fleet_tokens = 2;
        let note = crate::state::NoteKey { note: PromissoryNote::FiresOfTheGashlai, faction: Faction::TheEmbersOfMuaat };
        s.cards_mut().promissory.edit().insert(note, B);
        s
    }

    #[test]
    fn fires_of_the_gashlai_returns_a_fleet_token_grants_the_war_sun_and_sends_the_note_home() {
        use crate::generated::PromissoryNote;
        let [(a, _), _, _] = systems();
        let mut s = fires_state();
        fx::place_unit(&mut s, B, a, None, Unit::NotAWarSunYet);
        let roster = UnitRoster::default();
        assert!(fires_of_the_gashlai(&mut s, &kernel(), Scope(u32::MAX), &roster, B, A));
        assert_eq!(board::space_units(&s, a)[0].kind(), Unit::WarSun, "the unit on the board takes the upgrade's form (90.8)");
        assert_eq!(s.players().by_seat(A).unwrap().econ.fleet_tokens, 1);
        assert!(s.players().by_seat(B).unwrap().tech.owned.contains(Technology::WarSun));
        let note = crate::state::NoteKey { note: PromissoryNote::FiresOfTheGashlai, faction: Faction::TheEmbersOfMuaat };
        assert_eq!(s.cards().promissory.get(&note), Some(&A), "the card goes home");
        assert!(!fires_of_the_gashlai(&mut s, &kernel(), Scope(u32::MAX), &roster, B, A), "the note is no longer held");
    }

    #[test]
    fn fires_of_the_gashlai_changes_nothing_when_a_condition_fails() {
        let roster = UnitRoster::default();
        let run = |edit: &dyn Fn(&mut CoreState)| {
            let mut s = fires_state();
            edit(&mut s);
            let before = s.clone();
            assert!(!fires_of_the_gashlai(&mut s, &kernel(), Scope(u32::MAX), &roster, B, A));
            assert_eq!(s, before);
        };
        run(&|s| s.players_mut().by_seat_mut(A).unwrap().econ.fleet_tokens = 0);
        run(&|s| {
            s.players_mut().by_seat_mut(B).unwrap().tech.owned.insert(Technology::WarSun);
        });
        run(&|s| s.players_mut().by_seat_mut(B).unwrap().faction = Faction::TheEmbersOfMuaat);
        run(&|s| s.cards_mut().promissory.edit().clear());
        run(&|s| s.players_mut().by_seat_mut(A).unwrap().faction = Faction::ALL[0]);
    }
}
