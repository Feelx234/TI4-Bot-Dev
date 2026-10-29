use std::collections::BTreeMap;
use std::sync::OnceLock;

use ti4_engine_core::containers::{SeatSet, SmallList};
use ti4_engine_core::generated::{Attachment, Planet, System, Unit};
use ti4_engine_core::state::{CoreState, Piece, PlanetCell, PlanetFlags, Slice, SysCell, SysFlags, UnitList};
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::{GameState, SystemState};
use ti4_model::units::Unit as StrUnit;

use super::{Sources, SliceCodec};
use crate::core_sync::conv::{id, txt};
use crate::core_sync::ctx::Ctx;
use crate::core_sync::error::{R, bad};

const S: Slice = Slice::Board;

pub struct Board;

/// The system a planet is printed in (`SystemData.planets`), by planet index.
fn printed_system(p: Planet) -> Option<System> {
    static TABLE: OnceLock<Vec<Option<System>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = vec![None; Planet::COUNT];
        for s in System::ALL {
            for p in s.data().planets {
                t[p.index()] = Some(s);
            }
        }
        t
    })[p.index()]
}

pub(crate) fn piece(cx: &Ctx, u: &StrUnit) -> R<Piece> {
    let kind = id::<Unit>(S, "unit type", u.type_id.as_str())?;
    Ok(Piece::new(kind, cx.seat(S, u.owner.as_str())?).with_sustained(u.sustained_damage).with_galvanized(u.galvanized))
}

fn unit(cx: &Ctx, p: Piece) -> R<StrUnit> {
    Ok(StrUnit {
        type_id: UnitTypeId::new(txt(p.kind())),
        owner: PlayerId::new(cx.owner(S, p.owner())?),
        sustained_damage: p.sustained(),
        galvanized: p.galvanized(),
    })
}

pub(super) fn pieces(cx: &Ctx, units: &[StrUnit]) -> R<UnitList> {
    let mut l = UnitList::new();
    for u in units {
        l.push(piece(cx, u)?);
    }
    Ok(l)
}

pub(super) fn units(cx: &Ctx, l: &[Piece]) -> R<Vec<StrUnit>> {
    l.iter().map(|p| unit(cx, *p)).collect()
}

impl SliceCodec for Board {
    const SLICE: Slice = S;

    fn load(cx: &mut Ctx, gs: &GameState, _: Sources<'_>, core: &mut CoreState) -> R<()> {
        let board = &mut core.board;
        for (sid, st) in &gs.board {
            let sys = id::<System>(S, "system", sid.as_str())?;
            let cell = board.sys_mut(sys);
            cell.units = pieces(cx, &st.units)?;
            cell.cmd = st.command_tokens.iter().map(|p| cx.seat(S, p.as_str())).collect::<R<SeatSet>>()?;
            cell.flags.set(SysFlags::TOUCHED, true);
            // The planets' maps: each entry belongs to the planet's own system.
            for pid in st
                .planet_control
                .keys()
                .chain(st.planet_units.keys())
                .chain(st.coexisting.keys())
                .chain(st.purged_planets.iter())
            {
                let planet = id::<Planet>(S, "planet", pid.as_str())?;
                let placed = gs.placed_planets.get(pid).map(|s| id::<System>(S, "system", s.as_str())).transpose()?;
                if placed.or_else(|| printed_system(planet)) != Some(sys) {
                    return Err(bad(S, "planet entry stored under another system", pid));
                }
            }
            for (pid, owner) in &st.planet_control {
                board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?).control = Some(cx.seat(S, owner.as_str())?);
            }
            for (pid, us) in &st.planet_units {
                let c = board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?);
                c.units = pieces(cx, us)?;
                c.flags.set(PlanetFlags::UNITS_ENTRY, true);
            }
            for (pid, owners) in &st.coexisting {
                let c = board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?);
                c.coexist = owners.iter().map(|p| cx.seat(S, p.as_str())).collect::<R<SeatSet>>()?;
                c.flags.set(PlanetFlags::COEXIST_ENTRY, true);
            }
            for pid in &st.purged_planets {
                board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?).flags.set(PlanetFlags::PURGED, true);
            }
        }
        for pid in &gs.exhausted_planets {
            board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?).flags.set(PlanetFlags::EXHAUSTED, true);
        }
        for (pid, list) in &gs.planet_attachments {
            let c = board.planet_mut(id::<Planet>(S, "planet", pid.as_str())?);
            c.attach = list.iter().map(|a| id::<Attachment>(S, "attachment", a)).collect::<R<SmallList<_, 3>>>()?;
            c.flags.set(PlanetFlags::ATTACH_ENTRY, true);
        }
        for (pid, sid) in &gs.placed_planets {
            let planet = id::<Planet>(S, "planet", pid.as_str())?;
            board.planet_mut(planet).placed_in = Some(id::<System>(S, "system", sid.as_str())?);
        }
        for sid in &gs.purged_systems {
            board.sys_mut(id::<System>(S, "system", sid.as_str())?).flags.set(SysFlags::PURGED, true);
        }
        Ok(())
    }

    fn store(cx: &Ctx, core: &CoreState, gs: &mut GameState) -> R<()> {
        let board = core.board();
        let mut out: BTreeMap<SystemId, SystemState> = BTreeMap::new();
        gs.exhausted_planets.clear();
        gs.planet_attachments.clear();
        gs.placed_planets.clear();
        gs.purged_systems.clear();
        let empty = SysCell::default();
        for (sys, cell) in board.systems() {
            if cell == &empty {
                continue;
            }
            if cell.flags.has(SysFlags::PURGED) {
                gs.purged_systems.insert(SystemId::new(txt(sys)));
            }
            if cell.flags.has(SysFlags::TOUCHED) {
                out.insert(
                    SystemId::new(txt(sys)),
                    SystemState {
                        units: units(cx, &cell.units)?,
                        command_tokens: cell.cmd.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?,
                        ..SystemState::default()
                    },
                );
            }
        }
        let empty = PlanetCell::default();
        for (planet, cell) in board.planets() {
            if cell == &empty {
                continue;
            }
            let pid = PlanetId::new(txt(planet));
            let flags = cell.flags;
            if flags.has(PlanetFlags::EXHAUSTED) {
                gs.exhausted_planets.insert(pid.clone());
            }
            if flags.has(PlanetFlags::ATTACH_ENTRY) {
                gs.planet_attachments.insert(pid.clone(), cell.attach.iter().map(|a| txt(*a).to_owned()).collect());
            }
            if let Some(sys) = cell.placed_in {
                gs.placed_planets.insert(pid.clone(), SystemId::new(txt(sys)));
            }
            let home = cell.placed_in.or_else(|| printed_system(planet));
            let touches = cell.control.is_some()
                || flags.has(PlanetFlags::UNITS_ENTRY)
                || flags.has(PlanetFlags::COEXIST_ENTRY)
                || flags.has(PlanetFlags::PURGED);
            if !touches {
                continue;
            }
            let sys = home.ok_or_else(|| bad(S, "planet without a system", pid.as_str()))?;
            let st = out
                .get_mut(&SystemId::new(txt(sys)))
                .ok_or_else(|| bad(S, "planet entry in an untouched system", pid.as_str()))?;
            if let Some(owner) = cell.control {
                st.planet_control.insert(pid.clone(), PlayerId::new(cx.owner(S, owner)?));
            }
            if flags.has(PlanetFlags::UNITS_ENTRY) {
                st.planet_units.insert(pid.clone(), units(cx, &cell.units)?);
            }
            if flags.has(PlanetFlags::COEXIST_ENTRY) {
                st.coexisting.insert(
                    pid.clone(),
                    cell.coexist.iter().map(|s| cx.owner(S, s).map(PlayerId::new)).collect::<R<_>>()?,
                );
            }
            if flags.has(PlanetFlags::PURGED) {
                st.purged_planets.insert(pid.clone());
            }
        }
        gs.board = out;
        Ok(())
    }
}
