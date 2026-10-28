//! The offline dual of `Area::ExplorationRelics`: every query of [`crate::explore_core::CardsView`]
//! asked of one position (and of deterministic variants of it that hand out relics, fragments, notes,
//! legendary planets and a salvage yard, which the golden games rarely hold) next to the string
//! function it replaces, every answer compared. Used by `board_equivalence cards` and the tests.

use ti4_content::ContentStore;
use ti4_engine_core::generated::{Faction, Planet, PromissoryNote, Relic, System};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{ActionCardId, PlanetId, PlayerId, RelicId, SystemId};
use ti4_model::state::GameState;

use crate::core_sync::keys::text::note_text;
use crate::explore_core::CardsView;
use crate::generated::IdStr;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct CardsCheck {
    pub positions: u64,
    pub variants: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub mismatches: Vec<String>,
    /// The first distinct reasons a position was unmapped.
    pub reasons: Vec<String>,
}

impl CardsCheck {
    fn miss(&mut self, m: String) {
        if self.mismatches.len() < 60 {
            self.mismatches.push(m);
        }
    }

    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, label: &str, what: &str, string: &T, typed: &Option<T>) {
        self.compared += 1;
        match typed {
            Some(t) if t == string => {}
            other => self.miss(format!("{label}: {what}: string {string:?} vs typed {other:?}")),
        }
    }
}

fn mix(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A deterministic variant of the position: variant 0 is the position itself.
fn vary(gs: &GameState, sources: SourceSet, variant: u64) -> GameState {
    let mut s = gs.clone();
    if variant == 0 {
        return s;
    }
    let content = ContentStore::embedded();
    let ids: Vec<PlayerId> = s.players.iter().map(|p| p.id.clone()).collect();
    let n = ids.len().max(1);
    let r = |k: u64| mix(variant.wrapping_mul(1_000_003).wrapping_add(k));
    s.rules_version = [s.rules_version, 0, 15, 16][(r(1) % 4) as usize];
    for (i, id) in ids.iter().enumerate() {
        let i = i as u64;
        let Some(seat) = s.player_mut(id) else { continue };
        seat.relics.clear();
        seat.exhausted_relics.clear();
        for k in 0..(r(10 + i) % 4) {
            let relic = Relic::ALL[(r(20 + i * 7 + k) % Relic::COUNT as u64) as usize];
            let rid = RelicId::new(relic.as_str());
            if !seat.relics.contains(&rid) {
                if r(30 + i + k) % 3 == 0 {
                    seat.exhausted_relics.insert(rid.clone());
                }
                seat.relics.push(rid);
            }
        }
        seat.relic_fragments.clear();
        for (j, key) in ["CULTURAL", "FRONTIER", "HAZARDOUS", "INDUSTRIAL"].into_iter().enumerate() {
            let count = (r(40 + i * 5 + j as u64) % 5) as i32;
            if r(50 + i * 5 + j as u64) % 4 != 0 {
                seat.relic_fragments.insert(key.to_owned(), count);
            }
        }
        seat.exhausted_legendary.clear();
        for planet in ["tempesta", "garbozia"] {
            if r(60 + i) % 3 == 0 {
                seat.exhausted_legendary.insert(PlanetId::new(planet));
            }
        }
    }
    // Promissory notes: hand the existing ones around, add foreign copies of the movable ones.
    let factions: Vec<String> = s.players.iter().map(|p| p.faction.as_str().to_owned()).collect();
    for (f, faction) in factions.iter().enumerate() {
        for alias in ["convoys", "blood_pact", "dark_pact", "pop", "shareknowledge", "sever", "an", "cf", "ta"] {
            if r(100 + f as u64 * 11 + alias.len() as u64) % 2 == 0 {
                let holder = ids[(r(200 + f as u64) as usize) % n].clone();
                s.promissory_notes.insert(format!("{alias}:{faction}"), holder);
            }
        }
    }
    let keys: Vec<String> = s.promissory_notes.keys().cloned().collect();
    for (j, key) in keys.iter().enumerate() {
        let j = j as u64;
        if r(300 + j) % 3 == 0 {
            s.promissory_notes.insert(key.clone(), ids[(r(400 + j) as usize) % n].clone());
        }
        if r(500 + j) % 3 == 0 {
            s.promissory_faceup.insert(key.clone());
        }
    }
    // Control of the two legendary planets with a card of their own, and a salvage yard.
    for (k, planet) in ["tempesta", "garbozia"].into_iter().enumerate() {
        let pid = PlanetId::new(planet);
        let Some(system) = crate::planets::system_of(&s, content, sources, planet) else { continue };
        for state in s.board.values_mut() {
            state.planet_control.remove(&pid);
        }
        if r(600 + k as u64) % 4 != 0 {
            let owner = ids[(r(700 + k as u64) as usize) % n].clone();
            s.board.entry(system).or_default().planet_control.insert(pid, owner);
        }
    }
    s.salvage_yard = (0..(r(800) % 3)).map(|k| ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[(r(900 + k) % 40) as usize].as_str())).collect();
    s
}

/// Compare every query on one position and its variants.
pub fn compare(label: &str, gs: &GameState, sources: SourceSet, variants: u64, out: &mut CardsCheck) {
    out.positions += 1;
    for v in 0..variants.max(1) {
        let state = vary(gs, sources, v);
        compare_one(&format!("{label}#{v}"), &state, sources, out);
    }
}

fn compare_one(label: &str, gs: &GameState, sources: SourceSet, out: &mut CardsCheck) {
    use crate::{exploration, fracture, legendary, promissory, relics};
    out.variants += 1;
    let content = ContentStore::embedded();
    let v = match CardsView::try_of(gs) {
        Ok(v) => v,
        Err(e) => {
            out.unmapped += 1;
            if out.reasons.len() < 8 && !out.reasons.contains(&e.to_string()) {
                out.reasons.push(e.to_string());
            }
            return;
        }
    };
    let players: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    for p in &players {
        for r in Relic::ALL {
            let rid = RelicId::new(r.as_str());
            out.eq(label, &format!("holds {p} {rid}"), &relics::holds_text(gs, p, &rid), &v.holds(p, &rid));
            out.eq(label, &format!("ready {p} {rid}"), &relics::ready_text(gs, p, rid.as_str()), &v.ready(p, rid.as_str()));
        }
        out.eq(label, "triad_value", &relics::triad_value_text(gs, p), &v.triad_value(p));
        out.eq(label, "substitutes_strategy_token", &relics::substitutes_strategy_token_text(gs, p), &v.substitutes_strategy_token(p));
        out.eq(label, "grants_sustain", &relics::grants_sustain_text(gs, p), &v.grants_sustain(p));
        out.eq(label, "extra_barrage", &relics::extra_barrage_text(gs, p), &v.extra_barrage(p));
        out.eq(label, "space_dock_cannon", &relics::space_dock_cannon_text(gs, p), &v.space_dock_cannon(p));
        out.eq(label, "prerequisite_waivers", &relics::prerequisite_waivers_text(gs, p), &v.prerequisite_waivers(p));
        out.eq(label, "secret_objective_bonus", &relics::secret_objective_bonus_text(gs, p), &v.secret_objective_bonus(p));
        out.eq(label, "ignores_gravity_rifts", &relics::ignores_gravity_rifts_text(gs, p), &v.ignores_gravity_rifts(p));
        out.eq(label, "purgeable", &exploration::purgeable_text(gs, p), &v.purgeable(p));
        for planet in ["tempesta", "garbozia", "mallice", "primor"] {
            let pid = PlanetId::new(planet);
            out.eq(label, &format!("legendary {planet}"), &legendary::available_text(gs, p, &pid), &v.legendary_available(p, &pid));
        }
        out.eq(label, "ionian_available", &legendary::ionian_available_text(gs, p), &v.ionian_available(p));
        out.eq(label, "salvaged", &legendary::salvaged_text(gs, p), &v.salvaged(p));
        out.eq(label, "held_by", &promissory::held_by_text(gs, p), &v.held_by(p));
        out.eq(label, "action_notes_in_hand", &promissory::action_notes_in_hand_text(gs, p), &v.action_notes_in_hand(p));
        out.eq(label, "convoys_in_hand", &promissory::convoys_in_hand_text(gs, p), &v.convoys_in_hand(p));
        out.eq(label, "reaches_anyone", &promissory::reaches_anyone_text(gs, p), &v.reaches_anyone(p));
        for note in PromissoryNote::ALL {
            let alias = note_text(note);
            out.eq(label, &format!("holder_of {alias} {p}"), &promissory::holder_of_text(gs, alias, p), &v.holder_of(alias, p));
            out.eq(label, &format!("held_foreign {alias} {p}"), &promissory::held_foreign_text(gs, p, alias), &v.held_foreign(p, alias));
            out.eq(label, &format!("may_receive {alias} {p}"), &promissory::may_receive_text(gs, p, alias), &v.may_receive(p, alias));
        }
        for system in gs.board.keys().chain(std::iter::once(&SystemId::new(System::ALL[0].as_str()))) {
            out.eq(
                label,
                &format!("denies_movement_into {p} {system}"),
                &promissory::denies_movement_into_text(gs, p, system),
                &v.denies_movement_into(p, system),
            );
        }
    }
    // Printed data: independent of the position, compared once per variant (cheap).
    for planet in Planet::ALL {
        let pid = PlanetId::new(planet.as_str());
        out.eq(label, &format!("traits_of {pid}"), &exploration::traits_of_text(content, sources, &pid), &crate::explore_core::traits_of(content, sources, &pid));
    }
    for card in ti4_engine_core::generated::Explore::ALL {
        let id = card.as_str();
        out.eq(label, &format!("resolution {id}"), &exploration::resolution_text(content, id), &crate::explore_core::resolution(content, id));
        for planet in Planet::ALL.iter().step_by(7) {
            let pid = PlanetId::new(planet.as_str());
            let planet = pid.as_str();
            out.eq(label, &format!("attachment_for {id} {planet}"), &exploration::attachment_for_text(content, id, &pid), &crate::explore_core::attachment_for(content, id, &pid));
        }
    }
    for system in System::ALL {
        let sid = SystemId::new(system.as_str());
        out.eq(label, &format!("is_fracture_system {sid}"), &fracture::is_fracture_system_text(content, sources, &sid), &crate::explore_core::is_fracture_system(content, sources, &sid));
    }
    out.eq(label, "fracture::systems", &fracture::systems_text(content, sources), &crate::explore_core::fracture_systems(content, sources));
    out.eq(label, "fracture::egress_systems", &fracture::egress_systems_text(content, sources), &crate::explore_core::egress_systems(content, sources));
    for note in PromissoryNote::ALL {
        for faction in [Faction::ALL[0], Faction::ALL[1], Faction::TheMahactGeneSorcerers, Faction::ALL[5]] {
            let text = format!("{}:{}", note_text(note), faction.as_str());
            out.eq(label, &format!("is_play_area {text}"), &promissory::is_play_area_text(content, &text), &crate::explore_core::is_play_area(content, &text));
        }
    }
    // The lent view answers the string wrappers on the Core backend as well.
    let shared = std::rc::Rc::new(v);
    crate::explore_core::scoped(Some(&shared), || {
        crate::dual::with_mask(crate::dual::AreaMask::all(crate::dual::Backend::Core), || {
            for p in &players {
                let held = promissory::held_by(gs, p);
                out.eq(label, "scoped held_by", &promissory::held_by_text(gs, p), &Some(held));
                let triad = relics::triad_value(gs, p);
                out.eq(label, "scoped triad_value", &relics::triad_value_text(gs, p), &Some(triad));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::game;
    use crate::{dual, relics};
    use ti4_model::content_types::POK;

    /// The fixture game with real factions (the typed state cannot hold the scaffolding `generic`).
    fn real_game() -> GameState {
        let mut state = game(&["a", "b", "c"]);
        for (seat, faction) in state.players.iter_mut().zip(["hacan", "mentak", "mahact"]) {
            seat.faction = ti4_model::id::FactionId::new(faction);
        }
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        crate::promissory::deal(&mut state, ContentStore::embedded(), POK);
        state
    }

    #[test]
    fn the_typed_reads_equal_the_string_reads_on_a_fresh_game_and_its_variants() {
        let state = real_game();
        let mut out = CardsCheck::default();
        compare("fresh", &state, POK, 12, &mut out);
        assert!(out.mismatches.is_empty(), "{:?}", out.mismatches);
        assert_eq!(out.unmapped, 0, "{:?}", out.reasons);
        assert!(out.compared > 5_000, "{}", out.compared);
    }

    #[test]
    fn a_lent_view_answers_only_for_the_state_it_was_built_from() {
        let mut state = real_game();
        let me = PlayerId::new("a");
        state.player_mut(&me).unwrap().relics.push(RelicId::new("thetriad"));
        let view = CardsView::build(&state).expect("expressible");
        let other = state.clone();
        crate::explore_core::scoped(Some(&view), || {
            dual::with_mask(dual::AreaMask::all(dual::Backend::Core), || {
                assert_eq!(crate::explore_core::with(&state, |v| v.triad_value(&me)), Some(Some(3)));
                // Another state (even an equal one) is not the lent position: no typed answer on Core.
                assert_eq!(crate::explore_core::with(&other, |v| v.triad_value(&me)), None);
                assert_eq!(relics::triad_value(&other, &me), Some(3));
            });
        });
    }
}
