//! sardakk: Exotrireme II (`factions::sardakk::exotrireme`): "After a round of space combat, you may destroy this unit to destroy up to 2 ships in this system." Optional (the window's
//! own use / decline is the "may"); needs a side of the combat that owns the technology and an Exotrireme II in the system, and another ship to destroy. Every choice is asked before the first ship
//! is removed: which Exotrireme II (a damaged one and an undamaged one are different ships; one question only when there are several), then up to two ships. The ships are removed
//! and their destruction staged under the cause `technology:exo2` (the combat announces them). Registered for every seat (`technology:*:exo2:SPACE_COMBAT_ROUND_ENDED:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{System, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Runner, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, Piece};

use super::code;
use crate::common::has_tech_text;

/// Exotrireme II (`technology:*:exo2:SPACE_COMBAT_ROUND_ENDED:after`).
pub const EXO2: Slot = Slot(42_008);

fn fought_in(p: &Payload, who: Seat) -> bool {
    p.seat(Key::Attacker) == Some(who) || p.seat(Key::Defender) == Some(who)
}

/// `ships_in`: the ships standing in the space area, in the list order.
fn ships(s: &CoreState, system: System) -> Vec<Piece> {
    board::space_units(s, system).iter().copied().filter(|p| p.kind().data().ship).collect()
}

/// `exotrireme_ready`: the owner holds the technology, fought, has an Exotrireme II in the system and another ship is there to destroy.
fn ready(s: &CoreState, who: Seat, system: System, fought: bool) -> bool {
    if !fought || !has_tech_text(s, who, Technology::ExotriremeII) {
        return false;
    }
    let all = ships(s, system);
    all.iter().any(|p| p.owner() == who && p.kind() == Unit::ExotriremeII) && all.len() >= 2
}

/// `distinct`: the values of a ship list, in first-seen order.
fn distinct(pieces: &[Piece]) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for p in pieces {
        if !out.contains(p) {
            out.push(*p);
        }
    }
    out
}

fn condition(c: &CondCx<'_>) -> bool {
    c.event.payload.system(Key::System).is_some_and(|system| ready(c.state, c.def.owner, system, fought_in(&c.event.payload, c.def.owner)))
}

fn ask(eff: &mut Eff<'_, '_>, ask: TAsk, subtype: TSub, opts: Opts) -> Pick {
    let me = eff.owner();
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::T(ask))),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Technology(Technology::ExotriremeII)),
            subtype: Subtype::Reaction(RSub::G(GSub::T(subtype))),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    eff.ask(&choice)
}

fn ship_opts(pieces: &[Piece], victim: bool) -> Opts {
    pieces
        .iter()
        .enumerate()
        .map(|(index, p)| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Ship { index: u8::try_from(index).unwrap_or(u8::MAX), owner: p.owner(), unit: p.kind(), damaged: p.sustained(), victim }))))
        .collect()
}

fn chosen(pick: Pick, pieces: &[Piece]) -> Option<Piece> {
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Ship { index, .. })))) = pick else { return None };
    pieces.get(usize::from(index)).copied()
}

fn destroy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    use ti4_engine_core::rules::combat::{assign::remove_ship, ships_of, Rd};
    let me = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    if !ready(eff.read().0, me, system, fought_in(&eff.event.payload, me)) {
        return Ok(());
    }
    let mut pool = ships(eff.read().0, system);
    let own = distinct(&pool.iter().copied().filter(|p| p.owner() == me && p.kind() == Unit::ExotriremeII).collect::<Vec<_>>());
    let dreadnought = if let [only] = own.as_slice() {
        *only
    } else {
        let pick = ask(eff, TAsk::ExoSelf, TSub::ExoSelf, ship_opts(&own, false));
        let Some(piece) = chosen(pick, &own) else { return Ok(()) };
        piece
    };
    if let Some(at) = pool.iter().position(|p| *p == dreadnought) {
        pool.remove(at);
    }
    let mut victims: Vec<Piece> = Vec::new();
    for round in 0..2_u8 {
        if pool.is_empty() {
            break;
        }
        let candidates = distinct(&pool);
        let mut opts = ship_opts(&candidates, true);
        if round > 0 {
            opts.push(Opt::Decline);
        }
        let pick = ask(eff, TAsk::ExoVictim { round }, TSub::ExoVictim, opts);
        if matches!(pick, Pick::Opt(Opt::Decline)) {
            break;
        }
        let Some(piece) = chosen(pick, &candidates) else { return Ok(()) };
        if let Some(at) = pool.iter().position(|p| *p == piece) {
            pool.remove(at);
        }
        victims.push(piece);
    }
    // Everything is decided: it destroys itself first, then the chosen ships by owner.
    let cause = eff.codes().silent.g3[code::EXO2_CAUSE];
    let mut owners: Vec<Seat> = Vec::new();
    for v in &victims {
        if !owners.contains(&v.owner()) {
            owners.push(v.owner());
        }
    }
    let mut batches: Vec<(Seat, Vec<Piece>)> = vec![(me, vec![dreadnought])];
    for who in owners {
        batches.push((who, victims.iter().copied().filter(|v| v.owner() == who).collect()));
    }
    for (index, (who, batch)) in batches.into_iter().enumerate() {
        // `destroy_units_with_context`: every victim must stand there (count against what the board holds), else nothing is destroyed.
        let present = {
            let (state, kernel) = eff.read();
            let blanked = ti4_engine_core::query::reach::blanked(state);
            ships_of(Rd::new(state, kernel).with_blanked(blanked), who, system)
        };
        let mut remaining = present;
        let all_there = batch.iter().all(|v| match remaining.iter().position(|p| p == v) {
            Some(at) => {
                remaining.remove(at);
                true
            }
            None => false,
        });
        if !all_there {
            if index == 0 { // mutants:skip (the choices were made from the board a moment ago: the first batch is always there)
                return Ok(());
            }
            continue;
        }
        eff.inline().with_sx(|sx| {
            for piece in batch {
                remove_ship(sx, system, who, piece);
                sx.state.transient_mut().combat.pending_destructions.push(ti4_engine_core::state::PendingDestruction { system, owner: who, unit: piece.kind(), reason: cause, during_space_combat: true });
            }
        });
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(EXO2, AbilityKind::Technology, EventKind::SpaceCombatRoundEnded, Relation::After, destroy).optional().when(condition));
}
