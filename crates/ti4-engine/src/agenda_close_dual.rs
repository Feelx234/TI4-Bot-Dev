//! The core half of `Game::close_vote` (package B9c-2): the `rules::agenda::close::Close` window run beside the string
//! close (`agenda_close::Live::close_resolution`) in lockstep, on the `AgendaLaws` area.
//!
//! It compares the questions of the window (Leadership Rider's pool questions and the reaction cards played into
//! `AGENDA_RESOLVED`: Deadly Plot, Confusing and Confounding Legal Text), the typed `AGENDA_RESOLVED`, the position
//! (log, laws, predictions, riders paid, redirect) and what the close hands on: whether the agenda was discarded, the
//! effective outcome and the elected seat. The agenda's effect is a window of its own and runs after this one.
//! Bails (`None`): a deck reserve, diplomacy promises (a vote is an event of the promises), a nested activity the table
//! does not follow, an agenda or outcome the core has no typed form of, a position the codecs refuse, and a correct
//! prediction the core does not pay (the research and placement riders, a draw a faction hook changes or that leaves
//! the hand over its limit, a rules version below the pool question).

use ti4_engine_core::driver::Driver;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::Slice;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::agenda_close::Settled;
use crate::core_sync::{Footprint, Sources, with_core};
use crate::dual::{CoreWindow, Run};
use crate::dual_window::{Recording, apply_staged, drive_with, event_of, nested_check, nested_fp, tables};
use crate::event::Event;
use crate::generated::IdStr;
use crate::vote::VoteWindow;

const LABEL: &str = "game::close_vote";

const CLOSE: Footprint = Footprint::new(
    &[
        Slice::Meta,
        Slice::Seats,
        Slice::Players,
        Slice::Board,
        Slice::Strategy,
        Slice::Objectives,
        Slice::Decks,
        Slice::Agenda,
        Slice::Cards,
        Slice::Transient,
        Slice::Ledger,
        Slice::Pending,
    ],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Objectives, Slice::Decks, Slice::Agenda, Slice::Cards, Slice::Transient, Slice::Ledger, Slice::Pending],
);

/// The core half of `Game::close_vote`.
#[must_use]
pub fn close(
    gs: &mut GameState,
    content: &ti4_content::ContentStore,
    _sources: SourceSet,
    window: &VoteWindow,
    covert: Option<&str>,
    script: &Recording,
) -> Option<CoreWindow<Settled>> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::rules::agenda::close::{Close, Facts};
    if gs.deck_reserve.is_some() {
        crate::dual::note_bail(LABEL, "deck reserve");
        return None;
    }
    if gs.diplomacy.enabled {
        crate::dual::note_bail(LABEL, "diplomacy promises");
        return None;
    }
    let vote = Agenda::from_id(window.alias())?;
    let hidden = match covert {
        Some(id) => Some(Agenda::from_id(id)?),
        None => None,
    };
    let (mut cx, r) = tables(gs)?;
    nested_check(script)?;
    // The outcomes are the face-down agenda's under Covert Legislation.
    let elects = hidden.unwrap_or(vote).data().elects;
    let parse = |text: &str| cx.keys.parse_outcome(text, Some(elects));
    let winner = match window.winner() {
        Some(text) => Some(parse(text)?),
        None => None,
    };
    let ballot = window
        .ballot()
        .votes
        .iter()
        .map(|(player, outcome)| Some((r.seat(player.as_str())?, parse(outcome)?)))
        .collect::<Option<Vec<_>>>()?;
    let counts = window.ballot().counts.iter().map(|(outcome, votes)| Some((parse(outcome)?, *votes))).collect::<Option<Vec<_>>>()?;
    let choices = window.choices().iter().map(|outcome| parse(outcome)).collect::<Option<Vec<_>>>()?;
    // A command token in a Mahact fleet pool changes the reinforcements the pool questions show.
    if gs.agenda_predictions.values().any(|p| p.ends_with("|lead_rider")) && gs.players.iter().any(|p| gs.tokens_held_by_mahact(&p.id) > 0) {
        crate::dual::note_bail(LABEL, "a Mahact fleet token under Leadership Rider");
        return None;
    }
    let yssaril = gs.players.iter().any(|p| p.faction.as_str() == "yssaril");
    let mut hooked: u16 = 0;
    for (at, id) in r.seats.iter().enumerate().take(16) {
        let player = PlayerId::new(id.as_str());
        let bonus = [1, 3].iter().any(|n| crate::factions::hooks_economy::action_card_draw_bonus(gs, content, &player, *n) != 0);
        if yssaril || bonus {
            hooked |= 1 << at;
        }
    }
    let facts = Facts {
        trade_rider: r.code("trade_rider"),
        keleres_rider: r.code("keleres_rider"),
        drop_unofficial: gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK,
        blanked: crate::laws::blanked(gs),
        hooked,
    };
    let before = gs.clone();
    let kernel = crate::production_view::kernel();
    let mut driver = Driver::new(
        Close::new(facts, vote, hidden, winner, &ballot, &counts, &choices),
        Resolver::new(&[], None),
        AbilityTable::default(),
        1,
    );
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(CLOSE, script), |core, _| {
        let driven = drive_with(&mut driver, core, kernel, &r, &before, script, &|c, state, rendered| {
            let letnev = before.player(&PlayerId::new(r.seat_text(c.actor))).is_some_and(|p| p.faction.as_str() == "letnev");
            crate::strategy_cards_core::complete_tokens(c, state, rendered, letnev);
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    })
    .ok()?
    .ok()?;
    if driver.window.unsupported() {
        crate::dual::note_bail(LABEL, "a rider or draw the core does not pay");
        return None;
    }
    if !driven.finished || driver.window.failed() {
        return None;
    }
    apply_staged(gs, &r, &staged)?;
    let settled = if winner.is_some() {
        Settled {
            resolved: true,
            discarded: driver.window.discarded(),
            effective: match driver.window.effective() {
                Some(outcome) => Some(cx.keys.render_outcome(outcome)?),
                None => None,
            },
            elected: driver.window.elected().map(|seat| r.seat_text(seat).to_owned()),
        }
    } else {
        Settled::default()
    };
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new(settled).with_events_vec(events), questions: driven.rendered })
}
