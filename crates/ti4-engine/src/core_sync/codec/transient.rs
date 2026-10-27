use ti4_engine_core::containers::{Cow, Log, SmallList, SortedMap};
use ti4_engine_core::generated::{ActionCard, Planet, StrategyCard, System, Technology, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state as c;
use ti4_engine_core::state::{CoreState, Slice};
use ti4_model::id::{ActionCardId, PlanetId, PlayerId, StrategyCardId, SystemId, TechnologyId, UnitTypeId};
use ti4_model::state as m;
use ti4_model::state::GameState;

use super::board::{pieces, units};
use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, narrow, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Transient;

pub struct Transient;

fn sys(t: &SystemId) -> R<System> {
    id::<System>(S, "system", t.as_str())
}
fn planet(t: &PlanetId) -> R<Planet> {
    id::<Planet>(S, "planet", t.as_str())
}
fn unit(t: &UnitTypeId) -> R<Unit> {
    id::<Unit>(S, "unit type", t.as_str())
}
fn sid(s: System) -> SystemId {
    SystemId::new(txt(s))
}
fn pid(p: Planet) -> PlanetId {
    PlanetId::new(txt(p))
}
fn uid(u: Unit) -> UnitTypeId {
    UnitTypeId::new(txt(u))
}

fn seat_counts(cx: &Ctx, m: &std::collections::BTreeMap<PlayerId, u32>) -> R<SortedMap<Seat, u32, 4>> {
    let mut out = SortedMap::new();
    for (k, v) in m {
        out.insert(cx.seat(S, k.as_str())?, *v);
    }
    Ok(out)
}

fn seat_counts_usize(cx: &Ctx, m: &std::collections::BTreeMap<PlayerId, usize>) -> R<SortedMap<Seat, u32, 4>> {
    let mut out = SortedMap::new();
    for (k, v) in m {
        out.insert(cx.seat(S, k.as_str())?, narrow::<usize, u32>(S, "hits", *v)?);
    }
    Ok(out)
}

fn seat_counts_back(cx: &Ctx, m: &SortedMap<Seat, u32, 4>) -> R<std::collections::BTreeMap<PlayerId, u32>> {
    m.iter().map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), *v))).collect()
}

fn seat_counts_usize_back(cx: &Ctx, m: &SortedMap<Seat, u32, 4>) -> R<std::collections::BTreeMap<PlayerId, usize>> {
    m.iter().map(|(k, v)| Ok((PlayerId::new(cx.owner(S, *k)?), *v as usize))).collect()
}

pub(crate) fn die(cx: &mut Ctx, r: &m::CombatRollRecord) -> R<c::CombatDie> {
    Ok(c::CombatDie { player: cx.seat(S, r.player.as_str())?, unit: cx.texts.intern(S, &r.unit)?, roll: r.roll, target: r.target, hit: r.hit })
}

pub(crate) fn die_back(cx: &Ctx, d: &c::CombatDie) -> R<m::CombatRollRecord> {
    Ok(m::CombatRollRecord {
        player: PlayerId::new(cx.owner(S, d.player)?),
        unit: cx.texts.text(S, d.unit)?.to_owned(),
        roll: d.roll,
        target: d.target,
        hit: d.hit,
    })
}

pub(crate) fn reroll_set(cx: &mut Ctx, r: &m::RerollSet) -> R<c::RerollSet> {
    let mut rolls = Vec::with_capacity(r.rolls.len());
    for e in &r.rolls {
        let mut rerolled = 0u64;
        for i in &e.rerolled {
            if *i >= 64 {
                return Err(bad(S, "rerolled die index", i));
            }
            rerolled |= 1 << i;
        }
        let mut deltas = SortedMap::new();
        for (k, v) in &e.deltas {
            deltas.insert(narrow::<usize, u8>(S, "delta index", *k)?, *v);
        }
        let mut unit_types = SortedMap::new();
        for (k, v) in &e.unit_types {
            unit_types.insert(cx.texts.intern(S, k)?, *v);
        }
        rolls.push(c::RerollEntry {
            unit: cx.texts.intern(S, &e.unit)?,
            planet: e.planet.as_ref().map(planet).transpose()?,
            hits_on: e.hits_on.map(|h| narrow::<u32, u8>(S, "hits_on", h)).transpose()?,
            faces: e.faces.iter().map(|f| narrow::<u32, u8>(S, "face", *f)).collect::<R<_>>()?,
            rerolled,
            deltas,
            unit_types,
        });
    }
    Ok(c::RerollSet { kind: cx.texts.intern(S, &r.kind)?, system: sys(&r.system)?, rolls })
}

pub(crate) fn reroll_set_back(cx: &Ctx, r: &c::RerollSet) -> R<m::RerollSet> {
    let mut rolls = Vec::with_capacity(r.rolls.len());
    for e in &r.rolls {
        rolls.push(m::RerollEntry {
            unit: cx.texts.text(S, e.unit)?.to_owned(),
            planet: e.planet.map(pid),
            hits_on: e.hits_on.map(u32::from),
            faces: e.faces.iter().map(|f| u32::from(*f)).collect(),
            rerolled: (0..64usize).filter(|i| e.rerolled & (1 << i) != 0).collect(),
            deltas: e.deltas.iter().map(|(k, v)| (usize::from(*k), *v)).collect(),
            unit_types: e
                .unit_types
                .iter()
                .map(|(k, v)| Ok((cx.texts.text(S, *k)?.to_owned(), *v)))
                .collect::<R<_>>()?,
        });
    }
    Ok(m::RerollSet { kind: cx.texts.text(S, r.kind)?.to_owned(), system: sid(r.system), rolls })
}

fn invasion_step(cx: &mut Ctx, s: &m::InvasionStep) -> R<c::InvasionStep> {
    Ok(c::InvasionStep {
        planet: planet(&s.planet)?,
        kind: cx.texts.intern(S, &s.kind)?,
        round: s.round,
        before: pieces(cx, &s.before)?.to_vec(),
        after: pieces(cx, &s.after)?.to_vec(),
        dice: s
            .dice
            .iter()
            .map(|d| {
                Ok(c::InvasionDie {
                    planet: planet(&d.planet)?,
                    player: cx.seat(S, d.player.as_str())?,
                    group: cx.texts.intern(S, &d.group)?,
                    face: d.face,
                    target: d.target,
                    hit: d.hit,
                })
            })
            .collect::<R<_>>()?,
        hits: seat_counts_usize(cx, &s.hits)?,
        harrow_hits: narrow::<usize, u32>(S, "harrow_hits", s.harrow_hits)?,
    })
}

fn invasion_step_back(cx: &Ctx, s: &c::InvasionStep) -> R<m::InvasionStep> {
    Ok(m::InvasionStep {
        planet: pid(s.planet),
        kind: cx.texts.text(S, s.kind)?.to_owned(),
        round: s.round,
        before: units(cx, &s.before)?,
        after: units(cx, &s.after)?,
        dice: s
            .dice
            .iter()
            .map(|d| {
                Ok(m::InvasionDie {
                    planet: pid(d.planet),
                    player: PlayerId::new(cx.owner(S, d.player)?),
                    group: cx.texts.text(S, d.group)?.to_owned(),
                    face: d.face,
                    target: d.target,
                    hit: d.hit,
                })
            })
            .collect::<R<_>>()?,
        hits: seat_counts_usize_back(cx, &s.hits)?,
        harrow_hits: s.harrow_hits as usize,
    })
}

fn tech_source(s: &m::StartingTechSource) -> R<c::StartingTechSource> {
    Ok(match s {
        m::StartingTechSource::Listed(list) => {
            let techs = list.iter().map(|t| id::<Technology>(S, "technology", t.as_str())).collect::<R<Vec<_>>>()?;
            c::StartingTechSource::Listed(
                c::TechList::from_slice(&techs).ok_or_else(|| bad(S, "starting tech list too long", list.len()))?,
            )
        }
        m::StartingTechSource::OwnedByOthers => c::StartingTechSource::OwnedByOthers,
        m::StartingTechSource::Researchable => c::StartingTechSource::Researchable,
    })
}

impl SliceCodec for Transient {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let mut t = c::Transient::default();

        let cb = &mut t.combat;
        let mut staging = Vec::new();
        for (p, set) in &gs.reroll_staging {
            staging.push((cx.seat(S, p.as_str())?, reroll_set(cx, set)?));
        }
        cb.reroll_staging = Cow::new(staging);
        cb.last_reroll_player = gs.last_reroll_player.as_ref().map(|p| cx.seat(S, p.as_str())).transpose()?;
        cb.last_committed_unit = gs
            .last_committed_unit
            .as_ref()
            .map(|(p, s, pl, u)| {
                let piece = pieces(cx, std::slice::from_ref(u))?;
                Ok((cx.seat(S, p.as_str())?, sys(s)?, planet(pl)?, piece[0]))
            })
            .transpose()?;
        cb.last_sustain = gs
            .last_sustain
            .as_ref()
            .map(|(s, o, u, p, f)| Ok((sys(s)?, cx.seat(S, o.as_str())?, unit(u)?, cx.seat(S, p.as_str())?, *f)))
            .transpose()?;
        for (s, o, u, reason, during) in &gs.pending_destructions {
            cb.pending_destructions.push(c::PendingDestruction {
                system: sys(s)?,
                owner: cx.seat(S, o.as_str())?,
                unit: unit(u)?,
                reason: cx.texts.intern(S, reason)?,
                during_space_combat: *during,
            });
        }
        cb.last_ship_destroyed =
            gs.last_ship_destroyed.as_ref().map(|(s, o, u)| Ok((sys(s)?, cx.seat(S, o.as_str())?, unit(u)?))).transpose()?;
        cb.last_control_gained = gs
            .last_control_gained
            .as_ref()
            .map(|(s, p, g, f)| {
                Ok((sys(s)?, planet(p)?, cx.seat(S, g.as_str())?, f.as_ref().map(|f| cx.seat(S, f.as_str())).transpose()?))
            })
            .transpose()?;
        cb.last_combat_sides = gs
            .last_combat_sides
            .as_ref()
            .map(|(s, sides)| {
                let mut l: SmallList<Seat, 4> = SmallList::new();
                for p in sides {
                    l.push(cx.seat(S, p.as_str())?);
                }
                Ok((sys(s)?, l))
            })
            .transpose()?;
        cb.active_space_combat = gs
            .active_space_combat
            .as_ref()
            .map(|(s, a, d)| Ok((sys(s)?, cx.seat(S, a.as_str())?, cx.seat(S, d.as_str())?)))
            .transpose()?;
        cb.combat_round_hits = seat_counts(cx, &gs.combat_round_hits)?;
        let mut dice = Log::new();
        for d in &gs.combat_round_dice {
            dice.push(die(cx, d)?);
        }
        cb.combat_round_dice = dice;
        let p = &gs.combat_presentation;
        cb.presentation = Cow::new(c::CombatPresentation {
            battle_seq: p.battle_seq,
            phase: cx.texts.intern(S, &p.phase)?,
            round: p.round,
            round_start: pieces(cx, &p.round_start)?.to_vec(),
            barrage_start: pieces(cx, &p.barrage_start)?.to_vec(),
            barrage_hits: seat_counts(cx, &p.barrage_hits)?,
            barrage_dice: p.barrage_dice.iter().map(|d| die(cx, d)).collect::<R<_>>()?,
            remaining_hits: seat_counts_usize(cx, &p.remaining_hits)?,
        });
        cb.pending_reflective_hits = gs
            .pending_reflective_hits
            .as_ref()
            .map(|(s, v, n)| Ok((sys(s)?, cx.seat(S, v.as_str())?, narrow::<usize, u32>(S, "reflective hits", *n)?)))
            .transpose()?;
        cb.last_action_discarded = gs
            .last_action_discarded
            .as_ref()
            .map(|(p, a)| Ok((cx.seat(S, p.as_str())?, id::<ActionCard>(S, "action card", a.as_str())?)))
            .transpose()?;

        t.invasion.active = gs
            .active_invasion
            .as_ref()
            .map(|a| {
                Ok(Cow::new(c::ActiveInvasion {
                    system: sys(&a.system)?,
                    invader: cx.seat(S, a.invader.as_str())?,
                    seq: a.seq,
                    phase: cx.texts.intern(S, &a.phase)?,
                    planet: a.planet.as_ref().map(planet).transpose()?,
                    defender: a.defender.as_ref().map(|d| cx.seat(S, d.as_str())).transpose()?,
                    ground_round: a.ground_round,
                    last_step: a.last_step.as_ref().map(|s| invasion_step(cx, s)).transpose()?,
                }))
            })
            .transpose()?;

        let pr = &mut t.production;
        pr.fighters_produced_this_use = gs.fighters_produced_this_use;
        pr.nonfighter_ships_produced_this_use = gs.nonfighter_ships_produced_this_use;
        pr.units_produced_this_use = gs.units_produced_this_use;
        pr.discount_remaining = gs.production_discount_remaining;
        pr.value_swapped_planet = gs.production_value_swapped_planet.as_ref().map(planet).transpose()?;
        for (k, v) in &gs.gravleash_move_values {
            pr.gravleash_move_values.insert(sys(k)?, *v);
        }

        let ms = &mut t.misc;
        for bit in 0..8u8 {
            if gs.transient_flags.has(1 << bit) {
                ms.flags.set(1 << bit);
            }
        }
        for (p, a) in &gs.action_card_plays {
            ms.action_card_plays.push((cx.seat(S, p.as_str())?, id::<ActionCard>(S, "action card", a.as_str())?));
        }
        for (p, a) in &gs.declared_card_plays {
            ms.declared_card_plays.push((cx.seat(S, p.as_str())?, id::<ActionCard>(S, "action card", a.as_str())?));
        }
        ms.last_strategy_choice = gs
            .last_strategy_choice
            .as_ref()
            .map(|(p, s)| Ok((cx.seat(S, p.as_str())?, id::<StrategyCard>(S, "strategy card", s.as_str())?)))
            .transpose()?;
        for pick in &gs.starting_tech_picks {
            ms.starting_tech_picks.push(c::StartingTechPick {
                player: cx.seat(S, pick.player.as_str())?,
                of: pick.of,
                remaining: pick.remaining,
                source: tech_source(&pick.source)?,
            });
        }
        core.transient = t;
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let t = core.transient();
        let cb = &t.combat;
        gs.reroll_staging = cb
            .reroll_staging
            .iter()
            .map(|(s, set)| Ok((PlayerId::new(cx.owner(S, *s)?), reroll_set_back(cx, set)?)))
            .collect::<R<_>>()?;
        gs.last_reroll_player = cb.last_reroll_player.map(|s| cx.owner(S, s).map(PlayerId::new)).transpose()?;
        gs.last_committed_unit = cb
            .last_committed_unit
            .map(|(p, s, pl, u)| {
                let list = units(cx, &[u])?;
                Ok((PlayerId::new(cx.owner(S, p)?), sid(s), pid(pl), list.into_iter().next().expect("one unit")))
            })
            .transpose()?;
        gs.last_sustain = cb
            .last_sustain
            .map(|(s, o, u, p, f)| {
                Ok((sid(s), PlayerId::new(cx.owner(S, o)?), uid(u), PlayerId::new(cx.owner(S, p)?), f))
            })
            .transpose()?;
        gs.pending_destructions = cb
            .pending_destructions
            .iter()
            .map(|d| {
                Ok((
                    sid(d.system),
                    PlayerId::new(cx.owner(S, d.owner)?),
                    uid(d.unit),
                    cx.texts.text(S, d.reason)?.to_owned(),
                    d.during_space_combat,
                ))
            })
            .collect::<R<_>>()?;
        gs.last_ship_destroyed =
            cb.last_ship_destroyed.map(|(s, o, u)| Ok((sid(s), PlayerId::new(cx.owner(S, o)?), uid(u)))).transpose()?;
        gs.last_control_gained = cb
            .last_control_gained
            .map(|(s, p, g, f)| {
                Ok((
                    sid(s),
                    pid(p),
                    PlayerId::new(cx.owner(S, g)?),
                    f.map(|f| cx.owner(S, f).map(PlayerId::new)).transpose()?,
                ))
            })
            .transpose()?;
        gs.last_combat_sides = cb
            .last_combat_sides
            .as_ref()
            .map(|(s, sides)| {
                Ok((sid(*s), sides.iter().map(|x| cx.owner(S, *x).map(PlayerId::new)).collect::<R<_>>()?))
            })
            .transpose()?;
        gs.active_space_combat = cb
            .active_space_combat
            .map(|(s, a, d)| Ok((sid(s), PlayerId::new(cx.owner(S, a)?), PlayerId::new(cx.owner(S, d)?))))
            .transpose()?;
        gs.combat_round_hits = seat_counts_back(cx, &cb.combat_round_hits)?;
        gs.combat_round_dice = cb.combat_round_dice.iter().map(|d| die_back(cx, d)).collect::<R<_>>()?;
        let p = &cb.presentation;
        gs.combat_presentation = m::CombatPresentation {
            battle_seq: p.battle_seq,
            phase: cx.texts.text(S, p.phase)?.to_owned(),
            round: p.round,
            round_start: units(cx, &p.round_start)?,
            barrage_start: units(cx, &p.barrage_start)?,
            barrage_hits: seat_counts_back(cx, &p.barrage_hits)?,
            barrage_dice: p.barrage_dice.iter().map(|d| die_back(cx, d)).collect::<R<_>>()?,
            remaining_hits: seat_counts_usize_back(cx, &p.remaining_hits)?,
        };
        gs.pending_reflective_hits = cb
            .pending_reflective_hits
            .map(|(s, v, n)| Ok((sid(s), PlayerId::new(cx.owner(S, v)?), n as usize)))
            .transpose()?;
        gs.last_action_discarded = cb
            .last_action_discarded
            .map(|(p, a)| Ok((PlayerId::new(cx.owner(S, p)?), ActionCardId::new(txt(a)))))
            .transpose()?;

        gs.active_invasion = t
            .invasion
            .active
            .as_ref()
            .map(|a| {
                Ok(m::ActiveInvasion {
                    system: sid(a.system),
                    invader: PlayerId::new(cx.owner(S, a.invader)?),
                    seq: a.seq,
                    phase: cx.texts.text(S, a.phase)?.to_owned(),
                    planet: a.planet.map(pid),
                    defender: a.defender.map(|d| cx.owner(S, d).map(PlayerId::new)).transpose()?,
                    ground_round: a.ground_round,
                    last_step: a.last_step.as_ref().map(|s| invasion_step_back(cx, s)).transpose()?,
                })
            })
            .transpose()?;

        let pr = &t.production;
        gs.fighters_produced_this_use = pr.fighters_produced_this_use;
        gs.nonfighter_ships_produced_this_use = pr.nonfighter_ships_produced_this_use;
        gs.units_produced_this_use = pr.units_produced_this_use;
        gs.production_discount_remaining = pr.discount_remaining;
        gs.production_value_swapped_planet = pr.value_swapped_planet.map(pid);
        gs.gravleash_move_values = pr.gravleash_move_values.iter().map(|(k, v)| (sid(*k), *v)).collect();

        let ms = &t.misc;
        gs.transient_flags = m::TransientFlags::default();
        for bit in 0..8u8 {
            if ms.flags.has(1 << bit) {
                gs.transient_flags.set(1 << bit);
            }
        }
        gs.action_card_plays = ms
            .action_card_plays
            .iter()
            .map(|(p, a)| Ok((PlayerId::new(cx.owner(S, *p)?), ActionCardId::new(txt(*a)))))
            .collect::<R<_>>()?;
        gs.declared_card_plays = ms
            .declared_card_plays
            .iter()
            .map(|(p, a)| Ok((PlayerId::new(cx.owner(S, *p)?), ActionCardId::new(txt(*a)))))
            .collect::<R<_>>()?;
        gs.last_strategy_choice = ms
            .last_strategy_choice
            .map(|(p, s)| Ok((PlayerId::new(cx.owner(S, p)?), StrategyCardId::new(txt(s)))))
            .transpose()?;
        gs.starting_tech_picks = ms
            .starting_tech_picks
            .iter()
            .map(|p| {
                Ok(m::StartingTechPick {
                    player: PlayerId::new(cx.owner(S, p.player)?),
                    of: p.of,
                    remaining: p.remaining,
                    source: match p.source {
                        c::StartingTechSource::Listed(list) => {
                            m::StartingTechSource::Listed(list.as_slice().iter().map(|t| TechnologyId::new(txt(*t))).collect())
                        }
                        c::StartingTechSource::OwnedByOthers => m::StartingTechSource::OwnedByOthers,
                        c::StartingTechSource::Researchable => m::StartingTechSource::Researchable,
                    },
                })
            })
            .collect::<R<_>>()?;
        Ok(())
    }
}
