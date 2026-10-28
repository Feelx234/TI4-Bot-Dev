//! The offline dual of `Area::Strategy`: every read of [`crate::strategy_view::StratView`] asked of
//! one position next to the string function it replaces, every answer compared. Used by
//! `board_equivalence` (golden positions, templates) and the tests.

use ti4_content::ContentStore;
use ti4_engine_core::generated::StrategyCard;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, StrategyCardId};
use ti4_model::state::GameState;

use crate::generated::IdStr;
use crate::strategy_view::StratView;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct StrategyCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub mismatches: Vec<String>,
}

impl StrategyCheck {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.unmapped += o.unmapped;
        self.compared += o.compared;
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

/// Compare every strategy read on one position.
pub fn compare(label: &str, gs: &GameState, sources: SourceSet, out: &mut StrategyCheck) {
    out.positions += 1;
    let content = ContentStore::embedded();
    let Some(v) = StratView::full(gs, content, sources) else {
        out.unmapped += 1;
        return;
    };
    out.eq(label, "strategy_pick_order", &crate::phase::strategy_pick_order_text(gs), &v.pick_order());
    out.eq(label, "next_strategy_picker", &crate::phase::next_strategy_picker_text(gs), &v.next_picker());
    let open = crate::phase::next_strategy_picker_text(gs).map(|p| (p, gs.unclaimed_strategy_cards.clone()));
    out.eq(label, "draft", &open, &v.draft());
    let cards: Vec<StrategyCardId> = StrategyCard::ALL.iter().map(|c| StrategyCardId::new(c.as_str())).collect();
    for card in &cards {
        out.eq(label, &format!("secondary_costs_token {card}"), &crate::strategy::secondary_costs_token_text(content, card), &crate::strategy_view::secondary_costs_token(card));
    }
    let players: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    for p in &players {
        let unused: Vec<StrategyCardId> = gs.player(p).map_or_else(Vec::new, |s| s.unused_strategy_cards().into_iter().cloned().collect());
        out.eq(label, "unused_cards", &unused, &v.unused_cards(p));
        out.eq(label, "commodity_limit", &crate::strategy_cards::commodity_limit_text(gs, content, p), &crate::strategy_view::commodity_limit(gs, p));
        for card in &cards {
            out.eq(
                label,
                &format!("secondary_eligible {card}"),
                &crate::strategy::secondary_eligible_text(gs, content, sources, p, card),
                &v.secondary_eligible(p, card),
            );
        }
    }
}
