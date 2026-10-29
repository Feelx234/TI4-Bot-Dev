//! The core half of `Game::reveal_agenda` (package B9c-1): the `rules::agenda::reveal::Reveal` window run beside the
//! string chain (`agenda_reveal::Live::reveal_agenda`) in lockstep, on the `AgendaLaws` area.
//!
//! The questions it compares are Quash's, Political Favor's and Political Secret's offers (`UseOnAgenda`); the typed
//! `AGENDA_REVEALED` of each round is emitted through the resolver, with the nested reaction windows followed from the
//! recording (`dual_window::drive`). The plain events of the string (`AGENDA_DISCARDED`, `QUASH_USED`, ...) are not
//! compared. Bails (`None`): a deck reserve (a turn-redo replay draws through it), a nested activity the table does
//! not follow, an agenda the core does not know, a position the codecs refuse (more than 16 owners, generic
//! promissory notes).

use ti4_engine_core::driver::Driver;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::Slice;
use ti4_model::content_types::SourceSet;
use ti4_model::state::GameState;

use crate::core_sync::{Footprint, Sources, with_core};
use crate::dual::{CoreWindow, Run};
use crate::dual_window::{Recording, apply_staged, drive, event_of, nested_check, nested_fp, tables};
use crate::event::Event;
use crate::generated::IdStr;

const LABEL: &str = "game::reveal_agenda";

const REVEAL: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Strategy, Slice::Objectives, Slice::Decks, Slice::Agenda, Slice::Cards, Slice::Pending],
    &[Slice::Meta, Slice::Players, Slice::Decks, Slice::Agenda, Slice::Cards, Slice::Pending],
);

/// What the string chain returns: the agenda to vote on and its outcomes, `None` when nothing was left to elect;
/// the outer `None` when the chain stopped on a question nobody had answered yet or on an error.
pub type Found = Option<Option<(String, Vec<String>)>>;

/// The core half of `Game::reveal_agenda`.
#[must_use]
pub fn reveal(
    gs: &mut GameState,
    _content: &ti4_content::ContentStore,
    sources: SourceSet,
    initial: &str,
    script: &Recording,
) -> Option<CoreWindow<Found>> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::rules::agenda::reveal::{Facts, Reveal};
    if gs.deck_reserve.is_some() {
        crate::dual::note_bail(LABEL, "deck reserve");
        return None;
    }
    let first = Agenda::from_id(initial)?;
    let (mut cx, r) = tables(gs)?;
    nested_check(script)?;
    let facts = Facts {
        scope: crate::core_adapter::scope(sources),
        blanked: crate::laws::blanked(gs),
        no_choice: cx.texts.intern(Slice::Agenda, "no_choice").ok()?,
        replaced: cx.texts.intern(Slice::Agenda, "replaced").ok()?,
        quash: r.code("quash"),
        favor: r.code("political_favor"),
        secret: r.code("ps"),
    };
    let before = gs.clone();
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(Reveal::new(facts, first), Resolver::new(&[], None), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(REVEAL, script), |core, _| {
        let driven = drive(&mut driver, core, kernel, &r, &before, script);
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let found: Found = if driven.finished && !driver.window.failed() {
        Some(match driver.window.result() {
            None => None,
            Some((agenda, outcomes)) => {
                let outcomes = outcomes.iter().map(|o| cx.keys.render_outcome(*o)).collect::<Option<Vec<_>>>()?;
                Some((agenda.as_str().to_owned(), outcomes))
            }
        })
    } else {
        None
    };
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(found).with_events_vec(events), questions: driven.rendered })
}
