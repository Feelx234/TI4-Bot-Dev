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
    /// Seats holding Signal Jamming whose menu offered it, and whose menu withheld it (its offer reads the map): both shapes compared.
    pub jam_offered: usize,
    pub jam_withheld: usize,
    /// Every id the core listed for the faction source, so a test can demand that each action was reached.
    pub faction_ids: std::collections::BTreeSet<String>,
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
        self.jam_offered += o.jam_offered;
        self.jam_withheld += o.jam_withheld;
        self.faction_ids.extend(o.faction_ids.iter().cloned());
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
    // Only the latest rules version is supported by the core.
    g.rules_version = ti4_model::state::RULES_VERSION;
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
        ("faction", ti4_engine::faction_abilities::component_actions(g, content, pid)),
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
    // Signal Jamming where no seat has a ship on the map: nothing is in reach, so the menu withholds it.
    push("signal jamming, no ships", &|g| {
        for b in g.board.values_mut() {
            b.units.clear();
        }
        for seat in &mut g.players {
            seat.action_cards = vec![ActionCardId::new(ActionCard::SignalJamming.as_str())];
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

/// Variants that exercise the faction component actions: notes held (Promise of Protection, Trade Convoys, the pacts, Black Market Forgery,
/// Fires of the Gashlai), technologies ready or spent (Production Biomes, Lazax Gate Folding, Wormhole Generator, Mageon Implants), war suns and
/// a flagship for the Muaat, fragments for the Naaz, hands for the Yssaril, and the cases the core counts as gaps.
#[must_use]
pub fn faction_variants(base: &GameState) -> Vec<(&'static str, GameState)> {
    use crate::check_hooks::{add_space, home, ids};
    use ti4_engine_core::generated::Technology;
    use ti4_model::id::{ActionCardId, PlanetId, SystemId, TechnologyId};
    let mut out: Vec<(&'static str, GameState)> = vec![];
    let mut push = |label: &'static str, build: &dyn Fn(&mut GameState)| {
        let mut g = base.clone();
        build(&mut g);
        out.push((label, g));
    };
    let hold = |g: &mut GameState, note: &str, owner: &str, by: usize| {
        let holder = g.players[by % g.players.len()].id.clone();
        g.promissory_notes.insert(format!("{note}:{owner}"), holder);
    };
    push("faction notes", &|g| {
        for (i, (note, owner)) in [("pop", "mentak"), ("convoys", "hacan"), ("blood_pact", "empyrean"), ("dark_pact", "empyrean"), ("bmf", "naaz"), ("fires", "muaat")].into_iter().enumerate() {
            hold(g, note, owner, i);
            hold(g, note, owner, i + 3);
        }
        for seat in &mut g.players {
            seat.relic_fragments.insert("CULTURAL".to_owned(), 2);
        }
    });
    push("terraform held", &|g| hold(g, "terraform", "titans", 0));
    push("faction notes, spent faceup", &|g| {
        for (i, (note, owner)) in [("pop", "mentak"), ("convoys", "hacan"), ("blood_pact", "empyrean")].into_iter().enumerate() {
            hold(g, note, owner, i);
            g.promissory_faceup.insert(format!("{note}:{owner}"));
        }
    });
    push("faction techs", &|g| {
        for seat in &mut g.players {
            for t in [Technology::ProductionBiomes, Technology::LazaxGateFolding, Technology::WormholeGenerator, Technology::MageonImplants].map(|t| t.as_str()) {
                seat.technologies.insert(TechnologyId::new(t));
            }
            seat.strategic_tokens = 2;
            seat.action_cards = vec![ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str())];
        }
        let mecatol = SystemId::new("18");
        for (i, id) in ids(g).into_iter().enumerate().take(1) {
            let _ = i;
            g.system_mut(&mecatol).planet_control.insert(PlanetId::new(ti4_engine_core::generated::Planet::MecatolRex.as_str()), id);
        }
    });
    push("faction techs, spent and no tokens", &|g| {
        for seat in &mut g.players {
            for t in [Technology::ProductionBiomes, Technology::LazaxGateFolding, Technology::WormholeGenerator, Technology::MageonImplants].map(|t| t.as_str()) {
                seat.technologies.insert(TechnologyId::new(t));
                seat.exhausted_technologies.insert(TechnologyId::new(t));
            }
            seat.strategic_tokens = 0;
        }
    });
    push("muaat war suns", &|g| {
        for (i, id) in ids(g).iter().enumerate() {
            if let Some(h) = home(g, i) {
                add_space(g, &h, "warsun", id, 1);
                add_space(g, &h, "muaat_flagship", id, 1);
            }
            if let Some(seat) = g.player_mut(id) {
                seat.strategic_tokens = 3;
                seat.fleet_tokens = 3;
            }
        }
    });
    push("muaat flagship, avernus", &|g| {
        for (i, id) in ids(g).iter().enumerate() {
            if let Some(h) = home(g, i) {
                add_space(g, &h, "muaat_flagship", id, 1);
            }
            if let Some(seat) = g.player_mut(id) {
                seat.strategic_tokens = 3;
                seat.fleet_tokens = 3;
            }
        }
        if let Some(seat) = g.players.first() {
            let id = seat.id.clone();
            let system = SystemId::new("18");
            g.board.entry(system.clone()).or_default();
            assert!(ti4_engine::planets::place(g, &system, &PlanetId::new("avernus"), &id));
            for (i, id) in ids(g).iter().enumerate() {
                if i > 0 {
                    if let Some(h) = home(g, i) {
                        add_space(g, &h, "warsun", id, 1);
                    }
                }
            }
        }
    });
    push("muaat war suns, no tokens", &|g| {
        for (i, id) in ids(g).iter().enumerate() {
            if let Some(h) = home(g, i) {
                add_space(g, &h, "warsun", id, 1);
            }
            if let Some(seat) = g.player_mut(id) {
                seat.strategic_tokens = 0;
                seat.fleet_tokens = 0;
            }
        }
        hold(g, "fires", "muaat", 0);
    });
    push("naaz fragments", &|g| {
        let tables: [&[(&str, i32)]; 4] = [&[("CULTURAL", 2)], &[("FRONTIER", 2)], &[("HAZARDOUS", 1)], &[("INDUSTRIAL", 1), ("FRONTIER", 1)]];
        for (i, seat) in g.players.iter_mut().enumerate() {
            seat.relic_fragments = tables[i % 4].iter().map(|(k, n)| ((*k).to_owned(), *n)).collect();
        }
    });
    push("yssaril hands", &|g| {
        for (i, seat) in g.players.iter_mut().enumerate() {
            seat.action_cards = if i % 2 == 0 { vec![ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[0].as_str()), ActionCardId::new(ti4_engine_core::generated::ActionCard::ALL[3].as_str())] } else { vec![] };
        }
    });
    push("nekro c4r and vortex ready", &|g| {
        for seat in &mut g.players {
            seat.technologies.insert(TechnologyId::new(Technology::ERRORERROR.as_str()));
            seat.technologies.insert(TechnologyId::new(Technology::Vortex.as_str()));
        }
    });
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
                            if what == "faction" {
                                report.faction_ids.extend(core.iter().map(|o| o.id.clone()));
                            }
                            if what == "cards" && seat.action_cards.iter().any(|c| c.as_str() == "jamming") {
                                if core.iter().any(|o| o.label.contains("Signal Jamming")) {
                                    report.jam_offered += 1;
                                } else {
                                    report.jam_withheld += 1;
                                }
                            }
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
