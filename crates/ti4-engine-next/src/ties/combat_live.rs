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
        for u in guns {
            let id = IdStr::as_str(u);
            t.guns.push((u, state_code(core, id), r.code(id)));
        }
    }
    let mut present: Vec<Unit> = Vec::new();
    let mut pieces: Vec<Unit> = if let Some((system, _, _)) = core.transient().combat.active_space_combat {
        let cell = core.board().sys(system);
        let mut pieces: Vec<Unit> = cell.units.iter().map(|p| p.kind()).collect();
        // (The units on the planets are named only where they can fight as ships: a ground force or a structure rolls no die in a space combat, and each kind named
        // doubles the roll groups: a fight whose seats also hold infantry, a PDS and a dock on the planets went over `MAX_TYPES`, which names none: `CombatUnported`.)
        for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, system).iter() {
            pieces.extend(core.board().planet(*planet).units.iter().filter(|p| fights_as_a_ship(core, p.owner(), p.kind())).map(|p| p.kind()));
        }
        pieces
    } else {
        let types = hero_fight_types(core);
        if types.is_empty() {
            return t;
        }
        types
    };
    // A seat that owns an upgrade whose units still stand in the older form takes the new form when the combat opens (`technology::apply_unit_upgrades`):
    // its roll groups are named too, or the machine stands at a roller its texts do not name.
    if let Some((system, _, _)) = core.transient().combat.active_space_combat {
        let mut owned: Vec<(ti4_engine_core::ids::Seat, Unit)> = core.board().sys(system).units.iter().map(|p| (p.owner(), p.kind())).collect();
        for planet in ti4_engine_core::rules::combat::planets_of_sorted(core, system).iter() {
            owned.extend(core.board().planet(*planet).units.iter().filter(|p| fights_as_a_ship(core, p.owner(), p.kind())).map(|p| (p.owner(), p.kind())));
        }
        let milor_in_play = milor_in_play(core);
        for (owner, unit) in owned {
            let Some(p) = core.players().by_seat(owner) else { continue };
            if let Some(upgrade) = ti4_engine_core::query::tech::unlocked_upgrade(ti4_engine_core::tables::Scope(u32::MAX), unit.data().base_type, p.faction, &p.tech.owned) {
                pieces.push(upgrade);
            }
            // A seat that can research a unit upgrade while the combat goes on (Reveal Prototype: "at the start of a combat round, spend 4 resources
            // to research a unit upgrade"; the Nekro's Technological Singularity) may field any form of the base types it has units of: those are named
            // too, or the machine stands at the roller of the form it researched.
            // (`owned` holds the units that can fight as ships: a ground force or a structure that rolls no die in a space combat is not in it, and its forms are not
            // named: a fight that names more than `MAX_TYPES` unit types names none and the machine stood at every roller, `CombatUnported`.)
            if researches_in_combat(core, owner) {
                pieces.extend(forms_of(unit.data().base_type, p.faction, false));
            }
            // Brother Milor (the Yin agent, or a Ssruu that copies it): "after a player's unit is destroyed during combat: you may exhaust this card to
            // allow that player to place 2 fighters in the destroyed unit's system". The fighters land in the middle of the fight, for whichever side
            // lost the ship, in the form that side places: named too, or the machine stands at the roller of the fighters (`CombatUnported`, the Yin
            // game of the 6-player sweep: two Fighter I of the Yin seat in round 2).
            if milor_in_play {
                pieces.extend(forms_of(ti4_engine_core::generated::UnitBaseType::Fighter, p.faction, true));
            }
        }
    }
    for u in pieces {
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
    if present.len() > MAX_TYPES {
        return t;
    }
    for u in &present {
        let id = IdStr::as_str(*u);
        t.unit_codes.push((*u, state_code(core, id)));
        t.shown_codes.push((*u, r.code(id)));
    }
    for mask in 1_u32..(1 << present.len()) {
        let kinds: Vec<Unit> = present.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, u)| *u).collect();
        let joined = kinds.iter().map(|u| IdStr::as_str(*u)).collect::<Vec<_>>().join("+");
        t.groups.push((kinds, state_code(core, &joined), r.code(&joined)));
    }
    t
}

/// Whether a unit of `owner` standing on a planet of `system` can fight the system's space combat: a ship (none stands on a planet), a Naaz Eidolon Maximum, or a ground
/// force of the Nekro, whose flagship, the Alastor, chooses ground forces that fight as ships (the choice is a mark of the combat that outlives the flagship, so the
/// Nekro's ground forces are named whether or not the Alastor still stands).
fn fights_as_a_ship(core: &CoreState, owner: ti4_engine_core::ids::Seat, unit: Unit) -> bool {
    unit.data().ship
        || unit == Unit::EidolonMaximum
        || (unit.data().ground_force && core.players().by_seat(owner).is_some_and(|p| p.faction == ti4_engine_core::generated::Faction::TheNekroVirus))
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

/// Whether `seat` can gain a unit upgrade in the middle of a combat: it holds the action card Reveal Prototype, or it is the Nekro (Technological Singularity).
fn researches_in_combat(core: &CoreState, seat: ti4_engine_core::ids::Seat) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.faction == ti4_engine_core::generated::Faction::TheNekroVirus || p.cards.hand.contains(&ti4_engine_core::generated::ActionCard::RevealPrototype))
}

/// The unit types of the space combat Airo Shir Aur (Mahact hero) can start from this position: the active seat holds the unlocked hero, and the fight
/// is at a system next to one of its fleets that holds another seat's ships. No fight is open yet (`active_space_combat` is written when the card moves
/// the fleet), so the texts the machine names its rolls by are minted for the types of every such pair: the ships of the destination and of the fleet.
fn hero_fight_types(core: &CoreState) -> Vec<Unit> {
    use ti4_engine_core::generated::{Leader, UnitBaseType};
    let Some(active) = core.meta().active else { return Vec::new() };
    let unlocked = core.players().by_seat(active).is_some_and(|p| p.leaders.status.get(&Leader::AiroShirAur) == Some(&ti4_engine_core::state::LeaderStatus::Unlocked));
    // (The galaxy is read only when the map slice is loaded: the lockstep harness lends none.)
    if !unlocked || !core.loaded.has(ti4_engine_core::state::Slice::Map) {
        return Vec::new();
    }
    let Some(galaxy) = core.galaxy() else { return Vec::new() };
    let ship = |u: Unit| u.data().ship || u.data().base_type == UnitBaseType::Fighter;
    let mut out: Vec<Unit> = Vec::new();
    for (origin, cell) in core.board().systems() {
        let mine: Vec<Unit> = cell.units.iter().filter(|p| p.owner() == active && ship(p.kind())).map(|p| p.kind()).collect();
        if mine.is_empty() {
            continue;
        }
        for index in galaxy.adjacent(origin).iter() {
            let Some(to) = ti4_engine_core::generated::System::ALL.get(index).copied() else { continue };
            let there: Vec<(ti4_engine_core::ids::Seat, Unit)> = core.board().sys(to).units.iter().filter(|p| ship(p.kind())).map(|p| (p.owner(), p.kind())).collect();
            if there.iter().any(|(owner, _)| *owner != active) {
                out.extend(mine.iter().copied());
                out.extend(there.iter().map(|(_, u)| *u));
            }
        }
    }
    out
}

