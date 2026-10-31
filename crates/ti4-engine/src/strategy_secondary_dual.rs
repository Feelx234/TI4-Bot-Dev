//! `Area::Strategy` (package B8c-1): one step of the follower window of a strategic action as a lockstep window
//! (`Game::secondary_step`: the string `next_choice`, the table's answer and `take_choice`). The core half is
//! `ti4_engine_core::rules::strategy_secondary::SecondaryStep`; what it is told of each follower (may they be asked, what the
//! follow costs, the waiver options) are the string answers (`StrategySecondaryWindow::core_facts`).
//!
//! The core half answers `None` (the string result stands, counted as a bail under `strategy::secondary`) for a
//! preview window or one for a card nobody holds (the Winnu hero), a waiver whose payment the core tables do not carry
//! (an unknown module's, Jae Mir Kan while the Nomad watches an agent exhaust), a card the tables do not know and a
//! nested emission. A waiver taken (Acquiescence, Jae Mir Kan, Mindsieve, Enervate) is paid by the core step.

use ti4_content::ContentStore;
use ti4_engine_core::choice::strategy::Ask as SAsk;
use ti4_engine_core::choice::Ask;
use ti4_engine_core::generated::StrategyCard;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::rules::strategy_secondary::{FollowerFacts, Resolution, SecondaryStep, Waiver, WaiverEffect};
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
    reads: SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards, Slice::Marks, Slice::Pending]),
    writes: SliceSet::of(&[Slice::Players, Slice::Board, Slice::Cards, Slice::Marks, Slice::Pending]),
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
            waivers: f.waivers.iter().map(|w| Waiver { code: r.shape(w), effect: waiver_effect(gs, &w.id) }).collect(),
        });
    }
    // A waiver taken whose payment the tables do not carry: the string pays it.
    let taken = script.questions.first().and_then(|q| q.answer.as_deref());
    if let Some(answer) = taken.filter(|a| a.starts_with(WAIVED_SECONDARY_PREFIX)) {
        let unported = facts.followers.iter().zip(&followers).any(|(f, c)| {
            f.waivers.iter().zip(&c.waivers).any(|(option, w)| option.id == answer && w.effect == WaiverEffect::Unported)
        });
        if unported {
            crate::dual::note_bail("strategy::secondary", "a waiver taken whose payment the core tables do not carry");
            return None;
        }
    }
    let step = SecondaryStep::new(primary, card, window.exhausts_card(), facts.total, &followers, r.code("strategy_secondary"));
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut driver = crate::window_driver::driver_for(step, Resolver::new(&[], None), AbilityTable::default(), 1);
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

/// What taking the waiver `option` (`follow|waived|<index>|<waiver id>`) of `follower` on `primary`'s card costs, as the
/// core step pays it (`hooks_strategy::secondary_waived` of the modules that offer waivers).
fn waiver_effect(gs: &GameState, option: &str) -> WaiverEffect {
    use crate::core_adapter::typed;
    use ti4_engine_core::generated::{Leader, System};
    let Some(id) = option.strip_prefix(WAIVED_SECONDARY_PREFIX).and_then(|rest| rest.split_once('|')).map(|(_, id)| id) else {
        return WaiverEffect::Unported;
    };
    if id == crate::factions::firmament_plots::ENERVATE {
        WaiverEffect::Free
    } else if id == "acq" {
        crate::trade_write::note_key(crate::factions::winnu::ACQUIESCENCE).map_or(WaiverEffect::Unported, WaiverEffect::GiveBack)
    } else if let Some(note) = id.strip_prefix(crate::factions::naalu::MINDSIEVE_PREFIX) {
        crate::trade_write::note_key(note).map_or(WaiverEffect::Unported, WaiverEffect::HandOver)
    } else if let Some(system) = id.strip_prefix(crate::factions::mahact::AGENT_WAIVER_PREFIX) {
        // Exhausting an agent is heard by the Nomad's suite (a staged event the core does not make).
        if crate::factions::nomad::watches_agent_exhaustion(gs) {
            return WaiverEffect::Unported;
        }
        match (typed::<Leader>(crate::factions::mahact::AGENT), typed::<System>(system)) {
            (Some(leader), Some(system)) => WaiverEffect::Agent { leader, system },
            _ => WaiverEffect::Unported,
        }
    } else {
        WaiverEffect::Unported
    }
}
