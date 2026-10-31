//! The close of a vote as a [`Window`] (package B9c-2; string `Game::close_vote` / `settle_winner`, up to the agenda's
//! own effect, which is the [`Resolve`](super::effects::Resolve) window and runs after this one).
//!
//! A vote without a winner is logged and the predictions are dropped. A vote with one: the ballot is mirrored on the
//! state, `AGENDA_RESOLVED` is emitted through the resolver (Deadly Plot, Confusing / Confounding Legal Text are
//! played into it), the vote is logged (Covert Legislation lists the face-down agenda too) and, unless Deadly Plot
//! discarded the agenda ("resolved with no effect": the redirect and the predictions go with it), the outcome is
//! resolved: the redirect is taken, the elected seat read, every correct prediction paid in seat order and a law
//! enacted (`resolve_agenda_outcome`).
//!
//! Predictions are paid for the riders that the state alone settles: Imperial Rider (a bare prediction) a victory
//! point, Trade Rider five trade goods, Politics Rider three action cards and the speaker token, Keleres Rider an
//! action card and two trade goods, Leadership Rider three command tokens (a pool question each, rule 52.4) and
//! Sanction nothing, Technology Rider one free research (a child window of the production area, package W4). The riders that
//! place (Construction, Diplomacy, Warfare), a draw a faction hook changes, and a rules version below the pool question
//! (`RULES_COMMAND_TOKEN_POOL`) end the window `unsupported`: the adapter drops it and the string result stands.
//!
//! The plain events of the string's `Game::emit` (`AGENDA_RESOLVED`, `AGENDA_DISCARDED`, `COVERT_AGENDA_RESOLVED`,
//! `AGENDA_OUTCOME_REDIRECTED`, `AGENDA_PREDICTION_CORRECT`, `LAW_ENACTED`) go to the emission queue (`sx.plain`) at the
//! program points of the pushes; the `AGENDA_RESOLVED` event is mirrored when its emission returns (package P1).

use super::{is_law, undisputed_winner};
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt, Rider};
use crate::choice::strategy::{gain_pool_choice, Ask as SAsk, Opt as SOpt};
use crate::choice::{Ask, Choice, Opt, Pick};
use crate::containers::SmallList;
use crate::generated::{Agenda, System, Planet, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Report, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{fx, fx_agenda, Kernel, Next};
use crate::keys::{LedgerReason, Outcome, Prediction, PredictionTag};
use crate::query::tech::UnitRoster;
use crate::state::{Code, CoreState, EventKind, EventRec, Key, Payload, PlainArg, PlainKind, PlainRec, TransientFlags, Val};
use crate::tables::Scope;

/// `ti4_model::state::RULES_COMMAND_TOKEN_POOL`: from here Leadership Rider asks the pool of each token.
pub const RULES_COMMAND_TOKEN_POOL: u32 = 12;
/// `ti4_model::state::RULES_TECH_RIDER`: from here the placement riders ask for the place.
pub const RULES_TECH_RIDER: u32 = 9;
/// `ti4_model::state::RULES_FACTION_BUGFIXES`.
const RULES_FACTION_BUGFIXES: u32 = 17;

/// What the window cannot read off the state.
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    /// The sources of the staged `TRADE_GOODS_GAINED` events (`trade_rider`, `keleres_rider`).
    pub trade_rider: Code,
    pub keleres_rider: Code,
    /// The unofficial action cards leave the deck before a draw (`RULES_OFFICIAL_ACTION_DECK`).
    pub drop_unofficial: bool,
    /// `laws::blanked`.
    pub blanked: bool,
    /// Seats (bit per seat) whose action card draw a faction hook changes (Scheming, a draw bonus).
    pub hooked: u16,
    /// The research of a correct Technology Rider: the source scope and the faction units of it. `None`: the adapter found a
    /// position the research window does not carry (a seat whose own effects on a research are questions or forms, hooks the core
    /// table answers differently), and the rider ends the window `unsupported`.
    pub research: Option<(Scope, &'static UnitRoster)>,
    /// The pool `fleet` of the command token a Diplomacy Rider makes the other seats place.
    pub fleet_pool: Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    /// Paying the correct predictions, one seat at a time.
    Predict,
    /// Waiting on the pool of a Leadership Rider token.
    Lead,
    /// A rider's action cards are drawn, or its technology researched, as a child window (the hand limit may ask a discard; the
    /// research asks which technology).
    Draw,
    /// A placement rider has several places to choose from.
    Target,
    Done,
}

/// What follows a rider's draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tail {
    /// Politics Rider: the speaker token.
    Speaker,
    /// Keleres Rider: two trade goods.
    Goods,
    /// Technology Rider: nothing follows the research.
    Nothing,
}

/// The close of a vote.
#[derive(Debug, Clone)]
pub struct Close {
    facts: Facts,
    /// The agenda that was voted on (Covert Legislation itself under Covert Legislation).
    vote: Agenda,
    /// Covert Legislation: the face-down agenda the vote decides.
    covert: Option<Agenda>,
    winner: Option<Outcome>,
    ballot: SmallList<(Seat, Outcome), 8>,
    counts: SmallList<(Outcome, i64), 16>,
    choices: SmallList<Outcome, 16>,
    stage: Stage,
    /// The predictions still to pay, taken off the state when the outcome is resolved.
    queue: SmallList<(Seat, Prediction), 8>,
    at: usize,
    /// Leadership Rider: whose tokens, and how many are still to place.
    who: Seat,
    left: u8,
    elected: Option<Seat>,
    effective: Option<Outcome>,
    discarded: bool,
    failed: bool,
    unsupported: bool,
    /// The draw of a Politics or Keleres Rider and what follows it.
    draw: ErasedChild,
    tail: Tail,
    /// A placement rider waiting for its place: which, and the places on offer (`None` planet: a system).
    rider: Rider,
    spots: SmallList<(System, Option<Planet>), 24>,
}

impl Close {
    /// The close of `vote`, decided for `winner` (`None`: nobody won) with `ballot` (who backed what) and `counts`
    /// (the votes each outcome received); `choices` are the outcomes on offer.
    #[must_use]
    pub fn new(
        facts: Facts,
        vote: Agenda,
        covert: Option<Agenda>,
        winner: Option<Outcome>,
        ballot: &[(Seat, Outcome)],
        counts: &[(Outcome, i64)],
        choices: &[Outcome],
    ) -> Self {
        Self {
            facts,
            vote,
            covert,
            winner,
            ballot: SmallList::from_slice(ballot),
            counts: SmallList::from_slice(counts),
            choices: SmallList::from_slice(choices),
            stage: Stage::Start,
            queue: SmallList::new(),
            at: 0,
            who: Seat(0),
            left: 0,
            elected: None,
            effective: None,
            discarded: false,
            failed: false,
            unsupported: false,
            draw: ErasedChild::idle(),
            tail: Tail::Speaker,
            rider: Rider::Construction,
            spots: SmallList::new(),
        }
    }

    /// The seat the outcome elected (the redirect applied): a seat, or the controller of an elected planet. `None`
    /// for any other outcome, for a discarded agenda and for a vote without a winner.
    #[must_use]
    pub const fn elected(&self) -> Option<Seat> {
        self.elected
    }

    /// The outcome the agenda's effect resolves with: the winner, or the seat a redirect named. `None` when the vote
    /// has no winner or the agenda was discarded.
    #[must_use]
    pub const fn effective(&self) -> Option<Outcome> {
        self.effective
    }

    /// Deadly Plot discarded the agenda.
    #[must_use]
    pub const fn discarded(&self) -> bool {
        self.discarded
    }

    /// The emission of `AGENDA_RESOLVED` failed (the string returns the error).
    #[must_use]
    pub const fn failed(&self) -> bool {
        self.failed
    }

    /// The position held something the window does not express (a rider it does not pay, a draw a hook changes):
    /// its result is not to be compared.
    #[must_use]
    pub const fn unsupported(&self) -> bool {
        self.unsupported
    }

    fn finish(&mut self) {
        self.stage = Stage::Done;
    }

    fn stop(&mut self) {
        self.unsupported = true;
        self.finish();
    }

    /// `settle_winner` up to the resolution of the outcome.
    fn start(&mut self, sx: &mut Sx<'_>) {
        let Some(winner) = self.winner else {
            fx_agenda::log_voted(sx.state, self.vote, is_law(self.vote), None, self.counts.as_slice(), false, false);
            sx.state.agenda_mut().predictions.clear();
            self.finish();
            return;
        };
        sx.plain(PlainRec::two(PlainKind::AgendaResolved, PlainArg::Agenda(self.vote), PlainArg::Outcome(winner)));
        // The ballot is on the state while the window is open: a Deadly Plot guard reads it.
        sx.state.agenda_mut().votes = self.ballot.as_slice().iter().copied().collect();
        // `player` is the outcome's text in the string: a seat when a player was elected ("when you are elected" reads it as the
        // actor), else the outcome itself (Deadly Plot's guard reads that).
        let mut payload = Payload::new().with(Key::Agenda, Val::Agenda(self.vote));
        match winner {
            Outcome::Seat(elected) => {
                payload.set(Key::Player, Val::Seat(elected));
                payload.set(Key::ElectedPlayer, Val::Seat(elected));
            }
            _ => payload.set(Key::Player, Val::Outcome(winner)),
        }
        let rec = EventRec { id: 0, kind: EventKind::AgendaResolved, payload, cancelled: false };
        if sx.emit_mirrored(rec).is_err() || crate::kernel::fx_cards::announce_staged_cards(sx).is_err() {
            self.failed = true;
            self.finish();
            return;
        }
        sx.state.agenda_mut().votes.clear();
        let transient = &mut sx.state.transient_mut().misc.flags;
        self.discarded = transient.has(TransientFlags::AGENDA_DISCARDED);
        transient.clear(TransientFlags::AGENDA_DISCARDED);

        let speaker_decided = undisputed_winner(self.counts.as_slice(), self.choices.as_slice()).is_none();
        fx_agenda::log_voted(sx.state, self.vote, is_law(self.vote), Some(winner), self.counts.as_slice(), speaker_decided, self.discarded);
        if let Some(hidden) = self.covert {
            // The face-down agenda is turned up and listed with the outcome it was given.
            fx_agenda::log_voted(sx.state, hidden, is_law(hidden), Some(winner), self.counts.as_slice(), speaker_decided, self.discarded);
            sx.plain(PlainRec::two(PlainKind::CovertAgendaResolved, PlainArg::Agenda(hidden), PlainArg::Outcome(winner)));
        }
        if self.discarded {
            sx.plain(PlainRec::one(PlainKind::AgendaDiscarded, PlainArg::Agenda(self.vote)));
            // "Resolved with no effect": a redirect played into the same window is spent on an election that has none.
            let agenda = sx.state.agenda_mut();
            agenda.elected_override = None;
            agenda.predictions.clear();
            self.finish();
            return;
        }
        // `redirect_elected`, then `elected_seat_or_planet` of the effective outcome.
        let redirect = sx.state.agenda_mut().elected_override.take();
        if let Some(seat) = redirect {
            sx.plain(PlainRec::two(PlainKind::AgendaOutcomeRedirected, PlainArg::Outcome(winner), PlainArg::Seat(seat)));
        }
        let effective = redirect.map_or(winner, Outcome::Seat);
        self.effective = Some(effective);
        self.elected = Self::elected_by(sx.state, effective);
        let agenda = sx.state.agenda_mut();
        self.queue = agenda.predictions.iter().map(|(seat, prediction)| (*seat, *prediction)).collect();
        agenda.predictions.clear();
        self.stage = Stage::Predict;
    }

    /// `elected_seat_or_planet`: the seat the outcome names, else the controller of the planet it names.
    fn elected_by(s: &CoreState, outcome: Outcome) -> Option<Seat> {
        match outcome {
            Outcome::Seat(seat) => s.players().by_seat(seat).map(|_| seat),
            Outcome::Planet(planet) => s.board().planets().find(|(p, _)| *p == planet).and_then(|(_, cell)| cell.control),
            _ => None,
        }
    }

    /// Whether a faction hook changes the seat's draw (the adapter's mask): the draw is the string's. A hand that ends over its
    /// limit is the draw window's own question.
    fn hooked(&self, who: Seat) -> bool {
        self.facts.hooked & (1 << u16::from(who.0)) != 0
    }

    /// Start the draw of `count` cards for `who` as a child window; the run is unsupported when it cannot be followed.
    fn start_draw(&mut self, sx: &mut Sx<'_>, who: Seat, count: usize, tail: Tail) {
        if self.hooked(who) {
            self.stop();
            return;
        }
        match sx.table.links.start(&Spawn::DrawCards { who, count, drop_unofficial: self.facts.drop_unofficial }) {
            Some(window) => {
                self.draw.start(window);
                self.who = who;
                self.tail = tail;
                self.stage = Stage::Draw;
            }
            None => self.stop(),
        }
    }

    /// Technology Rider: "research 1 technology" as a child window (the Technology primary's free research, the card as the source of
    /// the question); the run is unsupported when it cannot be followed.
    fn start_research(&mut self, sx: &mut Sx<'_>, who: Seat) {
        let Some((scope, roster)) = self.facts.research else {
            self.stop();
            return;
        };
        match sx.table.links.start(&Spawn::RiderResearch { who, scope, roster }) {
            Some(window) => {
                self.draw.start(window);
                self.who = who;
                self.tail = Tail::Nothing;
                self.stage = Stage::Draw;
            }
            None => self.stop(),
        }
    }

    /// `rider_payoff` for one correct prediction. A rider that asks puts the window in the stage of its question.
    fn pay(&mut self, sx: &mut Sx<'_>, who: Seat, tag: Option<PredictionTag>) {
        if !sx.state.meta().rules_at_least(RULES_COMMAND_TOKEN_POOL) {
            self.stop();
            return;
        }
        match tag {
            None => {
                fx::adjust_victory_points(sx.state, who, 1, LedgerReason::ImperialRider);
            }
            Some(PredictionTag::Sanction) => {}
            Some(PredictionTag::LeadRider) => {
                self.who = who;
                self.left = 3;
                self.stage = Stage::Lead;
            }
            Some(PredictionTag::TradeRider) => {
                fx::gain_trade_goods_staged(sx.state, who, 5, self.facts.trade_rider);
            }
            Some(PredictionTag::PoliticRider) => self.start_draw(sx, who, 3, Tail::Speaker),
            Some(PredictionTag::KeleresRider) => self.start_draw(sx, who, 1, Tail::Goods),
            Some(PredictionTag::TechRider) => self.start_research(sx, who),
            Some(PredictionTag::ConstRider) => self.place_rider(sx, who, Rider::Construction),
            Some(PredictionTag::DiploRider) => self.place_rider(sx, who, Rider::Diplomacy),
            Some(PredictionTag::WarRider) => self.place_rider(sx, who, Rider::Warfare),
        }
    }

    /// The places a placement rider offers `who`, in the string's order: the controlled planets without a space dock yet (79.2; the dock I
    /// forms only: the string compares the unit's id with `spacedock`), the systems with a planet the seat controls, the systems with a
    /// ship of the seat.
    fn spots_of(core: &CoreState, who: Seat, rider: Rider) -> SmallList<(System, Option<Planet>), 24> {
        let mut out = SmallList::new();
        match rider {
            Rider::Construction => {
                for (system, planet) in crate::query::board::controlled_planets(core, who).as_slice() {
                    let docked = core.board().planet(*planet).units.as_slice().iter().any(|p| {
                        let d = p.kind().data();
                        d.base_type == UnitBaseType::Spacedock && d.replaces.is_none()
                    });
                    if !docked {
                        out.push((*system, Some(*planet)));
                    }
                }
            }
            Rider::Diplomacy => {
                for (system, _) in crate::query::board::controlled_planets(core, who).as_slice() {
                    if !out.as_slice().iter().any(|(s, _)| s == system) {
                        out.push((*system, None));
                    }
                }
            }
            Rider::Warfare => {
                for system in System::ALL {
                    if core.board().sys(system).units.as_slice().iter().any(|p| p.owner() == who) {
                        out.push((system, None));
                    }
                }
            }
        }
        out
    }

    /// `asked_rider_payoff`: one place is forced, several are asked, none pays nothing.
    fn place_rider(&mut self, sx: &mut Sx<'_>, who: Seat, rider: Rider) {
        if !sx.state.meta().rules_at_least(RULES_TECH_RIDER) {
            self.stop();
            return;
        }
        let spots = Self::spots_of(sx.state, who, rider);
        match spots.as_slice() {
            [] => {}
            [(system, planet)] => self.place(sx, who, rider, *system, *planet),
            _ => {
                self.who = who;
                self.rider = rider;
                self.spots = spots;
                self.stage = Stage::Target;
            }
        }
    }

    fn target_question(&self) -> Choice {
        let opts = self.spots.as_slice().iter().map(|(system, planet)| Opt::Agenda(AOpt::Spot { system: *system, planet: *planet })).collect();
        super::question(self.who, AAsk::RiderTarget { rider: self.rider }, crate::choice::Origin::ActionCard(match self.rider {
            Rider::Construction => crate::generated::ActionCard::ConstructionRider,
            Rider::Diplomacy => crate::generated::ActionCard::DiplomacyRider,
            Rider::Warfare => crate::generated::ActionCard::WarfareRider,
        }), opts)
    }

    /// The reward at the chosen place.
    fn place(&mut self, sx: &mut Sx<'_>, who: Seat, rider: Rider, system: System, planet: Option<Planet>) {
        match rider {
            Rider::Construction => {
                let (Some((scope, roster)), Some(planet)) = (self.facts.research, planet) else {
                    self.stop();
                    return;
                };
                let Some(dock) = crate::kernel::fx_strategy::placed_unit(sx.state, sx.kernel, scope, roster, who, UnitBaseType::Spacedock) else { return };
                // From rules version 17 a space-only form (Saar's Floating Factory) is placed as itself in the space area of the planet's system.
                let in_space = sx.state.meta().rules_at_least(RULES_FACTION_BUGFIXES) && dock.data().space_only;
                crate::kernel::fx::place_unit(sx.state, who, system, if in_space { None } else { Some(planet) }, dock);
            }
            Rider::Diplomacy => {
                let others: SmallList<Seat, 8> = sx.state.seats().seating_order().iter().copied().filter(|o| *o != who).collect();
                let holders: SmallList<Seat, 8> = others.as_slice().iter().copied().filter(|o| sx.state.players().by_seat(*o).is_some_and(|p| p.econ.fleet_tokens > 0)).collect();
                for other in holders.as_slice() {
                    if let Some(p) = sx.state.players_mut().by_seat_mut(*other) {
                        p.econ.fleet_tokens -= 1;
                    }
                    if fx::place_command_token(sx.state, *other, system) {
                        let payload = Payload::new().with(Key::Player, Val::Seat(*other)).with(Key::System, Val::System(system)).with(Key::Pool, Val::Code(self.facts.fleet_pool));
                        fx::stage(sx.state, fx::event(EventKind::CommandTokenPlaced, payload));
                    }
                }
            }
            Rider::Warfare => {
                fx::place_unit(sx.state, who, system, None, Unit::DreadnoughtI);
            }
        }
    }

    /// `enact_if_law`: an elected or "For" law stays in play (8.20), Classified Document Leaks publishes its secret.
    fn enact(&mut self, sx: &mut Sx<'_>) {
        let (Some(winner), resolved) = (self.winner, self.covert.unwrap_or(self.vote)) else { return };
        if !is_law(resolved) || winner == Outcome::Against {
            return;
        }
        if resolved == Agenda::ClassifiedDocumentLeaks && !matches!(winner, Outcome::Secret(_)) {
            // The string would publish whatever text the outcome is as an objective.
            self.stop();
            return;
        }
        fx_agenda::enact(sx.state, resolved, winner);
        sx.plain(PlainRec::two(PlainKind::LawEnacted, PlainArg::Agenda(resolved), PlainArg::Outcome(winner)));
        if let (Agenda::ClassifiedDocumentLeaks, Outcome::Secret(secret)) = (resolved, winner)
            && !self.facts.blanked
        {
            fx_agenda::leak_secret(sx.state, secret);
        }
    }
}

impl Window for Close {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Done => return Next::Done,
                Stage::Start => self.start(sx),
                Stage::Lead => return Next::Continue(Ask::Strategy(SAsk::GainPool { left: self.left })),
                Stage::Target => return Next::Continue(Ask::Agenda(AAsk::RiderTarget { rider: self.rider })),
                Stage::Predict => {
                    if self.at >= self.queue.len() {
                        self.enact(sx);
                        if self.stage == Stage::Predict {
                            self.finish();
                        }
                        continue;
                    }
                    let (seat, prediction) = self.queue.as_slice()[self.at];
                    self.at += 1;
                    let (hit, tag) = match prediction {
                        Prediction::Plain(outcome) => (Some(outcome) == self.winner, None),
                        Prediction::Tagged(outcome, tag) => (Some(outcome) == self.winner, Some(tag)),
                        Prediction::Barred(_) => (false, None),
                    };
                    if hit {
                        sx.plain(PlainRec::one(PlainKind::AgendaPredictionCorrect, PlainArg::Seat(seat)));
                    }
                    if hit {
                        self.pay(sx, seat, tag);
                    }
                    if hit && self.stage == Stage::Lead {
                        return Next::Continue(Ask::Strategy(SAsk::GainPool { left: self.left }));
                    }
                    if hit && self.stage == Stage::Target {
                        return Next::Continue(Ask::Agenda(AAsk::RiderTarget { rider: self.rider }));
                    }
                }
                Stage::Draw => match self.draw.step(sx) {
                    Some(Ran::Asks(a)) => return Next::Continue(a),
                    Some(Ran::Pass(next)) => return next,
                    Some(Ran::Done(Report::Left)) => self.stop(),
                    Some(Ran::Done(_)) | None => {
                        match self.tail {
                            Tail::Speaker => sx.state.meta_mut().speaker = self.who,
                            Tail::Goods => {
                                fx::gain_trade_goods_staged(sx.state, self.who, 2, self.facts.keleres_rider);
                            }
                            Tail::Nothing => {}
                        }
                        self.stage = Stage::Predict;
                    }
                },
            }
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            Stage::Lead => Some(gain_pool_choice(self.who, self.left)),
            Stage::Target => Some(self.target_question()),
            Stage::Draw => self.draw.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == Stage::Draw {
            return self.draw.answer(sx, pick);
        }
        if self.stage == Stage::Target {
            let Pick::Opt(Opt::Agenda(AOpt::Spot { system, planet })) = pick else { return Err(Illegal) };
            if !self.spots.as_slice().contains(&(system, planet)) {
                return Err(Illegal);
            }
            self.place(sx, self.who, self.rider, system, planet);
            self.stage = Stage::Predict;
            return Ok(());
        }
        let Pick::Opt(Opt::Strategy(SOpt::Pool(pool))) = pick else { return Err(Illegal) };
        if self.stage != Stage::Lead {
            return Err(Illegal);
        }
        fx::gain_token(sx.state, self.who, pool, 1);
        self.left -= 1;
        if self.left == 0 {
            self.stage = Stage::Predict;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::strategy::Pool;
    use crate::driver::{Driver, Status};
    use crate::generated::{ActionCard, AgendaType, Faction, Planet, SecretObjective};
    use crate::kernel::stage::AbilityTable;
    use crate::kernel::Resolver;
    use crate::state::{Player, Players, Seats};

    const A: Seat = Seat(0);
    const B: Seat = Seat(1);
    const C: Seat = Seat(2);

    fn facts() -> Facts {
        Facts { trade_rider: Code(1), keleres_rider: Code(2), drop_unofficial: false, blanked: false, hooked: 0, research: None, fleet_pool: Code(3) }
    }

    fn game() -> CoreState {
        let mut s = CoreState::all_loaded();
        let players: Vec<Player> = (0..3u8).map(|i| Player::new(Seat(i), Faction::ALL[usize::from(i)])).collect();
        let list = [A, B, C];
        s.seats = Seats::new(&list, &list, 3);
        s.players = Players::from_vec(players);
        s.meta_mut().rules_version = 35;
        s.meta_mut().round = 2;
        s
    }

    fn law() -> Agenda {
        let a = Agenda::ChecksAndBalances;
        assert_eq!(a.data().kind, AgendaType::Law);
        a
    }

    fn directive() -> Agenda {
        let a = Agenda::SeedOfAnEmpire;
        assert_eq!(a.data().kind, AgendaType::Directive);
        a
    }

    const COUNTS: [(Outcome, i64); 2] = [(Outcome::For, 3), (Outcome::Against, 1)];

    fn close(agenda: Agenda, winner: Option<Outcome>) -> Close {
        Close::new(facts(), agenda, None, winner, &[(A, Outcome::For), (B, Outcome::Against)], &COUNTS, &[Outcome::For, Outcome::Against])
    }

    /// The table of a window that runs children: the links of the areas and the printed-name groups (each card its own name).
    fn linked(groups: bool) -> AbilityTable {
        let mut t = AbilityTable::default();
        crate::driver::children::register(&mut t);
        if groups {
            t.facts.card_groups = (0..ActionCard::COUNT).map(|i| u8::try_from(i).unwrap()).collect();
        }
        t
    }

    /// Run the window, answering each pool question with `pools` in turn.
    fn run(window: Close, s: &mut CoreState, pools: &[Pool]) -> Driver<Close> {
        let k = Kernel::plain();
        let mut d = Driver::new(window, Resolver::new(&[A], Some(A)), linked(true), 1);
        let mut status = d.run(s, &k).unwrap();
        for pool in pools {
            assert!(matches!(status, Status::Asking(_)), "asks before {pool:?}");
            status = d.resume(s, &k, Pick::Opt(Opt::Strategy(SOpt::Pool(*pool)))).unwrap();
        }
        assert_eq!(status, Status::Done);
        d
    }

    #[test]
    fn a_vote_without_a_winner_is_logged_and_forgets_the_predictions() {
        let mut s = game();
        s.agenda_mut().predictions.insert(A, Prediction::Plain(Outcome::For));
        let d = run(close(law(), None), &mut s, &[]);
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.agenda, rec.outcome, rec.enacted, rec.discarded, rec.speaker_decided, rec.round), (law(), None, false, false, false, 2));
        assert!(s.agenda().predictions.is_empty());
        assert!(d.emitted.is_empty(), "nothing is announced");
        assert_eq!((d.window.elected(), d.window.effective(), d.window.discarded()), (None, None, false));
    }

    #[test]
    fn a_law_that_passes_is_announced_logged_and_enacted() {
        let mut s = game();
        let d = run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!(d.emitted.len(), 1);
        assert_eq!(d.emitted[0].kind, EventKind::AgendaResolved);
        assert_eq!(d.emitted[0].payload.get(Key::Agenda), Some(Val::Agenda(law())));
        assert_eq!(d.emitted[0].payload.get(Key::Player), Some(Val::Outcome(Outcome::For)));
        assert_eq!(d.emitted[0].payload.get(Key::ElectedPlayer), None, "only a seat is elected");
        assert_eq!(s.agenda().laws.get(&law()), Some(&Outcome::For));
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.law, rec.enacted, rec.outcome, rec.speaker_decided), (true, true, Some(Outcome::For), false));
        assert_eq!(rec.votes.get(&Outcome::For), Some(&3));
        assert_eq!(d.window.effective(), Some(Outcome::For));
        assert!(s.agenda().votes.is_empty(), "the ballot is only on the state while the window is open");
        assert_eq!(s.agenda().log.iter().count(), 1);
    }

    #[test]
    fn a_law_that_passes_queues_the_plain_events_around_the_mirrored_resolution_in_the_strings_order() {
        use crate::state::queue_kinds;
        use PlainKind::{AgendaResolved, LawEnacted};
        let mut s = game();
        run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!(queue_kinds(&s), [Ok(AgendaResolved), Err(EventKind::AgendaResolved), Ok(LawEnacted)]);
        let plain: Vec<PlainRec> = s.ledger.events.iter().filter_map(|e| e.plain().copied()).collect();
        assert_eq!(plain[0], PlainRec::two(AgendaResolved, PlainArg::Agenda(law()), PlainArg::Outcome(Outcome::For)));
        assert_eq!(plain[1], PlainRec::two(LawEnacted, PlainArg::Agenda(law()), PlainArg::Outcome(Outcome::For)));
        // The mirrored line is the event as the resolver ended it (trace id, payload), not a bare name.
        let typed = s.ledger.events.iter().find_map(|e| if let crate::state::Emit::Typed(t) = e { Some(t.clone()) } else { None }).expect("mirrored");
        assert_eq!((typed.kind, typed.payload.get(Key::Agenda)), (EventKind::AgendaResolved, Some(Val::Agenda(law()))));
        assert_ne!(typed.id, 0, "the trace id the emitter assigned");
        // A vote without a winner pushes nothing.
        let mut s = game();
        run(close(law(), None), &mut s, &[]);
        assert!(queue_kinds(&s).is_empty());
    }

    #[test]
    fn a_redirect_a_correct_prediction_and_a_law_queue_in_the_order_of_the_string() {
        use crate::state::queue_kinds;
        use PlainKind::{AgendaOutcomeRedirected, AgendaPredictionCorrect, AgendaResolved, LawEnacted};
        let mut s = game();
        s.agenda_mut().elected_override = Some(B);
        s.agenda_mut().predictions.insert(A, Prediction::Plain(Outcome::Seat(C)));
        let w = Close::new(facts(), Agenda::PoliticalCensure, None, Some(Outcome::Seat(C)), &[], &[(Outcome::Seat(C), 2)], &[Outcome::Seat(C)]);
        run(w, &mut s, &[]);
        assert_eq!(
            queue_kinds(&s),
            [Ok(AgendaResolved), Err(EventKind::AgendaResolved), Ok(AgendaOutcomeRedirected), Ok(AgendaPredictionCorrect), Ok(LawEnacted)]
        );
        let redirected = s.ledger.events.iter().filter_map(|e| e.plain()).find(|p| p.kind == AgendaOutcomeRedirected).copied();
        assert_eq!(redirected, Some(PlainRec::two(AgendaOutcomeRedirected, PlainArg::Outcome(Outcome::Seat(C)), PlainArg::Seat(B))));
    }

    #[test]
    fn a_covert_vote_and_a_deadly_plot_discard_queue_their_plain_events() {
        use crate::state::queue_kinds;
        use PlainKind::{AgendaDiscarded, AgendaResolved, CovertAgendaResolved};
        let mut s = game();
        s.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
        let w = Close::new(facts(), Agenda::CovertLegislation, Some(law()), Some(Outcome::For), &[], &COUNTS, &[Outcome::For, Outcome::Against]);
        run(w, &mut s, &[]);
        assert_eq!(queue_kinds(&s), [Ok(AgendaResolved), Err(EventKind::AgendaResolved), Ok(CovertAgendaResolved), Ok(AgendaDiscarded)]);
    }

    #[test]
    fn against_and_a_directive_enact_nothing() {
        let mut s = game();
        run(close(law(), Some(Outcome::Against)), &mut s, &[]);
        assert!(s.agenda().laws.is_empty());
        assert!(!s.agenda().log.last().unwrap().enacted);
        run(close(directive(), Some(Outcome::For)), &mut s, &[]);
        assert!(s.agenda().laws.is_empty());
        assert!(!s.agenda().log.last().unwrap().law);
    }

    #[test]
    fn a_tie_broken_by_the_speaker_is_recorded_as_such() {
        let mut s = game();
        let tied = Close::new(facts(), law(), None, Some(Outcome::For), &[], &[(Outcome::For, 2), (Outcome::Against, 2)], &[Outcome::For, Outcome::Against]);
        run(tied, &mut s, &[]);
        assert!(s.agenda().log.last().unwrap().speaker_decided);
    }

    #[test]
    fn the_election_of_a_seat_names_it_in_the_announcement() {
        let mut s = game();
        let w = Close::new(facts(), Agenda::PoliticalCensure, None, Some(Outcome::Seat(C)), &[(A, Outcome::Seat(C))], &[(Outcome::Seat(C), 2)], &[Outcome::Seat(B), Outcome::Seat(C)]);
        let d = run(w, &mut s, &[]);
        assert_eq!(d.emitted[0].payload.get(Key::ElectedPlayer), Some(Val::Seat(C)));
        assert_eq!(d.emitted[0].payload.get(Key::Player), Some(Val::Seat(C)), "the actor of 'when you are elected'");
        assert_eq!(d.window.elected(), Some(C));
        assert_eq!(s.agenda().laws.get(&Agenda::PoliticalCensure), Some(&Outcome::Seat(C)));
        assert!(!s.agenda().log.last().unwrap().speaker_decided);
    }

    #[test]
    fn a_redirect_moves_the_election_but_not_the_law() {
        let mut s = game();
        s.agenda_mut().elected_override = Some(B);
        let w = Close::new(facts(), Agenda::PoliticalCensure, None, Some(Outcome::Seat(C)), &[], &[(Outcome::Seat(C), 2)], &[Outcome::Seat(C)]);
        let d = run(w, &mut s, &[]);
        assert_eq!((d.window.effective(), d.window.elected()), (Some(Outcome::Seat(B)), Some(B)));
        assert_eq!(s.agenda().elected_override, None, "the redirect is spent");
        assert_eq!(s.agenda().log.last().unwrap().redirected_to, Some(Outcome::Seat(B)), "listed before it is taken");
        assert_eq!(s.agenda().laws.get(&Agenda::PoliticalCensure), Some(&Outcome::Seat(C)), "the vote's result is the law's");
    }

    #[test]
    fn an_elected_planet_elects_its_controller() {
        let mut s = game();
        let planet = Planet::ALL[5];
        s.board_mut().planet_mut(planet).control = Some(B);
        let w = Close::new(facts(), Agenda::HolyPlanetOfIxth, None, Some(Outcome::Planet(planet)), &[], &[(Outcome::Planet(planet), 1)], &[Outcome::Planet(planet)]);
        let d = run(w, &mut s, &[]);
        assert_eq!(d.window.elected(), Some(B));
        // For and Against elect nobody.
        let d = run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!(d.window.elected(), None);
    }

    #[test]
    fn a_discarded_agenda_is_resolved_with_no_effect() {
        let mut s = game();
        s.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
        s.agenda_mut().elected_override = Some(B);
        s.agenda_mut().predictions.insert(A, Prediction::Plain(Outcome::For));
        let d = run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert!(d.window.discarded());
        assert_eq!((d.window.elected(), d.window.effective()), (None, None));
        assert!(s.agenda().laws.is_empty());
        assert_eq!((s.agenda().elected_override, s.agenda().predictions.len()), (None, 0));
        let rec = s.agenda().log.last().unwrap();
        assert_eq!((rec.discarded, rec.enacted, rec.redirected_to), (true, false, None));
        assert!(!s.transient().misc.flags.has(TransientFlags::AGENDA_DISCARDED), "the mark is read once");
        assert_eq!(s.players().by_seat(A).unwrap().econ.victory_points, 0, "no rider pays");
    }

    #[test]
    fn covert_legislation_lists_the_face_down_agenda_and_enacts_it() {
        let mut s = game();
        let w = Close::new(
            facts(),
            Agenda::CovertLegislation,
            Some(law()),
            Some(Outcome::For),
            &[],
            &COUNTS,
            &[Outcome::For, Outcome::Against],
        );
        let d = run(w, &mut s, &[]);
        let agendas: Vec<(Agenda, bool)> = s.agenda().log.iter().map(|r| (r.agenda, r.law)).collect();
        assert_eq!(agendas[1], (law(), true));
        assert_eq!(agendas.len(), 2);
        assert_eq!(d.emitted[0].payload.get(Key::Agenda), Some(Val::Agenda(Agenda::CovertLegislation)));
        assert_eq!(s.agenda().laws.get(&law()), Some(&Outcome::For), "the face-down agenda is the law");
        assert!(s.agenda().laws.get(&Agenda::CovertLegislation).is_none());
    }

    #[test]
    fn classified_document_leaks_publishes_the_elected_secret() {
        let mut s = game();
        let secret = SecretObjective::ALL[3];
        let w = |facts| Close::new(facts, Agenda::ClassifiedDocumentLeaks, None, Some(Outcome::Secret(secret)), &[], &[(Outcome::Secret(secret), 1)], &[Outcome::Secret(secret)]);
        run(w(facts()), &mut s, &[]);
        assert_eq!(s.objectives().revealed.as_slice(), [crate::state::ObjectiveRef::Secret(secret)]);
        assert_eq!(s.agenda().laws.get(&Agenda::ClassifiedDocumentLeaks), Some(&Outcome::Secret(secret)));
        // Blanked, the law is not "in play": nothing leaks.
        let mut s = game();
        run(w(Facts { blanked: true, ..facts() }), &mut s, &[]);
        assert!(s.objectives().revealed.as_slice().is_empty());
        // Any other outcome is a text the string would publish as an objective: no window.
        let mut s = game();
        let bad = Close::new(facts(), Agenda::ClassifiedDocumentLeaks, None, Some(Outcome::For), &[], &[], &[]);
        let d = run(bad, &mut s, &[]);
        assert!(d.window.unsupported());
    }

    fn predict(s: &mut CoreState, seat: Seat, p: Prediction) {
        s.agenda_mut().predictions.insert(seat, p);
    }

    fn goods(s: &CoreState, seat: Seat) -> i16 {
        s.players().by_seat(seat).unwrap().econ.trade_goods
    }

    fn points(s: &CoreState, seat: Seat) -> i16 {
        s.players().by_seat(seat).unwrap().econ.victory_points
    }

    #[test]
    fn only_correct_predictions_pay() {
        let mut s = game();
        predict(&mut s, A, Prediction::Plain(Outcome::For));
        predict(&mut s, B, Prediction::Plain(Outcome::Against));
        predict(&mut s, C, Prediction::Barred(crate::keys::VoteBar::Assassin));
        let d = run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!((points(&s, A), points(&s, B), points(&s, C)), (1, 0, 0));
        assert!(s.agenda().predictions.is_empty(), "a prediction left behind would pay again");
        assert_eq!(s.ledger().vp.len(), 1);
        assert!(!d.window.unsupported());
    }

    #[test]
    fn trade_rider_pays_five_goods_and_announces_them() {
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::TradeRider));
        predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::Sanction));
        run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!((goods(&s, B), goods(&s, C)), (5, 0));
        let staged = &s.pending().staged;
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].payload.get(Key::Source), Some(Val::Code(Code(1))));
        assert_eq!(staged[0].payload.get(Key::Amount), Some(Val::Int(5)));
    }

    #[test]
    fn politics_rider_draws_three_and_takes_the_speaker_token() {
        let mut s = game();
        s.decks_mut().action.set(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2], ActionCard::ALL[3]]);
        predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
        run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!(s.players().by_seat(C).unwrap().cards.hand.len(), 3);
        assert_eq!(s.decks().action.cards().len(), 1);
        assert_eq!(s.meta().speaker, C);
    }

    #[test]
    fn a_draw_that_a_hook_changes_is_not_expressed() {
        let deck = vec![ActionCard::ALL[0]; 4];
        let mut s = game();
        s.decks_mut().action.set(deck.clone());
        predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
        let hooked = Facts { hooked: 1 << 2, ..facts() };
        let w = Close::new(hooked, law(), None, Some(Outcome::For), &[], &COUNTS, &[]);
        assert!(run(w, &mut s, &[]).window.unsupported());
        assert_eq!(s.players().by_seat(C).unwrap().cards.hand.len(), 0, "nothing was drawn");
        // Hand at 4: three more reaches the limit of seven exactly.
        let mut s = game();
        s.decks_mut().action.set(deck);
        s.players_mut().by_seat_mut(C).unwrap().cards.hand = vec![ActionCard::ALL[5]; 4].into_iter().collect();
        predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
        assert!(!run(close(law(), Some(Outcome::For)), &mut s, &[]).window.unsupported());
        assert_eq!(s.players().by_seat(C).unwrap().cards.hand.len(), 7);
    }

    /// W6 regression: a Politics Rider draw that ends over the hand limit was `unsupported` (a rider or draw the core does not pay,
    /// 8 of 141 closes per 20 golden games); the child draw window asks the discard, and the speaker token comes after it.
    #[test]
    fn a_politics_rider_draw_over_the_hand_limit_asks_the_discard_then_takes_the_speaker_token() {
        let mut s = game();
        s.decks_mut().action.set(vec![ActionCard::ALL[0], ActionCard::ALL[1], ActionCard::ALL[2]]);
        s.players_mut().by_seat_mut(C).unwrap().cards.hand = (10..15).map(|i| ActionCard::ALL[i]).collect();
        predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
        let k = Kernel::plain();
        let mut d = Driver::new(close(law(), Some(Outcome::For)), Resolver::new(&[A], Some(A)), linked(true), 1);
        let Status::Asking(choice) = d.run(&mut s, &k).unwrap() else { panic!("eight cards: a discard") };
        assert_eq!(choice.actor, C);
        assert_eq!(choice.ask, Ask::Cards(crate::choice::cards::Ask::HandLimit { held: 8 }));
        assert_eq!(s.meta().speaker, Seat(0), "the token waits for the discard");
        assert_eq!(d.resume(&mut s, &k, choice.pick(0).unwrap()).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(C).unwrap().cards.hand.len(), 7);
        assert_eq!(s.meta().speaker, C);
        assert!(!d.window.unsupported());
    }

    /// The Keleres Rider's goods come after its card's discard.
    #[test]
    fn a_keleres_rider_draw_over_the_hand_limit_asks_the_discard_then_pays_the_goods() {
        let mut s = game();
        s.decks_mut().action.set(vec![ActionCard::ALL[0]]);
        s.players_mut().by_seat_mut(B).unwrap().cards.hand = (10..17).map(|i| ActionCard::ALL[i]).collect();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::KeleresRider));
        let k = Kernel::plain();
        let mut d = Driver::new(close(law(), Some(Outcome::For)), Resolver::new(&[A], Some(A)), linked(true), 1);
        let Status::Asking(choice) = d.run(&mut s, &k).unwrap() else { panic!("eight cards: a discard") };
        assert_eq!(goods(&s, B), 0, "the goods follow the discard");
        assert_eq!(d.resume(&mut s, &k, choice.pick(0).unwrap()).unwrap(), Status::Done);
        assert_eq!((s.players().by_seat(B).unwrap().cards.hand.len(), goods(&s, B)), (7, 2));
    }

    /// Bail: a table without the link to the draw window (or without printed-name groups over the limit) ends the window unsupported.
    #[test]
    fn bail_a_rider_draw_without_the_link_or_the_groups_is_unsupported() {
        for table in [AbilityTable::default(), linked(false)] {
            let mut s = game();
            s.decks_mut().action.set(vec![ActionCard::ALL[0]; 4]);
            s.players_mut().by_seat_mut(C).unwrap().cards.hand = (10..17).map(|i| ActionCard::ALL[i]).collect();
            predict(&mut s, C, Prediction::Tagged(Outcome::For, PredictionTag::PoliticRider));
            let mut d = Driver::new(close(law(), Some(Outcome::For)), Resolver::new(&[A], Some(A)), table, 1);
            assert_eq!(d.run(&mut s, &Kernel::plain()).unwrap(), Status::Done);
            assert!(d.window.unsupported());
        }
    }

    #[test]
    fn keleres_rider_draws_one_card_and_two_goods() {
        let mut s = game();
        s.decks_mut().action.set(vec![ActionCard::ALL[0], ActionCard::ALL[1]]);
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::KeleresRider));
        run(close(law(), Some(Outcome::For)), &mut s, &[]);
        assert_eq!((s.players().by_seat(B).unwrap().cards.hand.len(), goods(&s, B)), (1, 2));
        assert_eq!(s.pending().staged[0].payload.get(Key::Source), Some(Val::Code(Code(2))));
    }

    #[test]
    fn leadership_rider_asks_the_pool_of_each_of_three_tokens() {
        let mut s = game();
        for seat in [A, B, C] {
            let p = s.players_mut().by_seat_mut(seat).unwrap();
            p.econ.tactic_tokens = 0;
            p.econ.fleet_tokens = 0;
            p.econ.strategic_tokens = 0;
        }
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::LeadRider));
        predict(&mut s, C, Prediction::Plain(Outcome::For));
        let d = run(close(law(), Some(Outcome::For)), &mut s, &[Pool::Fleet, Pool::Fleet, Pool::Tactic]);
        let p = s.players().by_seat(B).unwrap();
        assert_eq!((p.econ.fleet_tokens, p.econ.tactic_tokens, p.econ.strategic_tokens), (2, 1, 0));
        assert_eq!(points(&s, C), 1, "the next prediction is paid after the questions");
        assert!(!d.window.unsupported());
    }

    #[test]
    fn leadership_rider_puts_the_remaining_count_in_each_question() {
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::LeadRider));
        let k = Kernel::plain();
        let mut d = Driver::new(close(law(), Some(Outcome::For)), Resolver::new(&[A], Some(A)), AbilityTable::default(), 1);
        let mut status = d.run(&mut s, &k).unwrap();
        for left in [3, 2, 1] {
            let Status::Asking(c) = status else { panic!("asks with {left} left") };
            assert_eq!((c.actor, c.ask), (B, Ask::Strategy(SAsk::GainPool { left })));
            status = d.resume(&mut s, &k, Pick::Opt(Opt::Strategy(SOpt::Pool(Pool::Tactic)))).unwrap();
        }
        assert_eq!(status, Status::Done);
    }

    #[test]
    fn riders_that_place_and_old_games_are_not_expressed() {
        for tag in [PredictionTag::ConstRider, PredictionTag::DiploRider, PredictionTag::WarRider] {
            let mut s = game();
            predict(&mut s, B, Prediction::Tagged(Outcome::For, tag));
            assert!(run(close(law(), Some(Outcome::For)), &mut s, &[]).window.unsupported(), "{tag:?}");
            // Wrong prediction: nothing to pay, so the window stands.
            let mut s = game();
            predict(&mut s, B, Prediction::Tagged(Outcome::Against, tag));
            assert!(!run(close(law(), Some(Outcome::For)), &mut s, &[]).window.unsupported(), "{tag:?} missed");
        }
        let mut s = game();
        s.meta_mut().rules_version = RULES_COMMAND_TOKEN_POOL - 1;
        predict(&mut s, B, Prediction::Plain(Outcome::For));
        assert!(run(close(law(), Some(Outcome::For)), &mut s, &[]).window.unsupported());
        s.meta_mut().rules_version = RULES_COMMAND_TOKEN_POOL;
        let mut s2 = s.clone();
        predict(&mut s2, B, Prediction::Plain(Outcome::For));
        assert!(!run(close(law(), Some(Outcome::For)), &mut s2, &[]).window.unsupported());
    }

    // -- W4: the Technology Rider ------------------------------------------------------------------------

    fn researching() -> Facts {
        Facts { research: Some((Scope(u32::MAX), Box::leak(Box::default()))), ..facts() }
    }

    fn close_with(facts: Facts) -> Close {
        Close::new(facts, law(), None, Some(Outcome::For), &[(A, Outcome::For)], &COUNTS, &[Outcome::For, Outcome::Against])
    }

    /// A correct Technology Rider is the Technology primary's free research asked as the card's question, and nothing after it
    /// (no second, paid research); the technology is gained.
    #[test]
    fn a_correct_technology_rider_researches_one_free_technology_as_the_cards_question() {
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::TechRider));
        let k = Kernel::plain();
        let before = s.players().by_seat(B).unwrap().tech.owned;
        let mut d = Driver::new(close_with(researching()), Resolver::new(&[A], Some(A)), linked(true), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("asks which technology") };
        assert_eq!(c.actor, B);
        assert_eq!(c.ctx.origin, crate::choice::Origin::ActionCard(ActionCard::TechnologyRider));
        assert!(!c.can_decline(), "\"research 1 technology\" is not optional");
        let Some(Pick::Opt(Opt::Strategy(SOpt::Research(t)))) = c.pick(0) else { panic!("a technology") };
        let status = d.resume(&mut s, &k, c.pick(0).unwrap()).unwrap();
        assert_eq!(status, Status::Done, "no paid research follows");
        assert!(!d.window.unsupported());
        let owned = s.players().by_seat(B).unwrap().tech.owned;
        assert!(owned.contains(t) && !before.contains(t), "{t:?} was gained");
        assert_eq!(owned.len(), before.len() + 1);
    }

    #[test]
    fn a_wrong_technology_rider_prediction_researches_nothing() {
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::Against, PredictionTag::TechRider));
        let d = run(close_with(researching()), &mut s, &[]);
        assert!(!d.window.unsupported());
    }

    /// Bail: the adapter found a position the research window does not carry (`research: None`), or the table has no link to it.
    #[test]
    fn bail_a_technology_rider_without_the_research_window_is_unsupported() {
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::TechRider));
        assert!(run(close_with(facts()), &mut s, &[]).window.unsupported());
        let mut s = game();
        predict(&mut s, B, Prediction::Tagged(Outcome::For, PredictionTag::TechRider));
        let k = Kernel::plain();
        let mut d = Driver::new(close_with(researching()), Resolver::new(&[A], Some(A)), AbilityTable::default(), 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.unsupported(), "no link to the research window");
    }
}
