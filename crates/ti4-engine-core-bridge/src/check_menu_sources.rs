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
    /// Seats that were offered Sling Relay.
    pub sling: usize,
    /// How many options the core listed per source (the card and partner sources), so a test can demand that each shape was reached.
    pub listed: std::collections::BTreeMap<&'static str, usize>,
    /// Seats whose action cards the core could not list (a counted gap).
    pub gaps: usize,
    /// The first positions the core could not hold.
    pub unmapped_at: Vec<String>,
}

impl MenuReport {
    fn count(&mut self, what: &'static str, n: usize) {
        *self.listed.entry(what).or_default() += n;
    }
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
        self.sling += o.sling;
        self.gaps += o.gaps;
        self.unmapped_at.extend(o.unmapped_at.iter().take(8).cloned());
        for (k, n) in &o.listed {
            *self.listed.entry(k).or_default() += n;
        }
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

/// The string's lists of the sources the core lists besides the strategic action and the leaders.
fn string_lists(g: &GameState, p: &Position, pid: &PlayerId) -> Vec<(&'static str, Vec<ChoiceOption>)> {
    let content = ContentStore::embedded();
    vec![
        ("relics", ti4_engine::relics::available_actions(g, content, p.sources, Some(&p.galaxy), pid)),
        ("exploration", ti4_engine::exploration::available_actions(g, content, p.sources, pid)),
        ("transactions", ti4_engine::transactions::available_actions(g, content, &p.galaxy, pid)),
        ("cards", ti4_engine::action_cards::available_actions(g, content, p.sources, Some(&p.galaxy), pid)),
    ]
}

/// Variants that exercise the card and partner sources: relics and fragments, Enigmatic Device cards, hands of action cards, a salvage yard,
/// goods, and transactions already made.
#[must_use]
pub fn card_variants(base: &GameState) -> Vec<(&'static str, GameState)> {
    use ti4_engine_core::generated::{ActionCard, Relic};
    use ti4_model::id::{ActionCardId, RelicId};
    let mut out: Vec<(&'static str, GameState)> = vec![("cards as is", base.clone())];
    let mut push = |label: &'static str, build: &dyn Fn(&mut GameState)| {
        let mut g = base.clone();
        build(&mut g);
        out.push((label, g));
    };
    push("relics all", &|g| {
        for (i, seat) in g.players.iter_mut().enumerate() {
            seat.relics = Relic::ALL.iter().skip(i).step_by(3).map(|r| RelicId::new(r.as_str())).collect();
        }
    });
    push("relics every one for seat 0", &|g| {
        if let Some(seat) = g.players.first_mut() {
            seat.relics = Relic::ALL.iter().map(|r| RelicId::new(r.as_str())).collect();
        }
    });
    push("fragments", &|g| {
        let tables: [&[(&str, i32)]; 4] = [&[("CULTURAL", 3)], &[("CULTURAL", 2), ("FRONTIER", 1)], &[("HAZARDOUS", 3), ("INDUSTRIAL", 5), ("CULTURAL", 1)], &[("FRONTIER", 3)]];
        for (i, seat) in g.players.iter_mut().enumerate() {
            seat.relic_fragments = tables[i % 4].iter().map(|(k, n)| ((*k).to_owned(), *n)).collect();
        }
    });
    push("enigmatic device cards", &|g| {
        for (i, seat) in g.players.iter_mut().enumerate() {
            seat.trade_goods = 8;
            seat.exploration_cards = match i % 3 {
                0 => vec!["ed1".to_owned(), "ed2".to_owned()],
                1 => vec!["ed2".to_owned()],
                _ => vec![],
            };
        }
    });
    push("enigmatic device cards, broke", &|g| {
        for seat in &mut g.players {
            seat.exploration_cards = vec!["ed1".to_owned()];
            seat.trade_goods = 0;
        }
        g.exhausted_planets.extend(g.board.values().flat_map(|s| s.planet_control.keys().cloned()).collect::<Vec<_>>());
    });
    push("enigmatic device relic, broke", &|g| {
        for seat in &mut g.players {
            seat.relics = vec![RelicId::new("enigmaticdevice")];
        }
        g.exhausted_planets.extend(g.board.values().flat_map(|s| s.planet_control.keys().cloned()).collect::<Vec<_>>());
        for seat in &mut g.players {
            seat.trade_goods = 0;
        }
    });
    // Hands of action cards, six at a time, so every card is held somewhere, with goods and without.
    let all: Vec<&'static str> = ActionCard::ALL.iter().map(|c| c.as_str()).collect();
    for (label, goods) in [("hands, no goods", 0), ("hands, goods", 5)] {
        for offset in 0..6 {
            let hands = all.clone();
            let label: &'static str = Box::leak(format!("{label} {offset}").into_boxed_str());
            push(label, &move |g| {
                let n = g.players.len();
                for (i, seat) in g.players.iter_mut().enumerate() {
                    seat.trade_goods = goods;
                    seat.action_cards = hands.iter().skip(offset * 7 + i).step_by(n).take(7).map(|a| ActionCardId::new(*a)).collect();
                }
            });
        }
    }
    out
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
                let Some(rendered) = ti4_engine::menu_sources_view::rendered(&g, p.sources, Some(&p.galaxy), pid) else {
                    report.unmapped += 1;
                    if report.unmapped_at.len() < 8 {
                        report.unmapped_at.push(at.clone());
                    }
                    continue;
                };
                let by_name = |n: &str| rendered.iter().find(|(m, _)| *m == n).map(|(_, o)| o.clone());
                let (strategic, leaders, technology) = (by_name("strategic").unwrap_or_default(), by_name("leaders").unwrap_or_default(), by_name("technology").unwrap_or_default());
                // The string: the strategic options or the lone pass, and the leaders' component actions.
                let string_strategic: Vec<ChoiceOption> = match ti4_engine::strategy::strategic_action_options(&g, content, pid) {
                    Some(choice) => choice.options,
                    None => vec![ChoiceOption::labelled("pass", ti4_engine::strategy::ACTION_KIND, "pass")],
                };
                let string_leaders = ti4_engine::leaders::component_actions(&g, content, pid);
                let string_technology = ti4_engine::technology::component_actions(&g, content, p.sources, pid);
                let mut pairs: Vec<(&str, Vec<ChoiceOption>, Vec<ChoiceOption>)> =
                    vec![("strategic", string_strategic, strategic.clone()), ("leaders", string_leaders, leaders.clone()), ("technology", string_technology, technology.clone())];
                for (what, string) in string_lists(&g, p, pid) {
                    match by_name(what) {
                        Some(core) => {
                            report.count(what, core.len());
                            pairs.push((what, string, core));
                        }
                        None => report.gaps += 1,
                    }
                }
                for (what, string, core) in &pairs {
                    let (s, c): (Vec<_>, Vec<_>) = (string.iter().map(key).collect(), core.iter().map(key).collect());
                    if s != c && report.mismatches.iter().filter(|m| m.contains(&format!(" {what}: "))).count() < 6 {
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
                report.sling += technology.len();
                report.borrowed += leaders.iter().filter(|o| o.id.starts_with("component|leader|yssarilagent|")).count();
            }
        }
    }
}
