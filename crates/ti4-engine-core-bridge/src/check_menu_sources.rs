//! The lockstep of the action menu's sources the core lists itself (package F2, KM1): for every `(position variant, seat)` the string
//! engine's strategic options (or the lone "pass") and leader component actions, and the core's, rendered, compared option by option
//! (id, kind, label, order). The variants are those of `check_hooks` (every faction with a leader hook seated, leaders readied and unlocked,
//! no ships, no hand...) with strategy cards dealt, one and two per seat, some spent.

use ti4_content::ContentStore;
use ti4_engine::choice::ChoiceOption;
use ti4_engine::generated::IdStr;
use ti4_engine_core::generated::StrategyCard;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

use crate::workload::Position;

#[derive(Debug, Default, Clone)]
pub struct MenuReport {
    pub variants: usize,
    pub seats: usize,
    pub unmapped: usize,
    pub mismatches: Vec<String>,
    /// How many seats offered: a lone strategic card (bare id), several, the lone pass; their own leader actions; a copy through Ssruu.
    pub bare: usize,
    pub several: usize,
    pub lone_pass: usize,
    pub own_leaders: usize,
    pub borrowed: usize,
}

impl MenuReport {
    pub fn add(&mut self, o: &Self) {
        self.variants += o.variants;
        self.seats += o.seats;
        self.unmapped += o.unmapped;
        self.mismatches.extend(o.mismatches.iter().cloned());
        self.bare += o.bare;
        self.several += o.several;
        self.lone_pass += o.lone_pass;
        self.own_leaders += o.own_leaders;
        self.borrowed += o.borrowed;
    }
}

fn key(o: &ChoiceOption) -> (String, String, String) {
    (o.id.clone(), o.kind.clone(), o.label.clone())
}

/// Deal strategy cards: seat `i` gets `per_seat` cards, the first of each spent when `spend`.
fn dealt(base: &GameState, per_seat: usize, spend: bool) -> GameState {
    let mut g = base.clone();
    let mut next = 0;
    for seat in &mut g.players {
        seat.strategy_cards.clear();
        seat.exhausted_strategy_cards.clear();
        for _ in 0..per_seat {
            let card = StrategyCardId::new(StrategyCard::ALL[next % StrategyCard::ALL.len()].as_str());
            next += 1;
            seat.strategy_cards.push(card);
        }
        if spend {
            if let Some(first) = seat.strategy_cards.first().cloned() {
                seat.exhausted_strategy_cards.insert(first);
            }
        }
    }
    g
}

/// Compare the sources on one position's variants.
pub fn check_variants(p: &Position, variants: Vec<(&'static str, GameState)>, report: &mut MenuReport) {
    let content = ContentStore::embedded();
    for (name, base) in variants {
        for (deal, g) in [
            ("no cards", dealt(&base, 0, false)),
            ("one card", dealt(&base, 1, false)),
            ("two cards", dealt(&base, 2, false)),
            ("two cards, one spent", dealt(&base, 2, true)),
            ("one card, spent", dealt(&base, 1, true)),
        ] {
            report.variants += 1;
            for seat in &g.players {
                report.seats += 1;
                let pid: &PlayerId = &seat.id;
                let at = format!("{} [{name}, {deal}] {pid}", p.label);
                let Some((strategic, leaders)) = ti4_engine::menu_sources_view::rendered(&g, p.sources, pid) else {
                    report.unmapped += 1;
                    continue;
                };
                // The string: the strategic options or the lone pass, and the leaders' component actions.
                let string_strategic: Vec<ChoiceOption> = match ti4_engine::strategy::strategic_action_options(&g, content, pid) {
                    Some(choice) => choice.options,
                    None => vec![ChoiceOption::labelled("pass", ti4_engine::strategy::ACTION_KIND, "pass")],
                };
                let string_leaders = ti4_engine::leaders::component_actions(&g, content, pid);
                for (what, string, core) in [("strategic", &string_strategic, &strategic), ("leaders", &string_leaders, &leaders)] {
                    let (s, c): (Vec<_>, Vec<_>) = (string.iter().map(key).collect(), core.iter().map(key).collect());
                    if s != c && report.mismatches.len() < 40 {
                        report.mismatches.push(format!("{at} {what}: string {s:?} vs core {c:?}"));
                    }
                }
                match strategic.as_slice() {
                    [o] if o.id == "pass" => report.lone_pass += 1,
                    [o] if o.id == ti4_engine::strategy::STRATEGIC_ACTION_ID => report.bare += 1,
                    [_, _, ..] => report.several += 1,
                    _ => {}
                }
                report.own_leaders += leaders.iter().filter(|o| !o.id.starts_with("component|leader|yssarilagent|")).count();
                report.borrowed += leaders.iter().filter(|o| o.id.starts_with("component|leader|yssarilagent|")).count();
            }
        }
    }
}
