//! From `combat_live.rs` of the legacy crate: the display facts of a combat question and the texts of the space combat (copies; the
//! lockstep half and its tests stay in the legacy crate).

use ti4_engine_core::generated::Unit;
use ti4_engine_core::kernel::ability::{AftermathGap, CombatTexts};
use ti4_engine_core::state::{Code, CoreState};

use crate::generated::IdStr;
use crate::render::Render;

/// The words of a position the native aftermath does not run (`Unknown::UnportedAftermath`).
#[must_use]
pub const fn gap_text(why: AftermathGap) -> &'static str {
    match why {
        AftermathGap::NoProgress => "the aftermath made no progress (a stage that neither asks nor moves on)",
        AftermathGap::Negotiating => "the contact pause of the aftermath (structured diplomacy is dropped)",
        AftermathGap::NoCombatEnv => "a space combat whose environment cannot be built from the position",
        AftermathGap::CombatUnported => "the space combat machine reached a position it does not carry (cancelled hits, Duranium Armor, Devotion, staged destructions, a retreat the core does not move)",
        AftermathGap::RollEndsFight => "a roll that ended the fight inside the roll step",
        AftermathGap::NeutralSide => "a neutral side in the space combat",
        AftermathGap::EventScoring => "a scoring window that asks for something the core does not offer",
    }
}

/// The display facts of a combat question that only the position knows: the sustainers the casualty is chosen from, and the subtype of the
/// casualty question (its text is a render code the core's `Subtype::Casualty(Code)` cannot carry from a window rebuilt without the adapter).
pub fn complete(core: &CoreState, k: &ti4_engine_core::kernel::Kernel, choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::wire::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    let ti4_engine_core::choice::Ask::Combat(ask) = &choice.ask else { return };
    match ask {
        CAsk::RerollDie { .. } | CAsk::MunitionsRoll => roll_details(core, choice, rendered),
        CAsk::CancelHit { .. } => {
            rendered.details.insert("sustainers".to_owned(), super::core_combat::sustainers_detail(k, core, ask, choice.actor));
        }
        CAsk::AssignHit => {
            if let Some(ctx) = rendered.context.as_mut() {
                ctx.subtype = "assign_casualty".to_owned();
            }
        }
        _ => {}
    }
}

/// The questions of the combat the string asks in the battle they are fought in (`TimingContext::ask_seeing`: Ambush, Sacrifice, Splicer, Raid
/// Formation); every other question a combat child asks inside an emission names no battle. The nested rendering puts the battle on all of them.
pub fn after_nested(choice: &ti4_engine_core::choice::Choice, rendered: &mut crate::wire::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    // (Rear Admiral Farran's payment after a sustain is the leaders' question, asked in the same step.)
    let in_combat = match &choice.ask {
        ti4_engine_core::choice::Ask::Combat(ask) => !matches!(ask, CAsk::Ambush | CAsk::Sacrifice { .. } | CAsk::Splicer | CAsk::RaidDamage { .. }),
        ti4_engine_core::choice::Ask::Leaders(ti4_engine_core::choice::leaders::Ask::SustainCommander) => true,
        _ => false,
    };
    if !in_combat {
        return;
    }
    if let Some(ctx) = rendered.context.as_mut() {
        ctx.space_battle = false;
        ctx.target = None;
    }
}

/// The display facts of a roll step's questions (`dual_window::roll_details`): the staged dice, which one is asked about, and Munitions
/// Reserves' printed card and cost.
fn roll_details(core: &CoreState, c: &ti4_engine_core::choice::Choice, rendered: &mut crate::wire::choice::Choice) {
    use ti4_engine_core::choice::combat::Ask as CAsk;
    let ti4_engine_core::choice::Ask::Combat(ask) = &c.ask else { return };
    let set = core.transient().combat.reroll_staging.iter().find(|(s, _)| *s == c.actor).map(|(_, set)| set);
    let text = |code: Code| core.texts.codes.get(u32::from(code.0)).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
    let dice: Vec<serde_json::Value> = set
        .map(|set| {
            set.rolls
                .iter()
                .enumerate()
                .flat_map(|(unit, entry)| {
                    let name = text(entry.unit);
                    entry.faces.iter().enumerate().map(move |(die, face)| {
                        let bonus = u8::try_from(die).ok().and_then(|d| entry.deltas.get(&d).copied()).unwrap_or(0);
                        serde_json::json!({
                            "unit": name,
                            "unit_index": unit,
                            "die": die,
                            "face": face,
                            "bonus": bonus,
                            "hits_on": entry.hits_on,
                            "rerolled": die < 64 && entry.rerolled & (1 << die) != 0,
                        })
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    match ask {
        CAsk::RerollDie { die, .. } => {
            let entry = c.opts.iter().find_map(|o| match o {
                ti4_engine_core::choice::Opt::Combat(ti4_engine_core::choice::combat::Opt::RerollDie { entry, .. }) => Some(usize::from(*entry)),
                _ => None,
            });
            let current = set.zip(entry).map_or(0, |(set, e)| set.rolls[..e].iter().map(|r| r.faces.len()).sum::<usize>() + usize::from(*die));
            rendered.details.insert("dice".to_owned(), serde_json::Value::Array(dice));
            rendered.details.insert("current".to_owned(), current.into());
        }
        CAsk::MunitionsRoll => {
            let misses = dice
                .iter()
                .filter(|d| {
                    let on = d["hits_on"].as_i64();
                    on.is_some_and(|on| d["face"].as_i64().unwrap_or(0) + d["bonus"].as_i64().unwrap_or(0) < on)
                })
                .count();
            rendered.details.insert("kind".to_owned(), "ability_offer".into());
            rendered.details.insert("ability".to_owned(), crate::ties::faction_abilities::ability_card(ti4_content::ContentStore::embedded(), "munitions"));
            rendered.details.insert("cost".to_owned(), serde_json::json!({ "trade_goods": crate::ties::faction_abilities::MUNITIONS_COST, "have": core.players().by_seat(c.actor).map_or(0, |p| i64::from(p.econ.trade_goods.max(0))) }));
            rendered.details.insert("dice".to_owned(), serde_json::Value::Array(dice));
            rendered.details.insert("misses".to_owned(), misses.into());
        }
        _ => {}
    }
}

/// The most unit types one fight may name (the roll group texts are every subset of them).
const MAX_TYPES: usize = 10;

/// The most unit kinds ONE seat may field in a fight that names more than `MAX_TYPES` kinds in all (its subsets are the roll groups named: 2^12 - 1).
const MAX_SIDE_TYPES: usize = 12;

/// The kinds of unit with a space cannon on the board (in a space area or on a planet), each once, sorted.
///
/// Reads every planet cell once. (The scan this replaces, [`space_cannon_kinds_by_systems`], asked `planets_of` of every one of the 231 systems, and
/// that walks all 159 planets each time: about 12 percent of a core step.)
#[must_use]
pub fn space_cannon_kinds(core: &CoreState) -> Vec<Unit> {
    let board = core.board();
    let mut guns: Vec<Unit> = Vec::new();
    let mut note = |units: &ti4_engine_core::state::UnitList| {
        for piece in units.iter() {
            let u = piece.kind();
            if u.data().space_cannon.is_some() && !guns.contains(&u) {
                guns.push(u);
            }
        }
    };
    // (Only the cells that hold a unit at all: the board's index names them.)
    for system in ti4_engine_core::query::board::systems_of(ti4_engine_core::query::board::systems_with_any_units(core)) {
        note(&board.sys(system).units);
    }
    for planet in ti4_engine_core::query::board::planets_of_set(ti4_engine_core::query::board::planets_with_any_units(core)) {
        let cell = board.planet(planet);
        // A planet's units are listed under the system `planets_of` names it for: where it was placed, else its printed system.
        if !cell.units.is_empty() && (cell.placed_in.is_some() || planet.data().system.is_some()) {
            note(&cell.units);
        }
    }
    guns.sort_unstable();
    guns
}

/// The original scan of [`space_cannon_kinds`], system by system through `planets_of_sorted`; kept as the oracle of its test.
#[must_use]
pub fn space_cannon_kinds_by_systems(core: &CoreState) -> Vec<Unit> {
    let mut guns: Vec<Unit> = Vec::new();
    let systems: Vec<_> = core.board().systems().map(|(system, _)| system).collect();
    for system in systems {
        let mut here: Vec<Unit> = core.board().sys(system).units.iter().map(|p| p.kind()).collect();
        for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, system).iter() {
            here.extend(core.board().planet(*planet).units.iter().map(|p| p.kind()));
        }
        for u in here {
            if u.data().space_cannon.is_some() && !guns.contains(&u) {
                guns.push(u);
            }
        }
    }
    guns.sort_unstable();
    guns
}

/// The texts of the space combat at `core`: the fixed words, and for the unit types that can be named in the fight in `active_space_combat` (the
/// types present, with the other form of each dual-form unit) their names and every roll group's. State texts go to the position's table, shown
/// texts to the render table. A fight with more than ten types names none (the machine then stops at a roll it cannot name).
pub fn combat_texts(core: &mut CoreState, r: &Render) -> CombatTexts {
    // ONE table: the state texts and the shown texts are minted into the render's table, which continues the position's, and the position takes
    // it back. Minting the state texts into the position's table and the shown ones into the render's makes two tables that disagree about every
    // code minted after the split; the render's table, which the live loop commits to the position, then names a state code by a shown text (or
    // by none: `unknown text code`, the Argent and Nekro rows of the faction sweep).
    // (A `Render` that does not continue the position's table keeps the two apart, as before: the state texts go to the position.)
    let one = r.sync_codes(&mut core.texts.codes);
    let t = combat_texts_in(core, r, one);
    if one {
        r.sync_codes(&mut core.texts.codes);
    }
    t
}

fn combat_texts_in(core: &mut CoreState, r: &Render, one: bool) -> CombatTexts {
    let state_code = |core: &mut CoreState, text: &str| -> Code {
        if one {
            r.code(text)
        } else {
            Code(u16::try_from(core.texts.codes.intern(text.as_bytes(), ti4_engine_core::state::CODE_CAPACITY).expect("room for the text")).expect("a code fits"))
        }
    };
    let mut t = CombatTexts::default();
    for (slot, text) in t.phases.iter_mut().zip(["pre_roll", "barrage", "resolving_hits", "retreating", "complete"]) {
        *slot = state_code(core, text);
    }
    t.barrage_extra = state_code(core, "extra barrage");
    t.retreat_cause = state_code(core, "retreat_capacity");
    t.barrage_extra_shown = r.code("extra barrage");
    t.barrage_cause = r.code("unit_ability:anti_fighter_barrage");
    t.barrage_kind = r.code("anti_fighter_barrage");
    t.roll_cause = r.code("space_combat");
    t.hacan = r.code("hacan_flagship");
    t.retreat_event = r.code("retreat_capacity");
    t.retreat_pool = r.code("reinforcements");
    t.raid = r.code("raid_formation_damage");
    t.ac_subtype = r.code("assault_cannon_destroy");
    t.ac_cause = r.code("technology:assault_cannon");
    t.ambush = r.code("ambush");
    t.cause_core = r.code("technology:impulse_core");
    t.cause_devotion = r.code("faction_ability:devotion");
    t.casualty = r.code("assign_casualty");
    t.commander = r.code("letnevcommander");
    t.opponents = r.code("opponents");
    for (slot, text) in t.end.iter_mut().zip(["losers", "retreated", "retreat", "draw", "destruction", "round_limit"]) {
        *slot = r.code(text);
    }
    // The gunners of the space cannon offense (the tactical head runs before any combat is open): the offense stages each roll under its unit's
    // type id, as the string does.
    {
        let mut guns = space_cannon_kinds(core);
        for u in guns.drain(..) {
            let id = IdStr::as_str(u);
            t.guns.push((u, state_code(core, id), r.code(id)));
        }
    }
    // The kinds each seat can field in this fight, with the other form of each (`sides`), and all of them together (`present`).
    let mut sides: Vec<(ti4_engine_core::ids::Seat, Vec<Unit>)> = Vec::new();
    let add = |sides: &mut Vec<(ti4_engine_core::ids::Seat, Vec<Unit>)>, owner: ti4_engine_core::ids::Seat, unit: Unit| {
        let at = match sides.iter().position(|(o, _)| *o == owner) {
            Some(at) => at,
            None => {
                sides.push((owner, Vec::new()));
                sides.len() - 1
            }
        };
        let kinds = &mut sides[at].1;
        for u in std::iter::once(unit).chain(ti4_engine_core::query::tech::other_form(unit)) {
            if !kinds.contains(&u) {
                kinds.push(u);
            }
        }
    };
    let mut present: Vec<Unit> = Vec::new();
    let mut pieces: Vec<Unit> = if let Some((system, _, _)) = core.transient().combat.active_space_combat {
        let cell = core.board().sys(system);
        // (The units in the space area are named only where they can fight as ships too: the infantry and mechs a carrier holds roll no die in a space
        // combat, and each kind named, with its other form, doubles the roll groups: Sol carrying infantry beside a Dreadnought II, Super-Dreadnoughts and
        // an L1Z1X flagship went over `MAX_TYPES` with the two infantry forms, which names none: `CombatUnported`, policy seed 7793 rotation 0.)
        // (A dual-form unit in the space area whose other form is a ship takes it at the start of the combat round, "flip to ship forms": the Naaz
        // Eidolon a carrier holds fights as a Z-Grav Eidolon, which the machine then names, `CombatUnported` in the lobby sweep's Ghost table.)
        let mut owned: Vec<(ti4_engine_core::ids::Seat, Unit)> = cell.units.iter().filter(|p| fights_as_a_ship(core, system, p.owner(), p.kind()) || flips_into_a_ship(p.kind())).map(|p| (p.owner(), p.kind())).collect();
        // (The units on the planets are named only where they can fight as ships: a ground force or a structure rolls no die in a space combat, and each kind named
        // doubles the roll groups: a fight whose seats also hold infantry, a PDS and a dock on the planets went over `MAX_TYPES`, which names none: `CombatUnported`.)
        for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, system).iter() {
            owned.extend(core.board().planet(*planet).units.iter().filter(|p| fights_as_a_ship(core, system, p.owner(), p.kind())).map(|p| (p.owner(), p.kind())));
        }
        let mut pieces: Vec<Unit> = owned.iter().map(|(_, u)| *u).collect();
        for (owner, unit) in &owned {
            add(&mut sides, *owner, *unit);
        }
        // A seat that owns an upgrade whose units still stand in the older form takes the new form when the combat opens (`technology::apply_unit_upgrades`):
        // its roll groups are named too, or the machine stands at a roller its texts do not name.
        let milor_in_play = milor_in_play(core);
        // Ipswitch, Loose Cannon (the Mentak hero): "for each other player's ship that is destroyed during this combat, place 1 ship of that type from
        // your reinforcements in the active system". The ships land in the middle of the fight in the form the Mentak seat places: named too, or the
        // machine stands at the roller of a fleet it has no text for (`CombatUnported`, bench 4p seed 76: the hero purged, Ambush's hits, round 1).
        for (seat, unit) in ipswitch_forms(core, system, &owned) {
            pieces.push(unit);
            add(&mut sides, seat, unit);
        }
        for (owner, unit) in owned {
            let Some(p) = core.players().by_seat(owner) else { continue };
            if let Some(upgrade) = ti4_engine_core::query::tech::unlocked_upgrade(ti4_engine_core::tables::Scope(u32::MAX), unit.data().base_type, p.faction, &p.tech.owned) {
                pieces.push(upgrade);
                add(&mut sides, owner, upgrade);
            }
            // A seat that can research a unit upgrade while the combat goes on (Reveal Prototype: "at the start of a combat round, spend 4 resources
            // to research a unit upgrade"; the Nekro's Technological Singularity) may field any form of the base types it has units of: those are named
            // too, or the machine stands at the roller of the form it researched.
            // (`owned` holds the units that can fight as ships: a ground force or a structure that rolls no die in a space combat is not in it, and its forms are not
            // named: a fight that names more than `MAX_TYPES` unit types names none and the machine stood at every roller, `CombatUnported`.)
            if researches_in_combat(core, owner) {
                for u in forms_of(unit.data().base_type, p.faction, false) {
                    pieces.push(u);
                    add(&mut sides, owner, u);
                }
            }
            // Brother Milor (the Yin agent, or a Ssruu that copies it): "after a player's unit is destroyed during combat: you may exhaust this card to
            // allow that player to place 2 fighters in the destroyed unit's system". The fighters land in the middle of the fight, for whichever side
            // lost the ship, in the form that side places: named too, or the machine stands at the roller of the fighters (`CombatUnported`, the Yin
            // game of the 6-player sweep: two Fighter I of the Yin seat in round 2).
            if milor_in_play {
                for u in forms_of(ti4_engine_core::generated::UnitBaseType::Fighter, p.faction, true) {
                    pieces.push(u);
                    add(&mut sides, owner, u);
                }
            }
        }
        pieces
    } else {
        let types = hero_fight_types(core);
        if types.is_empty() {
            return t;
        }
        // (The kinds of each seat are named too: a table of all the pairs together can go over `MAX_TYPES`, past which only the subsets of each seat's own
        // kinds are roll groups, and a side with no kinds named leaves the machine standing at its first roller: `CombatUnported`.)
        for (owner, unit) in &types {
            add(&mut sides, *owner, *unit);
        }
        types.into_iter().map(|(_, unit)| unit).collect()
    };
    for u in pieces.drain(..) {
        if !present.contains(&u) {
            present.push(u);
        }
        if let Some(twin) = ti4_engine_core::query::tech::other_form(u)
            && !present.contains(&twin)
        {
            present.push(twin);
        }
    }
    present.sort_unstable();
    // Every subset of the kinds is a roll group (the units that roll together are one seat's, so the subsets of ONE seat's kinds are the groups
    // that can be named). Up to `MAX_TYPES` kinds in all, the subsets of all of them, in the order the first games minted them (their codes
    // are part of their fingerprints); past it, only the subsets of each seat's own kinds (at most `MAX_SIDE_TYPES` of them), seat by seat.
    let mut group_sets: Vec<Vec<Unit>> = Vec::new();
    if present.len() <= MAX_TYPES {
        group_sets.push(present.clone());
    } else if sides.is_empty() || sides.iter().any(|(_, kinds)| kinds.len() > MAX_SIDE_TYPES) {
        return t;
    } else {
        for (_, mut kinds) in sides {
            kinds.sort_unstable();
            group_sets.push(kinds);
        }
    }
    for u in &present {
        let id = IdStr::as_str(*u);
        t.unit_codes.push((*u, state_code(core, id)));
        t.shown_codes.push((*u, r.code(id)));
    }
    for set in &group_sets {
        for mask in 1_u32..(1 << set.len()) {
            let kinds: Vec<Unit> = set.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
            if t.groups.iter().any(|(k, _, _)| *k == kinds) {
                continue;
            }
            let joined = kinds.iter().map(|u| IdStr::as_str(*u)).collect::<Vec<_>>().join("+");
            t.groups.push((kinds, state_code(core, &joined), r.code(&joined)));
        }
    }
    t
}

/// Whether a unit takes a ship's form at the start of a space combat round when it stands in the space area (`flip_space_area`).
fn flips_into_a_ship(unit: Unit) -> bool {
    !unit.data().ship && ti4_engine_core::query::tech::other_form(unit).is_some_and(|twin| twin.data().ship)
}

/// Whether a unit of `owner` standing on a planet of `system` can fight the system's space combat: a ship (none stands on a planet), a Naaz Eidolon Maximum, or a ground
/// force a Nekro Alastor can choose or has chosen (the choice is a mark of the combat that outlives the flagship): the flagship stands in the space area, or the
/// mark names the unit.
fn fights_as_a_ship(core: &CoreState, system: ti4_engine_core::generated::System, owner: ti4_engine_core::ids::Seat, unit: Unit) -> bool {
    unit.data().ship
        || unit == Unit::EidolonMaximum
        || (unit.data().ground_force
            && (core.board().sys(system).units.iter().any(|p| p.owner() == owner && p.kind() == Unit::TheAlastor)
                || core.marks().tail.iter().any(|(key, _)| matches!(key, ti4_engine_core::keys::MarkKey::NekroAlastor(_, s, o, _, u) if *s == system && *o == owner && *u == unit))))
}

/// Every form of a base type a faction can field (its upgrades and its own faction units, and with `with_plain` the plain unit): what a seat that gains a unit in the middle of a
/// combat may end up with.
fn forms_of(base: ti4_engine_core::generated::UnitBaseType, faction: ti4_engine_core::generated::Faction, with_plain: bool) -> impl Iterator<Item = Unit> {
    let scope = ti4_engine_core::tables::Scope(u32::MAX);
    Unit::ALL.into_iter().filter(move |u| {
        scope.unit(*u).is_some_and(|d| d.base_type == base && d.faction.is_none_or(|tag| ti4_engine_core::query::tech::tag_is(tag, faction)) && match d.required_technology {
                ti4_engine_core::generated::Requirement::Known(_) => true,
                ti4_engine_core::generated::Requirement::None => with_plain,
                ti4_engine_core::generated::Requirement::NotInCorpus => false, // mutants:skip: the one such unit (Combat Transport II) is Ghemina's, a faction nobody seats, so no form of any seat ever differs
            })
    })
}

/// Whether Brother Milor can place fighters in this game: a seat holds the card (readied now, or exhausted and readied again before the next fight; a Ssruu
/// copies it only from a seat that holds it).
fn milor_in_play(core: &CoreState) -> bool {
    use ti4_engine_core::state::LeaderStatus;
    core.players().iter().any(|p| matches!(p.leaders.status.get(&ti4_engine_core::generated::Leader::BrotherMilor), Some(LeaderStatus::Readied | LeaderStatus::Exhausted)))
}

/// The forms the Mentak hero's seat may place in the fight at `system` (`owned`: the units that fight there, by owner): for a seat of the fight that
/// holds Ipswitch unlocked (it may purge it at the start of the combat) or purged for this combat, its forms of the base type of every other seat's ship
/// there (the plain form, a faction form, an upgrade it owns).
fn ipswitch_forms(core: &CoreState, system: ti4_engine_core::generated::System, owned: &[(ti4_engine_core::ids::Seat, Unit)]) -> Vec<(ti4_engine_core::ids::Seat, Unit)> {
    use ti4_engine_core::generated::{Leader, Requirement};
    use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
    use ti4_engine_core::state::LeaderStatus;
    let mut out = Vec::new();
    let recorded = match core.marks().tail.get(&MarkKey::MentakHero(system)) {
        Some(MarkVal::Atom(Atom::Seat(who))) => Some(*who),
        _ => None,
    };
    let mut seats: Vec<ti4_engine_core::ids::Seat> = owned.iter().map(|(s, _)| *s).collect();
    seats.dedup();
    for seat in seats {
        let Some(p) = core.players().by_seat(seat) else { continue };
        if recorded != Some(seat) && p.leaders.status.get(&Leader::IpswitchLooseCannon) != Some(&LeaderStatus::Unlocked) {
            continue;
        }
        let mut bases: Vec<ti4_engine_core::generated::UnitBaseType> = owned.iter().filter(|(s, u)| *s != seat && u.data().ship).map(|(_, u)| u.data().base_type).collect();
        bases.sort_unstable();
        bases.dedup();
        for base in bases {
            for u in forms_of(base, p.faction, true) {
                let fields = match u.data().required_technology {
                    Requirement::Known(t) => p.tech.owned.contains(t),
                    _ => true,
                };
                if fields {
                    out.push((seat, u));
                }
            }
        }
    }
    out
}

/// Whether `seat` can gain a unit upgrade in the middle of a combat: it holds the action card Reveal Prototype, or it is the Nekro (Technological Singularity).
fn researches_in_combat(core: &CoreState, seat: ti4_engine_core::ids::Seat) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.faction == ti4_engine_core::generated::Faction::TheNekroVirus || p.cards.hand.contains(&ti4_engine_core::generated::ActionCard::RevealPrototype))
}

/// The unit types of the space combat Airo Shir Aur (Mahact hero) can start from this position: the active seat holds the unlocked hero, and the fight
/// is at a system next to one of its fleets that holds another seat's ships. No fight is open yet (`active_space_combat` is written when the card moves
/// the fleet), so the texts the machine names its rolls by are minted for the types of every such pair: the ships of the destination and of the fleet.
fn hero_fight_types(core: &CoreState) -> Vec<(ti4_engine_core::ids::Seat, Unit)> {
    use ti4_engine_core::generated::{Leader, UnitBaseType};
    let Some(active) = core.meta().active else { return Vec::new() };
    let unlocked = core.players().by_seat(active).is_some_and(|p| p.leaders.status.get(&Leader::AiroShirAur) == Some(&ti4_engine_core::state::LeaderStatus::Unlocked));
    // (The galaxy is read only when the map slice is loaded: the lockstep harness lends none.)
    if !unlocked || !core.loaded.has(ti4_engine_core::state::Slice::Map) {
        return Vec::new();
    }
    let Some(galaxy) = core.galaxy() else { return Vec::new() };
    // (A dual-form unit in the space area whose other form is a ship fights as that ship, "flip to ship forms": the Naaz Eidolon a carrier holds is a
    // Z-Grav Eidolon in the fight, which the machine names, `CombatUnported` in bench 6p seed 10, the Mahact hero's fight against a Naaz fleet.)
    let ship = |u: Unit| u.data().ship || u.data().base_type == UnitBaseType::Fighter || flips_into_a_ship(u);
    let mut out: Vec<(ti4_engine_core::ids::Seat, Unit)> = Vec::new();
    for (origin, cell) in core.board().systems() {
        let mine: Vec<Unit> = cell.units.iter().filter(|p| p.owner() == active && ship(p.kind())).map(|p| p.kind()).collect();
        if mine.is_empty() {
            continue;
        }
        for index in galaxy.adjacent(origin).iter() {
            let Some(to) = ti4_engine_core::generated::System::ALL.get(index).copied() else { continue };
            let mut there: Vec<(ti4_engine_core::ids::Seat, Unit)> = core.board().sys(to).units.iter().filter(|p| ship(p.kind())).map(|p| (p.owner(), p.kind())).collect();
            // (The units that stand on the planets of the destination and fight its space combat as ships too: a Naaz Eidolon Maximum, the ground forces a Nekro
            // Alastor in the space area can choose. They roll in the fight, so the machine names their groups, or it stands at their roller: `CombatUnported`.)
            for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, to).iter() {
                there.extend(core.board().planet(*planet).units.iter().filter(|p| fights_as_a_ship(core, to, p.owner(), p.kind())).map(|p| (p.owner(), p.kind())));
            }
            if there.iter().any(|(owner, _)| *owner != active) {
                out.extend(mine.iter().map(|u| (active, *u)));
                out.extend(there.iter().copied());
            }
        }
    }
    out
}

