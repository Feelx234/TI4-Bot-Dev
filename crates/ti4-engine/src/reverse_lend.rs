//! The reverse lend (docs/todo/D2-prereqs.md D2-5 items 3, 4, 7, 9; docs/ENGINE_CORE_STATE.md 3.2): a leftover string
//! body runs on a `GameState` materialised from the `CoreState`, and what it wrote is loaded back.
//!
//! This is the free-function core of `Game::with_string`; the `Game` wrapper that owns the `CoreState` is D2-8. Everything
//! the body can reach is rebuilt for the occasion and handed back as a delta:
//!
//! | what | in | out |
//! |---|---|---|
//! | `GameState` | `materialize(core, cx, base)` (whole state) | `Codec::load_slice` for each slice of `fp.writes` |
//! | `GameRng` | seed + `rng_positions(core)` | positions -> `Rng` slice (when it is in `fp.writes`) |
//! | timing `Resolver` | `resolver_sync::rebuild_for_lend` from `pending.resolver` | `to_state(export(..))` -> `pending.resolver` (when `Pending` is in `fp.writes`) |
//! | `Render` codes (ability ids, event names) | `core.texts.codes` | the extended table, adopted before the slices load |
//! | dice, `Game::events` pushes, resolver trace | empty | returned as text/rolls in [`LentOut`] (P1 owns their typed form; nothing here invents a second event model) |
//! | questions | a [`StringTape`] as the table's default decider | a dry tape rolls everything back and returns [`Lent::Asking`] |
//!
//! **Correctness does not depend on the footprint**: the body sees the whole materialised state; the footprint only
//! limits what is loaded back, and a body that changed anything outside it (the audit, D2-5 item 7) is refused with an
//! `Unmapped` and nothing is stored. The body must read nothing outside `(state, window struct, tape)`: the host-only
//! side channels of `Game` (`rng_force`, the aftermath and strategic observers, `blocked`) are not reachable from a
//! [`StringCtx`].

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ti4_content::galaxy::Galaxy;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::state::GameState;

use crate::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Table};
use crate::core_sync::codec::{Codec, Sources};
use crate::core_sync::{Ctx, Footprint, R, bad, materialize};
use crate::dice::{Dice, Roll};
use crate::event::EventSequence;
use crate::resolver_sync;
use crate::rng::GameRng;
use crate::timing::Resolver;

/// What a lent string body can reach.
pub struct StringCtx<'a> {
    /// The whole materialised state; the footprint decides what is loaded back.
    pub state: &'a mut GameState,
    /// The string galaxy (`Map` stores nothing in the typed state), when the caller has one.
    pub galaxy: Option<&'a Galaxy>,
    /// The timing resolver, rebuilt for the occasion; its table answers from the tape.
    pub timing: Resolver,
    pub rng: GameRng,
    pub dice: Dice,
    pub seq: EventSequence,
    /// The pushes the body made to `Game::events`, as text.
    pub events: Vec<String>,
}

/// What a finished lent body leaves besides the slices it wrote.
#[derive(Debug)]
pub struct LentOut<T> {
    pub value: T,
    /// The resolver's trace lines.
    pub log: Vec<String>,
    /// The body's pushes to `Game::events`, as text, for the events view (P1).
    pub events: Vec<String>,
    /// Every die the body rolled (the ledger's dice rows are the typed form, P1).
    pub rolls: Vec<Roll>,
}

/// The result of a lent body.
#[derive(Debug)]
pub enum Lent<T> {
    Done(LentOut<T>),
    /// The body asked and the tape was dry: nothing was changed; put the question, append the answer to the tape and
    /// run the body again.
    Asking(Box<Choice>),
}

/// Answers a lent body's questions from a tape of option ids; the first question past the end is kept and refused.
pub struct StringTape {
    answers: VecDeque<String>,
    dry: Arc<Mutex<Option<Choice>>>,
}

impl StringTape {
    #[must_use]
    pub fn new(answers: &[String]) -> (Self, Arc<Mutex<Option<Choice>>>) {
        let dry = Arc::new(Mutex::new(None));
        (Self { answers: answers.iter().cloned().collect(), dry: Arc::clone(&dry) }, dry)
    }
}

impl Decider for StringTape {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        let Some(wanted) = self.answers.pop_front() else {
            *self.dry.lock().expect("tape lock") = Some(choice.clone());
            return Err(IllegalChoice::DeciderFailed {
                player: choice.player.clone(),
                prompt: choice.prompt.clone(),
                reason: "the tape of the lent body is dry".to_owned(),
            });
        };
        choice.answer_for(&wanted).ok_or_else(|| IllegalChoice::ScriptDiverged {
            player: choice.player.clone(),
            wanted,
            offered: choice.options.iter().map(|o| o.id.clone()).collect(),
        })
    }
}

/// Run `f` on the string state of `core` and load the slices of `fp.writes` back.
///
/// `base` is the last materialisation (or the state the game started from); `tape` the answers given to the body's
/// questions so far; `label` names the lent body in errors.
///
/// # Errors
/// A value the typed state cannot hold, an owner outside the game's table, a body that wrote outside `fp` (the audit)
/// or asked a question the resolver cannot take back. `core` and `cx` are unchanged on every error and on
/// [`Lent::Asking`].
pub fn with_string<T>(
    core: &mut CoreState,
    cx: &mut Ctx,
    base: &GameState,
    galaxy: Option<&Galaxy>,
    fp: Footprint,
    label: &'static str,
    tape: &[String],
    f: impl FnOnce(&mut StringCtx<'_>) -> T,
) -> R<Lent<T>> {
    cx.refresh(base)?;
    let before = materialize::materialize(core, cx, base)?;
    let mut gs = before.clone();
    let positions = materialize::rng_positions(core)?;
    let mut rng = GameRng::new(core.rng().seed());
    rng.set_positions(&positions);

    let carried = if core.loaded.has(Slice::Pending) { core.pending().resolver.clone() } else { Default::default() };
    let render = resolver_sync::render_for(&gs).with_codes(core.texts.codes.clone());
    let (tape_decider, dry) = StringTape::new(tape);
    let (timing, seq) = resolver_sync::rebuild_for_lend(&gs, &carried, Table::with_default(Box::new(tape_decider)), &render);

    let mut lent = StringCtx { state: &mut gs, galaxy, timing, rng, dice: Dice::new(), seq, events: Vec::new() };
    let value = f(&mut lent);
    let StringCtx { timing, rng, dice, seq, events, .. } = lent;

    if let Some(choice) = dry.lock().expect("tape lock").take() {
        return Ok(Lent::Asking(Box::new(choice)));
    }
    // What the body changed outside the state: the streams and the resolver.
    let after_positions = rng.positions();
    let mut after = resolver_sync::to_state(&resolver_sync::export(&timing, &seq, &render)?)?;
    // The initiative order never changes in a window; a carried state without one keeps none.
    after.initiative = carried.initiative.clone();
    let mut outside = materialize::written_outside(&before, &gs, fp.writes);
    if after_positions != positions && !fp.writes.has(Slice::Rng) {
        outside.push((Slice::Rng, "stream positions".to_owned()));
    }
    if (after != carried) && !fp.writes.has(Slice::Pending) {
        outside.push((Slice::Pending, "resolver".to_owned()));
    }
    if let Some((slice, field)) = outside.first() {
        return Err(bad(*slice, "lent body wrote outside its footprint", format!("{label}: {field}")));
    }

    // Commit on a copy: a refused slice leaves `core` as it was.
    let mut next = core.clone();
    next.texts.codes = render.codes_snapshot();
    let kept = cx.resolver.take();
    cx.resolver = fp.writes.has(Slice::Pending).then_some(after);
    let committed = (|| -> R<()> {
        cx.refresh(&gs)?;
        let src = Sources { galaxy, rng: Some(&after_positions) };
        for slice in Slice::ALL {
            if fp.writes.has(slice) {
                Codec::load_slice(cx, &gs, src, slice, &mut next)?;
            }
        }
        Ok(())
    })();
    cx.resolver = kept;
    committed?;
    *core = next;
    let rolls = dice.history().to_vec();
    Ok(Lent::Done(LentOut { value, log: timing.log().to_vec(), events, rolls }))
}

#[cfg(test)]
mod tests {
    use ti4_engine_core::state::SliceSet;
    use ti4_model::id::{FactionId, PlayerId};

    use super::*;
    use crate::core_sync::codec::Codec;

    /// The seated fixture of the `core_sync` tests: real factions, no promissory notes.
    fn game() -> GameState {
        let mut gs = crate::fixtures::game(&["a", "b", "c", "d"]);
        for (i, p) in gs.players.iter_mut().enumerate() {
            p.faction = FactionId::new(crate::generated::IdStr::as_str(ti4_engine_core::generated::Faction::ALL[i]));
        }
        gs.promissory_notes.clear();
        gs.promissory_faceup.clear();
        gs
    }

    const NO_MAP: SliceSet = SliceSet(SliceSet::ALL.0 & !(1 << Slice::Map as u32));

    fn loaded(gs: &GameState) -> (CoreState, Ctx) {
        let mut cx = Ctx::of(gs).unwrap();
        let core = Codec::load(&mut cx, gs, Sources::default(), NO_MAP).unwrap();
        (core, cx)
    }

    fn json(gs: &GameState) -> serde_json::Value {
        serde_json::to_value(gs).unwrap()
    }

    const PLAYERS: Footprint = Footprint::new(&[Slice::Players], &[Slice::Players]);

    #[test]
    fn a_state_materialises_to_the_state_it_was_loaded_from() {
        let gs = game();
        let (core, cx) = loaded(&gs);
        let mut elsewhere = gs.clone();
        elsewhere.players[0].trade_goods = 77;
        elsewhere.round = 9;
        let back = materialize::materialize(&core, &cx, &elsewhere).unwrap();
        assert_eq!(json(&back), json(&gs), "every loaded slice is stored over the base");
    }

    #[test]
    fn a_lent_body_runs_on_the_whole_state_and_only_its_slices_come_back() {
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let before = core.clone();
        let out = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "test", &[], |lent| {
            assert_eq!(lent.state.round, gs.round, "the body sees the whole state");
            lent.state.players[1].trade_goods = 9;
            lent.state.players.len()
        })
        .unwrap();
        let Lent::Done(out) = out else { panic!("the body asked nothing") };
        assert_eq!(out.value, 4);
        assert_eq!(core.players().get(1).econ.trade_goods, 9);
        let mut expect = before;
        expect.players_mut().edit(1).econ.trade_goods = 9;
        assert_eq!(core, expect, "no other slice, table or counter moved");
    }

    #[test]
    fn a_body_that_writes_outside_its_footprint_is_refused_and_nothing_is_stored() {
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let before = core.clone();
        let e = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "sneaky", &[], |lent| {
            lent.state.players[1].trade_goods = 9;
            lent.state.round += 1;
        })
        .unwrap_err();
        assert_eq!((e.slice, e.what), (Slice::Meta, "lent body wrote outside its footprint"));
        assert!(e.text.starts_with("sneaky: round"), "{e}");
        assert_eq!(core, before);
    }

    #[test]
    fn the_streams_are_a_write_too() {
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let before = core.clone();
        let draw = |lent: &mut StringCtx<'_>| lent.rng.die(crate::rng::domain::DICE, 10);
        let e = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "dice", &[], draw).unwrap_err();
        assert_eq!(e.slice, Slice::Rng);
        assert_eq!(core, before);
        let fp = Footprint::new(&[Slice::Rng], &[Slice::Rng]);
        let Lent::Done(out) = with_string(&mut core, &mut cx, &gs, None, fp, "dice", &[], draw).unwrap() else { panic!() };
        let positions = materialize::rng_positions(&core).unwrap();
        assert_eq!(positions.get("dice").copied().unwrap_or(0) > 0, true, "the stream moved in the state");
        // the same draw from the same start is the same die: the body drew from the state's own stream
        let (mut again, mut cx2) = loaded(&gs);
        let Lent::Done(twice) = with_string(&mut again, &mut cx2, &gs, None, fp, "dice", &[], draw).unwrap() else { panic!() };
        assert_eq!(out.value, twice.value);
        assert_eq!(materialize::rng_positions(&again).unwrap(), positions);
    }

    #[test]
    fn the_resolver_a_body_spent_keys_in_comes_back_in_the_state() {
        use std::sync::Arc;

        use crate::timing::{Ability, Frequency, Relation};
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let fp = Footprint::new(&[Slice::Pending], &[Slice::Pending]);
        let spend = |lent: &mut StringCtx<'_>| {
            let owner = lent.state.players[0].id.clone();
            lent.timing.register([Ability::new("ability:t:once", owner, "E", Relation::When, Arc::new(|_, _| Ok(()))).with_frequency(Frequency::OncePerRound)]);
            let event = lent.seq.next("E", std::collections::BTreeMap::new()).unwrap();
            lent.timing.emit(event, |_| {}).unwrap();
        };
        let Lent::Done(out) = with_string(&mut core, &mut cx, &gs, None, fp, "once", &[], spend).unwrap() else { panic!() };
        assert!(!out.log.is_empty(), "the trace comes back as text");
        let carried = core.pending().resolver.clone();
        assert_eq!(carried.used.len(), 1, "{carried:?}");
        assert_eq!(carried.next_id, 2, "the event counter continues");
        assert!(core.texts.codes.find(b"ability:t:once").is_some(), "the ability id is a code of the state's own table");
        // a second lend starts from it: the key is still spent, so the same ability does not fire again
        let again = |lent: &mut StringCtx<'_>| lent.timing.used_keys().len();
        let Lent::Done(seen) = with_string(&mut core, &mut cx, &gs, None, fp, "seen", &[], again).unwrap() else { panic!() };
        assert_eq!(seen.value, 1);
        // outside its footprint it is refused
        let e = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "once", &[], |lent| {
            lent.timing.spend_key("ability:t:other".into(), crate::timing::FrequencyScope::Round(1));
        })
        .unwrap_err();
        assert_eq!(e.slice, Slice::Pending);
    }

    fn pick_one() -> Choice {
        Choice::new(PlayerId::new("a"), "pick one", vec![ChoiceOption::new("x", "test"), ChoiceOption::new("y", "test")])
    }

    #[test]
    fn a_question_past_the_tape_rolls_everything_back_and_comes_out_as_the_question() {
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let before = core.clone();
        let body = |lent: &mut StringCtx<'_>| {
            lent.state.players[1].trade_goods = 9;
            lent.timing.table_mut().ask(&pick_one()).map(|o| o.id)
        };
        let Lent::Asking(q) = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "asks", &[], body).unwrap() else { panic!("a dry tape asks") };
        assert_eq!(q.prompt, "pick one");
        assert_eq!(core, before, "the write before the question was rolled back");
        // the answer is appended to the tape and the body runs again
        let Lent::Done(out) = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "asks", &["y".to_owned()], body).unwrap() else { panic!() };
        assert_eq!(out.value.ok().as_deref(), Some("y"));
        assert_eq!(core.players().get(1).econ.trade_goods, 9);
    }

    #[test]
    fn a_body_that_adds_an_owner_is_refused() {
        let gs = game();
        let (mut core, mut cx) = loaded(&gs);
        let before = core.clone();
        let e = with_string(&mut core, &mut cx, &gs, None, PLAYERS, "stranger", &[], |lent| {
            lent.state.players[0].duress_by = Some(PlayerId::new("stranger"));
        })
        .unwrap_err();
        assert!(e.slice == Slice::Seats || e.what.contains("owner") || e.what.contains("outside"), "{e}");
        assert_eq!(core, before);
    }
}
