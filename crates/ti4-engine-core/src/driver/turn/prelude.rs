//! The per-step prelude: the reconcile hooks of `Game::step_flow` that run before the arm of every step, in the string's order.
//!
//! 1. the picks of the starting technology that need no decision (`starting_tech::settle`; KP4),
//! 2. the events effects staged without a resolver in reach (`announce_staged_ground_events`),
//! 3. the gains no site announced (`announce_gains`: breakthroughs, relics, technologies),
//! 4. station control (`space_stations::reconcile_all`: Thunder's Edge, refused: nothing to do),
//! 5. the Wormhole Nexus latch, 6. Styx's victory point (`legendary::settle_control_points`),
//! 7. the map edits (Nova Seed, Riftwalker Meian) replayed onto the map: not ported, a gap when one is recorded,
//! 8. the Spec Ops II units destroyed since the last step roll to survive (`faction_techs::roll_spec_ops`).
//!
//! `laws::apply_to_galaxy` has no core counterpart: the core reads the laws, the lanes and the ion storm where it asks the map.
//!
//! It runs as the job [`TurnJob::Reconcile`], opened by whoever starts a step ([`super::advance_reconcile`]): an emission here can open
//! a reaction window, so it suspends and replays like every other job.

use super::inline::{emit_typed, gap, halted, Flow};
use crate::generated::{Planet, System};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::keys::{Atom, MarkKey, MarkVal, Word};
use crate::query::board;
use crate::state::{CoreState, EventKind, Key, MetaFlags, Payload, PlainArg, PlainKind, PlainRec, RngDomain, RollPurpose, RollRec, Val};

/// What a Spec Ops II survival roll needs (`faction_techs::SPEC_OPS_SURVIVES_ON`) and the sides of the die.
const SPEC_OPS_SURVIVES_ON: u8 = 5;
const DIE_SIDES: u32 = 10;

/// The events effects staged without a resolver in reach, announced in the order staged (a window may stage more).
fn announce_staged(sx: &mut Sx<'_>) -> Flow {
    // The ground forces an agenda's effect destroyed come first (`hooks_ground::try_announce_staged_events`), then the staged events.
    if crate::driver::agenda::ground::announce(sx) == Flow::Halt {
        return Flow::Halt;
    }
    while crate::kernel::fx::has_staged(sx.state) {
        for rec in crate::kernel::fx::drain_staged(sx.state) {
            if halted(sx) {
                return Flow::Halt;
            }
            if sx.emit_mirrored(rec).is_err() {
                if !halted(sx) {
                    gap(sx, TurnGap::EmissionRefused);
                }
                return Flow::Halt;
            }
        }
    }
    if halted(sx) { Flow::Halt } else { Flow::Done }
}

fn list(atoms: impl IntoIterator<Item = Atom>) -> MarkVal {
    MarkVal::List(atoms.into_iter().map(MarkVal::Atom).collect())
}

/// What a seat held at the last step, as the mark `private:#seen:<kind>:<seat>` keeps it; `None` before the first look.
fn seen(core: &CoreState, kind: Word, seat: Seat) -> Option<Vec<Atom>> {
    match core.marks().tail.get(&MarkKey::PrivateSeen(kind, seat))? {
        MarkVal::List(items) => Some(items.iter().filter_map(|v| if let MarkVal::Atom(a) = v { Some(*a) } else { None }).collect()),
        _ => Some(Vec::new()),
    }
}

/// `Game::announce_gains`: breakthroughs and relics gained since the last step as typed `BREAKTHROUGH_GAINED` and `RELIC_GAINED`, and
/// technologies as `TECHNOLOGY_GAINED` ("when you gain this card" effects). Detected by comparing each seat with what it held at the last
/// step rather than at the dozen grant sites, so no path can forget. The first look at the technologies records the starting set without
/// announcing it. Only while a faction module is seated (nothing else listens).
///
/// The payload of the relic and breakthrough events names the player only: the payload has no value for a relic or a breakthrough, and
/// no ability of the core reads it.
fn announce_gains(sx: &mut Sx<'_>) -> Flow {
    if !sx.table.facts.staging {
        return Flow::Done;
    }
    let mut gained: Vec<(EventKind, Payload)> = Vec::new();
    // Written only after every gain is announced, so a failed emit is seen again next step.
    let mut marks: Vec<(MarkKey, MarkVal)> = Vec::new();
    let seats: Vec<Seat> = sx.state.players().iter().map(|p| p.seat).collect();
    for seat in seats {
        let Some(p) = sx.state.players().by_seat(seat) else { continue };
        let breakthrough = p.cards.breakthrough;
        let relics: Vec<_> = p.cards.relics.as_slice().to_vec();
        let techs: Vec<_> = p.tech.owned.iter().collect();
        let who = Payload::new().with(Key::Player, Val::Seat(seat));
        let seen_bt = seen(sx.state, Word::Breakthrough, seat).unwrap_or_default();
        if let Some(b) = breakthrough
            && seen_bt.first() != Some(&Atom::Breakthrough(b))
        {
            gained.push((EventKind::BreakthroughGained, who.clone()));
        }
        let mut remaining = seen(sx.state, Word::Relics, seat).unwrap_or_default();
        for relic in &relics {
            match remaining.iter().position(|a| *a == Atom::Relic(*relic)) {
                Some(i) => {
                    remaining.remove(i);
                }
                None => gained.push((EventKind::RelicGained, who.clone())),
            }
        }
        if let Some(known) = seen(sx.state, Word::Technologies, seat) {
            for tech in &techs {
                if !known.contains(&Atom::Tech(*tech)) {
                    gained.push((EventKind::TechnologyGained, who.clone().with(Key::Technology, Val::Tech(*tech))));
                }
            }
        }
        marks.push((MarkKey::PrivateSeen(Word::Technologies, seat), list(techs.iter().map(|t| Atom::Tech(*t)))));
        marks.push((MarkKey::PrivateSeen(Word::Breakthrough, seat), list(breakthrough.map(Atom::Breakthrough))));
        marks.push((MarkKey::PrivateSeen(Word::Relics, seat), list(relics.iter().map(|r| Atom::Relic(*r)))));
    }
    // Propagation (Nekro): each research replaced by 3 command tokens since the last step is announced so the owner can place them
    // (`factions::nekro`); each announcement settles one (the rule writes the pending count down).
    for seat in sx.kernel.hooks.direct.nekro_pending_propagation(sx.state).as_slice().to_vec() {
        gained.push((EventKind::PropagationResearch, Payload::new().with(Key::Player, Val::Seat(seat))));
    }
    for (kind, payload) in gained {
        if emit_typed(sx, kind, payload) == Flow::Halt {
            return Flow::Halt;
        }
    }
    for (key, value) in marks {
        if sx.state.marks().tail.get(&key) != Some(&value) {
            sx.state.marks_mut().tail.edit().insert(key, value);
        }
    }
    Flow::Done
}

/// `GameState::nexus_unlocked`: "After a player's unit is placed in or moves into the Wormhole Nexus, or a player gains control of
/// Mallice." Sticky: once open it stays open, so this only ever sets the flag (`seating::nexus_is_triggered`).
fn nexus_latch(core: &mut CoreState) {
    if core.meta().flags.has(MetaFlags::NEXUS_UNLOCKED) {
        return;
    }
    let nexus = System::MalliceLockedNexus;
    let occupied = !board::space_units(core, nexus).is_empty() || board::planets_of(core, nexus).any(|p| !board::planet_units(core, p).is_empty());
    let held = [Planet::Mallice, Planet::LockedMallice].iter().any(|p| board::control(core, *p).is_some());
    if occupied || held {
        core.meta_mut().flags.set(MetaFlags::NEXUS_UNLOCKED, true);
    }
}

/// `faction_techs::roll_spec_ops`: every Spec Ops II destroyed since the last step rolls to survive (5 or more).
fn spec_ops(sx: &mut Sx<'_>) {
    let owed: Vec<(Seat, u32)> = sx.state.players().iter().filter(|p| p.misc.spec_ops_destroyed > 0).map(|p| (p.seat, p.misc.spec_ops_destroyed)).collect();
    for (owner, count) in owed {
        let faces: Vec<u8> = (0..count).map(|_| u8::try_from(sx.die(RngDomain::Dice, DIE_SIDES)).unwrap_or(u8::MAX)).collect();
        let scene = sx.state.meta().active_system;
        sx.state.ledger_mut().dice.push(RollRec {
            purpose: RollPurpose::Other(sx.table.codes.turn.spec_ops),
            owner: Some(owner),
            unit: None,
            planet: None,
            scene,
            hits_on: Some(SPEC_OPS_SURVIVES_ON),
            faces: faces.iter().copied().collect(),
            rerolled: 0,
        });
        let survived = u32::try_from(faces.iter().filter(|f| **f >= SPEC_OPS_SURVIVES_ON).count()).unwrap_or(0);
        if let Some(p) = sx.state.players_mut().by_seat_mut(owner) {
            p.misc.spec_ops_destroyed = 0;
            p.misc.spec_ops_card += survived;
        }
        for face in faces {
            let kind = if face >= SPEC_OPS_SURVIVES_ON { PlainKind::SpecOpsSurvived } else { PlainKind::SpecOpsLost };
            sx.plain(PlainRec::one(kind, PlainArg::Seat(owner)));
        }
    }
}

/// The reconcile job.
pub fn run_reconcile(sx: &mut Sx<'_>) -> Flow {
    if !sx.state.transient().misc.starting_tech_picks.is_empty() {
        // The picks that need no decision are given out first (KP4); a pick that needs one is the arm's question.
        if super::starting::settle(sx) == Flow::Halt {
            return Flow::Halt;
        }
    }
    if announce_staged(sx) == Flow::Halt || announce_gains(sx) == Flow::Halt {
        return Flow::Halt;
    }
    nexus_latch(sx.state);
    crate::rules::legendary::settle_styx(sx.state);
    if sx.state.marks().tail.iter().any(|(k, _)| matches!(k, MarkKey::MapEdit(_))) && !crate::kernel::fx_map::replay_map_edits(sx.state) {
        // A tile a faction effect replaced or swapped (Nova Seed, Riftwalker Meian) is replayed onto the map every step; an edit that no longer fits
        // the map is the counted gap.
        gap(sx, TurnGap::MapEdits);
        return Flow::Halt;
    }
    spec_ops(sx);
    if halted(sx) { Flow::Halt } else { Flow::Done }
}
