//! Equivalence of the typed technology queries (package A3) with the string engine: for every
//! `(position, seat, technology)` the same question asked of the string implementation, of the
//! core over a lent `CoreView` (the shape the flipped engine will use) and of the adapter's
//! `Core` path (typed tables over the string state), all compared.

use std::collections::BTreeSet;

use ti4_content::ContentStore;
use ti4_engine::technology_core::{self as tc, StringHooks};
use ti4_engine::technology as st;
use ti4_engine_core::generated::{Faction, Technology, UnitBaseType};
use ti4_engine_core::query::tech::{self as q};
use ti4_model::id::{PlayerId, TechnologyId};
use ti4_engine::generated::IdStr;

use crate::workload::Position;

#[derive(Debug, Default, Clone)]
pub struct TechReport {
    pub positions: usize,
    pub seats: usize,
    /// Per-technology answers compared (`can_research`, prerequisites with and without AIDA).
    pub tech_answers: usize,
    /// Per-seat answers compared (`researchable` lists, colours, specialties, synergy, unit forms, scars).
    pub seat_answers: usize,
    /// Printed-data answers compared (colour, upgrade, faction, replaced, prerequisites, deck).
    pub content_answers: usize,
    pub unmapped: usize,
    pub mismatches: Vec<String>,
}

impl TechReport {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.seats += o.seats;
        self.tech_answers += o.tech_answers;
        self.seat_answers += o.seat_answers;
        self.content_answers += o.content_answers;
        self.unmapped += o.unmapped;
        self.mismatches.extend(o.mismatches.iter().cloned());
    }

    fn miss(&mut self, what: String) {
        if self.mismatches.len() < 50 {
            self.mismatches.push(what);
        }
    }

    fn same<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: &T, other: &T, counter: u8) {
        match counter {
            0 => self.tech_answers += 1,
            1 => self.seat_answers += 1,
            _ => self.content_answers += 1,
        }
        if string != other {
            self.miss(format!("{what}: string {string:?} vs typed {other:?}"));
        }
    }
}

fn alias(t: Technology) -> TechnologyId {
    TechnologyId::new(<Technology as IdStr>::as_str(t))
}

/// The printed-data queries over the whole corpus, once (every technology, every faction, every
/// source combination).
pub fn check_tech_content(report: &mut TechReport) {
    let content = ContentStore::embedded();
    for t in Technology::ALL {
        let a = alias(t);
        report.same(&format!("colour_type {a}"), &st::colour_type_text(content, &a), &Some(q::colour_type(t)).map(|c| c.map(tc::colour_name)).flatten(), 2);
        report.same(&format!("is_unit_upgrade {a}"), &st::is_unit_upgrade_text(content, &a), &q::is_unit_upgrade(t), 2);
        report.same(
            &format!("faction_of {a}"),
            &st::faction_of_text(content, &a),
            &q::faction_of(t).map(<ti4_engine_core::generated::FactionTag as IdStr>::as_str),
            2,
        );
        report.same(&format!("prerequisites {a}"), &st::prerequisites_text(content, &a), &tc::colour_map(q::prerequisites(t)), 2);
        for f in Faction::ALL {
            let fid = <Faction as IdStr>::as_str(f);
            report.same(&format!("replaced_for_faction {fid} {a}"), &st::replaced_for_faction_text(content, fid, &a), &q::replaced_for_faction(f, t), 2);
        }
    }
    // The deck, for every combination of sources.
    use ti4_model::content_types::Source as S;
    for mask in 0u32..128 {
        let sources: ti4_model::content_types::SourceSet = [S::Base, S::Codex1, S::Codex2, S::Codex3, S::Codex4, S::Pok, S::ThundersEdge]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| mask >> i & 1 == 1)
            .map(|(_, s)| s)
            .collect();
        let want = st::active_aliases_text(content, sources);
        let got: BTreeSet<TechnologyId> = tc::ids(&q::active(tc::scope(sources))).into_iter().collect();
        let only_string: Vec<_> = want.difference(&got).collect();
        let only_typed: Vec<_> = got.difference(&want).collect();
        report.same(&format!("active_aliases {mask} (only string, only typed)"), &(only_string.clone(), only_typed.clone()), &(Vec::new(), Vec::new()), 2);
        report.same(&format!("scope {mask}"), &ti4_engine::core_adapter::scope(sources), &tc::scope(sources), 2);
    }
    report.same("RULES_TECH_EFFECTS", &ti4_model::state::RULES_TECH_EFFECTS, &q::RULES_TECH_EFFECTS, 2);
}

/// Every seat and technology of one position.
pub fn check_tech_position(position: &Position, report: &mut TechReport) {
    let content = ContentStore::embedded();
    let (state, sources) = (&position.state, position.sources);
    report.positions += 1;
    let Some((view, cx)) = tc::view(state, Some(&position.galaxy)) else {
        report.unmapped += 1;
        return;
    };
    let scope = tc::scope(sources);
    let blanked = ti4_engine::laws::blanked(state);
    for p in &state.players {
        let player: &PlayerId = &p.id;
        let Some(seat) = cx.owners.seat(player.as_str()) else {
            report.unmapped += 1;
            continue;
        };
        report.seats += 1;
        let hooks = StringHooks { state, content, sources, player };
        let name = format!("{} {player}", position.label);

        let want = st::researchable_text(state, content, sources, player);
        let got_view = tc::ids(&q::researchable(&view, scope, blanked, seat, &hooks));
        report.same(&format!("researchable view {name}"), &want, &got_view, 1);
        report.same(&format!("researchable adapter {name}"), &Some(want.clone()), &tc::researchable(state, content, sources, player), 1);
        let want = st::researchable_by_prerequisites_text(state, content, sources, player);
        let got_view = tc::ids(&q::researchable_by_prerequisites(&view, scope, blanked, seat, &hooks));
        report.same(&format!("researchable_by_prerequisites view {name}"), &want, &got_view, 1);
        report.same(
            &format!("researchable_by_prerequisites adapter {name}"),
            &Some(want),
            &tc::researchable_by_prerequisites(state, content, sources, player),
            1,
        );
        report.same(&format!("owned_colours {name}"), &st::owned_colours_text(state, content, player), &tc::colour_map(q::owned_colours(&view, seat)), 1);
        report.same(&format!("owned_colours adapter {name}"), &Some(st::owned_colours_text(state, content, player)), &tc::owned_colours(state, player), 1);
        report.same(
            &format!("specialties {name}"),
            &st::specialties_text(state, content, sources, player),
            &tc::colour_map(q::specialties(&view, scope, seat)),
            1,
        );
        report.same(
            &format!("specialties adapter {name}"),
            &Some(st::specialties_text(state, content, sources, player)),
            &tc::specialties_map(state, sources, player),
            1,
        );
        let joined_want: BTreeSet<String> = ti4_engine::synergy::joined_text(state, content, sources, player).into_iter().collect();
        let joined_got: BTreeSet<String> = q::COLOURS
            .into_iter()
            .zip(q::synergy_joined(&view, scope, seat))
            .filter(|(_, j)| *j)
            .map(|(c, _)| tc::colour_name(c).to_owned())
            .collect();
        report.same(&format!("synergy joined {name}"), &joined_want, &joined_got, 1);
        report.same(
            &format!("synergy joined adapter {name}"),
            &Some(ti4_engine::synergy::joined_text(state, content, sources, player)),
            &tc::synergy_joined_names(state, sources, player),
            1,
        );
        report.same(
            &format!("synergy pair adapter {name}"),
            &Some(ti4_engine::synergy::pair_text(state, content, sources, player)),
            &tc::synergy_pair(state, sources, player),
            1,
        );
        report.same(
            &format!("unowned faction technologies {name}"),
            &ti4_engine::entropic_scars::unowned_faction_technologies(state, content, sources, player).len(),
            &q::unowned_faction_technologies(&view, scope, seat).len(),
            1,
        );
        report.same(
            &format!("unowned faction technologies adapter {name}"),
            &Some(ti4_engine::entropic_scars::unowned_faction_technologies(state, content, sources, player).len()),
            &tc::unowned_faction_technologies_count(state, sources, player),
            1,
        );
        report.same(
            &format!("unit swaps {name}"),
            &Some(st::unit_swaps_text(state, content, sources, player)),
            &tc::unit_swaps(state, content, sources, player),
            1,
        );
        for base in UnitBaseType::ALL {
            let b = <UnitBaseType as IdStr>::as_str(base);
            let want = ti4_engine::action_cards::placed_unit_id_text(state, content, sources, player, b);
            // The typed form, then the faction override the string engine applies.
            let form = q::unit_form(&view, scope, tc::roster_for(content, sources).expect("embedded"), seat, base).map(<ti4_engine_core::generated::Unit as IdStr>::as_str);
            let got_view = form.map(|f| tc::overridden_form(state, content, sources, player, b, f));
            report.same(&format!("unit form {b} {name}"), &want, &got_view, 1);
            let got = tc::unit_form(state, content, sources, player, b).map(|f| tc::overridden_form(state, content, sources, player, b, f));
            report.same(&format!("unit form adapter {b} {name}"), &want, &got, 1);
        }

        for t in Technology::ALL {
            let a = alias(t);
            let w = st::can_research_text(state, content, sources, player, &a);
            report.same(&format!("can_research view {a} {name}"), &w, &q::can_research(&view, scope, blanked, seat, t, &hooks), 0);
            report.same(&format!("can_research adapter {a} {name}"), &Some(w), &tc::can_research(state, content, sources, player, &a), 0);
            for with_aida in [true, false] {
                let w = st::prerequisites_met_with_text(state, content, sources, player, &a, with_aida);
                let v = if with_aida {
                    q::prerequisites_met(&view, scope, blanked, seat, t, &hooks)
                } else {
                    q::prerequisites_met_without_aida(&view, scope, blanked, seat, t, &hooks)
                };
                report.same(&format!("prerequisites_met({with_aida}) view {a} {name}"), &w, &v, 0);
                report.same(
                    &format!("prerequisites_met({with_aida}) adapter {a} {name}"),
                    &Some(w),
                    &tc::prerequisites_met(state, content, sources, player, &a, with_aida),
                    0,
                );
            }
            report.same(&format!("owns {a} {name}"), &st::owns_technology(state, player, a.as_str()), &q::owns(&view, seat, t), 0);
        }
    }
    for (system, _) in &state.board {
        if let Some(ix) = <ti4_engine_core::generated::System as IdStr>::from_id(system.as_str()) {
            report.same(
                &format!("is_scar {system}"),
                &ti4_engine::entropic_scars::is_scar_text(content, sources, system),
                &q::is_scar(scope, ix),
                1,
            );
        }
    }
}
