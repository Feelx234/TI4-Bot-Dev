//! The offline dual of `Area::BoardQueries`: every query of [`crate::board_view::BoardView`] asked
//! of one position next to the string function it replaces, every answer compared. Used by
//! `board_equivalence` (golden positions, templates) and the tests.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId, SystemId, UnitTypeId};
use ti4_model::state::GameState;

use crate::board_view::BoardView;
use crate::production::Spend;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct BoardCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub law_variants: u64,
    pub mismatches: Vec<String>,
}

impl BoardCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        self.law_variants += o.law_variants;
        for m in &o.mismatches {
            self.miss(m.clone());
        }
    }

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

const BASES: [&str; 11] = [
    "carrier", "cruiser", "destroyer", "dreadnought", "fighter", "flagship", "infantry", "mech", "pds", "spacedock",
    "warsun",
];

/// Compare every query on one position.
pub fn compare(label: &str, gs: &GameState, galaxy: Option<&Galaxy>, sources: SourceSet, out: &mut BoardCheck) {
    use ti4_engine_core::generated::{Agenda, Planet, System, Unit};
    use crate::generated::IdStr;
    out.positions += 1;
    let content = ContentStore::embedded();
    let Some(v) = BoardView::of(gs, galaxy, sources) else {
        out.unmapped += 1;
        return;
    };
    let players: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    let planet_ids: Vec<PlanetId> = Planet::ALL.iter().map(|p| PlanetId::new(p.as_str())).collect();
    let system_ids: Vec<SystemId> = System::ALL.iter().map(|s| SystemId::new(s.as_str())).collect();

    for p in &players {
        let want: Vec<(SystemId, PlanetId)> = gs.controlled_planets(p).into_iter().map(|(s, q)| (s.clone(), q.clone())).collect();
        out.eq(label, "controlled_planets", &want, &v.controlled_planets(p));
        out.eq(label, "systems_with_token", &gs.systems_with_token(p).into_iter().cloned().collect::<BTreeSet<_>>(), &v.systems_with_token(p));
        out.eq(label, "systems_with_units_of", &gs.systems_with_units_of(p).into_iter().cloned().collect::<BTreeSet<_>>(), &v.systems_with_units_of(p));
        let spendable: Vec<PlanetId> = want.iter().map(|(_, q)| q.clone()).filter(|q| !gs.exhausted_planets.contains(q)).collect();
        out.eq(label, "spendable_planets", &spendable, &v.spendable_planets(p));
        out.eq(label, "tokens_in_reinforcements", &gs.tokens_in_reinforcements(p), &v.tokens_in_reinforcements(p, gs.tokens_held_by_mahact(p)));
        out.eq(label, "clockwise_from", &gs.clockwise_from(p), &v.clockwise_from(p));
        for base in BASES {
            let held = crate::supply::held(gs, content, sources, p, base) - crate::factions::crimson_cards::held_on_card(gs, content, sources, p, base);
            out.eq(label, &format!("held_on_board {base}"), &held, &v.held_on_board(p, base));
        }
        for u in Unit::ALL {
            let id = UnitTypeId::new(u.as_str());
            let on_card = crate::factions::crimson_cards::held_on_card(gs, content, sources, p, &crate::supply::base_type_of(content, sources, &id));
            let want = crate::supply::remaining(gs, content, sources, p, &id);
            out.eq(label, &format!("remaining {}", u.as_str()), &want, &v.remaining(p, &id, on_card));
        }
        out.eq(label, "research_team_waivers", &(0..4).map(|i| crate::laws::research_team_waivers(gs, p, ["BIOTIC", "CYBERNETIC", "PROPULSION", "WARFARE"][i])).collect::<Vec<_>>(),
            &(0..4).map(|i| v.research_team_waivers(p, ["BIOTIC", "CYBERNETIC", "PROPULSION", "WARFARE"][i])).collect::<Option<Vec<_>>>());
        // Fleet standing in every system the seat has units in, plain and with arrivals.
        for sys in gs.systems_with_units_of(p) {
            let half = |t: &str| crate::factions::hooks_combat::fighter_fleet_weight_halves(gs, content, p, t);
            let limit = if crate::fleet::is_unlimited(gs, p) { crate::fleet::UNLIMITED_FLEET_BILL } else { i64::from(crate::fleet::limit(gs, content, p)).max(0) };
            for arrival in [None, Some(("carrier", 1, true)), Some(("fighter", 3, true)), Some(("infantry", 4, true)), Some(("spacedock", 1, false)), Some(("fighter2", 2, true)), Some(("dreadnought", 2, true))] {
                let types = ti4_content::units::catalogue(content, sources);
                let arriving = arrival.and_then(|(t, count, in_space)| types.get(t).map(|kind| crate::fleet::Arrival { kind: *kind, count, in_space }));
                if arrival.is_some() && arriving.is_none() {
                    continue;
                }
                let want = crate::fleet::standing(gs, content, sources, p, sys, arriving);
                let got = v.standing(p, sys, limit, arrival, &half);
                out.eq(label, &format!("standing {sys:?} {arrival:?}"), &want, &got);
            }
        }
    }
    out.eq(label, "initiative_order", &gs.initiative_order(), &v.initiative_order());

    for pid in &planet_ids {
        let want = gs.board.values().find_map(|s| s.planet_control.get(pid)).cloned();
        out.eq(label, &format!("control {pid}"), &want, &v.control(pid));
        out.eq(label, &format!("system_of {pid}"), &crate::planets::system_of(gs, content, sources, pid.as_str()), &v.system_of(pid.as_str()));
        for kind in [Spend::Resources, Spend::Influence] {
            let extra = crate::factions::bastion_units::resource_bonus(gs, pid) * i64::from(matches!(kind, Spend::Resources));
            out.eq(label, &format!("planet_value_now {pid} {}", matches!(kind, Spend::Resources)), &crate::production::planet_value_now(gs, content, sources, pid, kind), &v.planet_value_now(pid, kind, extra));
        }
        out.eq(label, &format!("bonus_sources {pid}"), &crate::laws::planet_value_bonus_sources(gs, pid), &v.planet_value_bonus_sources(pid));
        out.eq(label, &format!("demilitarized {pid}"), &crate::laws::planet_is_demilitarized(gs, pid), &v.planet_is_demilitarized(pid));
        out.eq(label, &format!("production_forbidden {pid}"), &crate::laws::production_forbidden_on(gs, pid), &v.production_forbidden_on(pid));
        out.eq(label, &format!("bombardment_forbidden {pid}"), &crate::laws::bombardment_forbidden(gs, content, sources, pid), &v.bombardment_forbidden(pid));
        // Coexistence: the planet's own system and a foreign one.
        let home = crate::planets::system_of(gs, content, sources, pid.as_str());
        for sys in home.iter().chain(system_ids.first()) {
            out.eq(label, &format!("coexisters {pid}@{sys}"), &crate::coexistence::coexisters(gs, sys, pid), &v.coexisters(sys, pid));
            out.eq(label, &format!("in_coexistence {pid}@{sys}"), &crate::coexistence::in_coexistence(gs, sys, pid), &v.in_coexistence(sys, pid));
            for p in &players {
                out.eq(label, &format!("controls_for_scoring {pid}@{sys} {p}"), &crate::coexistence::controls_for_scoring(gs, sys, pid, p), &v.controls_for_scoring(sys, pid, p));
            }
        }
    }
    for sys in &system_ids {
        out.eq(label, &format!("planets_in {sys}"), &crate::planets::in_system(gs, content, sources, sys), &v.planets_in(sys));
        if let Some(g) = galaxy {
            let want: BTreeSet<SystemId> = g.adjacent(sys.as_str()).into_iter().map(SystemId::new).collect();
            out.eq(label, &format!("neighbours {sys}"), &want, &v.neighbours(sys));
        }
    }
    check_laws(label, gs, &v, out);
    let _ = Agenda::ALL;
}

fn check_laws(label: &str, gs: &GameState, v: &BoardView, out: &mut BoardCheck) {
    use ti4_engine_core::generated::Agenda;
    use crate::generated::IdStr;
    out.eq(label, "laws_in_play", &crate::laws::in_play(gs), &Some(v.laws_in_play()));
    for a in Agenda::ALL {
        let alias = a.as_str();
        out.eq(label, &format!("active {alias}"), &crate::laws::active(gs, alias), &v.law_active(alias));
        for p in gs.players.iter().map(|p| &p.id) {
            out.eq(label, &format!("holds_ministry {alias} {p}"), &crate::laws::holds_ministry(gs, p, alias), &v.holds_ministry(p, alias));
        }
    }
    out.eq(label, "fleet_pool_cap", &crate::laws::fleet_pool_cap(gs, 7), &Some(v.fleet_pool_cap(7)));
    out.eq(label, "action_card_limit", &crate::laws::action_card_limit(gs, 7), &Some(v.action_card_limit(7)));
}

/// Enact every law with a few outcomes on top of `gs` (and blank them once) and compare the law
/// queries: the golden games rarely hold every law.
pub fn compare_laws(label: &str, gs: &GameState, galaxy: Option<&Galaxy>, sources: SourceSet, out: &mut BoardCheck) {
    use ti4_engine_core::generated::{Agenda, Planet};
    use crate::generated::IdStr;
    let player = gs.players.first().map(|p| p.id.to_string());
    let controlled = gs.players.iter().find_map(|p| gs.controlled_planets(&p.id).first().map(|(_, q)| q.to_string()));
    for a in Agenda::ALL {
        for outcome in [Some("for".to_owned()), player.clone(), controlled.clone(), Some(Planet::ALL[0].as_str().to_owned())].into_iter().flatten() {
            for blank in [false, true] {
                let mut g = gs.clone();
                g.laws.insert(a.as_str().to_owned(), outcome.clone());
                // Cover the other law helpers too: a second law alongside.
                if blank {
                    g.phase = ti4_model::state::Phase::Action;
                    g.faction_marks.insert("keleres|laws_blank".to_owned(), g.turn_seq.to_string());
                }
                out.law_variants += 1;
                let Some(v) = BoardView::of(&g, galaxy, sources) else {
                    out.unmapped += 1;
                    continue;
                };
                check_laws(&format!("{label}+{}={outcome}{}", a.as_str(), if blank { " blank" } else { "" }), &g, &v, out);
                for pid in controlled.iter().chain(std::iter::once(&Planet::ALL[0].as_str().to_owned())) {
                    let pid = PlanetId::new(pid.clone());
                    out.eq(label, &format!("demilitarized {pid}"), &crate::laws::planet_is_demilitarized(&g, &pid), &v.planet_is_demilitarized(&pid));
                    out.eq(label, "bonus_sources", &crate::laws::planet_value_bonus_sources(&g, &pid), &v.planet_value_bonus_sources(&pid));
                    out.eq(label, "production_forbidden", &crate::laws::production_forbidden_on(&g, &pid), &v.production_forbidden_on(&pid));
                    out.eq(label, "bombardment_forbidden", &crate::laws::bombardment_forbidden(&g, ti4_content::ContentStore::embedded(), sources, &pid), &v.bombardment_forbidden(&pid));
                    for p in g.players.iter().map(|p| &p.id) {
                        out.eq(label, "research_team_waivers", &crate::laws::research_team_waivers(&g, p, "BIOTIC"), &v.research_team_waivers(p, "BIOTIC"));
                        out.eq(label, "research_team_waivers", &crate::laws::research_team_waivers(&g, p, "WARFARE"), &v.research_team_waivers(p, "WARFARE"));
                    }
                }
            }
        }
    }
}
