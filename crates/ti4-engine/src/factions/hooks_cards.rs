//! Faction hooks for hidden hands and card windows (`choice.rs`, `reactions.rs`, `action_cards.rs`): viewing hands, free-action discards, copying cards.
//!
//! Owned by one wave-B package at a time (`plans/BASE_FACTIONS_PLAN_2026-10-02.md`). Each
//! field is optional and called for every module; dispatch functions live here beside the
//! fields, iterate [`super::MODULES`] in order, and are what the shared engine calls. The rules
//! on `super::Hooks` (atomicity, ordering, ownership) apply. Tests install hooks with a
//! `#[cfg(test)] with_test_hooks(hooks, || ..)` that restores on panic, as in `hooks_combat.rs`.
//!
//! BF-00h-cards adds, besides one hook, three state-level routes that faction modules call:
//!
//! * **Reveals** ([`reveal`], [`reveal_hand`], [`revealed_to`], [`clear_reveals`],
//!   [`clear_reveals_from`]): "look at X's action cards / promissory notes / secret objectives".
//!   A reveal is a row in [`GameState::faction_marks`] (`cards:reveal:...`) naming the one seat that
//!   may see the cards. It is read back only through
//!   [`crate::choice::SeatObservation::revealed_action_cards`] and its two siblings, which answer
//!   for the seat the engine bound the view to; the public [`crate::choice::Observed`] has no
//!   accessor for it. The rows are empty unless a module reveals something, so games without
//!   faction modules serialise, hash and observe exactly as before.
//! * **Taking** ([`take_revealed_action_card`]): moves a card the taker has been shown, atomically.
//!   The choosing wrappers live in `action_cards.rs` ([`crate::action_cards::show_action_card`],
//!   [`crate::action_cards::take_from_revealed_hand`], [`crate::action_cards::look_at_hand_and_take`])
//!   because that is where the decision-delivery inventory scans.
//! * **Staged card events** ([`discard_chosen`], [`take_revealed_action_card`] stage
//!   `ACTION_CARD_DISCARDED` / `ACTION_CARD_TAKEN`): a hook holds a `TimingContext`, which has no
//!   resolver, so it cannot open a window itself. The effect is staged here and
//!   [`crate::reactions::announce_staged_card_events`] opens the windows.
//!
//! Typed events added by this package:
//!
//! | Event | Staged by | Announced by | Payload |
//! |---|---|---|---|
//! | `ACTION_CARD_TAKEN` | [`take_revealed_action_card`] | `reactions::announce_staged_card_events` | `player` (taker), `from` (previous holder); no card id (private to taker and owner) |
//!
//! `ACTION_CARD_DISCARDED` already exists (payload `player`, `card`); a staged discard is announced
//! with the same payload, and only then does the card reach the discard pile and
//! `last_action_discarded` (Reverse Engineer's window reads both).

use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::{ActionCardId, LeaderId, PlayerId};
use ti4_model::state::{GameState, LeaderStatus};

use crate::timing::TimingContext;

/// How long a reveal lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevealScope {
    /// Only while the choice that asked for it is open. The choosing wrappers in
    /// `action_cards.rs` clear it themselves, on every path out.
    Choice,
    /// Until the end of the current action. The engine must call [`clear_reveals`] with this scope
    /// when an action completes (see `plans/evidence/BF-00h-cards.md`, Requests).
    Action,
    /// Until the module that revealed it calls [`clear_reveals_from`] with the same `source`:
    /// "while a condition holds" is the module's condition, checked by the module.
    Standing,
}

impl RevealScope {
    const fn token(self) -> &'static str {
        match self {
            Self::Choice => "choice",
            Self::Action => "action",
            Self::Standing => "standing",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "choice" => Some(Self::Choice),
            "action" => Some(Self::Action),
            "standing" => Some(Self::Standing),
            _ => None,
        }
    }
}

/// What kind of hidden card a reveal shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RevealKind {
    /// Action cards in a hand (ids are action card aliases).
    ActionCards,
    /// Promissory notes in a hand (ids are note ids), faceup notes excluded: those are public.
    PromissoryNotes,
    /// Secret objectives in a hand (ids are secret objective aliases).
    SecretObjectives,
}

impl RevealKind {
    const fn token(self) -> &'static str {
        match self {
            Self::ActionCards => "action_cards",
            Self::PromissoryNotes => "promissory_notes",
            Self::SecretObjectives => "secret_objectives",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token {
            "action_cards" => Some(Self::ActionCards),
            "promissory_notes" => Some(Self::PromissoryNotes),
            "secret_objectives" => Some(Self::SecretObjectives),
            _ => None,
        }
    }
}

/// Cards of one kind that `owner` has shown to a viewer, as the viewer's observation reports them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revealed {
    /// The holder whose hand was shown.
    pub owner: PlayerId,
    /// Which kind of card.
    pub kind: RevealKind,
    /// The shown ids, in the order they were revealed, only those the owner still holds.
    pub ids: Vec<String>,
}

const REVEAL_PREFIX: &str = "cards:reveal:";
const STAGED_PREFIX: &str = "cards:staged:";

fn reveal_key(
    scope: RevealScope,
    source: &str,
    viewer: &PlayerId,
    owner: &PlayerId,
    kind: RevealKind,
) -> String {
    format!(
        "{REVEAL_PREFIX}{}|{source}|{viewer}|{owner}|{}",
        scope.token(),
        kind.token()
    )
}

/// Whether `owner` currently holds `id` as a hidden card of `kind`.
fn held(state: &GameState, owner: &PlayerId, kind: RevealKind, id: &str) -> bool {
    match kind {
        RevealKind::ActionCards => state
            .player(owner)
            .is_some_and(|seat| seat.action_cards.iter().any(|card| card.as_str() == id)),
        RevealKind::SecretObjectives => state.player(owner).is_some_and(|seat| {
            seat.secret_objectives
                .iter()
                .any(|card| card.as_str() == id)
        }),
        RevealKind::PromissoryNotes => {
            state.promissory_notes.get(id) == Some(owner) && !state.promissory_faceup.contains(id)
        }
    }
}

/// Show `viewer` the cards `ids` that `owner` holds, for `scope`. `source` names the effect (a
/// card or ability id) so a [`RevealScope::Standing`] reveal can be ended by the module that made
/// it.
///
/// Atomic: returns `false`, changing nothing, unless the seats are two different players, `ids` is
/// non-empty and the owner holds **every** one of them now. Revealing again merges into the same
/// row. Only `viewer`'s observation ever reports these cards.
pub fn reveal(
    state: &mut GameState,
    viewer: &PlayerId,
    owner: &PlayerId,
    kind: RevealKind,
    ids: &[String],
    scope: RevealScope,
    source: &str,
) -> bool {
    if viewer == owner
        || ids.is_empty()
        || state.player(viewer).is_none()
        || state.player(owner).is_none()
        || source.contains('|')
        || !ids.iter().all(|id| held(state, owner, kind, id))
    {
        return false;
    }
    let key = reveal_key(scope, source, viewer, owner, kind);
    let mut shown: Vec<String> = state
        .faction_marks
        .get(&key)
        .map(|row| row.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    for id in ids {
        if !shown.contains(id) {
            shown.push(id.clone());
        }
    }
    state.faction_marks.insert(key, shown.join(","));
    true
}

/// [`reveal`] for the whole of `owner`'s action-card hand: "look at that player's action cards".
/// Returns the number of distinct cards shown (0 when the hand is empty, and nothing is recorded).
pub fn reveal_hand(
    state: &mut GameState,
    viewer: &PlayerId,
    owner: &PlayerId,
    scope: RevealScope,
    source: &str,
) -> usize {
    let hand: Vec<String> = state
        .player(owner)
        .map(|seat| {
            seat.action_cards
                .iter()
                .map(|card| card.as_str().to_owned())
                .collect()
        })
        .unwrap_or_default();
    if reveal(
        state,
        viewer,
        owner,
        RevealKind::ActionCards,
        &hand,
        scope,
        source,
    ) {
        let mut distinct = hand;
        distinct.sort();
        distinct.dedup();
        distinct.len()
    } else {
        0
    }
}

/// What `viewer` may see of other players' hidden cards right now.
///
/// Rows for the same owner and kind (different scopes or sources) are merged; an id the owner no
/// longer holds is dropped, so a card that left the hand is never reported. Ordered by owner
/// id then kind, ids in reveal order.
#[must_use]
pub fn revealed_to(state: &GameState, viewer: &PlayerId) -> Vec<Revealed> {
    let mut merged: std::collections::BTreeMap<(PlayerId, RevealKind), Vec<String>> =
        std::collections::BTreeMap::new();
    for (key, row) in state.faction_marks.range(REVEAL_PREFIX.to_owned()..) {
        let Some(detail) = key.strip_prefix(REVEAL_PREFIX) else {
            break;
        };
        let parts: Vec<&str> = detail.split('|').collect();
        let [scope, _source, row_viewer, owner, kind] = parts[..] else {
            continue;
        };
        if RevealScope::from_token(scope).is_none() || row_viewer != viewer.as_str() {
            continue;
        }
        let Some(kind) = RevealKind::from_token(kind) else {
            continue;
        };
        let owner = PlayerId::new(owner);
        let ids = merged.entry((owner.clone(), kind)).or_default();
        for id in row.split(',') {
            if held(state, &owner, kind, id) && !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_owned());
            }
        }
    }
    merged
        .into_iter()
        .filter(|(_, ids)| !ids.is_empty())
        .map(|((owner, kind), ids)| Revealed { owner, kind, ids })
        .collect()
}

fn clear_where(state: &mut GameState, keep: impl Fn(&[&str]) -> bool) -> usize {
    let doomed: Vec<String> = state
        .faction_marks
        .keys()
        .filter(|key| {
            key.strip_prefix(REVEAL_PREFIX).is_some_and(|detail| {
                let parts: Vec<&str> = detail.split('|').collect();
                !keep(&parts)
            })
        })
        .cloned()
        .collect();
    for key in &doomed {
        state.faction_marks.remove(key);
    }
    doomed.len()
}

/// End every reveal of `scope`; returns how many rows ended. The engine calls this with
/// [`RevealScope::Action`] when an action completes; the choosing wrappers call it with
/// [`RevealScope::Choice`] through [`clear_reveals_from`].
pub fn clear_reveals(state: &mut GameState, scope: RevealScope) -> usize {
    clear_where(state, |parts| parts.first() != Some(&scope.token()))
}

/// End every reveal made by `source` (any scope); returns how many rows ended.
pub fn clear_reveals_from(state: &mut GameState, source: &str) -> usize {
    clear_where(state, |parts| parts.get(1) != Some(&source))
}

/// End the reveals of `owner`'s cards made by `source` to `viewer`.
pub fn clear_reveal_between(
    state: &mut GameState,
    source: &str,
    viewer: &PlayerId,
    owner: &PlayerId,
) -> usize {
    clear_where(state, |parts| {
        !(parts.get(1) == Some(&source)
            && parts.get(2) == Some(&viewer.as_str())
            && parts.get(3) == Some(&owner.as_str()))
    })
}

/// Whether `viewer` has been shown `card` in `owner`'s hand.
#[must_use]
pub fn is_revealed_action_card(
    state: &GameState,
    viewer: &PlayerId,
    owner: &PlayerId,
    card: &ActionCardId,
) -> bool {
    revealed_to(state, viewer).iter().any(|shown| {
        &shown.owner == owner
            && shown.kind == RevealKind::ActionCards
            && shown.ids.iter().any(|id| id == card.as_str())
    })
}

// -- staged card events -----------------------------------------------------------------------------

/// A card event staged by a hook for [`crate::reactions::announce_staged_card_events`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StagedCardEvent {
    /// `player` discarded `card` from their hand (`ACTION_CARD_DISCARDED`).
    Discarded {
        /// The discarding player.
        player: PlayerId,
        /// The card.
        card: ActionCardId,
    },
    /// `player` took `card` out of `from`'s hand (`ACTION_CARD_TAKEN`).
    Taken {
        /// The taker.
        player: PlayerId,
        /// The previous holder.
        from: PlayerId,
        /// The card.
        card: ActionCardId,
    },
}

fn stage(state: &mut GameState, event: &StagedCardEvent) {
    let next = state
        .faction_marks
        .range(STAGED_PREFIX.to_owned()..)
        .map_while(|(key, _)| key.strip_prefix(STAGED_PREFIX))
        .filter_map(|n| n.parse::<u64>().ok())
        .max()
        .map_or(0, |n| n + 1);
    let row = match event {
        StagedCardEvent::Discarded { player, card } => format!("discarded|{player}||{card}"),
        StagedCardEvent::Taken { player, from, card } => format!("taken|{player}|{from}|{card}"),
    };
    state
        .faction_marks
        .insert(format!("{STAGED_PREFIX}{next:08}"), row);
}

/// Remove and return every staged card event, in the order staged.
pub fn drain_staged(state: &mut GameState) -> Vec<StagedCardEvent> {
    let keys: Vec<String> = state
        .faction_marks
        .range(STAGED_PREFIX.to_owned()..)
        .map_while(|(key, _)| key.starts_with(STAGED_PREFIX).then(|| key.clone()))
        .collect();
    keys.into_iter()
        .filter_map(|key| {
            let row = state.faction_marks.remove(&key)?;
            let parts: Vec<&str> = row.split('|').collect();
            let [kind, player, from, card] = parts[..] else {
                return None;
            };
            let (player, card) = (PlayerId::new(player), ActionCardId::new(card));
            match kind {
                "discarded" => Some(StagedCardEvent::Discarded { player, card }),
                "taken" => Some(StagedCardEvent::Taken {
                    player,
                    from: PlayerId::new(from),
                    card,
                }),
                _ => None,
            }
        })
        .collect()
}

/// Whether any card event is waiting to be announced.
#[must_use]
pub fn has_staged(state: &GameState) -> bool {
    state
        .faction_marks
        .range(STAGED_PREFIX.to_owned()..)
        .next()
        .is_some_and(|(key, _)| key.starts_with(STAGED_PREFIX))
}

/// Move `card` from `owner`'s hand to `taker`'s, if `taker` has been shown it (see [`reveal`]).
///
/// Atomic: `false` and no change unless both seats exist and differ, the card is currently shown
/// to `taker` **and** `owner` holds it. The first copy in hand order goes. The taker's hand limit
/// is not enforced here (a Crafty hand has none): the caller runs
/// [`crate::action_cards::enforce_hand_limit`] after. Stages `ACTION_CARD_TAKEN`.
pub fn take_revealed_action_card(
    state: &mut GameState,
    taker: &PlayerId,
    owner: &PlayerId,
    card: &ActionCardId,
) -> bool {
    if taker == owner
        || state.player(taker).is_none()
        || !is_revealed_action_card(state, taker, owner, card)
    {
        return false;
    }
    let Some(at) = state
        .player(owner)
        .and_then(|seat| seat.action_cards.iter().position(|held| held == card))
    else {
        return false;
    };
    let moved = state
        .player_mut(owner)
        .map(|seat| seat.action_cards.remove(at));
    let Some(moved) = moved else {
        return false;
    };
    if let Some(seat) = state.player_mut(taker) {
        seat.action_cards.push(moved);
    }
    stage(
        state,
        &StagedCardEvent::Taken {
            player: taker.clone(),
            from: owner.clone(),
            card: card.clone(),
        },
    );
    true
}

/// Discard `card` from `player`'s hand as a free choice of the effect that asked (Stall Tactics:
/// "Discard 1 action card from your hand"). Atomic: `false` unless the player holds the card.
///
/// Only removes it from the hand and stages `ACTION_CARD_DISCARDED`; the card reaches the discard
/// pile when the staged event is announced (`reactions::announce_staged_card_events`), exactly as
/// a played card does.
pub fn discard_chosen(state: &mut GameState, player: &PlayerId, card: &ActionCardId) -> bool {
    let Some(at) = state
        .player(player)
        .and_then(|seat| seat.action_cards.iter().position(|held| held == card))
    else {
        return false;
    };
    if let Some(seat) = state.player_mut(player) {
        seat.action_cards.remove(at);
    }
    stage(
        state,
        &StagedCardEvent::Discarded {
            player: player.clone(),
            card: card.clone(),
        },
    );
    true
}

// -- copying another player's agent ---------------------------------------------------------------

/// Agents whose text `player` may use as its own through Ssruu, Clever Genome (`yssarilagent`):
/// "This card has the text ability of each other player's agent, even if that agent is exhausted."
///
/// Empty unless `player` holds `yssarilagent` readied. Every other seat's agents in seat order,
/// readied or exhausted (a purged or locked one is not an agent in play). This is the query only:
/// resolving the borrowed text needs `leaders::use_leader`'s per-leader dispatch without its
/// status gate, which lives in `leaders.rs` (see the evidence file's Requests).
#[must_use]
pub fn borrowable_agents(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> Vec<(PlayerId, LeaderId)> {
    let ssruu = LeaderId::new("yssarilagent");
    if state
        .player(player)
        .and_then(|seat| seat.leaders.get(&ssruu).copied())
        != Some(LeaderStatus::Readied)
    {
        return Vec::new();
    }
    state
        .players
        .iter()
        .filter(|seat| &seat.id != player)
        .flat_map(|seat| {
            seat.leaders
                .iter()
                .filter(|(leader, status)| {
                    matches!(status, LeaderStatus::Readied | LeaderStatus::Exhausted)
                        && content
                            .get(ContentType::Leaders, leader.as_str())
                            .and_then(|record| record.text("type"))
                            .is_some_and(|kind| kind.eq_ignore_ascii_case("agent"))
                })
                .map(|(leader, _)| (seat.id.clone(), leader.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

// -- the hook table -----------------------------------------------------------------------------

/// Hooks for this area. A module sets only the ones it needs (`..CardHooks::NONE`).
#[derive(Debug, Clone, Copy)]
#[allow(
    clippy::type_complexity,
    reason = "plain fn-pointer table; aliases would only move the signatures elsewhere"
)]
pub struct CardHooks {
    /// A card-specific transaction may reach a player without ordinary adjacency.
    pub transaction_reach: Option<fn(&GameState, &ContentStore, &PlayerId, &PlayerId) -> bool>,
    /// Whether a transaction between `active` (the player whose turn it is) and `other` is exempt
    /// from the once-per-player-per-turn limit. **Any** module's `true` exempts it.
    ///
    /// Yssaril `yssarilbt` Deepgloom Executable: "You can allow other players to use your STALL
    /// TACTICS or SCHEMING faction abilities; when you do, you may resolve a transaction with that
    /// player. During the action phase, that transaction does not count against the
    /// once-per-player transaction limit for that turn." The module keeps its own record (a
    /// `faction_marks` flag) of who was allowed; this hook only reports it. Called for every
    /// module; check your own condition. Consulted by [`transaction_exempt_from_limit`].
    pub transaction_limit_exempt:
        Option<fn(&GameState, &ContentStore, &PlayerId, &PlayerId) -> bool>,
    /// Whether `player` votes first on an agenda ("You vote first", Argent Flight's Zeal). **Any**
    /// module's `true` seats the player ahead of the rest of the voting order; several such seats
    /// keep clockwise order from the speaker. Called for every module; check your own condition.
    /// Consulted by [`votes_first`] from `vote::VoteWindow::new`.
    pub votes_first: Option<fn(&GameState, &PlayerId) -> bool>,
    /// Extra votes `player` casts, with the content corpus in hand (`vote.rs` banks it beside
    /// `leaders::vote_bonus`; the older `Hooks::vote_bonus` has no content). Summed over modules.
    pub vote_bonus_with_content: Option<fn(&GameState, &ContentStore, &PlayerId) -> i64>,
    /// Runs after Ssruu (`yssarilagent`) successfully used another seat's agent text through
    /// `leaders::use_leader_text`: `(context, borrower, source_agent)`. Ssruu is already exhausted
    /// by then; a module hangs follow-ups here (bookkeeping, a record of what was borrowed).
    /// Called for every module; check your own condition.
    pub borrowed_agent_used: Option<fn(&mut TimingContext<'_>, &PlayerId, &LeaderId)>,
}

impl CardHooks {
    /// No hooks.
    pub const NONE: Self = Self {
        transaction_reach: None,
        transaction_limit_exempt: None,
        votes_first: None,
        vote_bonus_with_content: None,
        borrowed_agent_used: None,
    };
}

#[cfg(test)]
thread_local! {
    /// Extra hooks a test registers beside the (empty) module table, per test thread.
    static TEST_HOOKS: std::cell::RefCell<Vec<CardHooks>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `run` with `extra` consulted after every module, on this thread only, restoring the
/// previous table afterwards even if `run` panics.
#[cfg(test)]
pub(crate) fn with_test_hooks<T>(extra: CardHooks, run: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_HOOKS.with(|hooks| {
                hooks.borrow_mut().pop();
            });
        }
    }
    TEST_HOOKS.with(|hooks| hooks.borrow_mut().push(extra));
    let _restore = Restore;
    run()
}

#[cfg(not(test))]
fn hooks() -> impl Iterator<Item = CardHooks> {
    super::MODULES.iter().map(|module| module.hooks.cards)
}

#[cfg(test)]
fn hooks() -> impl Iterator<Item = CardHooks> {
    let extra: Vec<CardHooks> = TEST_HOOKS.with(|hooks| hooks.borrow().clone());
    super::MODULES
        .iter()
        .map(|module| module.hooks.cards)
        .chain(extra)
}

#[must_use]
pub fn transaction_reach(
    state: &GameState,
    content: &ContentStore,
    a: &PlayerId,
    b: &PlayerId,
) -> bool {
    hooks()
        .filter_map(|h| h.transaction_reach)
        .any(|f| f(state, content, a, b))
}

/// Whether a faction module exempts the transaction between `active` and `other` from the
/// per-turn limit. `false` with every module empty. Called by
/// `transactions::available_actions`, `transactions::may_open_again` and
/// `transactions::TradeWindow::open_with_content` (BF-F5).
#[must_use]
pub fn transaction_exempt_from_limit(
    state: &GameState,
    content: &ContentStore,
    active: &PlayerId,
    other: &PlayerId,
) -> bool {
    hooks()
        .filter_map(|h| h.transaction_limit_exempt)
        .any(|f| f(state, content, active, other))
}

/// Whether a module seats `player` first in the voting order. `false` with every module empty.
#[must_use]
pub fn votes_first(state: &GameState, player: &PlayerId) -> bool {
    hooks()
        .filter_map(|h| h.votes_first)
        .any(|f| f(state, player))
}

/// Extra votes from modules that need the content corpus. `0` with every module empty.
#[must_use]
pub fn vote_bonus_with_content(
    state: &GameState,
    content: &ContentStore,
    player: &PlayerId,
) -> i64 {
    hooks()
        .filter_map(|h| h.vote_bonus_with_content)
        .map(|f| f(state, content, player))
        .sum()
}

/// Tell every module that `borrower` used `source_agent`'s text through Ssruu.
pub fn borrowed_agent_used(
    context: &mut TimingContext<'_>,
    borrower: &PlayerId,
    source_agent: &LeaderId,
) {
    for f in hooks().filter_map(|h| h.borrowed_agent_used) {
        f(context, borrower, source_agent);
    }
}

// -- COMMAND_TOKEN_PLACED -------------------------------------------------------------------------

/// The typed event name for "a command token was placed on the board".
pub const COMMAND_TOKEN_PLACED: &str = "COMMAND_TOKEN_PLACED";

/// Where a placed command token came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenPool {
    /// Taken from the player's command sheet: tactic pool (activation).
    Tactic,
    /// Taken from the command sheet: fleet pool.
    Fleet,
    /// Taken from the command sheet: strategy pool.
    Strategy,
    /// Taken from reinforcements (Stymie, Mahact-style effects, card and agenda placements).
    Reinforcements,
}

impl TokenPool {
    /// The payload token for `pool`.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Tactic => "tactic",
            Self::Fleet => "fleet",
            Self::Strategy => "strategy",
            Self::Reinforcements => "reinforcements",
        }
    }
}

/// Payload of `COMMAND_TOKEN_PLACED`: `player` (whose token), `system`, `pool` ([`TokenPool::token`]).
#[must_use]
pub fn command_token_placed_payload(
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    pool: TokenPool,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut payload = std::collections::BTreeMap::new();
    payload.insert("player".to_owned(), player.to_string().into());
    payload.insert("system".to_owned(), system.to_string().into());
    payload.insert("pool".to_owned(), pool.token().into());
    payload
}

/// Open the `COMMAND_TOKEN_PLACED` window for a token already placed. Call this at the placement
/// site, after the token is on the board, from any code holding a context and a resolver.
///
/// # Errors
/// [`crate::timing::TimingError`] when the window cannot be resolved.
pub fn announce_command_token_placed(
    context: &mut TimingContext<'_>,
    resolver: &mut crate::timing::Resolver,
    player: &PlayerId,
    system: &ti4_model::id::SystemId,
    pool: TokenPool,
) -> Result<(), crate::timing::TimingError> {
    let event = context.event_sequence.next(
        COMMAND_TOKEN_PLACED,
        command_token_placed_payload(player, system, pool),
    )?;
    resolver.emit_with_context(context, event, |_, _| {})?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::id::SecretObjectiveId;

    fn pid(name: &str) -> PlayerId {
        PlayerId::new(name)
    }

    fn card(name: &str) -> ActionCardId {
        ActionCardId::new(name)
    }

    fn deal(state: &mut GameState, who: &str, cards: &[&str]) {
        state.player_mut(&pid(who)).unwrap().action_cards =
            cards.iter().map(|name| card(name)).collect();
    }

    #[test]
    fn nothing_is_revealed_and_nothing_recorded_by_default() {
        let mut state = crate::fixtures::game(&["a", "b", "c"]);
        deal(&mut state, "b", &["sabotage", "bribery"]);
        assert!(revealed_to(&state, &pid("a")).is_empty());
        assert!(state.faction_marks.is_empty());
        assert!(!has_staged(&state));
        assert!(drain_staged(&mut state).is_empty());
        assert!(
            !transaction_exempt_from_limit(&state, ContentStore::embedded(), &pid("a"), &pid("b")),
            "no module exempts a transaction"
        );
    }

    #[test]
    fn a_reveal_reaches_only_its_viewer_and_only_while_held() {
        let mut state = crate::fixtures::game(&["a", "b", "c"]);
        deal(&mut state, "b", &["sabotage", "bribery", "sabotage"]);
        assert_eq!(
            reveal_hand(&mut state, &pid("a"), &pid("b"), RevealScope::Action, "mi"),
            2,
            "two distinct cards"
        );
        let seen = revealed_to(&state, &pid("a"));
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].owner, pid("b"));
        assert_eq!(seen[0].ids, vec!["sabotage", "bribery"]);
        assert!(revealed_to(&state, &pid("c")).is_empty(), "c was not shown");
        assert!(
            revealed_to(&state, &pid("b")).is_empty(),
            "the owner is not a viewer of their own reveal"
        );

        // A card that leaves the hand stops being reported, though the row remains.
        deal(&mut state, "b", &["bribery"]);
        assert_eq!(revealed_to(&state, &pid("a"))[0].ids, vec!["bribery"]);
    }

    #[test]
    fn a_reveal_is_atomic_and_refuses_what_is_not_held() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        deal(&mut state, "b", &["sabotage"]);
        let ids = ["sabotage".to_owned(), "bribery".to_owned()];
        assert!(!reveal(
            &mut state,
            &pid("a"),
            &pid("b"),
            RevealKind::ActionCards,
            &ids,
            RevealScope::Choice,
            "mi"
        ));
        assert!(state.faction_marks.is_empty(), "a refusal changes nothing");
        assert!(
            !reveal(
                &mut state,
                &pid("b"),
                &pid("b"),
                RevealKind::ActionCards,
                &ids[..1],
                RevealScope::Choice,
                "mi"
            ),
            "no self reveal"
        );
        assert_eq!(
            reveal_hand(&mut state, &pid("a"), &pid("a"), RevealScope::Choice, "x"),
            0
        );
        deal(&mut state, "b", &[]);
        assert_eq!(
            reveal_hand(&mut state, &pid("a"), &pid("b"), RevealScope::Choice, "x"),
            0,
            "an empty hand shows nothing and records nothing"
        );
        assert!(state.faction_marks.is_empty());
    }

    #[test]
    fn scopes_end_independently() {
        let mut state = crate::fixtures::game(&["a", "b", "c"]);
        deal(&mut state, "b", &["sabotage"]);
        deal(&mut state, "c", &["bribery"]);
        reveal_hand(&mut state, &pid("a"), &pid("b"), RevealScope::Action, "mi");
        reveal_hand(
            &mut state,
            &pid("a"),
            &pid("c"),
            RevealScope::Standing,
            "yssarilcommander",
        );
        reveal_hand(
            &mut state,
            &pid("a"),
            &pid("c"),
            RevealScope::Choice,
            "spynet",
        );
        assert_eq!(revealed_to(&state, &pid("a")).len(), 2);

        assert_eq!(clear_reveals(&mut state, RevealScope::Choice), 1);
        assert_eq!(revealed_to(&state, &pid("a")).len(), 2, "action + standing");
        assert_eq!(clear_reveals(&mut state, RevealScope::Action), 1);
        let left = revealed_to(&state, &pid("a"));
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].owner, pid("c"));
        assert_eq!(clear_reveals_from(&mut state, "yssarilcommander"), 1);
        assert!(revealed_to(&state, &pid("a")).is_empty());
        assert!(state.faction_marks.is_empty(), "nothing is left behind");
    }

    #[test]
    fn notes_and_secrets_reveal_only_what_is_held_and_hidden() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        state.player_mut(&pid("b")).unwrap().secret_objectives =
            vec![SecretObjectiveId::new("otf")];
        let note = "spynet:yssaril".to_owned();
        state.promissory_notes.insert(note.clone(), pid("b"));
        assert!(reveal(
            &mut state,
            &pid("a"),
            &pid("b"),
            RevealKind::SecretObjectives,
            &["otf".to_owned()],
            RevealScope::Choice,
            "yssarilcommander"
        ));
        assert!(reveal(
            &mut state,
            &pid("a"),
            &pid("b"),
            RevealKind::PromissoryNotes,
            std::slice::from_ref(&note),
            RevealScope::Choice,
            "yssarilcommander"
        ));
        assert_eq!(revealed_to(&state, &pid("a")).len(), 2);
        // A faceup note is public, so it is not a hidden-card reveal.
        state.promissory_faceup.insert(note.clone());
        assert_eq!(revealed_to(&state, &pid("a")).len(), 1);
        assert!(!reveal(
            &mut state,
            &pid("a"),
            &pid("b"),
            RevealKind::PromissoryNotes,
            &[note],
            RevealScope::Choice,
            "x"
        ));
    }

    #[test]
    fn taking_needs_the_card_to_have_been_shown_and_moves_exactly_one() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        deal(&mut state, "a", &["bribery"]);
        deal(&mut state, "b", &["sabotage", "sabotage"]);
        let before = state.clone();
        assert!(
            !take_revealed_action_card(&mut state, &pid("a"), &pid("b"), &card("sabotage")),
            "not shown: refused"
        );
        assert_eq!(state, before, "atomic refusal");

        reveal_hand(&mut state, &pid("a"), &pid("b"), RevealScope::Choice, "mi");
        assert!(take_revealed_action_card(
            &mut state,
            &pid("a"),
            &pid("b"),
            &card("sabotage")
        ));
        assert_eq!(state.player(&pid("a")).unwrap().action_cards.len(), 2);
        assert_eq!(state.player(&pid("b")).unwrap().action_cards.len(), 1);
        assert_eq!(
            drain_staged(&mut state),
            vec![StagedCardEvent::Taken {
                player: pid("a"),
                from: pid("b"),
                card: card("sabotage")
            }]
        );
        assert!(!has_staged(&state));
        assert!(
            !take_revealed_action_card(&mut state, &pid("b"), &pid("a"), &card("sabotage")),
            "b was never shown a's hand"
        );
    }

    #[test]
    fn a_chosen_discard_stages_in_order_and_refuses_a_card_not_held() {
        let mut state = crate::fixtures::game(&["a", "b"]);
        deal(&mut state, "a", &["sabotage", "bribery"]);
        let before = state.clone();
        assert!(!discard_chosen(&mut state, &pid("a"), &card("flank_speed")));
        assert_eq!(state, before);
        assert!(discard_chosen(&mut state, &pid("a"), &card("bribery")));
        assert!(discard_chosen(&mut state, &pid("a"), &card("sabotage")));
        assert!(state.player(&pid("a")).unwrap().action_cards.is_empty());
        assert_eq!(
            drain_staged(&mut state),
            vec![
                StagedCardEvent::Discarded {
                    player: pid("a"),
                    card: card("bribery")
                },
                StagedCardEvent::Discarded {
                    player: pid("a"),
                    card: card("sabotage")
                },
            ]
        );
    }

    #[test]
    fn ssruu_borrows_other_seats_agents_even_when_exhausted() {
        let content = ContentStore::embedded();
        let mut state = crate::fixtures::seated_game(
            &[("a", "yssaril"), ("b", "sol"), ("c", "hacan")],
            ti4_model::content_types::DEFAULT,
        );
        let borrowed = borrowable_agents(&state, content, &pid("a"));
        let names: Vec<&str> = borrowed.iter().map(|(_, id)| id.as_str()).collect();
        assert!(names.contains(&"solagent") && names.contains(&"hacanagent"));
        assert!(!names.contains(&"yssarilagent"), "not its own");
        state
            .player_mut(&pid("b"))
            .unwrap()
            .leaders
            .insert(LeaderId::new("solagent"), LeaderStatus::Exhausted);
        assert!(
            borrowable_agents(&state, content, &pid("a"))
                .iter()
                .any(|(_, id)| id.as_str() == "solagent"),
            "exhausted agents count"
        );
        assert!(
            borrowable_agents(&state, content, &pid("b")).is_empty(),
            "only the holder of Ssruu borrows"
        );
        state
            .player_mut(&pid("a"))
            .unwrap()
            .leaders
            .insert(LeaderId::new("yssarilagent"), LeaderStatus::Exhausted);
        assert!(borrowable_agents(&state, content, &pid("a")).is_empty());
    }

    #[test]
    fn command_token_placed_payload_and_window_carry_player_system_and_pool() {
        let payload = command_token_placed_payload(
            &pid("a"),
            &ti4_model::id::SystemId::new("27"),
            TokenPool::Reinforcements,
        );
        assert_eq!(payload.get("player").and_then(|v| v.as_str()), Some("a"));
        assert_eq!(payload.get("system").and_then(|v| v.as_str()), Some("27"));
        assert_eq!(
            payload.get("pool").and_then(|v| v.as_str()),
            Some("reinforcements")
        );
        assert_eq!(payload.len(), 3);

        // The window opens through a real resolver with no listener: it announces and returns.
        let mut state = crate::fixtures::game(&["a", "b"]);
        let mut resolver = crate::fixtures::armed_resolver(&state);
        let mut table = crate::choice::Table::new();
        let result = crate::fixtures::with_context(
            &mut state,
            ti4_model::content_types::DEFAULT,
            None,
            &mut table,
            |ctx| {
                announce_command_token_placed(
                    ctx,
                    &mut resolver,
                    &pid("a"),
                    &ti4_model::id::SystemId::new("27"),
                    TokenPool::Tactic,
                )
            },
        );
        assert!(result.is_ok());
        assert!(
            resolver
                .log()
                .iter()
                .any(|line| line.contains("COMMAND_TOKEN_PLACED")),
            "the event reached the resolver: {:?}",
            resolver.log()
        );
    }

    #[test]
    fn card_hooks_are_neutral_when_empty() {
        let state = crate::fixtures::game(&["a", "b"]);
        let content = ContentStore::embedded();
        assert!(!votes_first(&state, &pid("a")));
        assert_eq!(vote_bonus_with_content(&state, content, &pid("a")), 0);
    }

    #[test]
    fn a_test_hook_exempts_a_transaction_only_inside_its_scope() {
        let state = crate::fixtures::game(&["a", "b"]);
        let content = ContentStore::embedded();
        let exempt = CardHooks {
            transaction_limit_exempt: Some(|_, _, _, other| other.as_str() == "b"),
            ..CardHooks::NONE
        };
        with_test_hooks(exempt, || {
            assert!(transaction_exempt_from_limit(
                &state,
                content,
                &pid("a"),
                &pid("b")
            ));
            assert!(!transaction_exempt_from_limit(
                &state,
                content,
                &pid("b"),
                &pid("a")
            ));
        });
        assert!(!transaction_exempt_from_limit(
            &state,
            content,
            &pid("a"),
            &pid("b")
        ));
    }
}
