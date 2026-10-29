//! `Area::Strategy` (package B8c-1): one step of the follower window of a strategic action as a lockstep window
//! (`Game::secondary_step`: the string `next_choice`, the table's answer and `take_choice`). The core half is
//! `ti4_engine_core::rules::strategy_secondary::SecondaryStep`; what it is told of each follower (may they be asked, what the
//! follow costs, the waiver options) are the string answers (`StrategySecondaryWindow::core_facts`).
//!
//! The core half answers `None` (the string result stands, counted as a bail under `strategy::secondary`) for a
//! preview window or one for a card nobody holds (the Winnu hero), a waiver taken (the module pays in the string state),
//! a card the tables do not know and a nested emission.

use ti4_content::ContentStore;
use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::Ask;
use ti4_engine_core::driver::Driver;
use ti4_engine_core::generated::StrategyCard;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::rules::strategy_secondary::{FollowerFacts, Resolution, SecondaryStep};
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::core_sync::{with_core, Footprint, Sources};
use crate::dual::{CoreWindow, Run};
use crate::dual_window::{apply_staged, drive_with, nested_check, nested_fp, tables, Recording};
use crate::generated::IdStr;
use crate::strategy::{SecondaryResolution, StepOutcome, StrategySecondaryWindow, WAIVED_SECONDARY_PREFIX};

const STEP: Footprint = Footprint {
    reads: SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Marks, Slice::Pending]),
    writes: SliceSet::of(&[Slice::Players, Slice::Marks, Slice::Pending]),
};

/// The core half of one step of `window` (the window as it stood before the string step), on the position before it.
#[must_use]
pub fn secondary_step(
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    window: &StrategySecondaryWindow,
    script: &Recording,
) -> Option<CoreWindow<StepOutcome>> {
    nested_check(script)?;
    let Some(facts) = window.core_facts(gs, content, sources) else {
        crate::dual::note_bail("strategy::secondary", "a preview window or a card nobody holds");
        return None;
    };
    if script.questions.first().and_then(|q| q.answer.as_deref()).is_some_and(|a| a.starts_with(WAIVED_SECONDARY_PREFIX)) {
        crate::dual::note_bail("strategy::secondary", "a waiver taken (the module pays in the string state)");
        return None;
    }
    let card = StrategyCard::from_id(facts.card.as_str())?;
    let (mut cx, r) = tables(gs)?;
    let primary = r.seat(facts.primary.as_str())?;
    let mut followers = Vec::new();
    for f in &facts.followers {
        followers.push(FollowerFacts {
            who: r.seat(f.player.as_str())?,
            eligible: f.eligible,
            costs_token: f.costs_token,
            primary_instead: f.primary_instead,
            waivers: f.waivers.iter().map(|w| r.shape(w)).collect(),
        });
    }
    let step = SecondaryStep::new(primary, card, window.exhausts_card(), facts.total, &followers, r.code("strategy_secondary"));
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = Driver::new(step, Resolver::new(&[], None), AbilityTable::default(), 1);
    let recorded = script.questions.first().map(|q| q.choice.details.clone());
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(STEP, script), |core, _| {
        let driven = drive_with(&mut driver, core, &kernel, &r, &before, script, &|c, core, rendered| {
            // `tokens_left` is the follower's pool in the position the question is asked in; the purchase, pool, hint and
            // commodity facts are the string's display texts, which only the string side computes.
            if let Ask::Strategy(SAsk::Follow { .. }) = c.ask {
                let left = core.players().by_seat(c.actor).map_or(0, |p| i64::from(p.econ.strategic_tokens));
                rendered.details.insert("tokens_left".to_owned(), left.into());
                for (key, value) in recorded.iter().flatten() {
                    rendered.details.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let out = driver.window.outcome();
    let player = |s: ti4_engine_core::ids::Seat| Some(PlayerId::new(r.seats.get(usize::from(s.0))?.clone()));
    let value = StepOutcome {
        skipped: out.skipped.as_slice().iter().map(|s| player(*s)).collect::<Option<_>>()?,
        answered: match out.answered {
            Some((who, how)) => Some((
                player(who)?,
                match how {
                    Resolution::Declined => SecondaryResolution::Declined,
                    Resolution::Followed => SecondaryResolution::Followed,
                },
            )),
            None => None,
        },
        complete: out.complete,
    };
    Some(CoreWindow { run: Run::new(value), questions: driven.rendered })
}
