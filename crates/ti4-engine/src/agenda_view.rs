//! `Area::AgendaLaws` (package B9): the typed agenda reads of `ti4_engine_core::rules::agenda` behind
//! the string interface. One [`AgendaView`] is the typed view of one position (the codecs of the
//! Meta, Seats, Players, Board, Strategy, Objectives and Agenda slices). It implements the core's
//! `Env`: the answers of the faction modules come from the string engine, so the sums stay exact. A
//! text id the tables do not know makes a method answer `None` and the caller keeps the string
//! answer.

use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Agenda, AgendaElects, Planet};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::rules::agenda::{self as rules, Cx, Env, Tiebreak};
use ti4_engine_core::state::{Outcome, Slice, SliceSet};
use ti4_engine_core::tables::Scope;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlanetId, PlayerId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// What every agenda read needs.
pub const READS: SliceSet = SliceSet::of(&[
    Slice::Meta,
    Slice::Seats,
    Slice::Players,
    Slice::Board,
    Slice::Strategy,
    Slice::Objectives,
    Slice::Agenda,
]);
/// What `extra_votes` and the ready limit need.
pub const READS_LIGHT: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Agenda]);

/// The typed view of one position.
pub struct AgendaView<'a> {
    view: CoreView,
    cx: Ctx,
    state: &'a GameState,
    scope: Scope,
    blanked: bool,
}

fn typed<T: IdStr>(id: &str) -> Option<T> {
    T::from_id(id)
}

impl<'a> AgendaView<'a> {
    fn build(state: &'a GameState, sources: SourceSet, reads: SliceSet) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, reads).ok()?;
        Some(Self { view, cx, state, scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(state) })
    }

    /// The view for the full set of reads; `None` when something is not mapped.
    #[must_use]
    pub fn full(state: &'a GameState, sources: SourceSet) -> Option<Self> {
        Self::build(state, sources, READS)
    }

    /// The view for the reads that need no board.
    #[must_use]
    pub fn light(state: &'a GameState) -> Option<Self> {
        Self::build(state, ti4_model::content_types::FULL, READS_LIGHT)
    }

    fn rules_cx(&self) -> Cx<'_> {
        Cx { core: &self.view, scope: self.scope, blanked: self.blanked, env: self }
    }

    fn seat(&self, p: &PlayerId) -> Option<Seat> {
        self.cx.owners.seat(p.as_str())
    }

    fn pid(&self, s: Seat) -> Option<PlayerId> {
        self.cx.owners.id(s).map(PlayerId::new)
    }

    fn players(&self, seats: &[Seat]) -> Option<Vec<PlayerId>> {
        seats.iter().map(|s| self.pid(*s)).collect()
    }

    fn render(&self, o: Outcome) -> Option<String> {
        self.cx.keys.render_outcome(o)
    }

    /// An outcome text of the agenda `alias`'s vote (the hidden agenda's under Covert Legislation).
    fn parse(&self, alias: &str, text: &str) -> Option<Outcome> {
        let voted = rules::voted_agenda(&self.view, typed::<Agenda>(alias)?);
        let o = self.cx.keys.parse_outcome(text, Some(voted.data().elects))?;
        (self.render(o).as_deref() == Some(text)).then_some(o)
    }

    /// `vote::outcomes` (the corpus knows `alias`).
    #[must_use]
    pub fn outcomes(&self, alias: &str) -> Option<Vec<String>> {
        let agenda = typed::<Agenda>(alias)?;
        rules::outcomes(&self.rules_cx(), agenda).as_slice().iter().map(|o| self.render(*o)).collect()
    }

    /// `vote::voted_agenda`.
    #[must_use]
    pub fn voted_agenda(&self, alias: &str) -> Option<String> {
        Some(rules::voted_agenda(&self.view, typed::<Agenda>(alias)?).as_str().to_owned())
    }

    /// `vote::extra_votes`.
    #[must_use]
    pub fn extra_votes(&self, player: &PlayerId) -> Option<i64> {
        Some(rules::extra_votes(&self.view, self.seat(player)?))
    }

    /// `vote::flat_vote_amount`.
    #[must_use]
    pub fn flat_vote_amount(&self) -> Option<Option<i64>> {
        Some(rules::flat_vote_amount(&self.rules_cx()))
    }

    /// `vote::votable_planets`.
    #[must_use]
    pub fn votable_planets(&self, player: &PlayerId) -> Option<Vec<PlanetId>> {
        let seat = self.seat(player)?;
        Some(rules::votable_planets(&self.rules_cx(), seat).as_slice().iter().map(|p| PlanetId::new(p.as_str())).collect())
    }

    /// `agenda::resolve_agenda_phase`'s voting order.
    #[must_use]
    pub fn reveal_order(&self) -> Option<Vec<PlayerId>> {
        self.players(rules::reveal_order(&self.view).as_slice())
    }

    /// `VoteWindow::new`'s order.
    #[must_use]
    pub fn voting_order(&self) -> Option<Vec<PlayerId>> {
        self.players(rules::voting_order(&self.rules_cx()).as_slice())
    }

    /// `laws::agenda_ready_limit`.
    #[must_use]
    pub fn ready_limit(&self) -> Option<Option<usize>> {
        Some(rules::ready_limit(&self.view, self.blanked))
    }

    /// `laws::political_secret_bars`.
    #[must_use]
    pub fn political_secret_bars(&self, player: &PlayerId) -> Option<bool> {
        Some(rules::political_secret_bars(&self.view, self.seat(player)?))
    }

    fn ballot(&self, alias: &str, counts: &std::collections::BTreeMap<String, i64>, choices: &[String]) -> Option<(Vec<(Outcome, i64)>, Vec<Outcome>)> {
        let counts = counts.iter().map(|(k, v)| Some((self.parse(alias, k)?, *v))).collect::<Option<Vec<_>>>()?;
        let choices = choices.iter().map(|c| self.parse(alias, c)).collect::<Option<Vec<_>>>()?;
        Some((counts, choices))
    }

    /// `vote::undisputed_winner`.
    #[must_use]
    pub fn undisputed_winner(&self, alias: &str, counts: &std::collections::BTreeMap<String, i64>, choices: &[String]) -> Option<Option<String>> {
        let (counts, choices) = self.ballot(alias, counts, choices)?;
        match rules::undisputed_winner(&counts, &choices) {
            None => Some(None),
            Some(o) => Some(Some(self.render(o)?)),
        }
    }

    /// `vote::tiebreak_candidates`: the tied outcomes in text order, or everything on offer.
    #[must_use]
    pub fn tiebreak_candidates(&self, alias: &str, counts: &std::collections::BTreeMap<String, i64>, choices: &[String]) -> Option<Vec<String>> {
        let (counts, choices) = self.ballot(alias, counts, choices)?;
        match rules::tiebreak_candidates(&counts, &choices) {
            Tiebreak::Tied(list) => {
                let mut texts = list.as_slice().iter().map(|o| self.render(*o)).collect::<Option<Vec<_>>>()?;
                texts.sort();
                Some(texts)
            }
            Tiebreak::Silent(list) => list.as_slice().iter().map(|o| self.render(*o)).collect(),
        }
    }
}

/// `vote::is_law` of an agenda alias (needs no view).
#[must_use]
pub fn is_law(alias: &str) -> Option<bool> {
    Some(rules::is_law(typed::<Agenda>(alias)?))
}

impl Env for AgendaView<'_> {
    fn is_nekro(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| crate::factions::nekro::is_nekro(self.state, &p))
    }

    fn elder_qanoj(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| crate::leaders::elder_qanoj(self.state, &p))
    }

    fn votes_first(&self, who: Seat) -> bool {
        self.pid(who).is_some_and(|p| crate::factions::hooks_cards::votes_first(self.state, &p))
    }

    fn oceans(&self, who: Seat) -> SmallList<Planet, 4> {
        let mut out = SmallList::new();
        if let Some(p) = self.pid(who) {
            for o in crate::factions::deepwrought::oceans(self.state, &p) {
                if let Some(planet) = typed::<Planet>(o.as_str()) {
                    out.push(planet);
                }
            }
        }
        out
    }
}

/// The hint kind of a voted agenda, for tests.
#[must_use]
pub fn elects_of(alias: &str) -> Option<AgendaElects> {
    Some(typed::<Agenda>(alias)?.data().elects)
}
