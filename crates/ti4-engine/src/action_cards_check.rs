//! The offline dual of `Area::ActionCards`: every read of [`crate::action_cards_view::ActionView`] asked of
//! one position next to the string function it replaces, every answer compared. Used by
//! `board_equivalence` (golden positions, templates).

use ti4_content::ContentStore;
use ti4_engine_core::generated::ActionCard;
use ti4_model::id::ActionCardId;
use ti4_model::state::GameState;

use crate::action_cards_view::{ActionView, is_component_action};
use crate::generated::IdStr;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct ActionCardsCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    /// Reads the core declined (the documented bails), not mismatches.
    pub bailed: u64,
    /// The window reads of Harness Energy and Infiltrate compared (the tile catalogue, the rift hook, the box; W16).
    pub judged: u64,
    pub mismatches: Vec<String>,
}

impl ActionCardsCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        self.bailed += o.bailed;
        self.judged += o.judged;
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

/// Compare every action-card read on one position.
pub fn compare(label: &str, gs: &GameState, out: &mut ActionCardsCheck) {
    out.positions += 1;
    let content = ContentStore::embedded();
    let Some(v) = ActionView::of(gs) else {
        out.unmapped += 1;
        return;
    };
    let cards: Vec<ActionCardId> = ActionCard::ALL.iter().map(|c| ActionCardId::new(c.as_str())).collect();
    for card in &cards {
        out.eq(label, &format!("is_component_action {card}"), &crate::action_cards::is_component_action_text(content, card), &is_component_action(card));
    }
    for p in gs.players.iter().map(|p| p.id.clone()) {
        out.eq(label, "hand_limit", &crate::action_cards::hand_limit_text(gs, content, &p), &v.hand_limit(&p, content));
        let barred = crate::laws::action_cards_forbidden(gs, &p) || crate::laws::political_secret_bars(gs, &p);
        out.eq(label, "may_play", &!barred, &v.may_play(&p));
        for card in &cards {
            match v.cost_is_payable(&p, card) {
                None => out.bailed += 1,
                typed => out.eq(label, &format!("cost_is_payable {card}"), &crate::action_cards::cost_is_payable_text(gs, content, ti4_model::content_types::FULL, &p, card), &typed),
            }
            match v.has_legal_target(&p, card, ti4_model::content_types::FULL) {
                None => out.bailed += 1,
                typed => out.eq(label, &format!("has_legal_target {card}"), &crate::action_cards::has_legal_target_text(gs, content, ti4_model::content_types::FULL, &p, card), &typed),
            }
        }
    }
    reactions(label, gs, content, &v, &cards, out);
}

/// The window reads: every reaction card asked in every occupied system, with each seat (and nobody) as actor.
fn reactions(label: &str, gs: &GameState, content: &ContentStore, v: &ActionView<'_>, cards: &[ActionCardId], out: &mut ActionCardsCheck) {
    const WINDOW_CARDS: [&str; 11] =
        ["bunker", "blitz", "disable", "f_prototype", "upgrade", "rally", "rescue", "intercept", "emergency", "harness", "infiltrate"];
    let systems: Vec<&ti4_model::id::SystemId> = gs.board.iter().filter(|(_, b)| !b.units.is_empty() || b.planet_units.values().any(|u| !u.is_empty())).map(|(id, _)| id).collect();
    let players: Vec<&ti4_model::id::PlayerId> = gs.players.iter().map(|p| &p.id).collect();
    for card in cards.iter().filter(|c| WINDOW_CARDS.contains(&c.as_str())) {
        for sys in &systems {
            // Infiltrate's window names the planet gained: every planet of the system that holds a unit or has a controller, and none.
            let planets: Vec<Option<String>> = std::iter::once(None)
                .chain(
                    gs.system_state(sys)
                        .planet_units
                        .keys()
                        .chain(gs.system_state(sys).planet_control.keys())
                        .map(|p| p.as_str().to_owned())
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .map(Some),
                )
                .collect();
            for who in &players {
                for actor in std::iter::once(None).chain(players.iter().map(|p| Some(p.as_str()))) {
                    for planet in &planets {
                        let mut fields: std::collections::BTreeMap<String, serde_json::Value> = actor.map(|a| ("player".to_owned(), serde_json::json!(a))).into_iter().collect();
                        if let Some(p) = planet {
                            fields.insert("planet".to_owned(), serde_json::json!(p));
                        }
                        let event = crate::event::Event::new(1, "WINDOW", fields);
                        let string = crate::action_cards::reaction_window_text(gs, content, who, card, &event, sys);
                        match v.reaction_can_matter(who, card, sys.as_str(), actor, planet.as_deref()) {
                            None => out.bailed += 1,
                            typed => {
                                out.judged += u64::from(matches!(card.as_str(), "harness" | "infiltrate"));
                                out.eq(label, &format!("reaction_can_matter {card} {sys} actor {actor:?} planet {planet:?}"), &string, &typed);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::System;
    use ti4_model::id::{PlanetId, PlayerId, SystemId};

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: u64) -> u64 {
            self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n
        }
    }

    /// Package W16: the judges of the windows of Harness Energy (the tile catalogue and the rifts a Cabal tear makes) and of Infiltrate (the
    /// swaps of the box, counted as they go, and the rules versions) answer as the string's `reaction_can_matter` does, in every occupied
    /// system, for every seat and actor, with and without the planet the window names, over many random boards: structures of every seat
    /// on every planet, anomalies with ships in them, reinforcements run down to zero, a Cabal tear, rules versions 0, 1 and 23. The core
    /// answered `None` (a counted bail) for both cards before.
    #[test]
    fn the_judges_of_harness_energy_and_infiltrate_agree_with_the_strings_over_many_positions() {
        let content = ContentStore::embedded();
        let planets: Vec<(SystemId, PlanetId)> = ti4_content::galaxy::all_planets(content, ti4_model::content_types::POK)
            .iter()
            .filter(|(_, p)| p.system_id().is_some() && !p.is_placed_during_play())
            .map(|(id, p)| (SystemId::new(p.system_id().unwrap()), PlanetId::new(*id)))
            .collect();
        let anomalies: Vec<SystemId> = System::ALL.iter().filter(|s| s.data().anomalies.any()).map(|s| SystemId::new(s.as_str())).collect();
        assert!(planets.len() > 30 && anomalies.len() > 5);
        let players = ["a", "b", "c"].map(PlayerId::new);
        let mut out = ActionCardsCheck::default();
        for seed in 0..40_u64 {
            let mut rng = Lcg(seed + 1);
            let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan"), ("c", "cabal")], ti4_model::content_types::POK);
            state.rules_version = [0, 1, 23][rng.next(3) as usize];
            for _ in 0..14 {
                let (system, planet) = &planets[rng.next(planets.len() as u64) as usize];
                let owner = &players[rng.next(3) as usize];
                state.system_mut(system).set_control(planet.clone(), owner.clone());
                for _ in 0..rng.next(3) {
                    let kind = ["pds", "spacedock", "infantry", "pds2", "spacedock2"][rng.next(5) as usize];
                    crate::fixtures::put_on_planet(&mut state, system, planet, kind, &players[rng.next(3) as usize], 1);
                }
            }
            for _ in 0..4 {
                let system = &anomalies[rng.next(anomalies.len() as u64) as usize];
                crate::fixtures::put(&mut state, system, "cruiser", &players[rng.next(3) as usize], 1);
            }
            // The reinforcements of a seat run down: six PDS or three space docks of a on the board of one far planet.
            if seed % 3 == 0 {
                let (system, planet) = &planets[rng.next(planets.len() as u64) as usize];
                crate::fixtures::put_on_planet(&mut state, system, planet, "pds", &players[0], 6 - usize::try_from(seed % 2).unwrap());
                crate::fixtures::put_on_planet(&mut state, system, planet, "spacedock", &players[1], 3);
            }
            compare(&format!("seed {seed}"), &state, &mut out);
        }
        assert!(out.mismatches.is_empty(), "{:?}", out.mismatches);
        assert!(out.judged > 3_000, "both judges were compared: {}", out.judged);
    }

    /// Package W16 (Bribery's bill): the potential goods of a seat, as the core computes them from the position and the kernel's Keleres
    /// hooks (`rules::production::supply::potential_goods`), are the string's `supply::potential_goods` for every seat of random tables, and
    /// Bribery is judged payable by the core exactly when the string says it is (`cost_is_payable` at rules version 23, goods taken from
    /// the core). Keleres is dropped by decision: its hooks answer nothing here, which the lockstep also shows.
    #[test]
    fn the_potential_goods_of_a_seat_are_the_strings_over_many_positions() {
        use crate::core_sync::{CoreView, Ctx, Sources};
        let mut rng = Lcg(99);
        let mut payable = [0_u64; 2];
        for seed in 0..40_u64 {
            let count = 3 + rng.next(4) as usize;
            let seats: Vec<(&str, &str)> = ["a", "b", "c", "d", "e", "f"].iter().copied().zip(["sol", "hacan", "letnev", "xxcha", "yssaril", "naalu"]).take(count).collect();
            let mut state = crate::fixtures::seated_game(&seats, ti4_model::content_types::POK);
            state.rules_version = [22, 23, 24][rng.next(3) as usize];
            for p in &mut state.players {
                p.trade_goods = i32::try_from(rng.next(5)).unwrap() - i32::from(rng.next(6) == 0);
                p.commodities = i32::try_from(rng.next(4)).unwrap();
            }
            let mut cx = Ctx::of(&state).expect("mapped");
            let view = CoreView::of(&state, &mut cx, Sources { galaxy: None, rng: None }, ti4_engine_core::state::SliceSet::of(&[ti4_engine_core::state::Slice::Meta, ti4_engine_core::state::Slice::Seats, ti4_engine_core::state::Slice::Players, ti4_engine_core::state::Slice::Board, ti4_engine_core::state::Slice::Marks])).expect("loaded");
            for p in &state.players {
                let seat = cx.owners.seat(p.id.as_str()).expect("a seat");
                let core = ti4_engine_core::rules::production::supply::potential_goods(&view, crate::production_view::kernel(), seat);
                assert_eq!(core, crate::supply::potential_goods(&state, &p.id), "seed {seed} {}", p.id);
                let bribery = ti4_engine_core::generated::ActionCard::Bribery;
                let core_ok = ti4_engine_core::rules::action_cards::cost_is_payable(&view, seat, bribery, core);
                let string_ok = crate::action_cards::cost_is_payable_text(&state, ContentStore::embedded(), ti4_model::content_types::FULL, &p.id, &ActionCardId::new("bribery"));
                if state.rules_version >= ti4_model::state::RULES_ACTION_CARD_COSTS {
                    assert_eq!(core_ok, Some(string_ok), "seed {seed} {}: Bribery payable", p.id);
                }
                payable[usize::from(string_ok)] += 1;
            }
        }
        assert!(payable[0] > 10 && payable[1] > 10, "the tables cover seats with and without a good: {payable:?}");
    }
}
