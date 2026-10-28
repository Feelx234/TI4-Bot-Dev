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
            match v.has_legal_target(&p, card) {
                None => out.bailed += 1,
                typed => out.eq(label, &format!("has_legal_target {card}"), &crate::action_cards::has_legal_target_text(gs, content, ti4_model::content_types::FULL, &p, card), &typed),
            }
        }
    }
}
