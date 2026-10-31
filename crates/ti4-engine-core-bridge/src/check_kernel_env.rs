//! The three-way agreement of the `Env`s (package KF3, trap T12): for every `(position variant, seat)` the question asked of the string
//! engine's views (`LeaderView`, `StratView`: they call the string functions) and of the core's own [`KernelEnv`] (the hook table of
//! the live kernel and the core queries, no string state), all compared. Until they agree the oracle would compare the core with
//! itself. The variants are those of `check_leaders` (every leader unlocked or locked, goods, technologies, exhausted planets...).

use ti4_content::ContentStore;
use ti4_engine::core_sync::{CoreView, Ctx, Sources};
use ti4_engine::generated::IdStr;
use ti4_engine::leaders_view::LeaderView;
use ti4_engine::strategy_view::StratView;
use ti4_engine_core::driver::kernel_env::KernelEnv;
use ti4_engine_core::generated::{Leader, Planet, StrategyCardKind, System, Technology};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::hooks::CombatCtx;
use ti4_engine_core::rules::{abilities, leaders, strategy};
use ti4_engine_core::state::{Slice, SliceSet};

use crate::workload::Position;

#[derive(Debug, Default, Clone)]
pub struct EnvReport {
    pub positions: usize,
    pub variants: usize,
    pub seats: usize,
    pub answers: usize,
    pub unmapped: usize,
    pub mismatches: Vec<String>,
    /// Hook slots the live kernel has no function for where the string engine answers (`Some` vs `None`): the faction modules that are
    /// not ported (since lane F2 only the leaders of the dropped factions), counted by method, never a silent agreement.
    pub gaps: std::collections::BTreeMap<String, usize>,
    /// What the string side answered per hook method and leader (`[false, true]` counts), over the answers compared: a hook whose string
    /// answer was never `true` (or never `false`) was not compared on that side.
    pub outcomes: std::collections::BTreeMap<String, [usize; 2]>,
}

impl EnvReport {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.variants += o.variants;
        self.seats += o.seats;
        self.answers += o.answers;
        self.unmapped += o.unmapped;
        self.mismatches.extend(o.mismatches.iter().cloned());
        for (k, n) in &o.gaps {
            *self.gaps.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &o.outcomes {
            let mine = self.outcomes.entry(k.clone()).or_default();
            mine[0] += n[0];
            mine[1] += n[1];
        }
    }

    /// An answer that may be missing in the core: string `Some(x)` against core `None` is a GAP of that method (counted), `Some` against a
    /// different `Some` is a mismatch.
    fn same_hook(&mut self, method: &str, what: &str, string: Option<bool>, core: Option<bool>) {
        self.answers += 1;
        if let Some(answer) = string {
            self.outcomes.entry(method.to_owned()).or_default()[usize::from(answer)] += 1;
        }
        match (string, core) {
            (a, b) if a == b => {}
            (Some(_), None) => *self.gaps.entry(method.to_owned()).or_default() += 1,
            (a, b) => {
                if self.mismatches.len() < 60 {
                    self.mismatches.push(format!("{what}: string view {a:?} vs KernelEnv {b:?}"));
                }
            }
        }
    }

    fn same<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: T, core: T) {
        self.answers += 1;
        if string != core && self.mismatches.len() < 60 {
            self.mismatches.push(format!("{what}: string view {string:?} vs KernelEnv {core:?}"));
        }
    }
}

/// Every slice but the random streams (the Envs read none).
fn reads() -> SliceSet {
    let all: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Rng).collect();
    SliceSet::of(&all)
}

/// Compare every `Env` answer on one position and its variants.
pub fn check_position(p: &Position, report: &mut EnvReport) {
    check_variants(p, crate::check_leaders::variants(&p.state), report);
}

/// Compare every `Env` answer on the given variants of one position.
pub fn check_variants(p: &Position, variants: Vec<(&'static str, ti4_model::state::GameState)>, report: &mut EnvReport) {
    let content = ContentStore::embedded();
    report.positions += 1;
    let kernel = ti4_engine::live::kernel::kernel_for(p.sources);
    for (name, g) in variants {
        report.variants += 1;
        let label = format!("{} [{name}]", p.label);
        let Some(lv) = LeaderView::of(&g, Some(content), p.sources, Some(&p.galaxy), ti4_engine::leaders_view::READS_ABILITIES) else {
            report.unmapped += 1;
            report.mismatches.push(format!("{label}: LeaderView refused the position"));
            continue;
        };
        let Some(sv) = StratView::full(&g, content, p.sources) else {
            report.unmapped += 1;
            report.mismatches.push(format!("{label}: StratView refused the position"));
            continue;
        };
        let Ok(mut cx) = Ctx::of(&g) else {
            report.unmapped += 1;
            continue;
        };
        let view = match CoreView::of(&g, &mut cx, Sources { galaxy: Some(&p.galaxy), rng: None }, reads()) {
            Ok(v) => v,
            Err(e) => {
                report.unmapped += 1;
                report.mismatches.push(format!("{label}: CoreView refused the position: {e}"));
                continue;
            }
        };
        let env = KernelEnv::new(&view, kernel);
        for (i, player) in g.players.iter().enumerate() {
            report.seats += 1;
            let Ok(index) = u8::try_from(i) else { continue };
            let who = Seat(index);
            let at = |what: &str| format!("{label} {} {what}", player.id);
            // abilities::Env
            for ctx in [CombatCtx::Space, CombatCtx::Ground] {
                report.same(&at("combat_hook"), abilities::Env::combat_hook(&lv, who, ctx), abilities::Env::combat_hook(&env, who, ctx));
            }
            report.same(&at("fleet_supply_hook"), abilities::Env::fleet_supply_hook(&lv, who, 7), abilities::Env::fleet_supply_hook(&env, who, 7));
            report.same(&at("status_tokens_hook"), abilities::Env::status_tokens_hook(&lv, who, 2), abilities::Env::status_tokens_hook(&env, who, 2));
            report.same(&at("trades_action_cards_hook"), abilities::Env::trades_action_cards_hook(&lv, who), abilities::Env::trades_action_cards_hook(&env, who));
            report.same(&at("ignores_neighbours_hook"), abilities::Env::ignores_neighbours_hook(&lv, who), abilities::Env::ignores_neighbours_hook(&env, who));
            for t in Technology::ALL {
                report.same(&at(&format!("waived {}", t.as_str())), abilities::Env::waived_hook(&lv, who, t), abilities::Env::waived_hook(&env, who, t));
            }
            for card in std::iter::once(None).chain(StrategyCardKind::ALL.iter().map(|k| Some(*k))) {
                report.same(&at(&format!("substitutes_primary_hook {card:?}")), abilities::Env::substitutes_primary_hook(&lv, who, card), abilities::Env::substitutes_primary_hook(&env, who, card));
                report.same(&at(&format!("secondary_is_free_hook {card:?}")), abilities::Env::secondary_is_free_hook(&lv, who, card), abilities::Env::secondary_is_free_hook(&env, who, card));
                // strategy::Env
                report.same(&at(&format!("secondary_is_free {card:?}")), strategy::Env::secondary_is_free(&sv, who, card), strategy::Env::secondary_is_free(&env, who, card));
                report.same(&at(&format!("substitutes_primary {card:?}")), strategy::Env::substitutes_primary(&sv, who, card), strategy::Env::substitutes_primary(&env, who, card));
            }
            report.same(&at("leadership_affordable"), strategy::Env::leadership_affordable(&sv, who), strategy::Env::leadership_affordable(&env, who));
            report.same(&at("can_research"), strategy::Env::can_research(&sv, who), strategy::Env::can_research(&env, who));
            report.same(&at("has_specialty_research_planet"), strategy::Env::has_specialty_research_planet(&sv, who), strategy::Env::has_specialty_research_planet(&env, who));
            report.same(&at("technology_secondary_affordable"), strategy::Env::technology_secondary_affordable(&sv, who), strategy::Env::technology_secondary_affordable(&env, who));
            report.same(&at("secret_hand_all_scored"), strategy::Env::secret_hand_all_scored(&sv, who), strategy::Env::secret_hand_all_scored(&env, who));
            for system in System::ALL {
                if view.galaxy().is_some_and(|g| g.coord_of(system).is_some()) {
                    report.same(&at(&format!("can_produce_in {}", system.as_str())), strategy::Env::can_produce_in(&sv, who, system), strategy::Env::can_produce_in(&env, who, system));
                }
            }
            // leaders::Env
            for l in Leader::ALL {
                report.same_hook(&format!("commander_unlocked {}", l.as_str()), &at(&format!("commander_unlocked {}", l.as_str())), leaders::Env::faction_commander_unlocked(&lv, who, l), leaders::Env::faction_commander_unlocked(&env, who, l));
                report.same_hook(&format!("leader_action {}", l.as_str()), &at(&format!("leader_action {}", l.as_str())), leaders::Env::leader_action(&lv, who, l), leaders::Env::leader_action(&env, who, l));
                report.same(&at(&format!("imperia_grants {}", l.as_str())), leaders::Env::imperia_grants(&lv, who, l), leaders::Env::imperia_grants(&env, who, l));
                if matches!(l, Leader::Zeu | Leader::RinTheMastersLegacy) {
                    report.same(&at(&format!("other_action_resolves {}", l.as_str())), leaders::Env::other_action_resolves(&lv, who, l), leaders::Env::other_action_resolves(&env, who, l));
                }
            }
        }
        for planet in Planet::ALL {
            report.same(&format!("{label} resource_bonus {}", planet.as_str()), leaders::Env::resource_bonus(&lv, planet), leaders::Env::resource_bonus(&env, planet));
        }
        report.same(&format!("{label} mecatol_area"), leaders::Env::mecatol_area(&lv), leaders::Env::mecatol_area(&env));
    }
}
