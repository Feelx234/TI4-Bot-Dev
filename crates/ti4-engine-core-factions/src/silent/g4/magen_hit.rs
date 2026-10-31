//! Magen Defense Grid, the second effect (`magen_defense::combat_start`, the string's `technology:*:md_hit:GROUND_COMBAT_STARTED:after`): "At the start of ground combat on a planet that contains 1 or more of your
//! structures: produce 1 hit and assign it to 1 of your opponent's ground forces." Mandatory. The owner assigns the hit (asked with several kinds of ground force there: identical units are one option, in the
//! order the planet lists them); the unit's OWNER then decides on SUSTAIN DAMAGE as for any hit an effect assigns (`invasion::assign_selected_ground_hit_in_timing`: the card with its captions), and the unit is
//! damaged or destroyed with the event of the result (cause `md`), a destroyed Spec Ops II counted for its survival roll.

use ti4_engine_core::choice::generic::{Content, Rule};
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent4::{Ask as UAsk, Opt as UOpt, Subtype as USub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype, Target};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Planet, System, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::reach;
use ti4_engine_core::rules::invasion::{can_sustain_here, is_ground_force_here};
use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, Piece, Val};

use super::code;
use crate::magen_defense::{active, structure_planets};

/// Magen Defense Grid's hit (`technology:*:md_hit:GROUND_COMBAT_STARTED:after`).
pub const MD_HIT: Slot = Slot(44_004);

fn rd<'a>(s: &'a CoreState, k: &'a Kernel) -> Rd<'a> {
    Rd::new(s, k).with_blanked(reach::blanked(s))
}

/// `opponent_in`: the side of the combat that is not `owner`, when `owner` is a side.
fn opponent_in(event: &ti4_engine_core::state::EventRec, owner: Seat) -> Option<Seat> {
    let attacker = event.payload.seat(Key::Attacker)?;
    let defender = event.payload.seat(Key::Defender)?;
    if attacker == owner {
        Some(defender)
    } else if defender == owner {
        Some(attacker)
    } else {
        None
    }
}

fn condition(c: &CondCx<'_>) -> bool {
    let me = c.def.owner;
    active(c.state, me)
        && opponent_in(c.event, me).is_some()
        && c.event.payload.system(Key::System).zip(c.event.payload.planet(Key::Planet)).is_some_and(|(system, planet)| structure_planets(c.state, me, system).contains(&planet))
}

/// The opponent's ground forces on the planet, identical units one entry.
fn targets(s: &CoreState, k: &Kernel, opponent: Seat, system: System, planet: Planet) -> Vec<Piece> {
    let mut found: Vec<Piece> = Vec::new();
    for piece in s.board().planet(planet).units.iter().copied().filter(|p| p.owner() == opponent) {
        if is_ground_force_here(rd(s, k), opponent, system, planet, piece) && !found.contains(&piece) {
            found.push(piece);
        }
    }
    found
}

/// `faction_techs::note_destroyed`: a destroyed Spec Ops II is counted for its survival roll.
fn note_destroyed(s: &mut CoreState, piece: Piece) {
    if piece.kind() == Unit::SpecOpsII {
        if let Some(p) = s.players_mut().by_seat_mut(piece.owner()) {
            p.misc.spec_ops_destroyed += 1;
        }
    }
}

fn ctx(origin: Origin, subtype: Subtype, target: Option<Target>) -> Ctx {
    Ctx { origin, subtype, optional: false, target, outstanding: SmallList::new(), space_battle: false, invasion_seq: None }
}

fn hit(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let (Some(system), Some(planet), Some(opponent)) = (eff.event.payload.system(Key::System), eff.event.payload.planet(Key::Planet), opponent_in(eff.event, me)) else { return Ok(()) };
    let candidates = {
        let (s, k) = eff.read();
        targets(s, k, opponent, system, planet)
    };
    let target = match candidates.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        _ => {
            let opts: Opts = {
                let (s, k) = eff.read();
                candidates
                    .iter()
                    .enumerate()
                    .map(|(index, piece)| {
                        let data = rd(s, k).data(piece.kind());
                        let sturdy = !piece.sustained() && data.is_some_and(|d| d.sustain_damage);
                        let cost_halves = data.and_then(|d| d.cost_halves).unwrap_or(0);
                        Opt::Reaction(ROpt::G(GOpt::U(UOpt::Target { index: u8::try_from(index).unwrap_or(u8::MAX), owner: opponent, unit: piece.kind(), damaged: piece.sustained(), sturdy, cost_halves })))
                    })
                    .collect()
            };
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::U(UAsk::MagenHit))),
                ctx: ctx(Origin::Content(Content::Technology(Technology::MagenDefenseGridOmegaOmega)), Subtype::Reaction(RSub::G(GSub::U(USub::MagenHit))), None),
                opts,
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::U(UOpt::Target { index, .. })))) = eff.ask(&choice) else { return Ok(()) };
            let Some(picked) = candidates.get(usize::from(index)) else { return Ok(()) };
            *picked
        }
    };
    // `assign_selected_ground_hit_in_timing`: the unit must still be there, a ground force.
    {
        let (s, k) = eff.read();
        if !s.board().planet(planet).units.iter().any(|p| *p == target) || !is_ground_force_here(rd(s, k), opponent, system, planet, target) {
            return Ok(());
        }
    }
    let sustain = {
        let can = {
            let (s, k) = eff.read();
            can_sustain_here(rd(s, k), target, system, planet)
        };
        if can {
            let staging = eff.adapter_facts().staging;
            let rule = Origin::Rule(Rule::Other(eff.codes().silent.g4[code::SUSTAIN_RULE]));
            let choice = Choice {
                actor: opponent,
                ask: Ask::Reaction(RAsk::G(GAsk::U(UAsk::GroundSustain { unit: target.kind(), system, planet, staging }))),
                ctx: ctx(rule, Subtype::Reaction(RSub::G(GSub::U(USub::GroundSustain))), Some(Target::Planet { system, planet })),
                opts: [Opt::Reaction(ROpt::G(GOpt::U(UOpt::Sustain))), Opt::Decline].into_iter().collect::<Opts>(),
            };
            matches!(eff.ask(&choice), Pick::Opt(Opt::Reaction(ROpt::G(GOpt::U(UOpt::Sustain)))))
        } else {
            false
        }
    };
    let cause = eff.codes().silent.g4[code::MD_CAUSE];
    let mut payload = Payload::new()
        .with(Key::System, Val::System(system))
        .with(Key::Planet, Val::Planet(planet))
        .with(Key::Player, Val::Seat(opponent))
        .with(Key::Unit, Val::Unit(target.kind()))
        .with(Key::Cause, Val::Code(cause));
    let kind = {
        let s = eff.state();
        let units = &mut s.board_mut().planet_mut(planet).units;
        let at = units.iter().position(|p| *p == target);
        if sustain {
            if let Some(at) = at {
                units.as_mut_slice()[at] = target.with_sustained(true);
            }
            EventKind::GroundForceSustained
        } else {
            if let Some(at) = at {
                units.remove(at);
            }
            note_destroyed(s, target);
            payload.set(Key::Damaged, Val::Flag(target.sustained()));
            if target.galvanized() {
                payload.set(Key::Galvanized, Val::Flag(true));
            }
            EventKind::GroundForceDestroyed
        }
    };
    eff.emit(fx::event(kind, payload))?;
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(MD_HIT, AbilityKind::Technology, EventKind::GroundCombatStarted, Relation::After, hit).when(condition));
}
