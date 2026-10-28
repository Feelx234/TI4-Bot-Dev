//! The window dual of `vote::VoteWindow` (package B9; docs/ENGINE_CORE_DRIVER.md 3).
//!
//! The string vote is driven one answer at a time by the game, so there is no closure to run twice. On
//! `agenda=dual` the window keeps a [`Log`]: the position it opened in and every question it was asked with
//! the option id it was answered with. When the vote finishes, the core [`Vote`] window is run from the
//! opening position through the D1 driver, answered with the same ids, and compared: every question
//! (`compare_choice`: ids, kinds, labels, payload, context, decision hashes), the winner and the position
//! it leaves (the planets exhausted).
//!
//! The core window has every stage of the string window (Genetic Recombination and its tribute, planets, trade
//! goods, Predictive Intelligence, the tiebreak) and the Executive Order spender. It does not express the
//! Deepwrought oceans, a Valefar Assimilator carrying Genetic Recombination, a Keleres agent that is open or
//! could be offered to a voter with commodities, a Last Bastion resource bonus under the Executive Order, or a
//! vote on more than eight outcomes: for such a position the log records nothing and the string result stands.
//! A change the window does not make itself between two answers (extra votes, vote bonuses, trade goods, fleet
//! tokens, Gila, Blood Pact notes) is not replayed: the comparison is skipped.

use ti4_model::content_types::SourceSet;
use ti4_model::id::PlayerId;
use ti4_model::state::GameState;

use crate::choice::Choice;

/// What the window saw, if the dual is on and the position is expressible.
#[derive(Clone, Default)]
pub struct Log(Option<Box<Inner>>);

impl std::fmt::Debug for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "Log(recording)" } else { "Log(off)" })
    }
}

/// The log is bookkeeping of the dual, not part of the window's identity.
impl PartialEq for Log {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Log {}

#[derive(Clone)]
struct Inner {
    /// The position at the first answer (cards played between the reveal and the first vote count).
    open: Option<GameState>,
    /// What each voter's extra votes were then: a change later in the vote (a card played between voters) is
    /// not replayed.
    extras: Vec<i64>,
    /// What the position held at the first answer that the window does not change itself.
    facts: Vec<Fact>,
    /// The last answer, to tell the window's own spending from outside changes.
    last: Option<(String, usize)>,
    spender: Option<PlayerId>,
    unstable: bool,
    alias: String,
    choices: Vec<String>,
    order: Vec<PlayerId>,
    sources: Option<SourceSet>,
    script: Vec<(Choice, String)>,
}

impl Log {
    /// Start recording a vote on `alias` (only on `agenda=dual`, and only for a position the core holds).
    #[must_use]
    pub fn open(state: &GameState, alias: &str, choices: &[String], order: &[PlayerId]) -> Self {
        #[cfg(feature = "typed-core")]
        {
            if crate::dual::backend(crate::dual::Area::AgendaLaws) == crate::dual::Backend::Dual {
                return Self(Some(Box::new(Inner {
                    open: None,
                    extras: Vec::new(),
                    facts: Vec::new(),
                    last: None,
                    spender: None,
                    unstable: false,
                    alias: alias.to_owned(),
                    choices: choices.to_vec(),
                    order: order.to_vec(),
                    sources: None,
                    script: Vec::new(),
                })));
            }
        }
        let _ = (state, alias, choices, order);
        Self(None)
    }

    /// The Executive Order spender (set before the first question).
    pub fn set_spender(&mut self, player: &PlayerId) {
        if let Some(inner) = self.0.as_mut() {
            inner.spender = Some(player.clone());
        }
    }

    /// One question was answered.
    pub fn note(&mut self, state: &GameState, sources: SourceSet, asked: &Choice, answer: &str) {
        let Some(inner) = self.0.as_mut() else { return };
        let extras: Vec<i64> = inner.order.iter().map(|p| crate::vote::extra_votes_text(state, p)).collect();
        let facts: Vec<Fact> = inner.order.iter().map(|p| Fact::of(state, p)).collect();
        if inner.open.is_none() {
            #[cfg(feature = "typed-core")]
            if !supported(state, &inner.choices, &inner.order, inner.spender.as_ref()) {
                self.0 = None;
                return;
            }
            inner.open = Some(state.clone());
            inner.extras = extras;
            inner.facts = facts;
        } else {
            if inner.extras != extras {
                inner.unstable = true;
            }
            // What the window changes itself: goods by a spend, fleet tokens by a tribute.
            let spent = inner.last.as_ref().filter(|(id, _)| id.starts_with("spend|"));
            let paid = inner.last.as_ref().filter(|(id, _)| id.starts_with("recombine|") || id.starts_with("tribute|"));
            for (i, (now, then)) in facts.iter().zip(&inner.facts).enumerate() {
                let own_spend = spent.is_some_and(|(_, who)| *who == i);
                if now.same_but(then, own_spend, paid.is_some()) {
                    continue;
                }
                inner.unstable = true;
            }
            inner.facts = facts;
        }
        inner.last = Some((answer.to_owned(), inner.order.iter().position(|p| *p == asked.player).unwrap_or(usize::MAX)));
        inner.sources = Some(sources);
        inner.script.push((asked.clone(), answer.to_owned()));
    }

    /// The vote finished with `winner`; `state` is the position it left. Compares and stops recording.
    pub fn finish(&mut self, state: &GameState, winner: Option<&str>, counts: &std::collections::BTreeMap<String, i64>) {
        let Some(inner) = self.0.take() else { return };
        #[cfg(feature = "typed-core")]
        if !inner.unstable {
            replay(&inner, state, winner, counts);
        }
        #[cfg(not(feature = "typed-core"))]
        let _ = (inner, state, winner, counts);
    }
}

/// What one voter holds that the core window snapshots or reads: compared between answers.
#[derive(Clone, PartialEq, Eq)]
struct Fact {
    bonus: i64,
    gila: bool,
    pact: Vec<PlayerId>,
    goods: i32,
    fleet: i32,
}

impl Fact {
    fn of(state: &GameState, p: &PlayerId) -> Self {
        let seat = state.player(p);
        Self {
            bonus: crate::leaders::vote_bonus(state, p) + vote_bonus_content(state, p),
            gila: crate::promissory::has_commander_ability(state, p, "hacancommander"),
            pact: pact_partners(state, p),
            goods: seat.map_or(0, |s| s.trade_goods),
            fleet: seat.map_or(0, |s| s.fleet_tokens),
        }
    }

    /// Equal apart from what the window itself spends or pays.
    fn same_but(&self, then: &Self, own_spend: bool, paid: bool) -> bool {
        self.bonus == then.bonus
            && self.gila == then.gila
            && self.pact == then.pact
            && (own_spend || self.goods == then.goods)
            && (paid || self.fleet == then.fleet)
    }
}

fn vote_bonus_content(state: &GameState, p: &PlayerId) -> i64 {
    crate::factions::hooks_cards::vote_bonus_with_content(state, ti4_content::ContentStore::embedded(), p)
}

/// The seats whose backing of an outcome gives `p` the Blood Pact's votes.
fn pact_partners(state: &GameState, p: &PlayerId) -> Vec<PlayerId> {
    use std::collections::BTreeMap;
    state
        .players
        .iter()
        .map(|q| q.id.clone())
        .filter(|q| q != p)
        .filter(|q| {
            let votes: BTreeMap<PlayerId, String> = std::iter::once((q.clone(), "x".to_owned())).collect();
            crate::factions::empyrean_units::blood_pact_votes(state, &votes, p, "x") > 0
        })
        .collect()
}

#[cfg(feature = "typed-core")]
fn supported(state: &GameState, choices: &[String], order: &[PlayerId], spender: Option<&PlayerId>) -> bool {
    if choices.len() > ti4_engine_core::choice::agenda::Tally::MAX {
        return false;
    }
    // A Valefar Assimilator carrying Genetic Recombination: the carrier is not the technology.
    if state.players.iter().any(|q| crate::factions::nekro::assimilated_card(state, &q.id, crate::factions::mahact::RECOMBINATION).is_some()) {
        return false;
    }
    // Last Bastion's dock raises the resource value of its planet: the Executive Order spends resources.
    if let Some(s) = spender
        && state.controlled_planets(s).iter().any(|(_, pl)| crate::factions::bastion_units::resource_bonus(state, pl) != 0)
    {
        return false;
    }
    order.iter().all(|p| {
        crate::factions::deepwrought::oceans(state, p).is_empty()
            && !crate::factions::keleres::agent_could_grant(state, p)
            && crate::factions::keleres::spendable_commodities(state, p) == 0
    })
}

#[cfg(feature = "typed-core")]
fn replay(inner: &Inner, now: &GameState, winner: Option<&str>, counts: &std::collections::BTreeMap<String, i64>) {
    let compared = replay_inner(inner, now, winner, counts).is_some();
    crate::dual::note_window("vote::VoteWindow", compared);
}

/// `None` when the core could not follow the position (nothing was compared).
fn replay_inner(inner: &Inner, now: &GameState, winner: Option<&str>, counts: &std::collections::BTreeMap<String, i64>) -> Option<()> {
    use ti4_engine_core::generated::Agenda;
    use ti4_engine_core::kernel::stage::{AbilityTable, Window};
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::rules::agenda::window::{Facts, Vote};
    use ti4_engine_core::state::Slice;

    use crate::core_sync::{Ctx, Footprint, Sources, with_core};
    use crate::dual::{Area, Run, compare_choice, compare_runs, compare_value};
    use crate::dual_window::{Asked, Recording, drive};
    use crate::generated::IdStr;
    use crate::render::Render;

    const LABEL: &str = "vote::VoteWindow";
    const FOOTPRINT: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Cards, Slice::Pending],
        &[Slice::Board, Slice::Players],
    );
    let (Some(sources), Some(agenda)) = (inner.sources, Agenda::from_id(&inner.alias)) else { return None };
    let open = inner.open.as_ref()?;
    let Ok(mut cx) = Ctx::of(open) else { return None };
    let r = Render::new(cx.owners.ids().to_vec());
    let voted = match open.covert_agenda.as_deref() {
        Some(hidden) if inner.alias == "covert" => Agenda::from_id(hidden),
        _ => Some(agenda),
    };
    let elects = voted.map(|a| a.data().elects)?;
    let choices = inner.choices.iter().map(|c| cx.keys.parse_outcome(c, Some(elects))).collect::<Option<Vec<_>>>()?;
    let order = inner.order.iter().map(|p| cx.owners.seat(p.as_str())).collect::<Option<Vec<_>>>()?;
    let mut facts = Facts::new(crate::core_adapter::scope(sources), crate::laws::blanked(open));
    for (p, seat) in inner.order.iter().zip(&order) {
        let at = usize::from(seat.0) & 7;
        for partner in pact_partners(open, p) {
            if let Some(other) = cx.owners.seat(partner.as_str()) {
                facts.pact[at].insert(other);
            }
        }
    }
    let mut window = Vote::new(agenda, &choices, &order, facts);
    if let Some(spender) = inner.spender.as_ref() {
        let seat = cx.owners.seat(spender.as_str())?;
        window = window.with_spender(seat);
    }
    let speaker = cx.owners.seat(open.speaker.as_str())?;
    let mut driver = ti4_engine_core::driver::Driver::new(window, Resolver::new(&order, Some(speaker)), AbilityTable::default(), 1);
    let mut script = Recording::default();
    script.questions = inner.script.iter().map(|(c, a)| Asked { choice: c.clone(), answer: Some(a.clone()) }).collect();
    let kernel = crate::production_view::kernel();
    let mut core_state = open.clone();
    let driven = with_core(&mut core_state, &mut cx, Sources::default(), FOOTPRINT, |core, _| drive(&mut driver, core, kernel, &r, open, &script));
    let Ok(Ok(driven)) = driven else { return None };
    let area = Area::AgendaLaws;
    if compare_value(area, &format!("{LABEL}.questions"), &inner.script.len(), &driven.rendered.len()) {
        for ((asked, _), rendered) in inner.script.iter().zip(&driven.rendered) {
            compare_choice(area, LABEL, Some(asked), Some(rendered));
        }
    }
    // The votes each outcome received (zero tallies are not recorded by the string ballot).
    let core_counts: std::collections::BTreeMap<String, i64> =
        driver.window.counts().as_slice().iter().filter_map(|(o, c)| Some((cx.keys.render_outcome(*o)?, *c))).collect();
    compare_value(area, &format!("{LABEL}.counts"), counts, &core_counts);
    let core_winner = driver.window.winner().and_then(|o| cx.keys.render_outcome(o));
    let winner = winner.map(ToOwned::to_owned);
    let mut string_view = open.clone();
    string_view.exhausted_planets = now.exhausted_planets.clone();
    // What the window also changes: spent trade goods, returned fleet tokens, exhausted technologies.
    for seat in &mut string_view.players {
        if let Some(after) = now.player(&seat.id) {
            seat.trade_goods = after.trade_goods;
            seat.fleet_tokens = after.fleet_tokens;
            seat.exhausted_technologies = after.exhausted_technologies.clone();
        }
    }
    compare_runs(area, LABEL, &string_view, &Run::new(winner), &core_state, &Run::new(core_winner));
    compare_value(area, &format!("{LABEL}.finished"), &true, &(driven.finished && driver.window.is_done()));
    let _ = <Vote as Window>::stage;
    Some(())
}
