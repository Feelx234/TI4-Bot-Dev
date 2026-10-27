//! The offline dual of `Area::AgendaLaws`: every read of [`crate::agenda_view::AgendaView`] asked of one
//! position next to the string function it replaces, every answer compared. Used by `board_equivalence`
//! (golden positions, templates) and the tests.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_engine_core::generated::Agenda;
use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::agenda_view::AgendaView;
use crate::generated::IdStr;

/// What was compared.
#[derive(Debug, Default, Clone)]
pub struct AgendaCheck {
    pub positions: u64,
    pub unmapped: u64,
    pub compared: u64,
    pub mismatches: Vec<String>,
}

impl AgendaCheck {
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

/// Compare every agenda read on one position.
pub fn compare(label: &str, gs: &GameState, sources: SourceSet, out: &mut AgendaCheck) {
    out.positions += 1;
    let content = ContentStore::embedded();
    let Some(v) = AgendaView::full(gs, sources) else {
        out.unmapped += 1;
        return;
    };
    out.eq(label, "reveal_order", &crate::agenda::reveal_order_text(gs), &v.reveal_order());
    out.eq(label, "ready_limit", &crate::laws::agenda_ready_limit_text(gs), &v.ready_limit());
    out.eq(label, "flat_vote_amount", &crate::laws::flat_votes(gs).then_some(1), &v.flat_vote_amount());
    out.eq(label, "voting_order", &crate::vote::VoteWindow::new(gs, "ratification", Vec::new()).order().to_vec(), &v.voting_order());
    let players: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    for p in &players {
        out.eq(label, "extra_votes", &crate::vote::extra_votes_text(gs, p), &v.extra_votes(p));
        out.eq(label, "votable_planets", &crate::vote::votable_planets_text(gs, content, sources, p), &v.votable_planets(p));
        out.eq(label, "political_secret_bars", &crate::laws::political_secret_bars_text(gs, p), &v.political_secret_bars(p));
    }
    for agenda in Agenda::ALL {
        let alias = agenda.as_str();
        out.eq(label, &format!("is_law {alias}"), &crate::vote::is_law_text(content, alias), &crate::agenda_view::is_law(alias));
        let choices = crate::vote::outcomes_text(gs, content, sources, alias);
        out.eq(label, &format!("outcomes {alias}"), &choices, &v.outcomes(alias));
        // Synthetic ballots over the outcomes: silence, one winner, a tie of the first two.
        let mut ballots: Vec<BTreeMap<String, i64>> = vec![BTreeMap::new()];
        if let Some(first) = choices.first() {
            ballots.push(BTreeMap::from([(first.clone(), 4)]));
            if let Some(second) = choices.get(1) {
                ballots.push(BTreeMap::from([(first.clone(), 3), (second.clone(), 3)]));
                ballots.push(BTreeMap::from([(second.clone(), 5), (first.clone(), 1)]));
            }
        }
        for counts in &ballots {
            let ballot = crate::vote::Ballot { votes: BTreeMap::new(), counts: counts.clone() };
            out.eq(
                label,
                &format!("undisputed_winner {alias} {counts:?}"),
                &crate::vote::undisputed_winner_text(&ballot, &choices),
                &v.undisputed_winner(alias, counts, &choices),
            );
            out.eq(
                label,
                &format!("tiebreak_candidates {alias} {counts:?}"),
                &crate::vote::tiebreak_candidates_text(&ballot, &choices),
                &v.tiebreak_candidates(alias, counts, &choices),
            );
        }
    }
}
