//! The window dual of `vote::VoteWindow` (package B9; docs/ENGINE_CORE_DRIVER.md 3).
//!
//! The string vote is driven one answer at a time by the game, so there is no closure to run twice. On
//! `agenda=dual` the window keeps a [`Log`]: the position it opened in and every question it was asked with
//! the option id it was answered with. When the vote finishes, the core [`Vote`] window is run from the
//! opening position through the D1 driver, answered with the same ids, and compared: every question
//! (`compare_choice`: ids, kinds, labels, payload, context, decision hashes), the winner and the position
//! it leaves (the planets exhausted).
//!
//! The core does not express the Genetic Recombination, trade goods and Predictive Intelligence stages, the
//! Deepwrought oceans, the faction vote bonuses, the Executive Order spender or a vote on more than eight
//! outcomes: for such a position [`Log::open`] records nothing and the string result stands.

use ti4_engine_core::containers::SeatSet;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::Choice;

/// What the window saw, if the dual is on and the position is expressible.
#[derive(Clone, Default)]
pub struct Log(Option<Box<Inner>>);

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "Log(recording)" } else { "Log(off)" })
    }
}

/// The log is bookkeeping of the dual, not part of the window's identity.
impl PartialEq for Log {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Log {}

#[derive(Clone)]
struct Inner {
    /// The position at the first answer (cards played between the reveal and the first vote count).
    open: Option<GameState>,
    /// What each voter's extra votes were then: a change later in the vote (a card played between voters) is
    /// not replayed.
    extras: Vec<i64>,
    unstable: bool,
    alias: String,
    choices: Vec<String>,
    order: Vec<PlayerId>,
    sources: Option<SourceSet>,
    script: Vec<(Choice, String)>,
}

impl Log {
    /// Start recording a vote on `alias` (only on `agenda=dual`, and only for a position the core holds).
    #[must_use]
    pub fn open(state: &GameState, alias: &str, choices: &[String], order: &[PlayerId]) -> Self {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::AgendaLaws) == crate::dual::Backend::Dual {
                return Self(Some(Box::new(Inner {
                    open: None,
                    extras: Vec::new(),
                    unstable: false,
                    alias: alias.to_owned(),
                    choices: choices.to_vec(),
                    order: order.to_vec(),
                    sources: None,
                    script: Vec::new(),
                })));
            }
        }
        let _ = (state, alias, choices, order);
        Self(None)
    }

    /// The window was changed in a way the dual does not follow (the Executive Order spender).
    pub fn stop(&mut self) {
        self.0 = None;
    }

    /// One question was answered.
    pub fn note(&mut self, state: &GameState, sources: SourceSet, asked: &Choice, answer: &str) {
        let Some(inner) = self.0.as_mut() else { return };
        let extras: Vec<i64> = inner.order.iter().map(|p| crate::vote::extra_votes_text(state, p)).collect();
        if inner.open.is_none() {
            #[cfg(feature = "typed-core")]
            if !supported(state, &inner.choices, &inner.order) {
                self.0 = None;
                return;
            }
            inner.open = Some(state.clone());
            inner.extras = extras;
        } else if inner.extras != extras {
            inner.unstable = true;
        }
        inner.sources = Some(sources);
        inner.script.push((asked.clone(), answer.to_owned()));
    }

    /// The vote finished with `winner`; `state` is the position it left. Compares and stops recording.
    pub fn finish(&mut self, state: &GameState, winner: Option<&str>) {
        let Some(inner) = self.0.take() else { return };
        #[cfg(feature = "typed-core")]
        if !inner.unstable {
            replay(&inner, state, winner);
        }
        #[cfg(not(feature = "typed-core"))]
        let _ = (inner, state, winner);
    }
}

#[cfg(feature = "typed-core")]
fn supported(state: &GameState, choices: &[String], order: &[PlayerId]) -> bool {
    use std::collections::BTreeMap;
    if choices.len() > ti4_engine_core::choice::agenda::Tally::MAX {
        return false;
    }
    order.iter().all(|p| {
        crate::factions::mahact::recombination_holder(state, p).is_none()
            && crate::factions::deepwrought::oceans(state, p).is_empty()
            && !crate::promissory::has_commander_ability(state, p, "hacancommander")
            && !crate::vote::predictive_ready(state, p)
            && crate::leaders::vote_bonus(state, p) == 0
            && crate::factions::hooks_cards::vote_bonus_with_content(state, ti4_content::ContentStore::embedded(), p) == 0
            && choices.iter().all(|c| {
                // Blood Pact: as if every other seat had already backed `c`.
                let votes: BTreeMap<PlayerId, String> = order.iter().filter(|o| *o != p).map(|o| (o.clone(), c.clone())).collect();
                crate::factions::empyrean_units::blood_pact_votes(state, &votes, p, c) == 0
            })
    })
}

#[cfg(feature = "typed-core")]
fn replay(inner: &Inner, now: &GameState, winner: Option<&str>) {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::kernel::stage::{AbilityTable, Window};
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::agenda::window::{Facts, Vote};
    use ti4_engine_core::state::Slice;

    use crate::core_sync::{Ctx, Footprint, Sources, with_core};
    use crate::dual::{Area, Run, compare_choice, compare_runs, compare_value};
    use crate::dual_window::{Asked, Recording, drive};
    use crate::generated::IdStr;
    use crate::render::Render;

    const LABEL: &str = "vote::VoteWindow";
    const FOOTPRINT: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Pending],
        &[Slice::Board],
    );
    let (Some(sources), Some(agenda)) = (inner.sources, Agenda::from_id(&inner.alias)) else { return };
    let Some(open) = inner.open.as_ref() else { return };
    let Ok(mut cx) = Ctx::of(open) else { return };
    let r = Render::new(cx.owners.ids().to_vec());
    let voted = match open.covert_agenda.as_deref() {
        Some(hidden) if inner.alias == "covert" => Agenda::from_id(hidden),
        _ => Some(agenda),
    };
    let Some(elects) = voted.map(|a| a.data().elects) else { return };
    let Some(choices) = inner.choices.iter().map(|c| cx.keys.parse_outcome(c, Some(elects))).collect::<Option<Vec<_>>>() else { return };
    let Some(order) = inner.order.iter().map(|p| cx.owners.seat(p.as_str())).collect::<Option<Vec<_>>>() else { return };
    let mut qanoj = SeatSet::EMPTY;
    for (p, seat) in inner.order.iter().zip(&order) {
        if crate::leaders::elder_qanoj(open, p) {
            qanoj.insert(*seat);
        }
    }
    let facts = Facts { scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(open), qanoj };
    let window = Vote::new(agenda, &choices, &order, facts);
    let Some(speaker) = cx.owners.seat(open.speaker.as_str()) else { return };
    let mut driver = ti4_engine_core::driver::Driver::new(window, Resolver::new(&order, Some(speaker)), AbilityTable::default(), 1);
    let mut script = Recording::default();
    script.questions = inner.script.iter().map(|(c, a)| Asked { choice: c.clone(), answer: Some(a.clone()) }).collect();
    let kernel = Kernel::plain();
    let mut core_state = open.clone();
    let driven = with_core(&mut core_state, &mut cx, Sources::default(), FOOTPRINT, |core, _| drive(&mut driver, core, &kernel, &r, open, &script));
    let Ok(Ok(driven)) = driven else { return };
    let area = Area::AgendaLaws;
    if compare_value(area, &format!("{LABEL}.questions"), &inner.script.len(), &driven.rendered.len()) {
        for ((asked, _), rendered) in inner.script.iter().zip(&driven.rendered) {
            compare_choice(area, LABEL, Some(asked), Some(rendered));
        }
    }
    let core_winner = driver.window.winner().and_then(|o| cx.keys.render_outcome(o));
    let winner = winner.map(ToOwned::to_owned);
    let mut string_view = open.clone();
    string_view.exhausted_planets = now.exhausted_planets.clone();
    compare_runs(area, LABEL, &string_view, &Run::new(winner), &core_state, &Run::new(core_winner));
    compare_value(area, &format!("{LABEL}.finished"), &true, &(driven.finished && driver.window.is_done()));
    let _ = <Vote as Window>::stage;
}
