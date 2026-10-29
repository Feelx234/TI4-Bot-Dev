//! The effect of a passed agenda as a [`Window`] (package B9; string `agenda_effects::resolve_with`).
//!
//! Ported effects (they ask nothing, so the window is one step):
//! * Political Censure, Shard of the Throne, Crown of Emphidia: the elected player gains a victory point,
//!   capped at the target (`agenda_effects::effect_censure`).
//! * Classified Document Leaks: the elected scored secret objective becomes a public one
//!   (`agenda_effects::effect_classified`).
//!
//! * Standing and availability-only arms (the Ministers, the Research Teams, ..., Representative Government,
//!   Revolution on For): the card does nothing when it passes.
//! * Anti-Intellectual Revolution and Representative Government (Against), Miscount Disclosed (a law
//!   repealed): `Done::Deferred`, as the string effect defers what needs a question.
//! * Judicial Abolishment (the elected law leaves play), New Constitution (every law, on For), Mutiny (a point
//!   for or against each For voter), Holy Planet of Ixth (the controller of the elected planet gains a point).
//! * Nexus Sovereignty: Against puts a gamma wormhole token into Mecatol Rex (For is a standing rule).
//! * A law leaving play (`laws::repeal`): the law is removed; Political Censure's holder loses its point.
//! * Archived Secret (the elected player draws a secret objective) and Search Warrant (two): a draw, and the
//!   hand limit question of rule 45.4 after each (package A4: `fx::draw_secret`, `query::objectives::hand_step`).
//!
//! Not ported, so the adapter runs the string effect alone (documented bails of the dual): every other agenda
//! effect; here an outcome that is not what the effect elects (a card with `For`/`Against` on the table).

use crate::choice::objectives::{Ask as OAsk, Opt as OOpt};
use crate::choice::{Ask, Choice, Opt, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, Planet, SecretObjective, System, Wormhole};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::keys::{LedgerReason, Outcome};
use crate::query::objectives as q;
use crate::state::{CoreState, ObjectiveRef, SysFlags};

use super::table_effects::{EffectFacts, Table};

mod questions;

pub use crate::kernel::fx::VICTORY_TARGET;

/// The sides of the game's dice (`dice::Dice`: tens).
const DIE_SIDES: u32 = 10;

/// What the effect does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The elected player gains a point (`agenda` is the ledger reason).
    Point(Agenda, Seat),
    /// The elected secret objective becomes public.
    Reveal(SecretObjective),
    /// The law leaves play.
    Repeal(Agenda),
    /// New Constitution: every law leaves play.
    RepealAll,
    /// Mutiny: each voter (in the string ballot's order) gains `delta` points.
    Voters(Agenda, i32, SmallList<Seat, 8>),
    /// Holy Planet of Ixth: whoever controls the planet gains a point.
    Controller(Agenda, Planet),
    /// Nexus Sovereignty, Against: a gamma wormhole token in the Mecatol Rex system of the map.
    Gamma,
    /// Archived Secret, Search Warrant: the seat draws this many secret objectives, each followed by the hand limit.
    Draw(Seat, u8),
    /// The card does nothing when it passes.
    Nothing,
    /// An effect of [`Table`] (the table writes of `table_effects`).
    Table(Table),
    /// Seed of an Empire: the seat with the most (`true`) or fewest points gains one.
    Seed(Agenda, bool),
    /// Homeland Defense Act, Against: each seat destroys one of its PDS.
    Defense,
    /// Executive Sanctions, Against: each seat with a hand discards the card a die picks.
    Sanctions,
    /// Ixthian Artifact, For: a die; a 6 is a windfall that needs a payer (deferred), anything else destroys every ship that
    /// cannot sustain damage.
    Crater,
}

/// How the effect ended (`agenda_effects::Effect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Done {
    Resolved,
    /// The effect needs a question this path cannot ask (`Effect::Deferred`).
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Apply,
    /// Waiting on "return a secret objective to the deck" (a draw went over the limit).
    Asking,
    Done,
}

/// One agenda effect.
#[derive(Debug, Clone)]
pub struct Resolve {
    kind: Kind,
    stage: Stage,
    /// A repeal found the law in play.
    repealed: bool,
    done: Done,
    /// Draws made so far, and whether the hand limit of the last one is still to be checked.
    drawn: u8,
    checking: bool,
    /// What a [`Kind::Table`] reads (the voters, the scopes, two interned texts).
    facts: Option<EffectFacts>,
    /// The position held something the table effect does not express: the adapter drops the window.
    unsupported: bool,
    /// Seed of an Empire: the tied seats the speaker names one of.
    tied: SmallList<Seat, 8>,
    /// Homeland Defense Act: the seat (in seating order) being served.
    cursor: u8,
    /// The faces of the dice the effect rolled, in order (Executive Sanctions, Ixthian Artifact).
    faces: SmallList<u32, 16>,
}

impl Resolve {
    /// The effect of `agenda` electing `outcome`; `None` when the core has no such effect.
    #[must_use]
    pub fn of(agenda: Agenda, outcome: Outcome, for_voters: &[Seat]) -> Option<Self> {
        use Agenda as A;
        let mut done = Done::Resolved;
        let kind = match (agenda, outcome) {
            (A::PoliticalCensure | A::ShardOfTheThrone | A::TheCrownOfEmphidia, Outcome::Seat(who)) => Kind::Point(agenda, who),
            (A::ClassifiedDocumentLeaks, Outcome::Secret(s)) => Kind::Reveal(s),
            (A::JudicialAbolishment, Outcome::Law(law)) => Kind::Repeal(law),
            (A::MiscountDisclosed, Outcome::Law(law)) => {
                done = Done::Deferred;
                Kind::Repeal(law)
            }
            (A::NewConstitution, Outcome::For) => Kind::RepealAll,
            (A::NewConstitution, _) => Kind::Nothing,
            (A::Mutiny, Outcome::For | Outcome::Against) => {
                Kind::Voters(agenda, if outcome == Outcome::For { 1 } else { -1 }, SmallList::from_slice(for_voters))
            }
            (A::ArchivedSecret, Outcome::Seat(who)) => Kind::Draw(who, 1),
            (A::SearchWarrant, Outcome::Seat(who)) => Kind::Draw(who, 2),
            (A::NexusSovereignty, Outcome::Against) => Kind::Gamma,
            (A::NexusSovereignty, Outcome::For) => Kind::Nothing,
            (A::HolyPlanetOfIxth, Outcome::Planet(p)) => Kind::Controller(agenda, p),
            (A::AntiIntellectualRevolution | A::RepresentativeGovernmentBaseGame, Outcome::For) => Kind::Nothing,
            (A::AntiIntellectualRevolution | A::RepresentativeGovernmentBaseGame, Outcome::Against) => {
                done = Done::Deferred;
                Kind::Nothing
            }
            (
                A::RepresentativeGovernment
                | A::ResearchTeamBiotic
                | A::ResearchTeamCybernetic
                | A::ResearchTeamPropulsion
                | A::ResearchTeamWarfare
                | A::SenateSanctuary
                | A::TerraformingInitiative
                | A::MinisterOfExploration
                | A::MinisterOfPolicy
                | A::CommitteeFormation
                | A::ImperialArbiter
                | A::MinisterOfCommerce
                | A::MinisterOfIndustry
                | A::MinisterOfPeace
                | A::MinisterOfSciences
                | A::MinisterOfWar
                | A::ProphecyOfIxth
                | A::TheCrownOfThalnos
                | A::ChecksAndBalances
                | A::CovertLegislation,
                _,
            ) => Kind::Nothing,
            _ => return None,
        };
        Some(Self { kind, stage: Stage::Apply, repealed: false, done, drawn: 0, checking: false, facts: None, unsupported: false, tied: SmallList::new(), cursor: 0, faces: SmallList::new() })
    }

    /// [`Resolve::of`], then the table effects: `facts` carries the ballot and everything the table reads.
    #[must_use]
    pub fn of_facts(agenda: Agenda, outcome: Outcome, facts: &EffectFacts) -> Option<Self> {
        if let Some(window) = Self::of(agenda, outcome, facts.for_voters.as_slice()) {
            return Some(window);
        }
        let special = match (agenda, outcome) {
            (Agenda::SeedOfAnEmpire, Outcome::For) => Some(Kind::Seed(agenda, true)),
            (Agenda::SeedOfAnEmpire, _) => Some(Kind::Seed(agenda, false)),
            (Agenda::HomelandDefenseAct, Outcome::Against) => Some(Kind::Defense),
            (Agenda::ExecutiveSanctions, Outcome::Against) => Some(Kind::Sanctions),
            (Agenda::ExecutiveSanctions, Outcome::For) | (Agenda::IxthianArtifact, Outcome::Against) => Some(Kind::Nothing),
            (Agenda::IxthianArtifact, Outcome::For) => Some(Kind::Crater),
            _ => None,
        };
        if let Some(kind) = special {
            return Some(Self { kind, stage: Stage::Apply, repealed: false, done: Done::Resolved, drawn: 0, checking: false, facts: Some(facts.clone()), unsupported: false, tied: SmallList::new(), cursor: 0, faces: SmallList::new() });
        }
        let (table, deferred) = Table::of(agenda, outcome, facts.has_galaxy)?;
        let done = if deferred { Done::Deferred } else { Done::Resolved };
        Some(Self { kind: Kind::Table(table), stage: Stage::Apply, repealed: false, done, drawn: 0, checking: false, facts: Some(facts.clone()), unsupported: false, tied: SmallList::new(), cursor: 0, faces: SmallList::new() })
    }

    /// Whether the table effect met a position it does not express (its result is not to be compared).
    #[must_use]
    pub const fn unsupported(&self) -> bool {
        self.unsupported
    }

    /// `laws::repeal` of `law`.
    #[must_use]
    pub const fn repeal(law: Agenda) -> Self {
        Self { kind: Kind::Repeal(law), stage: Stage::Apply, repealed: false, done: Done::Resolved, drawn: 0, checking: false, facts: None, unsupported: false, tied: SmallList::new(), cursor: 0, faces: SmallList::new() }
    }

    /// Whether the effect rolls dice (it needs the random streams of the state).
    #[must_use]
    pub const fn rolls(&self) -> bool {
        matches!(self.kind, Kind::Sanctions | Kind::Crater)
    }

    /// The faces of the dice rolled, in order.
    #[must_use]
    pub fn faces(&self) -> &[u32] {
        self.faces.as_slice()
    }

    /// Whether the table effect reads the map slice.
    #[must_use]
    pub const fn needs_map(&self) -> bool {
        matches!(self.kind, Kind::Table(t) if t.needs_map())
    }

    /// Whether the effect turns a card of a deck, as a draw does.
    #[must_use]
    pub const fn draws(&self) -> bool {
        matches!(self.kind, Kind::Draw(..)) || matches!(self.kind, Kind::Table(t) if t.draws_from_deck())
    }

    /// Whether the repealed law was in play.
    #[must_use]
    pub const fn repealed(&self) -> bool {
        self.repealed
    }

    /// How the effect ended.
    #[must_use]
    pub const fn done(&self) -> Done {
        self.done
    }

    /// Ixthian Artifact's crater: every ship that cannot sustain damage leaves every system.
    fn crater(core: &mut CoreState, scope: crate::tables::Scope) {
        let doomed = |p: &crate::state::Piece| {
            let d = p.kind().data();
            d.ship && !d.sustain_damage && scope.0 & crate::tables::source_bit(d.source) != 0
        };
        for system in System::ALL {
            if core.board().sys(system).units.as_slice().iter().any(doomed) {
                core.board_mut().sys_mut(system).units.retain(|p| !doomed(p));
            }
        }
    }

    /// `laws::repeal`: the law leaves play; Political Censure's holder loses its point.
    fn leave(core: &mut CoreState, law: Agenda) -> bool {
        let owner = core.agenda_mut().laws.remove(&law);
        if let (Agenda::PoliticalCensure, Some(Outcome::Seat(holder))) = (law, owner) {
            Self::point(core, law, holder, -1);
        }
        owner.is_some()
    }

    /// `agenda_effects::adjust_victory_points` with +1: capped at the target, the ledger row records the
    /// change that happened (zero included).
    fn point(core: &mut CoreState, agenda: Agenda, who: Seat, delta: i32) {
        fx::adjust_victory_points(core, who, delta, LedgerReason::Agenda(agenda));
    }
}

impl Resolve {
    /// Archived Secret and Search Warrant: `count` draws, each followed by the hand limit (rule 45.4).
    fn draw(&mut self, sx: &mut Sx<'_>, who: Seat, count: u8) -> Next {
        loop {
            if self.checking || self.stage == Stage::Asking {
                match q::hand_step(sx.state, who) {
                    q::HandStep::Within => {
                        self.checking = false;
                        self.stage = Stage::Apply;
                    }
                    q::HandStep::Return(only) => fx::return_secret(sx.state, who, only),
                    q::HandStep::Ask { held, .. } => {
                        self.stage = Stage::Asking;
                        return Next::Continue(Ask::Objectives(OAsk::ReturnSecret { held }));
                    }
                }
                continue;
            }
            if self.drawn >= count {
                self.stage = Stage::Done;
                return Next::Done;
            }
            self.drawn += 1;
            // An empty deck draws nothing, and then there is no hand to check.
            self.checking = fx::draw_secret(sx.state, who).is_some();
        }
    }
}

impl Window for Resolve {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if let Kind::Draw(who, count) = self.kind {
            return self.draw(sx, who, count);
        }
        match self.kind {
            Kind::Seed(agenda, high) => return self.seed(sx, agenda, high),
            Kind::Defense => {
                self.stage = Stage::Apply;
                return self.defense(sx);
            }
            _ => {}
        }
        if self.stage == Stage::Apply {
            match &self.kind {
                Kind::Point(agenda, who) => Self::point(sx.state, *agenda, *who, 1),
                Kind::Repeal(law) => {
                    self.repealed = Self::leave(sx.state, *law);
                    // Miscount Disclosed with the law not in play: nothing was deferred.
                    if !self.repealed {
                        self.done = Done::Resolved;
                    }
                }
                Kind::RepealAll => {
                    let laws: Vec<Agenda> = sx.state.agenda().laws.iter().map(|(a, _)| *a).collect();
                    for law in laws {
                        Self::leave(sx.state, law);
                    }
                }
                Kind::Voters(agenda, delta, who) => {
                    for seat in who.as_slice() {
                        Self::point(sx.state, *agenda, *seat, *delta);
                    }
                }
                Kind::Controller(agenda, planet) => {
                    if let Some(owner) = crate::query::board::control(sx.state, *planet) {
                        Self::point(sx.state, *agenda, owner, 1);
                    }
                }
                Kind::Gamma => {
                    // `seating::mecatol_on`: the Thunder's Edge tile when the map has it, else the base one.
                    let placed = |tile: System| sx.state.board().systems().any(|(s, c)| s == tile && c.flags.has(SysFlags::TOUCHED));
                    let mecatol = if placed(System::MecatolRex112) { System::MecatolRex112 } else { System::MecatolRex18 };
                    sx.state.explore_mut().wormhole_tokens.insert(Wormhole::Gamma, mecatol);
                }
                Kind::Sanctions => {
                    // Each seat with a hand discards the card at `face % held`: random through the seeded dice. The card
                    // is gone (`discarded` with no pile).
                    for seat in SmallList::<Seat, 8>::from_slice(sx.state.seats().seating_order()).as_slice() {
                        let held = sx.state.players().by_seat(*seat).map_or(0, |p| p.cards.hand.len());
                        if held == 0 {
                            continue;
                        }
                        let face = sx.die(crate::state::RngDomain::Dice, DIE_SIDES);
                        self.faces.push(face);
                        if let Some(p) = sx.state.players_mut().by_seat_mut(*seat) {
                            p.cards.hand.remove(usize::try_from(face).unwrap_or(0) % held);
                        }
                    }
                }
                Kind::Crater => {
                    let face = sx.die(crate::state::RngDomain::Dice, DIE_SIDES);
                    self.faces.push(face);
                    if face >= 6 {
                        self.done = Done::Deferred;
                    } else if let Some(facts) = &self.facts {
                        Self::crater(sx.state, facts.scope);
                    } else {
                        self.unsupported = true;
                    }
                }
                Kind::Nothing | Kind::Draw(..) | Kind::Seed(..) | Kind::Defense => {}
                Kind::Table(table) => {
                    if let Some(facts) = &self.facts
                        && !table.apply(sx.state, facts)
                    {
                        self.unsupported = true;
                    }
                }
                Kind::Reveal(secret) => {
                    let card = ObjectiveRef::Secret(*secret);
                    let revealed = &mut sx.state.objectives_mut().revealed;
                    if !revealed.as_slice().contains(&card) {
                        revealed.push(card);
                    }
                }
            }
            self.stage = Stage::Done;
        }
        Next::Done
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.kind {
            Kind::Seed(..) => return self.seed_question(s),
            Kind::Defense => return self.defense_question(s),
            _ => {}
        }
        let Kind::Draw(who, _) = self.kind else { return None };
        if self.stage != Stage::Asking {
            return None;
        }
        match q::hand_step(s, who) {
            q::HandStep::Ask { held, hand } => Some(crate::choice::objectives::return_secret_choice(who, held, hand.as_slice())),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match self.kind {
            Kind::Seed(agenda, _) => return self.seed_answer(sx, agenda, pick),
            Kind::Defense => return self.defense_answer(sx, pick),
            _ => {}
        }
        let Kind::Draw(who, _) = self.kind else { return Err(Illegal) };
        match pick {
            Pick::Opt(Opt::Objectives(OOpt::Return(secret)))
                if self.stage == Stage::Asking
                    && sx.state.players().by_seat(who).is_some_and(|p| p.cards.secrets.as_slice().contains(&secret)) =>
            {
                fx::return_secret(sx.state, who, secret);
                self.stage = Stage::Apply;
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_ported_effects_open_a_window() {
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::Seat(Seat(0)), &[]).is_some());
        assert!(Resolve::of(Agenda::PoliticalCensure, Outcome::For, &[]).is_none());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::Secret(SecretObjective::ALL[0]), &[]).is_some());
        assert!(Resolve::of(Agenda::ClassifiedDocumentLeaks, Outcome::For, &[]).is_none());
        assert!(Resolve::of(Agenda::Mutiny, Outcome::Empty, &[]).is_none());
        assert!(!Resolve::repeal(Agenda::PoliticalCensure).repealed());
    }

    // -- the card, relic and dice effects (package B9c-2) ----------------------------------------------------------

    use crate::driver::Driver;
    use crate::generated::{ActionCard, Faction, Relic, Unit};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Code, Player, Players, Seats};
    use crate::tables::Scope;

    struct Seeds(u8);
    impl crate::state::SeedSource for Seeds {
        fn seed_for(&self, _: u64, _: crate::state::RngDomain) -> [u8; 32] {
            [self.0; 32]
        }
    }

    fn facts(for_voters: &[Seat]) -> EffectFacts {
        EffectFacts {
            for_voters: SmallList::from_slice(for_voters),
            against_voters: SmallList::new(),
            mahact_held: [0; 8],
            scope: Scope(!0),
            default_scope: Scope(!0),
            source: Code(0),
            pool: Code(0),
            has_galaxy: false,
            defense_act: Code(0),
            hooked: 0,
            drop_unofficial: false,
            blanked: false,
        }
    }

    fn table(n: u8) -> CoreState {
        let mut s = CoreState::all_loaded();
        let players: Vec<Player> = (0..n).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        let list: Vec<Seat> = (0..n).map(Seat).collect();
        s.seats = Seats::new(&list, &list, n);
        s.players = Players::from_vec(players);
        s
    }

    fn run(window: Resolve, s: &mut CoreState, seed: u8) -> Driver<Resolve> {
        let k = Kernel::plain();
        let mut d = Driver::new(window, Resolver::new(&[], None), AbilityTable::default(), 1).with_seeds(std::rc::Rc::new(Seeds(seed)));
        assert_eq!(d.run(s, &k).unwrap(), crate::driver::Status::Done);
        d
    }

    fn hand(s: &mut CoreState, seat: Seat, n: usize) {
        s.players_mut().by_seat_mut(seat).unwrap().cards.hand = vec![ActionCard::ALL[3]; n].into_iter().collect();
    }

    fn held(s: &CoreState, seat: Seat) -> usize {
        s.players().by_seat(seat).unwrap().cards.hand.len()
    }

    #[test]
    fn unconventional_measures_for_draws_two_cards_for_each_for_voter() {
        let (a, b) = (Seat(0), Seat(1));
        let mut s = table(3);
        s.decks_mut().action.set(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2], ActionCard::ALL[4], ActionCard::ALL[5]]);
        let f = facts(&[a, Seat(2)]);
        let w = Resolve::of_facts(Agenda::UnconventionalMeasures, Outcome::For, &f).unwrap();
        assert!(w.draws(), "a turn-redo reserve would draw through the deck");
        let d = run(w, &mut s, 1);
        assert_eq!((held(&s, a), held(&s, b), held(&s, Seat(2))), (2, 0, 2));
        assert_eq!(s.decks().action.cards().len(), 1);
        assert!(!d.window.unsupported());
        // A hooked seat, a hand that would overflow the limit (7) or a lowered limit: not expressed.
        for (hooked, cards, blanked_limit) in [(1_u16, 0, false), (0, 6, false)] {
            let mut s = table(3);
            s.decks_mut().action.set(vec![ActionCard::ALL[0]; 4]);
            hand(&mut s, a, cards);
            let f = EffectFacts { hooked, blanked: blanked_limit, ..facts(&[a]) };
            let d = run(Resolve::of_facts(Agenda::UnconventionalMeasures, Outcome::For, &f).unwrap(), &mut s, 1);
            assert!(d.window.unsupported(), "hooked {hooked}, {cards} cards");
        }
        // Five cards and two more reach the limit exactly: plain.
        let mut s = table(3);
        s.decks_mut().action.set(vec![ActionCard::ALL[0]; 4]);
        hand(&mut s, a, 5);
        assert!(!run(Resolve::of_facts(Agenda::UnconventionalMeasures, Outcome::For, &facts(&[a])).unwrap(), &mut s, 1).window.unsupported());
        assert_eq!(held(&s, a), 7);
    }

    #[test]
    fn minister_of_antiquities_gives_the_elected_seat_the_top_relic() {
        let mut s = table(2);
        let other = Relic::ALL.iter().copied().find(|r| *r != Relic::ShardOfTheThrone).unwrap();
        s.decks_mut().relic.set(vec![other, Relic::ShardOfTheThrone]);
        let f = facts(&[]);
        let w = Resolve::of_facts(Agenda::MinisterOfAntiquities, Outcome::Seat(Seat(1)), &f).unwrap();
        assert!(w.draws());
        run(w, &mut s, 1);
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().cards.relics.as_slice(), [other]);
        assert_eq!(s.decks().relic.cards().len(), 1);
        run(Resolve::of_facts(Agenda::MinisterOfAntiquities, Outcome::Seat(Seat(1)), &f).unwrap(), &mut s, 1);
        assert_eq!(s.players().by_seat(Seat(1)).unwrap().econ.victory_points, 1, "the Shard scores");
    }

    #[test]
    fn executive_sanctions_against_discards_the_card_a_die_picks_from_each_hand() {
        let f = facts(&[]);
        let mut distinct = std::collections::BTreeSet::new();
        for seed in 0..16 {
            let mut s = table(3);
            hand(&mut s, Seat(0), 3);
            hand(&mut s, Seat(2), 5);
            let w = Resolve::of_facts(Agenda::ExecutiveSanctions, Outcome::Against, &f).unwrap();
            assert!(w.rolls());
            let d = run(w, &mut s, seed);
            assert_eq!((held(&s, Seat(0)), held(&s, Seat(1)), held(&s, Seat(2))), (2, 0, 4));
            assert_eq!(d.window.faces().len(), 2, "one die for each hand, none for the empty one");
            assert!(d.window.faces().iter().all(|f| (1..=10).contains(f)));
            distinct.insert(d.window.faces().to_vec());
        }
        assert!(distinct.len() > 1, "the dice follow the stream");
        // The card picked is the one at `face % held`.
        let mut s = table(1);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand = ActionCard::ALL[..4].iter().copied().collect();
        let d = run(Resolve::of_facts(Agenda::ExecutiveSanctions, Outcome::Against, &f).unwrap(), &mut s, 3);
        let gone = usize::try_from(d.window.faces()[0]).unwrap() % 4;
        let mut left: Vec<ActionCard> = ActionCard::ALL[..4].to_vec();
        left.remove(gone);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().cards.hand.as_slice(), left.as_slice());
        // For rolls nothing.
        let w = Resolve::of_facts(Agenda::ExecutiveSanctions, Outcome::For, &f).unwrap();
        assert!(!w.rolls());
    }

    #[test]
    fn ixthian_artifact_for_destroys_ships_that_cannot_sustain_unless_a_six_or_more_is_rolled() {
        let f = facts(&[]);
        let (mut deferred, mut cratered) = (0, 0);
        for seed in 0..40 {
            let mut s = table(2);
            let (home, far) = (System::ALL[3], System::ALL[4]);
            for unit in [Unit::CarrierI, Unit::DreadnoughtI, Unit::FighterI] {
                fx::place_unit(&mut s, Seat(0), home, None, unit);
            }
            fx::place_unit(&mut s, Seat(1), far, None, Unit::WarSun);
            fx::place_unit(&mut s, Seat(1), far, Some(Planet::ALL[3]), Unit::InfantryI);
            let w = Resolve::of_facts(Agenda::IxthianArtifact, Outcome::For, &f).unwrap();
            let d = run(w, &mut s, seed);
            assert_eq!(d.window.faces().len(), 1);
            let ships = s.board().sys(home).units.len();
            if d.window.faces()[0] >= 6 {
                deferred += 1;
                assert_eq!((d.window.done(), ships), (Done::Deferred, 3));
            } else {
                cratered += 1;
                assert_eq!((d.window.done(), ships), (Done::Resolved, 1), "only the dreadnought sustains");
                assert_eq!(s.board().sys(home).units.as_slice()[0].kind(), Unit::DreadnoughtI);
                assert_eq!(s.board().sys(far).units.len(), 1, "the war sun sustains");
                assert_eq!(s.board().planet(Planet::ALL[3]).units.len(), 1, "ground forces are not ships");
            }
        }
        assert!(deferred > 0 && cratered > 0, "{deferred} deferred, {cratered} cratered");
        assert!(!Resolve::of_facts(Agenda::IxthianArtifact, Outcome::Against, &f).unwrap().rolls());
    }
}
