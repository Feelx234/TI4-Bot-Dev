//! The typed copies of the two windows of the action phase (package D1b; docs/todo/D1b.md D1b-1): the tactical action and the
//! aftermath that follows its movement step. A child of `d1_pending`, which calls these from `observe` (string field -> typed
//! window) and `roundtrip` (typed window -> the comparable key of the string field, compared with the field it was read from).
//!
//! The aftermath payload holds where the chain stands (`AStage`), not the live sub-window: the space combat, the invasion and
//! the production step are the windows of their own areas and stay with the string until the composer runs them (see
//! `rules::aftermath`). Counted bails (`Err` of the `*_of` functions): a seat, system or unit the tables do not know, a hold
//! with more than 12 pieces.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{System, Unit as CUnit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::state::{AStage, Aftermath, Before, NoteHold, PauseLimit, Piece, TStage, Tactical, Window};

use super::super::{Aftermath as AStr, AftermathWindow, Game, TacticalStage, TacticalWindow};
use crate::combat::NoteHoldings;
use crate::generated::IdStr;
use crate::objectives::EventScoreLimit;
use crate::render::Render;

type Res<T> = Result<T, &'static str>;

fn seat(r: &Render, p: &ti4_model::id::PlayerId) -> Res<Seat> {
    r.seat(p.as_str()).ok_or("a seat the table does not know")
}

fn system(id: &ti4_model::id::SystemId) -> Res<System> {
    <System as IdStr>::from_id(id.as_str()).ok_or("a system the tables do not know")
}

/// The note holdings of the string as `(holder, issuer)` pairs, in the map's order (seating order is the key order of the
/// string: `PlayerId`s order as text, and so does the seat table of the tests; the pairs keep the string's order).
pub(super) fn notes_of(notes: &NoteHoldings, r: &Render) -> Res<NoteHold> {
    let mut out = NoteHold::new();
    for (holder, issuers) in notes {
        for issuer in issuers {
            out.push((seat(r, holder)?, seat(r, issuer)?));
        }
    }
    Ok(out)
}

/// The inverse of [`notes_of`]: a holder with no issuer has no entry, so the map is compared after dropping the empty ones.
pub(super) fn notes_back(notes: &NoteHold, r: &Render) -> NoteHoldings {
    let mut out = NoteHoldings::new();
    for (holder, issuer) in notes.as_slice() {
        out.entry(r.player(*holder)).or_default().insert(r.player(*issuer));
    }
    out
}

fn nonempty(notes: &NoteHoldings) -> NoteHoldings {
    notes.iter().filter(|(_, issuers)| !issuers.is_empty()).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// The comparable key of a tactical window (strings), read from the string window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TKey {
    player: String,
    stage: TStageKey,
    notes: NoteHoldings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TStageKey {
    Activating,
    Moving,
    Loading { origin: String, ship: (String, String, bool, bool), path: Vec<String>, loaded: Vec<usize>, closed: bool },
}

pub(super) fn tactical_key(w: &TacticalWindow) -> TKey {
    let stage = match &w.stage {
        TacticalStage::Activating => TStageKey::Activating,
        TacticalStage::Moving => TStageKey::Moving,
        TacticalStage::Loading { origin, ship, path, window } => {
            let (loaded, closed) = window.progress();
            TStageKey::Loading {
                origin: origin.to_string(),
                ship: (ship.type_id.to_string(), ship.owner.to_string(), ship.sustained_damage, ship.galvanized),
                path: path.clone(),
                loaded: loaded.to_vec(),
                closed,
            }
        }
    };
    TKey { player: w.player.to_string(), stage, notes: nonempty(&w.notes_at_start) }
}

pub(super) fn tactical_of(g: &Game<'_>, r: &Render) -> Res<Option<Window>> {
    let Some(w) = &g.tactical else { return Ok(None) };
    let player = seat(r, &w.player)?;
    let stage = match &w.stage {
        TacticalStage::Activating => TStage::Activating,
        TacticalStage::Moving => TStage::Moving,
        TacticalStage::Loading { origin, ship, path, window } => {
            let (loaded, closed) = window.progress();
            let kind = <CUnit as IdStr>::from_id(ship.type_id.as_str()).ok_or("a unit type the tables do not know")?;
            let piece = Piece::new(kind, seat(r, &ship.owner)?).with_sustained(ship.sustained_damage).with_galvanized(ship.galvanized);
            let path: Option<SmallList<System, 8>> = path.iter().map(|s| <System as IdStr>::from_id(s)).collect();
            let loaded: Option<SmallList<u16, 12>> = loaded.iter().map(|i| u16::try_from(*i).ok()).collect();
            TStage::Loading {
                origin: system(origin)?,
                ship: piece,
                path: path.ok_or("a system the tables do not know")?,
                loaded: loaded.ok_or("a hold above 65535 candidates")?,
                closed,
            }
        }
    };
    Ok(Some(Window::Tactical(Tactical { player, stage, notes_at_start: notes_of(&w.notes_at_start, r)? })))
}

pub(super) fn tactical_back(t: &Tactical, r: &Render) -> TKey {
    let stage = match &t.stage {
        TStage::Activating => TStageKey::Activating,
        TStage::Moving => TStageKey::Moving,
        TStage::Loading { origin, ship, path, loaded, closed } => TStageKey::Loading {
            origin: origin.as_str().to_owned(),
            ship: (ship.kind().as_str().to_owned(), r.player(ship.owner()).to_string(), ship.sustained(), ship.galvanized()),
            path: path.as_slice().iter().map(|s| s.as_str().to_owned()).collect(),
            loaded: loaded.as_slice().iter().map(|i| usize::from(*i)).collect(),
            closed: *closed,
        },
    };
    TKey { player: r.player(t.player).to_string(), stage, notes: nonempty(&notes_back(&t.notes_at_start, r)) }
}

/// The comparable key of an aftermath window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AKey {
    player: String,
    system: String,
    stage: (u8, bool),
    sides: Vec<String>,
    capitals: Vec<(String, usize)>,
    feats_noted: bool,
    pause: Option<(u64, bool)>,
    notes: NoteHoldings,
}

fn stage_of(s: &AStr) -> (u8, bool) {
    match s {
        AStr::Fighting(_) => (0, false),
        AStr::Negotiating { invade } => (1, *invade),
        AStr::Invading(_) => (2, false),
        AStr::Producing(_) => (3, false),
        AStr::Done => (4, false),
    }
}

/// The captial counts of a `BeforeCombat`, read from its `Debug` form: the fields are private to `combat.rs` (a file of
/// another package) and the type has no accessor for them. The form is `capitals: {PlayerId("a"): 1, ...}`.
pub(super) fn capitals_of(before: &crate::combat::BeforeCombat) -> Vec<(String, usize)> {
    let text = format!("{before:?}");
    let Some(at) = text.find("capitals: {") else { return Vec::new() };
    let rest = &text[at + "capitals: {".len()..];
    let Some(end) = rest.find('}') else { return Vec::new() };
    rest[..end]
        .split(", ")
        .filter_map(|entry| {
            let (key, n) = entry.split_once("): ")?;
            let name = key.split_once("(\"")?.1.trim_end_matches('"');
            Some((name.to_owned(), n.trim().parse().ok()?))
        })
        .collect()
}

pub(super) fn aftermath_key(w: &AftermathWindow) -> AKey {
    AKey {
        player: w.player.to_string(),
        system: w.system.to_string(),
        stage: stage_of(&w.stage),
        sides: w.before_combat.sides().iter().map(ToString::to_string).collect(),
        capitals: capitals_of(&w.before_combat),
        feats_noted: w.feats_noted,
        pause: w.pending_event_scoring.map(|(occurrence, limit)| (occurrence.0, limit == EventScoreLimit::OnePerPlayer)),
        notes: nonempty(&w.notes_at_tactical_start),
    }
}

pub(super) fn aftermath_of(g: &Game<'_>, r: &Render) -> Res<Option<Window>> {
    let Some(w) = &g.aftermath else { return Ok(None) };
    let stage = match &w.stage {
        AStr::Fighting(_) => AStage::Fighting,
        AStr::Negotiating { invade } => AStage::Negotiating { invade: *invade },
        AStr::Invading(_) => AStage::Invading,
        AStr::Producing(_) => AStage::Producing,
        AStr::Done => AStage::Done,
    };
    let mut sides = SmallList::new();
    for p in w.before_combat.sides() {
        sides.push(seat(r, p)?);
    }
    let mut capitals = SmallList::new();
    for (p, n) in capitals_of(&w.before_combat) {
        let seat = r.seat(&p).ok_or("a seat the table does not know")?;
        capitals.push((seat, u16::try_from(n).map_err(|_| "a capital count above 65535")?));
    }
    let notes = notes_of(&w.notes_at_tactical_start, r)?;
    Ok(Some(Window::Aftermath(Box::new(Aftermath {
        player: seat(r, &w.player)?,
        system: system(&w.system)?,
        stage,
        before_combat: Before { sides, capitals, notes: notes.clone() },
        feats_noted: w.feats_noted,
        event_scoring: w.pending_event_scoring.map(|(o, l)| {
            (o.0, if l == EventScoreLimit::OnePerPlayer { PauseLimit::OnePerPlayer } else { PauseLimit::AnyPerPlayer })
        }),
        notes_at_start: notes,
    }))))
}

pub(super) fn aftermath_back(a: &Aftermath, r: &Render) -> AKey {
    AKey {
        player: r.player(a.player).to_string(),
        system: a.system.as_str().to_owned(),
        stage: match a.stage {
            AStage::Fighting => (0, false),
            AStage::Negotiating { invade } => (1, invade),
            AStage::Invading => (2, false),
            AStage::Producing => (3, false),
            AStage::Done => (4, false),
        },
        sides: a.before_combat.sides.as_slice().iter().map(|s| r.player(*s).to_string()).collect(),
        capitals: a.before_combat.capitals.as_slice().iter().map(|(s, n)| (r.player(*s).to_string(), usize::from(*n))).collect(),
        feats_noted: a.feats_noted,
        pause: a.event_scoring.map(|(o, l)| (o, l == PauseLimit::OnePerPlayer)),
        notes: nonempty(&notes_back(&a.notes_at_start, r)),
    }
}

/// Where the chain stood when a step ended: the `Settled` of the core, in plain data for `game.rs`.
#[derive(Debug, Clone, Copy)]
pub(in super::super) enum Ended {
    /// `holds`, `contacts`.
    Fighting(bool, bool),
    /// `contacts`.
    Invading(bool),
    /// `invade`.
    Negotiating(bool),
    Producing,
}

/// The stage tag of a core stage, as [`stage_of`] gives it for the string's.
fn tag(s: AStage) -> (u8, bool) {
    match s {
        AStage::Fighting => (0, false),
        AStage::Negotiating { invade } => (1, invade),
        AStage::Invading => (2, false),
        AStage::Producing => (3, false),
        AStage::Done => (4, false),
    }
}

/// Compare the step the string's `AftermathWindow` went to with the core's `driver::aftermath::next` (`d1::aftermath.next`).
pub(in super::super) fn transition(ended: Ended, to: &AStr) {
    use ti4_engine_core::driver::aftermath::{Settled, next};
    if !super::active() {
        return;
    }
    let settled = match ended {
        Ended::Fighting(holds, contacts) => Settled::Fighting { holds, contacts },
        Ended::Invading(contacts) => Settled::Invading { contacts },
        Ended::Negotiating(invade) => Settled::Negotiating { invade },
        Ended::Producing => Settled::Producing,
    };
    let _ = crate::dual::compare_value(crate::dual::Area::Driver, "d1::aftermath.next", &stage_of(to), &tag(next(settled)));
}
