//! The faction component actions that the core owns (package W11, lent-bodies lane 5): each is a function over the state or a
//! [`crate::kernel::stage::Window`] in its own file section. The listing of the actions and the dispatch (`perform_component`)
//! are the driver's (D1b); the adapter calls the bodies from the string dispatch and compares.

use crate::choice::generic::Content;
use crate::choice::promissory::{Ask as PAsk, Opt as POpt, Subtype as PSub};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Report, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{Kernel, Next, fx, fx_strategy};
use crate::query::economy::Spend;
use crate::query::{board, fleet};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, Payload, Val};
use crate::tables::Scope;

/// Lazax Gate Folding (Winnu): "ACTION: Exhaust this card to place 1 infantry from your reinforcements on Mecatol Rex" (when the
/// seat controls it). No question: the planet is the first the seat controls in the Mecatol system (`mecatol`), the unit is the
/// form the seat places (`None`: it places nothing, or a faction rule forbids it). Returns whether the action resolved: the card is
/// ready, the seat controls a planet there and the box holds an infantry for it; the card is exhausted even when the unit then
/// cannot be placed (the string exhausts first), in which case the action did not resolve.
pub fn lazax_gate_folding(core: &mut CoreState, scope: Scope, who: Seat, mecatol: System, form: Option<Unit>) -> bool {
    let ready = core.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(Technology::LazaxGateFolding) && !p.tech.exhausted.contains(Technology::LazaxGateFolding));
    if !ready {
        return false;
    }
    let Some(planet) = board::controlled_planets(core, who).as_slice().iter().find(|(system, _)| *system == mecatol).map(|(_, planet)| *planet) else {
        return false;
    };
    let Some(unit) = form else { return false };
    if fleet::remaining_all(core, scope, who, unit) <= 0 {
        return false;
    }
    fx_strategy::exhaust_technology(core, who, Technology::LazaxGateFolding);
    fx::place_unit(core, who, mecatol, Some(planet), unit);
    true
}

/// ???_ERROR_ERROR_??? (Nekro Virus), the ACTION that repairs: "Exhaust this card to repair all of your damaged units." No question: every
/// unit of the seat, in the space areas and on the planets, is repaired and the card is exhausted. `false` (nothing written) when the seat
/// does not own the card; the offer (a ready card and a damaged unit) is the caller's.
pub fn c4r_repair(core: &mut CoreState, who: Seat) -> bool {
    // The card is the seat's own, or the Valefar Assimilator that carries its text; a card already exhausted does not repair.
    let Some(carrier) = fx_strategy::text_carrier(core, who, Technology::ERRORERROR) else { return false };
    if !fx_strategy::exhaust_technology(core, who, carrier) {
        return false;
    }
    let damaged = |list: &crate::state::UnitList| list.as_slice().iter().any(|p| p.owner() == who && p.sustained());
    let repair = |list: &mut crate::state::UnitList| {
        for piece in list.as_mut_slice() {
            if piece.owner() == who {
                *piece = piece.with_sustained(false);
            }
        }
    };
    let systems: Vec<System> = core.board().systems().filter(|(_, c)| damaged(&c.units)).map(|(s, _)| s).collect();
    let planets: Vec<crate::generated::Planet> = core.board().planets().filter(|(_, c)| damaged(&c.units)).map(|(p, _)| p).collect();
    for system in systems {
        repair(&mut core.board_mut().sys_mut(system).units);
    }
    for planet in planets {
        repair(&mut core.board_mut().planet_mut(planet).units);
    }
    true
}

/// Terraform (the Titans promissory note, a component action of its holder): "Attach this card to a non-home planet you control other
/// than Mecatol Rex: +1 resource, +1 influence, and all planet traits" once, anywhere in the game; the card is then in play, no longer a
/// note in hand. No question here (the planet is the option the caller chose): every condition is read again, and nothing is written
/// unless all hold. `home` says the planet is somebody's home planet (the seated homes and the printed homeworlds: a string read).
pub fn terraform(core: &mut CoreState, who: Seat, planet: Planet, home: bool) -> bool {
    use crate::generated::{Attachment, PromissoryNote};
    use crate::state::{NoteKey, PlanetFlags};
    let Some(own) = core.players().by_seat(who).map(|p| p.faction) else { return false };
    let Some(note) = core
        .cards()
        .promissory
        .iter()
        .find(|(key, holder)| **holder == who && key.note == PromissoryNote::Terraform && key.faction != own)
        .map(|(key, _)| *key)
    else {
        return false;
    };
    let attached = core.board().planets().any(|(_, c)| c.attach.as_slice().contains(&Attachment::Terraform));
    let controlled = board::controlled_planets(core, who).as_slice().iter().any(|(_, p)| *p == planet);
    if attached || !controlled || home || matches!(planet, Planet::MecatolRex | Planet::MecatolRexTe) {
        return false;
    }
    let cell = core.board_mut().planet_mut(planet);
    cell.attach.push(Attachment::Terraform);
    cell.flags.set(PlanetFlags::ATTACH_ENTRY, true);
    let note: NoteKey = note;
    core.cards_mut().promissory_faceup.edit().insert(note, ());
    true
}

/// Vaults of the Heir (Mahact breakthrough, a component action of Thunder's Edge): "ACTION: Exhaust this card and purge 1 of your
/// technologies to gain 1 relic." No question (the technology is the option the caller chose); every condition is read again and nothing is
/// written unless all hold: a Mahact seat holding the breakthrough, not yet exhausted this round (the `MahactVaultsExhausted` mark), a relic
/// left in the deck and the technology owned. The card is exhausted by the mark (it readies with the status phase), the technology is purged
/// (a unit upgrade takes its unit form with it) and the top relic is gained.
pub fn vaults_of_the_heir(core: &mut CoreState, scope: Scope, who: Seat, tech: Technology) -> bool {
    use crate::keys::{Atom, MarkKey, MarkVal};
    let Some(p) = core.players().by_seat(who) else { return false };
    if p.faction != crate::generated::Faction::TheMahactGeneSorcerers
        || p.cards.breakthrough != Some(crate::generated::Breakthrough::VaultsOfTheHeir)
        || !p.tech.owned.contains(tech)
        || core.marks().tail.get(&MarkKey::MahactVaultsExhausted(who)).is_some()
        || core.decks().relic.cards().is_empty()
    {
        return false;
    }
    core.marks_mut().tail.edit().insert(MarkKey::MahactVaultsExhausted(who), MarkVal::Atom(Atom::Num(1)));
    fx_strategy::purge_technology(core, scope, who, tech);
    crate::kernel::fx_relic::relic_gain(core, who);
    true
}

// -- Orbital Drop -----------------------------------------------------------------------------------

/// The suspension points of [`OrbitalDrop`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OStage {
    Start,
    Planet,
    Deploy,
    Paying,
    Done,
}

/// What the position tells the Orbital Drop window besides the state: the forms the seat places and the Sol mech, the scope, and the
/// interned texts (the question's source `orbital_drop` and the reason of the token event, the same text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbitalFacts {
    /// The infantry the seat places (`None`: it places none, or a faction rule forbids it).
    pub infantry: Option<Unit>,
    /// The Sol mech, whoever plays the card (`None`: the tables have none).
    pub mech: Option<Unit>,
    /// The seat has 3 resources to spend now (`production::available`).
    pub can_pay: bool,
    pub scope: Scope,
    /// The interned `orbital_drop`.
    pub source: Code,
}

/// Orbital Drop (Sol): "ACTION: Spend 1 token from your strategy pool to place 2 infantry from your reinforcements on any planet you
/// control. Then, you may spend 3 resources to place 1 mech on that planet." The planet is asked when the seat controls more than one
/// (any planet it controls; the question comes before the token is looked at, and nothing is written when there is none to spend); the
/// token is spent and announced, the infantry that the box allows are placed, and the mech is offered when the box holds one and the
/// seat can pay: a decline, or the payment (a child window) and then the mech. [`Self::done`] is whether the action resolved.
/// `unsupported`: the table has no link to the payment window, the caller declines the run.
#[derive(Debug, Clone)]
pub struct OrbitalDrop {
    pub who: Seat,
    facts: OrbitalFacts,
    stage: OStage,
    chosen: Option<(System, Planet)>,
    pay: ErasedChild,
    pub done: bool,
    pub unsupported: bool,
}

impl OrbitalDrop {
    #[must_use]
    pub fn new(who: Seat, facts: OrbitalFacts) -> Self {
        Self { who, facts, stage: OStage::Start, chosen: None, pay: ErasedChild::idle(), done: false, unsupported: false }
    }

    fn spots(&self, s: &CoreState) -> Vec<(System, Planet)> {
        board::controlled_planets(s, self.who).as_slice().to_vec()
    }

    fn ctx(&self, subtype: PSub) -> Ctx {
        Ctx {
            origin: Origin::FactionAbility(Content::Other(self.facts.source)),
            subtype: Subtype::Promissory(subtype),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    /// The planet is known: spend the token, place the infantry, offer the mech.
    fn dropped(&mut self, s: &mut CoreState, spot: (System, Planet)) {
        self.chosen = Some(spot);
        if !fx::spend_strategic_token(s, self.who) {
            // 22.3: it cannot resolve, so it is not performed.
            self.stage = OStage::Done;
            return;
        }
        fx::stage(
            s,
            EventRec {
                id: 0,
                kind: EventKind::StrategyTokenSpent,
                payload: Payload::new().with(Key::Player, Val::Seat(self.who)).with(Key::Reason, Val::Code(self.facts.source)),
                cancelled: false,
            },
        );
        if let Some(infantry) = self.facts.infantry {
            let room = fleet::remaining_all(s, self.facts.scope, self.who, infantry).clamp(0, 2);
            for _ in 0..room {
                fx::place_unit(s, self.who, spot.0, Some(spot.1), infantry);
            }
        }
        self.done = true;
        let mech_left = self.facts.mech.is_some_and(|mech| fleet::remaining_all(s, self.facts.scope, self.who, mech) > 0);
        self.stage = if mech_left && self.facts.can_pay { OStage::Deploy } else { OStage::Done };
    }

    fn deploy_ask(&self) -> Option<Ask> {
        let ((system, planet), mech) = (self.chosen?, self.facts.mech?);
        Some(Ask::Promissory(PAsk::OrbitalDeploy { system, planet, mech }))
    }
}

impl Window for OrbitalDrop {
    type Stage = OStage;

    fn stage(&self) -> OStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == OStage::Start {
            let spots = self.spots(sx.state);
            match spots.as_slice() {
                [] => self.stage = OStage::Done,
                [only] => self.dropped(sx.state, *only),
                _ => self.stage = OStage::Planet,
            }
        }
        if self.stage == OStage::Paying {
            match self.pay.step(sx) {
                Some(Ran::Asks(ask)) => return Next::Continue(ask),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    if let (Report::Paid(true), Some((system, planet)), Some(mech)) = (report, self.chosen, self.facts.mech) {
                        fx::place_unit(sx.state, self.who, system, Some(planet), mech);
                    }
                    self.stage = OStage::Done;
                }
                None => self.stage = OStage::Done,
            }
        }
        match self.stage {
            OStage::Planet => Next::Continue(Ask::Promissory(PAsk::OrbitalPlanet)),
            OStage::Deploy => self.deploy_ask().map_or(Next::Done, Next::Continue),
            OStage::Start | OStage::Paying | OStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            OStage::Planet => Some(Choice {
                actor: self.who,
                ask: Ask::Promissory(PAsk::OrbitalPlanet),
                ctx: self.ctx(PSub::OrbitalPlanet),
                opts: self.spots(s).into_iter().map(|(system, planet)| Opt::Promissory(POpt::Spot(system, planet))).collect::<Opts>(),
            }),
            OStage::Deploy => Some(Choice {
                actor: self.who,
                ask: self.deploy_ask()?,
                ctx: self.ctx(PSub::OrbitalDeploy),
                opts: [Opt::Promissory(POpt::Deploy), Opt::Decline].into_iter().collect::<Opts>(),
            }),
            OStage::Paying => self.pay.ask_of(s, k),
            OStage::Start | OStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (OStage::Planet, Pick::Opt(Opt::Promissory(POpt::Spot(system, planet)))) if self.spots(sx.state).contains(&(system, planet)) => {
                self.dropped(sx.state, (system, planet));
                Ok(())
            }
            (OStage::Deploy, Pick::Opt(Opt::Decline)) => {
                self.stage = OStage::Done;
                Ok(())
            }
            (OStage::Deploy, Pick::Opt(Opt::Promissory(POpt::Deploy))) => {
                match sx.table.links.start(&Spawn::Bill { who: self.who, kind: Spend::Resources, cost: ORBITAL_MECH_COST }) {
                    Some(window) => {
                        self.pay.start(window);
                        self.stage = OStage::Paying;
                        Ok(())
                    }
                    None => {
                        self.unsupported = true;
                        self.stage = OStage::Done;
                        Ok(())
                    }
                }
            }
            (OStage::Paying, pick) => self.pay.answer(sx, pick),
            _ => Err(Illegal),
        }
    }
}

/// What the mech of Orbital Drop costs, in resources.
pub const ORBITAL_MECH_COST: i64 = 3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Faction, Planet, UnitBaseType};
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..2u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s
    }

    fn infantry() -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Infantry && u.data().faction.is_none()).expect("infantry")
    }

    /// Two systems with a planet each, in system order.
    fn two() -> [(System, Planet); 2] {
        let mut found: Vec<(System, Planet)> = Vec::new();
        for planet in Planet::ALL {
            if let Some(system) = planet.data().system
                && !found.iter().any(|(s, _)| *s == system)
            {
                found.push((system, planet));
            }
        }
        found.sort_unstable();
        [found[0], found[1]]
    }

    fn ready(s: &mut CoreState) {
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::LazaxGateFolding);
    }

    fn standing(s: &CoreState, planet: Planet) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|p| p.owner() == Seat(0)).count()
    }

    #[test]
    fn lazax_gate_folding_exhausts_the_card_and_places_the_infantry_on_the_planet_held_in_the_mecatol_system() {
        let [(mecatol, planet), (other, elsewhere)] = two();
        let mut s = table();
        ready(&mut s);
        for p in [planet, elsewhere] {
            s.board_mut().planet_mut(p).control = Some(Seat(0));
        }
        assert!(lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, Some(infantry())));
        assert_eq!((standing(&s, planet), standing(&s, elsewhere)), (1, 0), "on the planet of that system only");
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::LazaxGateFolding));
        assert!(!lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, Some(infantry())), "exhausted");
        assert_eq!(standing(&s, planet), 1);
        assert_ne!(other, mecatol);
    }

    #[test]
    fn lazax_gate_folding_changes_nothing_without_the_card_the_planet_or_a_unit_to_place() {
        let [(mecatol, planet), (other, elsewhere)] = two();
        let run = |own: bool, held: Option<Planet>, form: Option<Unit>| {
            let mut s = table();
            if own {
                ready(&mut s);
            }
            if let Some(p) = held {
                s.board_mut().planet_mut(p).control = Some(Seat(0));
            }
            let before = s.clone();
            let done = lazax_gate_folding(&mut s, Scope(u32::MAX), Seat(0), mecatol, form);
            assert!(!done);
            assert_eq!(s, before, "nothing written");
        };
        run(false, Some(planet), Some(infantry()));
        run(true, None, Some(infantry()));
        run(true, Some(elsewhere), Some(infantry()));
        run(true, Some(planet), None);
        assert_ne!(other, mecatol);
    }

    #[test]
    fn the_nekro_repair_repairs_every_unit_of_the_seat_in_space_and_on_planets_and_exhausts_the_card() {
        let [(system, planet), (other, elsewhere)] = two();
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::ERRORERROR);
        let ship = Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Dreadnought && u.data().faction.is_none()).expect("a dreadnought");
        let damaged = |seat: Seat, unit: Unit| crate::state::Piece::new(unit, seat).with_sustained(true);
        s.board_mut().sys_mut(system).units.push(damaged(Seat(0), ship));
        s.board_mut().sys_mut(other).units.push(damaged(Seat(1), ship));
        s.board_mut().planet_mut(planet).units.push(damaged(Seat(0), ship));
        s.board_mut().planet_mut(elsewhere).units.push(damaged(Seat(0), ship));
        s.board_mut().planet_mut(elsewhere).units.push(damaged(Seat(1), ship));
        assert!(c4r_repair(&mut s, Seat(0)));
        let sustained = |s: &CoreState, seat: Seat| -> usize {
            let mut all = s.board().systems().map(|(_, c)| c.units.as_slice().to_vec()).collect::<Vec<_>>();
            all.extend(s.board().planets().map(|(_, c)| c.units.as_slice().to_vec()));
            all.iter().flatten().filter(|p| p.owner() == seat && p.sustained()).count()
        };
        assert_eq!((sustained(&s, Seat(0)), sustained(&s, Seat(1))), (0, 2), "only the seat's own units are repaired");
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::ERRORERROR));
    }

    #[test]
    fn the_nekro_repair_writes_nothing_for_a_seat_without_the_card() {
        let [(system, _), _] = two();
        let mut s = table();
        let ship = Unit::ALL.iter().copied().find(|u| u.data().base_type == UnitBaseType::Dreadnought && u.data().faction.is_none()).expect("a dreadnought");
        s.board_mut().sys_mut(system).units.push(crate::state::Piece::new(ship, Seat(0)).with_sustained(true));
        let before = s.clone();
        assert!(!c4r_repair(&mut s, Seat(0)));
        assert_eq!(s, before);
    }

    #[test]
    fn terraform_attaches_the_card_once_to_a_controlled_non_home_planet_and_turns_the_note_faceup() {
        use crate::generated::{Attachment, PromissoryNote};
        use crate::state::NoteKey;
        let [(_, planet), (_, other)] = two();
        let note = NoteKey { note: PromissoryNote::Terraform, faction: Faction::from_index(1).unwrap() };
        let setup = || {
            let mut s = table();
            s.cards_mut().promissory.edit().insert(note, Seat(0));
            for p in [planet, other] {
                s.board_mut().planet_mut(p).control = Some(Seat(0));
            }
            s
        };
        let refused = |mut s: CoreState, p: Planet, home: bool| {
            let before = s.clone();
            assert!(!terraform(&mut s, Seat(0), p, home));
            assert_eq!(s, before, "nothing written");
        };
        let mut s = setup();
        assert!(terraform(&mut s, Seat(0), planet, false));
        assert!(s.board().planet(planet).attach.as_slice().contains(&Attachment::Terraform));
        assert!(s.cards().promissory_faceup.get(&note).is_some(), "the card is in play");
        assert!(!terraform(&mut s, Seat(0), other, false), "once in the game, on any planet");
        // The conditions, one at a time.
        refused(setup(), planet, true);
        let mut mecatol = setup();
        mecatol.board_mut().planet_mut(Planet::MecatolRex).control = Some(Seat(0));
        refused(mecatol, Planet::MecatolRex, false);
        let mut not_mine = setup();
        not_mine.board_mut().planet_mut(planet).control = Some(Seat(1));
        refused(not_mine, planet, false);
        let mut no_note = setup();
        no_note.cards_mut().promissory.edit().insert(note, Seat(1));
        refused(no_note, planet, false);
        let mut own_note = setup();
        own_note.cards_mut().promissory.edit().remove(&note);
        own_note.cards_mut().promissory.edit().insert(NoteKey { note: PromissoryNote::Terraform, faction: Faction::from_index(0).unwrap() }, Seat(0));
        refused(own_note, planet, false);
        let mut elsewhere = setup();
        elsewhere.board_mut().planet_mut(other).attach.push(Attachment::Terraform);
        refused(elsewhere, planet, false);
    }

    #[test]
    fn vaults_of_the_heir_marks_the_card_spent_purges_the_technology_and_gains_the_top_relic_and_checks_every_condition() {
        use crate::generated::{Breakthrough, Relic};
        use crate::keys::{Atom, MarkKey, MarkVal};
        let tech = Technology::LazaxGateFolding;
        let setup = || {
            let mut s = table();
            let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
            p.faction = Faction::TheMahactGeneSorcerers;
            p.cards.breakthrough = Some(Breakthrough::VaultsOfTheHeir);
            p.tech.owned.insert(tech);
            p.tech.exhausted.insert(tech);
            s.decks_mut().relic.set(vec![Relic::ALL[3], Relic::ALL[4]]);
            s
        };
        let mut s = setup();
        assert!(vaults_of_the_heir(&mut s, Scope(u32::MAX), Seat(0), tech));
        let p = s.players().by_seat(Seat(0)).unwrap();
        assert!(!p.tech.owned.contains(tech) && !p.tech.exhausted.contains(tech), "purged, and its exhausted mark with it");
        assert_eq!(p.cards.relics.as_slice(), [Relic::ALL[3]]);
        assert_eq!(s.marks().tail.get(&MarkKey::MahactVaultsExhausted(Seat(0))), Some(&MarkVal::Atom(Atom::Num(1))));
        assert!(!vaults_of_the_heir(&mut s, Scope(u32::MAX), Seat(0), tech), "spent until the status phase");
        // Each condition alone, nothing written.
        let refused = |mut s: CoreState| {
            let before = s.clone();
            assert!(!vaults_of_the_heir(&mut s, Scope(u32::MAX), Seat(0), tech));
            assert_eq!(s, before);
        };
        let mut other_faction = setup();
        other_faction.players_mut().by_seat_mut(Seat(0)).unwrap().faction = Faction::from_index(0).unwrap();
        refused(other_faction);
        let mut no_breakthrough = setup();
        no_breakthrough.players_mut().by_seat_mut(Seat(0)).unwrap().cards.breakthrough = None;
        refused(no_breakthrough);
        let mut not_owned = setup();
        not_owned.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(tech);
        refused(not_owned);
        let mut no_relic = setup();
        no_relic.decks_mut().relic.set(vec![]);
        refused(no_relic);
        let mut spent = setup();
        spent.marks_mut().tail.edit().insert(MarkKey::MahactVaultsExhausted(Seat(0)), MarkVal::Atom(Atom::Num(1)));
        refused(spent);
    }

    fn orbital_driver(facts: OrbitalFacts, links: bool) -> crate::driver::Driver<OrbitalDrop> {
        use crate::kernel::stage::AbilityTable;
        let mut t = AbilityTable::default();
        if links {
            crate::driver::children::register(&mut t);
        }
        crate::driver::Driver::new(OrbitalDrop::new(Seat(0), facts), crate::kernel::Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), t, 1)
    }

    fn orbital_facts(can_pay: bool) -> OrbitalFacts {
        let plain = |base| Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none());
        OrbitalFacts { infantry: plain(UnitBaseType::Infantry), mech: plain(UnitBaseType::Mech), can_pay, scope: Scope(u32::MAX), source: Code(9) }
    }

    fn orbital_table(tokens: i16, planets: usize) -> CoreState {
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.strategic_tokens = tokens;
        for (_, planet) in two().iter().take(planets) {
            s.board_mut().planet_mut(*planet).control = Some(Seat(0));
        }
        s
    }

    #[test]
    fn orbital_drop_spends_the_token_announces_it_places_two_infantry_and_offers_the_mech_when_the_seat_can_pay() {
        use crate::choice::promissory::{Ask as PAsk, Opt as POpt};
        use crate::choice::{Ask, Opt, Pick};
        use crate::driver::Status;
        let k = Kernel::plain();
        let [(system, planet), _] = two();
        let mut s = orbital_table(2, 1);
        let mut d = orbital_driver(orbital_facts(true), true);
        // One planet: no planet question; the token is spent and announced, two infantry stand, the mech is a question.
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("the mech is offered") };
        let mech = orbital_facts(true).mech.unwrap();
        assert_eq!(q.ask, Ask::Promissory(PAsk::OrbitalDeploy { system, planet, mech }));
        assert_eq!(q.opts.as_slice(), &[Opt::Promissory(POpt::Deploy), Opt::Decline]);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1);
        assert_eq!(standing(&s, planet), 2);
        assert_eq!(s.pending().staged.len(), 1, "the token spent is announced");
        assert!(d.window.done);
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(standing(&s, planet), 2);
        // The seat cannot pay, or the box holds no mech: no question.
        let mut s = orbital_table(1, 1);
        let mut d = orbital_driver(orbital_facts(false), true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done);
        // No token: nothing is spent or placed (the planet question comes first with several planets).
        let mut s = orbital_table(0, 1);
        let before = s.clone();
        let mut d = orbital_driver(orbital_facts(true), true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        assert_eq!(s, before);
        // No planet: nothing.
        let mut s = orbital_table(1, 0);
        let mut d = orbital_driver(orbital_facts(true), true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
    }

    #[test]
    fn orbital_drop_asks_the_planet_among_those_the_seat_controls_and_the_box_cuts_the_infantry() {
        use crate::choice::promissory::{Ask as PAsk, Opt as POpt};
        use crate::choice::{Ask, Opt, Pick};
        use crate::driver::{DriverError, Status};
        let k = Kernel::plain();
        let [(system, planet), (other, elsewhere)] = two();
        let mut s = orbital_table(1, 2);
        let mut d = orbital_driver(orbital_facts(false), true);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("the planet is asked") };
        assert_eq!(q.ask, Ask::Promissory(PAsk::OrbitalPlanet));
        assert_eq!(q.opts.as_slice(), &[Opt::Promissory(POpt::Spot(system, planet)), Opt::Promissory(POpt::Spot(other, elsewhere))]);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 1, "nothing before the answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)), Err(DriverError::Illegal));
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Promissory(POpt::Spot(other, elsewhere)))).unwrap(), Status::Done);
        assert_eq!((standing(&s, planet), standing(&s, elsewhere)), (0, 2));
        // One unit left in the box (the infantry is capped here by giving the seat a form with plastic: a mech, four in the box): one is placed.
        let mut s = orbital_table(1, 1);
        let mut facts = orbital_facts(false);
        facts.infantry = facts.mech;
        for _ in 0..3 {
            fx::place_unit(&mut s, Seat(0), other, Some(elsewhere), facts.mech.unwrap());
        }
        let mut d = orbital_driver(facts, true);
        d.run(&mut s, &k).unwrap();
        assert_eq!(standing(&s, planet), 1);
        // The mech without a link to the payment window: the run is declined.
        let mut s = orbital_table(1, 1);
        let mut d = orbital_driver(orbital_facts(true), false);
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, Pick::Opt(Opt::Promissory(POpt::Deploy))).unwrap();
        assert!(d.window.unsupported);
    }

    /// A planet worth `at_least` resources, placed on a tile.
    fn rich(at_least: u8) -> (System, Planet) {
        Planet::ALL.iter().copied().filter(|p| p.data().resources >= at_least).find_map(|p| p.data().system.map(|s| (s, p))).expect("a rich planet")
    }

    #[test]
    fn orbital_drop_names_the_token_event_by_its_reason_and_offers_no_mech_from_an_empty_box() {
        use crate::driver::Status;
        let k = Kernel::plain();
        let [(_, planet), _] = two();
        let mut s = orbital_table(1, 1);
        let mut d = orbital_driver(orbital_facts(false), true);
        d.run(&mut s, &k).unwrap();
        let event = s.pending().staged.first().cloned().expect("the token event");
        assert_eq!(event.kind, EventKind::StrategyTokenSpent);
        assert_eq!(event.payload.get(Key::Reason), Some(Val::Code(Code(9))), "the reason is the source text");
        assert_eq!(event.payload.get(Key::Player), Some(Val::Seat(Seat(0))));
        // The four mechs of the box already stand on the board: the offer is not made although the seat could pay.
        let mut s = orbital_table(1, 1);
        let facts = orbital_facts(true);
        for _ in 0..4 {
            fx::place_unit(&mut s, Seat(0), two()[1].0, Some(two()[1].1), facts.mech.unwrap());
        }
        let mut d = orbital_driver(facts, true);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done);
        let _ = planet;
    }

    #[test]
    fn orbital_drop_pays_three_resources_through_the_payment_window_and_only_then_lands_the_mech() {
        use crate::choice::promissory::{Opt as POpt};
        use crate::choice::{Opt, Pick};
        use crate::driver::Status;
        let k = Kernel::plain();
        let (system, planet) = rich(3);
        let mech = orbital_facts(true).mech.unwrap();
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.strategic_tokens = 1;
        s.board_mut().planet_mut(planet).control = Some(Seat(0));
        let mut d = orbital_driver(orbital_facts(true), true);
        d.run(&mut s, &k).unwrap();
        let mut status = d.resume(&mut s, &k, Pick::Opt(Opt::Promissory(POpt::Deploy))).unwrap();
        let mut asked = Vec::new();
        while let Status::Asking(q) = status {
            asked.push(format!("{:?}", q.ask));
            status = d.resume(&mut s, &k, Pick::Opt(q.opts.as_slice()[0])).unwrap();
        }
        let on = |s: &CoreState| s.board().planet(planet).units.as_slice().iter().filter(|p| p.kind() == mech).count();
        assert_eq!(on(&s), 1, "paid, then the mech: {asked:?}");
        assert!(s.board().planet(planet).flags.has(crate::state::PlanetFlags::EXHAUSTED), "the planet paid");
        let _ = system;
        // A seat that cannot actually pay (no resources to spend) gets no mech although it was offered.
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.strategic_tokens = 1;
        let poor = two()[0].1;
        s.board_mut().planet_mut(poor).control = Some(Seat(0));
        s.board_mut().planet_mut(poor).flags.set(crate::state::PlanetFlags::EXHAUSTED, true);
        let mut d = orbital_driver(orbital_facts(true), true);
        d.run(&mut s, &k).unwrap();
        let mut status = d.resume(&mut s, &k, Pick::Opt(Opt::Promissory(POpt::Deploy))).unwrap();
        while let Status::Asking(q) = status {
            status = d.resume(&mut s, &k, Pick::Opt(q.opts.as_slice()[0])).unwrap();
        }
        assert_eq!(on_mech(&s, poor, mech), 0);
        // Two resources do not pay the three: the bill is not met and no mech lands.
        let two_only = Planet::ALL.iter().copied().find(|p| p.data().resources == 2 && p.data().system.is_some()).expect("a planet worth 2");
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.strategic_tokens = 1;
        s.board_mut().planet_mut(two_only).control = Some(Seat(0));
        let mut d = orbital_driver(orbital_facts(true), true);
        d.run(&mut s, &k).unwrap();
        let mut status = d.resume(&mut s, &k, Pick::Opt(Opt::Promissory(POpt::Deploy))).unwrap();
        while let Status::Asking(q) = status {
            status = d.resume(&mut s, &k, Pick::Opt(q.opts.as_slice()[0])).unwrap();
        }
        assert_eq!(on_mech(&s, two_only, mech), 0, "3 resources are owed");
    }

    fn on_mech(s: &CoreState, planet: Planet, mech: Unit) -> usize {
        s.board().planet(planet).units.as_slice().iter().filter(|p| p.kind() == mech).count()
    }
}
