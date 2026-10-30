//! The core half of the space cannon offense of the aftermath prelude (package D1b2, A2b-10b): the adapter of
//! `rules::combat::space_cannon_offense`. The string site is `AftermathWindow::new`'s prelude (`game.rs`
//! `cannon_prelude`), wrapped in `dual_window!(Area::Driver, "combat::space_cannon_offense", ..)` on `driver=dual`.
//!
//! What stays with the string (bails, each counted under the label with its reason): irreproducible dice, a seat of a faction
//! dropped by decision, seats not in id order (the string iterates its gunners by player id), a Jol-Nar commander or a Heart of
//! Ixth (their reroll windows ask), a Letnev commander that pays on a sustain without being unlocked, games below the rules
//! version of the faction effects, neutral units in the system, nested activity the reaction table does not follow.

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine_core::choice::combat::Ask as CAsk;
use ti4_engine_core::driver::Driver;
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
use crate::dual_window::{DiceSeeds, Recording, apply_staged, drive_windowed, event_of, nested_check, nested_fp, tables};
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
    if gs.rules_version < ti4_model::state::RULES_FACTION_EFFECTS {
        return Some("rules version below the faction effects");
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
    let board = gs.system_state(a.system);
    if board.units.iter().chain(board.planet_units.values().flatten()).any(|u| crate::neutral_units::is_neutral(&u.owner)) {
        return Some("neutral units in the system");
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
    let (mut cx, r) = tables(gs)?;
    nested_check(script)?;
    if r.seats.windows(2).any(|w| w[0] >= w[1]) {
        note_bail(LABEL, "seats not in id order");
        return None;
    }
    let active = r.seat(a.active.as_str())?;
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(a.system.as_str())?;
    let before = gs.clone();
    let env = crate::core_combat::window_env(&before, &cx, sources)?;
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
        kind: r.code("space_cannon"),
        cause: r.code("unit_ability:space_cannon"),
        casualty: r.code("assign_casualty"),
        commander: r.code("letnevcommander"),
    };
    let seeds = std::rc::Rc::new(DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
    let seats: Vec<ti4_engine_core::ids::Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::ids::Seat)).collect();
    let resolver = Resolver::new(&seats, Some(active));
    let mut driver = Driver::new(SpaceCannonOffense::new(facts), resolver, AbilityTable::default(), 1).with_seeds(seeds);
    let mut staged = Vec::new();
    let mut rng_after = None;
    let src = Sources { galaxy, rng: Some(rng_before) };
    let driven = with_core(gs, &mut cx, src, nested_fp(OFFENSE, script), |core, _| {
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
    .ok()?
    .ok()?;
    apply_staged(gs, &r, &staged)?;
    let positions = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
    let events: Vec<Event> = driver.emitted.iter().map(|e| event_of(&r, e)).collect::<Option<_>>()?;
    let value: Out = (driver.window.scoring, driver.window.drawn.clone(), positions);
    Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
}
