//! Lane C8: determinisation of the hidden cards at a search root (`docs/C8_RESUME.md`; `docs/FORK_RNG_THEORY.md` 4.8).
//!
//! A search must not peek. [`determinize`] turns a position into a copy in which every card the searching seat has not seen is drawn
//! anew from the search's own [`SearchRng`] (never the game's streams), uniformly among the arrangements consistent with what that seat
//! has seen. What it resamples:
//!
//! | hidden | resampled as |
//! |---|---|
//! | the secret objectives in the other seats' hands and the secret deck | one pool: redealt over the same slots (hand sizes and deck size kept) |
//! | the unrevealed public objectives (the deck: stage I then stage II) | per stage, from the game's pool ([`crate::Table::objective_pools`]: which five were dealt is hidden too) minus every objective revealed or scored |
//! | the agenda deck | its order, except the agendas the seat itself put back with Politics (it knows where they are: kept in place) |
//! | the relic deck, the four exploration decks | their order (the search does not branch on them; without this a search would play on the real next card) |
//!
//! The draw is a relabelling: a uniform random permutation `sigma` of each class's unseen pool, applied to every state that holds the cards
//! (the live core and the positions the stop's question was asked in, `Asking::at` / the nested question's), so the copies stay consistent
//! with each other. A permutation of the pool applied to the true arrangement is a uniform draw of the arrangements: the result tells
//! nothing about the true order (the tests check it).
//!
//! Kept as they are (never in a pool): the seat's own hand and secrets, everything public (revealed, scored, laws, discards), the cards
//! other seats showed this seat (`CardsReveal` marks with this seat as the viewer), and, conservatively, every card named anywhere in the
//! state's open windows, scratch, trade, agenda and reserve slices or in the stop's question (a card a window holds or a question names:
//! moving it would make the window's state contradict the decks). How many were kept that way is in [`Report::pinned`].
//!
//! Not resampled (not hidden-information safe yet, see `docs/C8_RESUME.md`): the action cards (the deck and the other hands: action cards
//! are not simulated in v1), the promissory notes in hands, the agenda a covert draw put aside, the Politics player's two looked-at
//! agendas while its window is open (they are out of the deck then and named by the question), anything a faction outside the v1 four
//! may peek at.

use std::collections::HashSet;
use std::fmt::Write as _;

use ti4_engine_core::Seat;
use ti4_engine_core::choice::Choice;
use ti4_engine_core::containers::Deck;
use ti4_engine_core::generated::{Agenda, PublicObjective, SecretObjective};
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::query::objectives::stage_of;
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::{CoreState, ObjectiveRef};

use crate::Position;
use crate::chance::SearchRng;

/// What a determinisation did: the size of each class's unseen pool and how many cards were kept because the state names them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    /// The secret objectives redealt (other hands and the deck).
    pub secrets: usize,
    /// The public objectives drawn per stage (unrevealed, from the game's pool: deck cards and cards out of the game).
    pub objectives: [usize; 2],
    pub agendas: usize,
    pub relics: usize,
    pub explores: usize,
    /// Cards of a pool kept in place because an open window, the scratch, a trade, the stop's question or a known Politics placement names
    /// them.
    pub pinned: usize,
}

/// A copy of `pos` with every card `seat` has not seen drawn anew from `rng` (see the module doc), in [`crate::Mode::Search`] (a
/// determinised copy never answers a chance request from the game's streams: their future is hidden too). `pos` is not touched; the
/// game's streams are neither read nor moved.
#[must_use]
pub fn determinize(pos: &Position, seat: Seat, rng: &mut SearchRng) -> Position {
    determinize_report(pos, seat, rng).0
}

/// [`determinize`] with its [`Report`].
#[must_use]
pub fn determinize_report(pos: &Position, seat: Seat, rng: &mut SearchRng) -> (Position, Report) {
    let mut out = pos.fork_search();
    let mut report = Report::default();
    let states = pos.card_states();
    let text = mentioned(&states, pos.stop_choice(), seat);
    let named: HashSet<&str> = text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).collect();
    let mut pinned = 0usize;
    let mut keep = |name: String, extra: bool| {
        let kept = extra || named.contains(name.as_str());
        pinned += usize::from(kept);
        !kept
    };

    // The classes' pools, in a fixed order (the enum order) so that one seed gives one draw.
    let secrets: Vec<SecretObjective> = common(&states, |s| {
        let mut v: Vec<SecretObjective> = s.decks.secret.cards().to_vec();
        for p in s.players.iter().filter(|p| p.seat != seat) {
            v.extend_from_slice(p.cards.secrets.as_slice());
        }
        v
    })
    .into_iter()
    .filter(|c| keep(format!("{c:?}"), false))
    .collect();
    let table = pos.table();
    let objectives: [Vec<PublicObjective>; 2] = [0, 1].map(|stage| {
        common(&states, |s| {
            let public = |o: PublicObjective| s.objectives.revealed.as_slice().contains(&ObjectiveRef::Public(o)) || s.objectives.scored.iter().any(|set| set.public.contains(o));
            let mut v: Vec<PublicObjective> = table.objective_pools[stage].iter().copied().filter(|o| !public(*o)).collect();
            // A card still in the deck is always a candidate of its stage, whatever the pool says (a position not built by `start`).
            v.extend(s.decks.objective.cards().iter().copied().filter(|o| stage_of(*o) == Some(stage as u8 + 1)));
            v
        })
        .into_iter()
        .filter(|c| keep(format!("{c:?}"), false))
        .collect()
    });
    let placed: Vec<Agenda> = pos.placed_by(seat).collect();
    let agendas: Vec<Agenda> = common(&states, |s| s.decks.agenda.cards().to_vec()).into_iter().filter(|c| keep(format!("{c:?}"), placed.contains(c))).collect();
    let relics = common(&states, |s| s.decks.relic.cards().to_vec()).into_iter().filter(|c| keep(format!("{c:?}"), false)).collect::<Vec<_>>();
    let explores: Vec<Vec<_>> =
        (0..ti4_engine_core::generated::ExploreType::COUNT).map(|k| common(&states, |s| s.decks.explore[k].cards().to_vec()).into_iter().filter(|c| keep(format!("{c:?}"), false)).collect()).collect();
    report.pinned = pinned;
    report.secrets = secrets.len();
    report.objectives = [objectives[0].len(), objectives[1].len()];
    report.agendas = agendas.len();
    report.relics = relics.len();
    report.explores = explores.iter().map(Vec::len).sum();

    // One permutation per class, drawn in this order.
    let secrets = Relabel::draw(secrets, rng);
    let objectives = objectives.map(|pool| Relabel::draw(pool, rng));
    let agendas = Relabel::draw(agendas, rng);
    let relics = Relabel::draw(relics, rng);
    let explores: Vec<_> = explores.into_iter().map(|pool| Relabel::draw(pool, rng)).collect();

    out.rewrite_card_states(&mut |s: &mut CoreState| {
        secrets.deck(&mut s.decks.secret);
        for i in 0..s.players.len() {
            let p = s.players.get(i);
            if p.seat == seat || !p.cards.secrets.as_slice().iter().any(|c| secrets.moves(*c)) {
                continue;
            }
            for c in s.players.edit(i).cards.secrets.as_mut_slice() {
                *c = secrets.of(*c);
            }
        }
        let objective = &mut s.decks.objective;
        let relabelled: Vec<PublicObjective> = objective.cards().iter().map(|o| objectives.iter().fold(*o, |o, r| r.of(o))).collect();
        if relabelled.as_slice() != objective.cards() {
            objective.set(relabelled);
        }
        agendas.deck(&mut s.decks.agenda);
        relics.deck(&mut s.decks.relic);
        for (deck, r) in s.decks.explore.iter_mut().zip(&explores) {
            r.deck(deck);
        }
    });
    (out, report)
}

/// A permutation of one class's unseen pool: `of(pool[i]) = image[i]`, the identity on every other card.
struct Relabel<T> {
    pool: Vec<T>,
    image: Vec<T>,
}

impl<T: Copy + Ord> Relabel<T> {
    /// A uniform permutation of `pool` (Fisher-Yates over the search's stream).
    fn draw(pool: Vec<T>, rng: &mut SearchRng) -> Self {
        let mut image = pool.clone();
        for i in (1..image.len()).rev() {
            let j = rng.below(i + 1);
            image.swap(i, j);
        }
        Self { pool, image }
    }

    fn of(&self, card: T) -> T {
        self.pool.binary_search(&card).map_or(card, |i| self.image[i])
    }

    fn moves(&self, card: T) -> bool {
        self.of(card) != card
    }

    /// Relabel a deck (its order allocation replaced only when a card changes).
    fn deck(&self, deck: &mut Deck<T>) {
        if !deck.cards().iter().any(|c| self.moves(*c)) {
            return;
        }
        let cards = deck.cards().iter().map(|c| self.of(*c)).collect();
        deck.set(cards);
    }
}

/// The cards `of` lists in every state (sorted, without repeats): a card is unseen only if it is a candidate in each of them.
fn common<T: Copy + Ord>(states: &[&CoreState], of: impl Fn(&CoreState) -> Vec<T>) -> Vec<T> {
    let mut out = of(states[0]);
    out.sort_unstable();
    out.dedup();
    for s in &states[1..] {
        let mut more = of(s);
        more.sort_unstable();
        out.retain(|c| more.binary_search(c).is_ok());
    }
    out
}

/// The `Debug` text of what in the states may name a card that must not move: the open windows and the pending machine, the
/// scratch, the trade, the agenda slice (but its logs), the deck reserve, the marks (a reveal only when `seat` is its viewer), the players without the
/// hidden hands, and the stop's question.
fn mentioned(states: &[&CoreState], choice: Option<&Choice>, seat: Seat) -> String {
    let mut text = String::new();
    for s in states {
        let a = &s.agenda;
        // (The agenda slice without its logs: a resolved agenda is out of the deck, and the logs grow with the game.)
        let _ = write!(text, "{:?} {:?} {:?} {:?} {:?}", s.pending, s.transient, s.trade, s.cards.reserve, (&a.laws, a.in_play, &a.choices, a.veto_replacement, a.covert, &a.votes, &a.predictions));
        for (key, value) in s.marks.tail.iter() {
            if let MarkKey::CardsReveal(_, _, viewer, _, _) = key
                && *viewer != seat
            {
                continue;
            }
            let _ = write!(text, " {key:?} {value:?}");
        }
        for p in s.players.iter() {
            let mut p = p.clone();
            p.cards.hand.clear();
            if p.seat != seat {
                p.cards.secrets.clear();
            }
            let _ = write!(text, " {p:?}");
        }
    }
    if let Some(c) = choice {
        let _ = write!(text, " {c:?}");
    }
    text
}

/// What `seen` shows, as text: every accessor of the view that takes no imagined position (the facts, each seat's public standing, the
/// scored and revealed objectives and their progress, the agenda facts, the board as the seat sees it, the bound seat's own cards and what
/// others showed it). Two positions with the same digest for a seat look the same to it (as far as the view goes).
#[must_use]
pub fn seen_digest(seen: &Seen<'_>) -> String {
    let mut t = String::new();
    let me = seen.me();
    let _ = write!(
        t,
        "me {me:?} round {} phase {:?} active {:?} speaker {:?} players {:?} initiative {:?} seating {:?} custodians {}",
        seen.round(),
        seen.phase(),
        seen.active(),
        seen.speaker(),
        seen.players(),
        seen.initiative_order(),
        seen.seating_order(),
        seen.custodians_removed()
    );
    for &s in seen.players() {
        let _ = write!(t, "\nseat {:?} scored {:?} progress {:?}", seen.seat(s), seen.scored_by(s), seen.revealed_objective_progress(s, Default::default()));
        let _ = write!(t, " publics {:?} tokens {:?} units-in {:?} controlled {:?}", seen.scoreable_publics(s), seen.systems_with_token(s), seen.systems_with_units_of(s), seen.controlled_planets(s));
        let _ = write!(t, " prediction {:?}", seen.agenda_prediction_of(s));
    }
    let _ = write!(t, "\nrevealed {:?} agenda {:?} resolved {} facts {:?}", seen.revealed_objectives(), seen.agenda_in_play(), seen.agendas_resolved_this_round(), seen.agenda_facts());
    let _ = write!(t, "\nnotes {:?} support {:?} invasion {:?} battle {:?} active-system {:?}", seen.promissory_notes_in_play(), seen.support_holders(), seen.invasion(), seen.space_battle(), seen.active_system());
    for system in seen.map_systems() {
        let _ = write!(t, "\n{system:?} {:?}", seen.system(system).units());
    }
    let _ = write!(
        t,
        "\nmine: cards {:?} secrets {:?} notes {:?} shown {:?} secret-progress {:?}",
        seen.my_action_cards(),
        seen.my_secrets(),
        seen.my_notes(),
        seen.revealed_cards(),
        seen.my_secret_progress(Default::default())
    );
    t
}
