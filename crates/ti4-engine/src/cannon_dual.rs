//! The core half of the space cannon offense of the aftermath prelude (package D1b2, A2b-10b): the adapter of
//! `rules::combat::space_cannon_offense`. The string site is `AftermathWindow::new`'s prelude (`game.rs`
//! `cannon_prelude`), wrapped in `dual_window!(Area::Driver, "combat::space_cannon_offense", ..)` on `driver=dual`.
//!
//! What stays with the string (bails, each counted under the label with its reason): irreproducible dice, a seat of a faction
//! dropped by decision, seats not in id order (the string iterates its gunners by player id), a Jol-Nar commander or a Heart of
//! Ixth (their reroll windows ask), a Letnev commander that pays on a sustain without being unlocked, nested activity the reaction table
//! does not follow. (A game below the rules version of the faction effects is carried since W12: the sustain offer of the absorbing window
//! pays the commander unasked there. The neutral force is carried since W11: the core holds it as an owner without a seat.)

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::choice::combat::Ask as CAsk;
use ti4_engine_core::generated::Unit as CUnit;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::rules::combat::space_cannon_offense::{Facts, SpaceCannonOffense};
use ti4_engine_core::state::Slice;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use crate::core_sync::{Footprint, Sources, with_core};
use crate::dual::{CoreWindow, Run, note_bail};
use crate::dual_window::{DiceSeeds, Recording, apply_staged, drive_windowed, event_of, nested_check, nested_fp, tables, tables_of_sides};
use crate::event::Event;
use crate::generated::IdStr;

/// The label of the window in the dual report.
pub const LABEL: &str = "combat::space_cannon_offense";

/// What the string prelude was called with.
#[derive(Debug, Clone, Copy)]
pub struct Args<'a> {
    pub active: &'a PlayerId,
    pub system: &'a SystemId,
    /// The resolver carries the timing machinery (`ctx.emit` announces).
    pub announce: bool,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
}

/// What the offense leaves besides the state: the feat occurrence of the event-scoring pause, the dice of the window (hit
/// number, faces) and where the streams stand.
pub type Out = (Option<u64>, Vec<(u32, Vec<u32>)>, crate::rng::RngPositions);

const OFFENSE: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// With a map lent (the guns that reach in from adjacent systems).
const OFFENSE_MAPPED: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// Why the position holds something the core window does not carry (`None`: it is carried).
#[must_use]
pub fn unported(gs: &GameState, a: &Args<'_>) -> Option<&'static str> {
    if !a.reproducible {
        return Some("irreproducible dice (test faces or a stream restore)");
    }
    if crate::tactical_dual::dropped_faction_at_the_table(gs) {
        return Some("a faction dropped by decision at the table");
    }
    let heart = ti4_model::id::RelicId::new("heartofixth");
    if gs.players.iter().any(|p| {
        crate::relics::holds(gs, &p.id, &heart)
            || crate::promissory::has_commander_ability(gs, &p.id, "jolnarcommander")
            || (crate::promissory::has_commander_ability(gs, &p.id, "letnevcommander")
                && p.leaders.get(ti4_model::id::text("letnevcommander")) != Some(&ti4_model::state::LeaderStatus::Unlocked))
    }) {
        return Some("a reroll question (Jol-Nar commander, Heart of Ixth) or a commander payment not held as unlocked");
    }
    None
}

/// The core half of the prelude's offense. `rng_before` is where the streams stood before the string step. `None` for the
/// positions of [`unported`], nested activity the reaction table does not follow, a seat order that is not the id order, or a
/// position the codecs refuse.
#[must_use]
pub fn space_cannon_offense(
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    a: Args<'_>,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<Out>> {
    if let Some(why) = unported(gs, &a) {
        note_bail(LABEL, why);
        return None;
    }
    let Some((mut cx, r)) = tables(gs) else {
        note_bail(LABEL, "owner tables");
        return None;
    };
    nested_check(script)?;
    if r.seats.windows(2).any(|w| w[0] >= w[1]) {
        note_bail(LABEL, "seats not in id order");
        return None;
    }
    let active = r.seat(a.active.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let Some(env) = crate::core_combat::window_env(&before, &cx, sources) else {
        note_bail(LABEL, "window env");
        return None;
    };
    let reaching: Vec<CUnit> = crate::combat::reaching_units(content, sources)
        .iter()
        .map(|id| <CUnit as IdStr>::from_id(id))
        .collect::<Option<_>>()?;
    let facts = Facts {
        active,
        system,
        announce: a.announce,
        staging: crate::supply::staging_enabled(&before),
        reaching,
        mapped: galaxy.is_some(),
        kind: r.code("space_cannon"),
        cause: r.code("unit_ability:space_cannon"),
        casualty: r.code("assign_casualty"),
        commander: r.code("letnevcommander"),
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let seats: Vec<ti4_engine_core::ids::Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::ids::Seat)).collect();
    let resolver = Resolver::new(&seats, Some(active));
    let mut driver = crate::window_driver::driver_for(SpaceCannonOffense::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, nested_fp(if galaxy.is_some() { OFFENSE_MAPPED } else { OFFENSE }, script), |core, _| {
        let driven = drive_windowed(&mut driver, core, &env.kernel, None, &r, &before, script, &|window, choice, st, rendered| {
            let ti4_engine_core::choice::Ask::Combat(ask) = &choice.ask else { return };
            match ask {
                CAsk::CancelHit { .. } => {
                    rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, choice.actor));
                }
                CAsk::AssignHit => {
                    let Some((gunner, bound)) = window.absorbing() else { return };
                    let producer = PlayerId::new(r.seats[usize::from(gunner.0)].clone());
                    if let Some(context) = rendered.context.take() {
                        rendered.context = Some(context.with_hit(crate::decision_context::HitDetail {
                            cause: crate::decision_context::HitCause::SpaceCannon,
                            destroy: false,
                            restriction: if bound { crate::decision_context::HitRestriction::NonFighter } else { crate::decision_context::HitRestriction::Any },
                            producer: Some(producer),
                        }));
                    }
                }
                _ => {}
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    })
    ;
    let driven = match driven {
        Ok(Ok(driven)) => driven,
        Ok(Err(why)) => {
            note_bail(LABEL, &format!("driver: {why:?}"));
            return None;
        }
        Err(why) => {
            note_bail(LABEL, &format!("lend: {why:?}"));
            return None;
        }
    };
    apply_staged(gs, &r, &staged)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let value: Out = (driver.window.scoring, driver.window.first_draws(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

// --- the head of the aftermath chain -----------------------------------------------------------------

/// The label of the head window in the dual report.
pub const HEAD_LABEL: &str = "aftermath::head";

/// What the string head (`AftermathWindow::head`) was called with.
#[derive(Debug, Clone, Copy)]
pub struct HeadArgs<'a> {
    pub active: &'a PlayerId,
    pub system: &'a SystemId,
    pub announce: bool,
    pub reproducible: bool,
    /// The action skipped to the Commit Ground Forces step.
    pub skip_to_commit: bool,
}

/// What the head leaves besides the state: the feat occurrence of the cannon's pause, the sides and the capital counts of the
/// system before the fight, the dice and where the streams stand.
pub type HeadOut = (Option<u64>, Vec<String>, Vec<(String, usize)>, Vec<(u32, Vec<u32>)>, crate::rng::RngPositions);

const HEAD: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Map, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending, Slice::Rng, Slice::Ledger],
);
const HEAD_NO_MAP: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Meta, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Pending, Slice::Rng, Slice::Ledger],
);

/// The core half of `AftermathWindow::head` / `head_at_commit`. `None` for the positions of [`unported`] (the skipped action has
/// no cannon: only the first three gates apply to it), a seat order that is not the id order, neutral units, nested activity the
/// reaction table does not follow, or a position the codecs refuse.
#[must_use]
pub fn aftermath_head(
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    galaxy: Option<&Galaxy>,
    a: HeadArgs<'_>,
    rng_before: &crate::rng::RngPositions,
    script: &Recording,
) -> Option<CoreWindow<HeadOut>> {
    use ti4_engine_core::driver::windows::aftermath::{Facts as HeadFacts, Head};
    let cannon_args = Args { active: a.active, system: a.system, announce: a.announce, reproducible: a.reproducible };
    if let Some(why) = unported(gs, &cannon_args) {
        note_bail(HEAD_LABEL, why);
        return None;
    }
    if !content.is_embedded() {
        note_bail(HEAD_LABEL, "content other than the embedded one");
        return None;
    }
    let Some((mut cx, r)) = tables(gs) else {
        note_bail(HEAD_LABEL, "owner tables");
        return None;
    };
    nested_check(script)?;
    if r.seats.windows(2).any(|w| w[0] >= w[1]) {
        note_bail(HEAD_LABEL, "seats not in id order");
        return None;
    }
    let limits = match crate::tactical_dual::limits_of(gs, content, a.active, a.system) {
        Ok(limits) => limits,
        Err(why) => {
            note_bail(HEAD_LABEL, why);
            return None;
        }
    };
    let active = r.seat(a.active.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let Some(env) = crate::tactical_dual::env_of(&before, &cx, sources) else {
        note_bail(HEAD_LABEL, "window env");
        return None;
    };
    let reaching: Vec<CUnit> = crate::combat::reaching_units(content, sources)
        .iter()
        .map(|id| <CUnit as IdStr>::from_id(id))
        .collect::<Option<_>>()?;
    let cannon = Facts {
        active,
        system,
        announce: a.announce,
        staging: crate::supply::staging_enabled(&before),
        reaching,
        mapped: galaxy.is_some(),
        kind: r.code("space_cannon"),
        cause: r.code("unit_ability:space_cannon"),
        casualty: r.code("assign_casualty"),
        commander: r.code("letnevcommander"),
    };
    let facts = HeadFacts {
        player: active,
        system,
        limits,
        cannon,
        notes: ti4_engine_core::state::NoteHold::new(),
        skip_to_commit: a.skip_to_commit,
        pre_roll: cx.texts.intern(Slice::Transient, "pre_roll").ok()?,
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let seats: Vec<ti4_engine_core::ids::Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::ids::Seat)).collect();
    let resolver = Resolver::new(&seats, Some(active));
    let mut driver = crate::window_driver::driver_for(Head::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy, rng: Some(rng_before) };
    let fp = if galaxy.is_some() { HEAD } else { HEAD_NO_MAP };
    let driven = with_core(gs, &mut cx, src, nested_fp(fp, script), |core, _| {
        let driven = drive_windowed(&mut driver, core, &env.kernel, None, &r, &before, script, &|window: &Head, choice, st, rendered| {
            let ti4_engine_core::choice::Ask::Combat(ask) = &choice.ask else { return };
            match ask {
                CAsk::CancelHit { .. } => {
                    rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(&env.kernel, st, ask, choice.actor));
                }
                CAsk::AssignHit => {
                    let Some((gunner, bound)) = window.absorbing() else { return };
                    let producer = PlayerId::new(r.seats[usize::from(gunner.0)].clone());
                    if let Some(context) = rendered.context.take() {
                        rendered.context = Some(context.with_hit(crate::decision_context::HitDetail {
                            cause: crate::decision_context::HitCause::SpaceCannon,
                            destroy: false,
                            restriction: if bound { crate::decision_context::HitRestriction::NonFighter } else { crate::decision_context::HitRestriction::Any },
                            producer: Some(producer),
                        }));
                    }
                }
                _ => {}
            }
        });
        staged = std::mem::take(&mut core.pending_mut().staged);
        rng_after = Some(core.rng().clone());
        driven
    });
    let driven = match driven {
        Ok(Ok(driven)) => driven,
        Ok(Err(why)) => {
            note_bail(HEAD_LABEL, &format!("driver: {why:?}"));
            return None;
        }
        Err(why) => {
            note_bail(HEAD_LABEL, &format!("lend: {why:?}"));
            return None;
        }
    };
    apply_staged(gs, &r, &staged)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let w = &driver.window;
    let snapshot = w.before.as_ref()?;
    let sides = snapshot.sides.as_slice().iter().map(|s| r.player(*s).to_string()).collect();
    let capitals = snapshot.capitals.as_slice().iter().map(|(s, n)| (r.player(*s).to_string(), usize::from(*n))).collect();
    let value: HeadOut = (w.scoring, sides, capitals, w.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}

// --- fleet::enforce_everywhere ---------------------------------------------------------------------------

/// The label of the table-wide enforcement in the dual report.
pub const ENFORCE_ALL_LABEL: &str = "fleet::enforce_everywhere";

/// The core half of `fleet::enforce_everywhere`: the number of units removed. `None` for a table the codecs refuse, nested
/// activity, seats not in id order, or more than two half-weight fighter forms.
#[must_use]
pub fn enforce_everywhere(gs: &mut GameState, content: &ContentStore, sources: SourceSet, script: &Recording) -> Option<CoreWindow<Option<usize>>> {
    use ti4_engine_core::driver::windows::enforce_all::EnforceAll;
    if !content.is_embedded() || script.nested || script.questions.iter().any(|q| q.answer.is_none()) {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let mut pairs = Vec::new();
    for seat in gs.players.iter().map(|p| p.id.clone()).collect::<Vec<_>>() {
        for system in gs.board.keys() {
            if gs.system_state(system).units_of(&seat).is_empty() {
                continue;
            }
            let limits = match crate::tactical_dual::limits_of(gs, content, &seat, system) {
                Ok(limits) => limits,
                Err(why) => {
                    note_bail(ENFORCE_ALL_LABEL, why);
                    return None;
                }
            };
            pairs.push((r.seat(seat.as_str())?, <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?, limits));
        }
    }
    let before = gs.clone();
    let kernel = crate::core_adapter::kernel_for(sources);
    let seats: Vec<ti4_engine_core::ids::Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::ids::Seat)).collect();
    let resolver = Resolver::new(&seats, seats.first().copied());
    let mut driver = crate::window_driver::driver_for(EnforceAll::new(pairs), resolver, AbilityTable::default(), 1);
    let driven = with_core(gs, &mut cx, Sources::default(), crate::tactical_dual::ENFORCE, |core, _| {
        crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &|_, _, _| {})
    });
    let driven = match driven {
        Ok(Ok(driven)) => driven,
        _ => {
            note_bail(ENFORCE_ALL_LABEL, "a position the codecs refuse");
            return None;
        }
    };
    Some(CoreWindow { run: Run::new(Some(driver.window.removed as usize)).with_events_vec(Vec::new()), questions: driven.rendered })
}

// --- Minister of War ----------------------------------------------------------------------------------------

/// The label of the Minister of War window in the dual report.
pub const MINISTER_LABEL: &str = "turn::minister_of_war";

/// What the Minister of War leaves besides the state: whether the law was discarded and the system the token came back from.
pub type MinisterOut = (bool, Option<String>);

const MINISTER: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Transient],
    &[Slice::Players, Slice::Board, Slice::Agenda, Slice::Transient],
);

/// The core half of `Game::minister_of_war` (called once the string found a token on the board). `None` for a law that is blanked
/// or not elected, nested activity, or a position the codecs refuse.
#[must_use]
pub fn minister_of_war(gs: &mut GameState, script: &Recording) -> Option<CoreWindow<MinisterOut>> {
    use ti4_engine_core::driver::windows::minister::MinisterOfWar;
    use ti4_engine_core::generated::Agenda;
    if script.nested || crate::laws::blanked(gs) {
        return None;
    }
    let owner_id = PlayerId::new(crate::laws::elected(gs, "minister_war")?.clone());
    let (mut cx, r) = tables(gs)?;
    let owner = r.seat(owner_id.as_str())?;
    let mut systems = Vec::new();
    for (system, here) in &gs.board {
        if here.command_tokens.contains(&owner_id) {
            systems.push(<ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?);
        }
    }
    let window = MinisterOfWar::new(
        owner,
        systems,
        Agenda::MinisterOfWar,
        r.code("Minister of War: discard to retrieve a command token and act again"),
        r.code("minister_war"),
    );
    let before = gs.clone();
    let kernel = ti4_engine_core::kernel::Kernel::plain();
    let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[owner], Some(owner)), AbilityTable::default(), 1);
    let recorded: Vec<_> = script.questions.iter().map(|q| q.choice.details.clone()).collect();
    let asked = std::cell::Cell::new(0_usize);
    let driven = with_core(gs, &mut cx, Sources::default(), MINISTER, |core, _| {
        crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|choice, _, rendered| {
            let at = asked.get();
            asked.set(at + 1);
            // The display facts under the offer are the recorded question's (the caller's caption).
            if matches!(choice.ask, ti4_engine_core::choice::Ask::Agenda(_)) {
                if let Some(details) = recorded.get(at) {
                    rendered.details = details.clone();
                }
            }
        })
    });
    let driven = match driven {
        Ok(Ok(driven)) => driven,
        _ => {
            note_bail(MINISTER_LABEL, "a position the codecs refuse");
            return None;
        }
    };
    let w = &driver.window;
    let value: MinisterOut = (w.discarded, w.retrieved.map(|s| s.as_str().to_owned()));
    Some(CoreWindow { run: Run::new(value).with_events_vec(Vec::new()), questions: driven.rendered })
}

// --- the feats of a finished combat ------------------------------------------------------------------------

const FEATS: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Players]);

/// What a fight started with, in the seats of the typed state (`combat::BeforeCombat`): the sides, their capital ships and the notes held.
pub(crate) fn before_snapshot(r: &crate::render::Render, before: &crate::combat::BeforeCombat) -> Option<ti4_engine_core::state::Before> {
    let mut snapshot = ti4_engine_core::state::Before::default();
    for side in before.sides() {
        snapshot.sides.push(r.seat(side.as_str())?);
    }
    for (player, n) in before.capitals() {
        snapshot.capitals.push((r.seat(player.as_str())?, u16::try_from(n).ok()?));
    }
    for (holder, issuers) in before.notes() {
        for issuer in issuers {
            snapshot.notes.push((r.seat(holder.as_str())?, r.seat(issuer.as_str())?));
        }
    }
    Some(snapshot)
}

/// The core half of `combat::note_combat_feats_at`: the feats the finished fight earned, recorded against `occurrence`; whether any
/// was earned. `None` for a content store other than the embedded one, a seat the tables do not know, or neutral sides.
#[must_use]
pub fn combat_feats(
    gs: &mut GameState,
    content: &ContentStore,
    sources: SourceSet,
    system: &SystemId,
    before: &crate::combat::BeforeCombat,
    outcome: &crate::combat::CombatOutcome,
    occurrence: ti4_model::state::FeatOccurrence,
) -> Option<bool> {
    use ti4_engine_core::rules::combat::feats::{feats, record, Outcome};
    if !content.is_embedded() {
        return None;
    }
    let (mut cx, r) = tables(gs)?;
    let sys = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str())?;
    let snapshot = before_snapshot(&r, before)?;
    let winner = match &outcome.winner {
        Some(w) => Some(r.seat(w.as_str())?),
        None => None,
    };
    let result = Outcome { winner, rounds: outcome.rounds };
    let kernel = crate::core_adapter::kernel_for(sources);
    with_core(gs, &mut cx, Sources::default(), FEATS, |core, _| {
        let rd = ti4_engine_core::rules::combat::Rd::new(core, kernel);
        let list = feats(rd, sys, &snapshot, result);
        record(core, &list, occurrence.0);
        !list.is_empty()
    })
    .ok()
}

// --- the end of the space combat -------------------------------------------------------------------------

/// The label of the end of a fight in the dual report.
pub const OVER_LABEL: &str = "aftermath::fight_over";

/// What the string `AftermathWindow::fight_over` was called with.
#[derive(Debug, Clone, Copy)]
pub struct OverArgs<'a> {
    pub active: &'a PlayerId,
    pub system: &'a SystemId,
    pub before: &'a crate::combat::BeforeCombat,
    pub outcome: &'a crate::combat::CombatOutcome,
    pub announce: bool,
}

const OVER: Footprint = Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Rng, Slice::Ledger],
    &[Slice::Board, Slice::Transient, Slice::Players, Slice::Marks, Slice::Pending, Slice::Ledger],
);

/// Why the end of the fight holds something the core window does not carry (`None`: it is carried): a dropped faction, Titans
/// (Coalescence's forced combats are a hook the combat kernel does not register), a diplomacy table (a contact pauses the chain),
/// Shard of the Throne changing hands (a law's theft).
#[must_use]
pub fn over_unported(gs: &GameState, _a: &OverArgs<'_>) -> Option<&'static str> {
    if crate::tactical_dual::dropped_faction_at_the_table(gs) {
        return Some("a faction dropped by decision at the table");
    }
    if gs.players.iter().any(|p| p.faction.as_str() == "titans") {
        return Some("Titans of Ul at the table (Coalescence)");
    }
    if gs.diplomacy.enabled {
        return Some("structured diplomacy on");
    }
    if gs.laws.contains_key("shard_of_the_throne") {
        return Some("Shard of the Throne in play (its theft)");
    }
    None
}

/// The core half of `AftermathWindow::fight_over` up to the contacts: `(holds, false)`. `None` for a position the codecs refuse,
/// nested activity the reaction table does not follow, or a limit the core cannot name.
#[must_use]
pub fn fight_over(gs: &mut GameState, content: &ContentStore, sources: SourceSet, a: OverArgs<'_>, script: &Recording) -> Option<CoreWindow<(bool, bool)>> {
    use ti4_engine_core::driver::windows::aftermath::{FightOver, OverFacts};
    use ti4_engine_core::rules::combat::feats::Outcome;
    if !content.is_embedded() {
        note_bail(OVER_LABEL, "content other than the embedded one");
        return None;
    }
    // (The neutral force is a side of the fight even when its last unit fell in it: the owner table puts it in.)
    let sides: Vec<&PlayerId> = a.before.sides().iter().collect();
    let Some((mut cx, r)) = tables_of_sides(gs, &sides) else {
        note_bail(OVER_LABEL, "owner tables");
        return None;
    };
    nested_check(script)?;
    let player = r.seat(a.active.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let mut sides = ti4_engine_core::containers::SmallList::new();
    for side in a.before.sides() {
        sides.push(r.seat(side.as_str())?);
    }
    let winner = match &a.outcome.winner {
        Some(w) => Some(r.seat(w.as_str())?),
        None => None,
    };
    let mut pairs = Vec::new();
    for seat in gs.players.iter().map(|p| p.id.clone()).collect::<Vec<_>>() {
        if gs.system_state(a.system).units_of(&seat).is_empty() {
            continue;
        }
        match crate::tactical_dual::limits_of(gs, content, &seat, a.system) {
            Ok(limits) => pairs.push((r.seat(seat.as_str())?, system, limits)),
            Err(why) => {
                note_bail(OVER_LABEL, why);
                return None;
            }
        }
    }
    let before = gs.clone();
    let Some(env) = crate::tactical_dual::env_of(&before, &cx, sources) else {
        note_bail(OVER_LABEL, "window env");
        return None;
    };
    let facts = OverFacts {
        player,
        system,
        outcome: Outcome { winner, rounds: a.outcome.rounds },
        sides,
        announce: a.announce,
        opponents_key: r.code("opponents"),
        pairs,
    };
    let seats: Vec<ti4_engine_core::ids::Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::ids::Seat)).collect();
    let mut driver = crate::window_driver::driver_for(FightOver::new(facts), Resolver::new(&seats, Some(player)), AbilityTable::default(), 1);
    let mut staged = Vec::new();
    let driven = with_core(gs, &mut cx, Sources::default(), nested_fp(OVER, script), |core, _| {
        let driven = crate::dual_window::drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
        staged = std::mem::take(&mut core.pending_mut().staged);
        driven
    });
    let driven = match driven {
        Ok(Ok(driven)) => driven,
        Ok(Err(why)) => {
            note_bail(OVER_LABEL, &format!("driver: {why:?}"));
            return None;
        }
        Err(why) => {
            note_bail(OVER_LABEL, &format!("lend: {why:?}"));
            return None;
        }
    };
    apply_staged(gs, &r, &staged)?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    Some(CoreWindow { run: Run::new((driver.window.holds, false)).with_events_vec(events), questions: driven.rendered })
}
