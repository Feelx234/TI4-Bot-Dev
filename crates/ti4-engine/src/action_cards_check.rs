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
    pub mismatches: Vec<String>,
}

impl ActionCardsCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
        self.bailed += o.bailed;
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
            for who in &players {
                for actor in std::iter::once(None).chain(players.iter().map(|p| Some(p.as_str()))) {
                    let event = crate::event::Event::new(1, "WINDOW", actor.map(|a| ("player".to_owned(), serde_json::json!(a))).into_iter().collect());
                    let string = crate::action_cards::reaction_window_text(gs, content, who, card, &event, sys);
                    match v.reaction_can_matter(who, card, sys.as_str(), actor) {
                        None => out.bailed += 1,
                        typed => out.eq(label, &format!("reaction_can_matter {card} {sys} actor {actor:?}"), &string, &typed),
                    }
                }
            }
        }
    }
}
