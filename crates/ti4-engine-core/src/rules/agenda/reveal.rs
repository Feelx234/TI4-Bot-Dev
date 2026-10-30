//! The agenda reveal as a [`Window`] (package B9c-1; string `Game::reveal_agenda`, `agenda_reveal::Live`).
//!
//! One loop: the agenda counter moves, the outcomes are read (Covert Legislation draws a face-down agenda's, which is
//! the embedded [`CovertDraw`]), an agenda that elects nothing is logged, discarded and the next one drawn, any other is
//! announced (`AGENDA_REVEALED` through the A5 resolver, so the cards "when an agenda is revealed" can be played into
//! it) and then offered to Quash (every seat with the ability and a strategy token, in seat order), to Political
//! Favor (its holder, the Xxcha's token pays) and, when something replaced the agenda (those two or a Veto played
//! into the window), logged as replaced and the chain continues from the replacement. The agenda that stays is
//! offered to the holders of Political Secret and the window ends with it and its outcomes.
//!
//! The plain events of the string's `Game::emit` (`AGENDA_REVEALED:{agenda}`, `AGENDA_DISCARDED:{agenda}`, `QUASH_USED`,
//! `POLITICAL_FAVOR_USED`, `POLITICAL_SECRET_USED:{owner}`) are written to the emission queue (`sx.plain`) at the program
//! points of the pushes, and the `AGENDA_REVEALED` event is mirrored when its emission returns (`sx.emit_mirrored`), so the
//! queue holds the lines of `Game::events` in the string's order (package P1).

use super::covert::CovertDraw;
use super::window::KernelEnv;
use super::{is_law, outcomes, Cx};
use crate::choice::agenda::{Ask as AAsk, Opt as AOpt, UseSource};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Opt, Opts, Origin, Pick};
use crate::containers::SmallList;
use crate::generated::{Ability, Agenda, PromissoryNote};
use crate::ids::Seat;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, fx_agenda, Kernel, Next};
use crate::keys::{Prediction, VoteBar};
use crate::state::{Code, CoreState, EventKind, EventRec, Key, NoteKey, Outcome, Payload, PlainArg, PlainKind, PlainRec, Val};
use crate::tables::Scope;

/// `ti4_model::state::RULES_AGENDA_GRANT_REVEAL`.
pub const RULES_AGENDA_GRANT_REVEAL: u32 = 14;
/// `ti4_model::state::RULES_COVERT_LEGISLATION`.
pub const RULES_COVERT_LEGISLATION: u32 = 33;

/// What the window cannot read off the state: the game's source scope, the blanked laws, and the interned texts the
/// string writes (the log's `discard_reason`s are the state's texts; the `reason`s of the strategy token events are
/// the renderer's).
#[derive(Debug, Clone, Copy)]
pub struct Facts {
    pub scope: Scope,
    pub blanked: bool,
    /// The state texts `no_choice` and `replaced` (`AgendaRecord::discard_reason`).
    pub no_choice: Code,
    pub replaced: Code,
    /// The reasons `quash` and `political_favor` of the staged `STRATEGY_TOKEN_SPENT` events.
    pub quash: Code,
    pub favor: Code,
    /// The renderer's text `ps`: the source of Political Secret's question (the generic note has no id of its own).
    pub secret: Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The top of the loop (not a suspension point).
    Top,
    /// Quash's offers.
    Quash,
    /// Political Favor's offer.
    Favor,
    /// Political Secret's offers.
    Secret,
    Done,
}

/// One offer: who is asked, whose card or ability it is and the note it returns (Favor, Secret).
#[derive(Debug, Clone, Copy)]
struct Offer {
    holder: Seat,
    owner: Seat,
    note: Option<NoteKey>,
}

/// The reveal.
#[derive(Debug, Clone)]
pub struct Reveal {
    facts: Facts,
    alias: Agenda,
    stage: Stage,
    /// The offers of the current stage and the position in them; `listed`: they were read at the stage's start.
    offers: SmallList<Offer, 8>,
    at: usize,
    listed: bool,
    /// The agenda that goes to the vote and its outcomes.
    result: Option<(Agenda, SmallList<Outcome, 16>)>,
    choices: SmallList<Outcome, 16>,
    /// The emission failed (the string returns the error).
    failed: bool,
}

impl Reveal {
    #[must_use]
    pub fn new(facts: Facts, initial: Agenda) -> Self {
        Self {
            facts,
            alias: initial,
            stage: Stage::Top,
            offers: SmallList::new(),
            at: 0,
            listed: false,
            result: None,
            choices: SmallList::new(),
            failed: false,
        }
    }

    /// The agenda that goes to the vote and its outcomes; `None` when nothing was left to elect.
    #[must_use]
    pub fn result(&self) -> Option<(Agenda, &[Outcome])> {
        self.result.as_ref().map(|(a, o)| (*a, o.as_slice()))
    }

    /// The emission of `AGENDA_REVEALED` failed (the string returns the error).
    #[must_use]
    pub const fn failed(&self) -> bool {
        self.failed
    }

    fn finish(&mut self) -> Next {
        self.stage = Stage::Done;
        Next::Done
    }

    /// The top of the loop: the counter, the outcomes, the discard of an agenda that elects nothing or the
    /// announcement. Returns `true` when the window is finished.
    fn top(&mut self, sx: &mut Sx<'_>) -> bool {
        loop {
            let meta = sx.state.meta_mut();
            meta.agenda_seq = meta.agenda_seq.saturating_add(1);
            sx.plain(PlainRec::one(PlainKind::AgendaRevealed, PlainArg::Agenda(self.alias)));
            let choices = self.reveal_choices(sx);
            if choices.is_empty() {
                sx.plain(PlainRec::one(PlainKind::AgendaDiscarded, PlainArg::Agenda(self.alias)));
                if !sx.state.meta().rules_at_least(RULES_AGENDA_GRANT_REVEAL) {
                    return true;
                }
                fx_agenda::log_discarded(sx.state, self.alias, is_law(self.alias), self.facts.no_choice);
                let Some(next) = sx.state.decks_mut().agenda.draw() else { return true };
                self.alias = next;
                continue;
            }
            let agenda = sx.state.agenda_mut();
            agenda.choices = choices.as_slice().iter().copied().collect();
            agenda.in_play = Some(self.alias);
            self.choices = choices;
            let rec = EventRec {
                id: 0,
                kind: EventKind::AgendaRevealed,
                payload: Payload::new().with(Key::Agenda, Val::Agenda(self.alias)),
                cancelled: false,
            };
            if sx.emit_mirrored(rec).is_err() {
                self.failed = true;
                return true;
            }
            self.stage = Stage::Quash;
            self.listed = false;
            return false;
        }
    }

    /// `reveal_choices`: Covert Legislation draws its face-down agenda's outcomes, any other agenda reads its own.
    fn reveal_choices(&self, sx: &mut Sx<'_>) -> SmallList<Outcome, 16> {
        if self.alias == Agenda::CovertLegislation && sx.state.meta().rules_at_least(RULES_COVERT_LEGISLATION) {
            let mut draw = CovertDraw::new(self.facts.scope, self.facts.blanked);
            let _ = draw.step(sx);
            return draw.choices().iter().copied().collect();
        }
        let env = KernelEnv { core: sx.state, kernel: sx.kernel };
        outcomes(&Cx { core: sx.state, scope: self.facts.scope, blanked: self.facts.blanked, env: &env }, self.alias)
    }

    /// The offers of the stage `Quash`: each seat with a strategy token and the ability, in seat order.
    fn list_quash(&mut self, s: &CoreState) {
        self.offers = SmallList::new();
        if s.decks().agenda.is_empty() {
            return;
        }
        for p in s.players().iter() {
            if p.econ.strategic_tokens > 0 && p.faction.data().abilities.contains(&Ability::Quash) {
                self.offers.push(Offer { holder: p.seat, owner: p.seat, note: None });
            }
        }
    }

    /// The offers of a note of the table held by someone other than its owner (`Game::lent_notes`), in the text
    /// order of the note ids (the faction names; the map is ordered by the typed faction, whose integers follow the id texts).
    fn list_notes(&mut self, s: &CoreState, note: PromissoryNote) {
        self.offers = SmallList::new();
        let mut lent: SmallList<(NoteKey, Seat, Seat), 8> = SmallList::new();
        for (key, holder) in s.cards().promissory.iter() {
            if key.note != note {
                continue;
            }
            let faction_of = |seat: Seat| s.players().by_seat(seat).map(|p| p.faction);
            if faction_of(*holder) == Some(key.faction) {
                continue;
            }
            // `promissory::seat_of`: the first seat in seating order that plays the faction.
            let owner = s.seats().seating_order().iter().copied().find(|seat| faction_of(*seat) == Some(key.faction));
            let Some(owner) = owner else { continue };
            lent.push((*key, *holder, owner));
        }
        for (key, holder, owner) in lent.as_slice() {
            self.offers.push(Offer { holder: *holder, owner: *owner, note: Some(*key) });
        }
    }

    fn source(&self) -> UseSource {
        match self.stage {
            Stage::Quash => UseSource::Quash,
            Stage::Favor => UseSource::Favor,
            _ => UseSource::Secret,
        }
    }

    /// The ask of the current offer.
    fn ask(&self) -> Ask {
        let o = self.offers.as_slice()[self.at];
        Ask::Agenda(AAsk::UseOnAgenda { source: self.source(), agenda: self.alias, owner: o.owner })
    }

    /// The deck's top card becomes the replacement (`ti4_model::deck_reserve::take_top`).
    fn replace(sx: &mut Sx<'_>) -> bool {
        let Some(next) = sx.state.decks_mut().agenda.draw() else { return false };
        sx.state.agenda_mut().veto_replacement = Some(next);
        true
    }

    /// `note_strategy_token_spent`: the staged `STRATEGY_TOKEN_SPENT`.
    fn spent(sx: &mut Sx<'_>, who: Seat, reason: Code) {
        fx::stage(
            sx.state,
            EventRec {
                id: 0,
                kind: EventKind::StrategyTokenSpent,
                payload: Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Reason, Val::Code(reason)),
                cancelled: false,
            },
        );
    }
}

impl Window for Reveal {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        loop {
            match self.stage {
                Stage::Done => return Next::Done,
                Stage::Top => {
                    if self.top(sx) {
                        return self.finish();
                    }
                }
                Stage::Quash => {
                    if !self.listed {
                        self.listed = true;
                        self.at = 0;
                        if sx.state.agenda().veto_replacement.is_some() {
                            self.offers = SmallList::new();
                        } else {
                            let s: &CoreState = sx.state;
                            self.list_quash(s);
                        }
                    }
                    if self.at < self.offers.len() {
                        return Next::Continue(self.ask());
                    }
                    self.stage = Stage::Favor;
                    self.listed = false;
                }
                Stage::Favor => {
                    if !self.listed {
                        self.listed = true;
                        self.at = 0;
                        if sx.state.agenda().veto_replacement.is_some() {
                            self.offers = SmallList::new();
                        } else {
                            let s: &CoreState = sx.state;
                            self.list_notes(s, PromissoryNote::PoliticalFavor);
                        }
                    }
                    // `offer_political_favor`: a card whose owner cannot pay, or an empty deck, is not offered.
                    while self.at < self.offers.len() {
                        let o = self.offers.as_slice()[self.at];
                        let payable = sx.state.players().by_seat(o.owner).is_some_and(|p| p.econ.strategic_tokens > 0);
                        if payable && !sx.state.decks().agenda.is_empty() {
                            return Next::Continue(self.ask());
                        }
                        self.at += 1;
                    }
                    // Replaced (Quash, Favor, or a Veto played into the window): log it and reveal the replacement.
                    if let Some(replacement) = sx.state.agenda_mut().veto_replacement.take() {
                        sx.plain(PlainRec::one(PlainKind::AgendaDiscarded, PlainArg::Agenda(self.alias)));
                        if sx.state.meta().rules_at_least(RULES_AGENDA_GRANT_REVEAL) {
                            fx_agenda::log_discarded(sx.state, self.alias, is_law(self.alias), self.facts.replaced);
                        }
                        // Covert Legislation replaced: the agenda the speaker drew for it goes with it.
                        sx.state.agenda_mut().covert = None;
                        self.alias = replacement;
                        self.stage = Stage::Top;
                        continue;
                    }
                    self.stage = Stage::Secret;
                    self.listed = false;
                }
                Stage::Secret => {
                    if !self.listed {
                        self.listed = true;
                        self.at = 0;
                        let s: &CoreState = sx.state;
                        self.list_notes(s, PromissoryNote::PoliticalSecret);
                    }
                    // Already barred from voting on this agenda: not offered.
                    while self.at < self.offers.len() {
                        let o = self.offers.as_slice()[self.at];
                        if !sx.state.agenda().predictions.contains_key(&o.owner) {
                            return Next::Continue(self.ask());
                        }
                        self.at += 1;
                    }
                    self.result = Some((self.alias, self.choices.clone()));
                    return self.finish();
                }
            }
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        if !matches!(self.stage, Stage::Quash | Stage::Favor | Stage::Secret) || self.at >= self.offers.len() {
            return None;
        }
        let o = self.offers.as_slice()[self.at];
        let source = self.source();
        let content = match source {
            UseSource::Quash => Content::Ability(Ability::Quash),
            UseSource::Favor => Content::Promissory(PromissoryNote::PoliticalFavor),
            UseSource::Secret => Content::Other(self.facts.secret),
        };
        let mut opts = Opts::default();
        opts.push(Opt::Agenda(AOpt::UseCard));
        opts.push(Opt::Decline);
        Some(super::question(o.holder, AAsk::UseOnAgenda { source, agenda: self.alias, owner: o.owner }, Origin::Content(content), opts))
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if !matches!(self.stage, Stage::Quash | Stage::Favor | Stage::Secret) || self.at >= self.offers.len() {
            return Err(Illegal);
        }
        let o = self.offers.as_slice()[self.at];
        match pick {
            Pick::Opt(Opt::Decline) => self.at += 1,
            Pick::Opt(Opt::Agenda(AOpt::UseCard)) => match self.stage {
                Stage::Quash => {
                    if let Some(p) = sx.state.players_mut().by_seat_mut(o.holder) {
                        p.econ.strategic_tokens -= 1;
                    }
                    Self::spent(sx, o.holder, self.facts.quash);
                    // The offer ends with the first user, with or without a replacement to hand over.
                    if Self::replace(sx) {
                        sx.plain(PlainRec::bare(PlainKind::QuashUsed));
                    }
                    self.at = self.offers.len();
                }
                Stage::Favor => {
                    if let Some(p) = sx.state.players_mut().by_seat_mut(o.owner) {
                        p.econ.strategic_tokens -= 1;
                    }
                    Self::spent(sx, o.owner, self.facts.favor);
                    if Self::replace(sx) {
                        if let Some(note) = o.note {
                            fx::give_back_note(sx.state, note);
                        }
                        sx.plain(PlainRec::bare(PlainKind::PoliticalFavorUsed));
                    }
                    self.at = self.offers.len();
                }
                _ => {
                    sx.state.agenda_mut().predictions.insert(o.owner, Prediction::Barred(VoteBar::PoliticalSecret));
                    if let Some(note) = o.note {
                        fx::give_back_note(sx.state, note);
                    }
                    sx.plain(PlainRec::one(PlainKind::PoliticalSecretUsed, PlainArg::Seat(o.owner)));
                    self.at += 1;
                }
            },
            _ => return Err(Illegal),
        }
        Ok(())
    }
}
