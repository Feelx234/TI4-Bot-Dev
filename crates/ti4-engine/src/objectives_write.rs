//! `Area::Objectives` (package A4), the mutation half: the writes of `objectives.rs`, `secrets.rs` and
//! `GameState::reveal_objective` (a victory point moved, a public objective revealed, a secret drawn, a score
//! recorded) as `ti4_engine_core::rules::objectives` functions over the slices they touch, and the two objective
//! questions (`rules::objective_windows`: score an objective, return a secret over the hand limit) as lockstep
//! windows.
//!
//! Every function answers `None` when the position is outside what the core holds (more than 16 owners, an id
//! the tables do not know, a deck reserve of a turn redo, a value the codecs refuse): the caller keeps the string
//! write. The string callers keep what the core cannot do (the price of a bought objective, the leaders a score
//! unlocks, the staged event of Yin's breakthrough, the log, who is asked next in a scoring window); see
//! `docs/ENGINE_KNOWN_ISSUES.md`, A4.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

/// The scoring window's arguments that the core window needs and the string window keeps private.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreArgs {
    /// The status window (publics and secrets in one list); an event window offers secrets only.
    pub status: bool,
    /// The feat occurrence the one-per-player cap records the score under.
    pub occurrence: Option<u64>,
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{PublicObjective, SecretObjective};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::query::objectives as q;
    use ti4_engine_core::rules::objective_windows::{HandLimit, ScoreOne};
    use ti4_engine_core::rules::objectives as rules;
    use ti4_engine_core::state::{ObjectiveRef, Slice};
    use ti4_model::id::{ObjectiveId, PlayerId, SecretObjectiveId};
    use ti4_model::state::GameState;

    use super::ScoreArgs;
    use crate::choice::Choice;
    use crate::core_sync::keys::{parse_ledger, render_ledger};
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::{Recording, lend};
    use crate::generated::IdStr;

    /// A victory point move and its ledger row.
    const ADJUST: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Ledger], &[Slice::Players, Slice::Ledger]);
    /// The objective deck and the revealed list.
    const REVEAL: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Decks, Slice::Objectives], &[Slice::Decks, Slice::Objectives]);
    /// A secret drawn into a hand.
    const DRAW: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks], &[Slice::Players, Slice::Decks]);
    /// A score: the scored sets, the hand, the points and their ledger, the end of the game.
    const SCORE: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Ledger],
        &[Slice::Players, Slice::Objectives, Slice::Ledger],
    );
    /// The scoring question: a score and the end of the game it can cause.
    const SCORE_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Ledger, Slice::Pending],
        &[Slice::Meta, Slice::Players, Slice::Objectives, Slice::Ledger],
    );
    /// The hand limit question: the hand and the secret deck.
    const HAND_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Decks, Slice::Pending],
        &[Slice::Players, Slice::Decks],
    );

    fn objective(alias: &str) -> Option<ObjectiveRef> {
        PublicObjective::from_id(alias)
            .map(ObjectiveRef::Public)
            .or_else(|| SecretObjective::from_id(alias).map(ObjectiveRef::Secret))
    }

    /// `objectives::adjust_victory_points`: the points actually moved. A reason the ledger codec does not type
    /// bails.
    pub fn adjust(gs: &mut GameState, player: &PlayerId, delta: i32, reason: &str) -> Option<i32> {
        let typed = parse_ledger(reason).filter(|r| render_ledger(*r) == reason)?;
        lend(gs, ADJUST, Some(player), |core, who| {
            ti4_engine_core::kernel::fx::adjust_victory_points(core, who.expect("a seated call"), delta, typed)
        })
    }

    /// `GameState::reveal_objective`. The outer `None` is a bail.
    pub fn reveal_top(gs: &mut GameState) -> Option<Option<ObjectiveId>> {
        if gs.deck_reserve.is_some() {
            return None;
        }
        lend(gs, REVEAL, None, |core, _| rules::reveal_top(core).map(|o| ObjectiveId::new(o.as_str())))
    }

    /// `objectives::reveal_stage`.
    pub fn reveal_stage(gs: &mut GameState, stage: u8) -> Option<Option<ObjectiveId>> {
        if gs.deck_reserve.is_some() {
            return None;
        }
        lend(gs, REVEAL, None, |core, _| rules::reveal_stage(core, stage).map(|o| ObjectiveId::new(o.as_str())))
    }

    /// The draw of `secrets::draw`, before the hand limit.
    pub fn draw_secret(gs: &mut GameState, player: &PlayerId) -> Option<Option<SecretObjectiveId>> {
        if gs.deck_reserve.is_some() {
            return None;
        }
        lend(gs, DRAW, Some(player), |core, who| {
            ti4_engine_core::kernel::fx::draw_secret(core, who.expect("a seated call")).map(|s| SecretObjectiveId::new(s.as_str()))
        })
    }

    /// The state half of `objectives::award`. `points` is what the string side read from the content: the core
    /// table must agree, or it bails.
    pub fn score_public(gs: &mut GameState, player: &PlayerId, alias: &ObjectiveId, points: i32) -> Option<()> {
        let obj = objective(alias.as_str())?;
        let table = match obj {
            ObjectiveRef::Public(p) => q::points_of(p),
            ObjectiveRef::Secret(s) => q::secret_points(s),
        };
        if i32::from(table) != points {
            return None;
        }
        lend(gs, SCORE, Some(player), |core, who| {
            ti4_engine_core::kernel::fx::score_public(core, who.expect("a seated call"), obj);
        })
    }

    /// The state half of `secrets::award`; the outer `None` is a bail, the inner one a secret not in the hand.
    pub fn score_secret(gs: &mut GameState, player: &PlayerId, alias: &SecretObjectiveId, points: i32) -> Option<Option<i32>> {
        let secret = SecretObjective::from_id(alias.as_str())?;
        if i32::from(q::secret_points(secret)) != points {
            return None;
        }
        lend(gs, SCORE, Some(player), |core, who| {
            ti4_engine_core::kernel::fx::score_secret(core, who.expect("a seated call"), secret).then_some(points)
        })
    }

    // -- the windows -----------------------------------------------------------------------------------

    /// The core half of `scoring_step`: one player's turn in a scoring window. `asked` is the question the
    /// string window put (its options are the offers). `None` where the core cannot follow: a plot or any option
    /// that is no objective, a bought objective (the price is the string's), a position the codecs refuse.
    pub fn score_one(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: ti4_model::content_types::SourceSet,
        asked: &Choice,
        args: ScoreArgs,
        script: &Recording,
    ) -> Option<CoreWindow<(bool, Option<ObjectiveId>)>> {
        if crate::dual_window::outside(gs, script) {
            return None;
        }
        let mut offers = Vec::new();
        for option in asked.options.iter().filter(|o| !o.is_decline()) {
            let obj = objective(&option.id)?;
            if matches!(obj, ObjectiveRef::Public(p) if q::cost_of(p).is_some()) {
                return None;
            }
            offers.push(obj);
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(asked.player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(
            ScoreOne::new(seat, args.status, &offers, args.occurrence),
            Resolver::new(&[seat], Some(seat)),
            AbilityTable::default(),
            1,
        );
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), SCORE_WINDOW, |core, _| {
            crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script)
        })
        .ok()?
        .ok()?;
        let scored = driver.window.scored;
        // What the core does not do: the leaders a public score unlocks and Yin's staged event.
        let public_now = scored.is_some_and(|o| before_revealed(&before, o) || matches!(o, ObjectiveRef::Public(_)));
        if let (Some(obj), true) = (scored, public_now) {
            crate::objectives::award_tail(gs, content, sources, asked.player.clone(), &ObjectiveId::new(crate::render::objectives::objective_text(obj)));
        }
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        let value = (driven.finished, scored.map(|o| ObjectiveId::new(crate::render::objectives::objective_text(o))));
        Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
    }

    fn before_revealed(gs: &GameState, obj: ObjectiveRef) -> bool {
        let text = crate::render::objectives::objective_text(obj);
        gs.revealed_objectives.iter().any(|o| o.as_str() == text)
    }

    /// The core half of `secrets::enforce_hand_limit`.
    pub fn hand_limit(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if crate::dual_window::outside(gs, script) {
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(HandLimit::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), HAND_WINDOW, |core, _| {
            crate::dual_window::drive(&mut driver, core, &kernel, &r, &before, script)
        })
        .ok()?
        .ok()?;
        let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
        Some(CoreWindow { run: Run::new(driven.finished).with_events_vec(events), questions: driven.rendered })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{ObjectiveId, PlayerId, SecretObjectiveId};
    use ti4_model::state::GameState;

    use super::ScoreArgs;
    use crate::choice::Choice;
    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn adjust(_: &mut GameState, _: &PlayerId, _: i32, _: &str) -> Option<i32> {
        None
    }
    pub fn reveal_top(_: &mut GameState) -> Option<Option<ObjectiveId>> {
        None
    }
    pub fn reveal_stage(_: &mut GameState, _: u8) -> Option<Option<ObjectiveId>> {
        None
    }
    pub fn draw_secret(_: &mut GameState, _: &PlayerId) -> Option<Option<SecretObjectiveId>> {
        None
    }
    pub fn score_public(_: &mut GameState, _: &PlayerId, _: &ObjectiveId, _: i32) -> Option<()> {
        None
    }
    pub fn score_secret(_: &mut GameState, _: &PlayerId, _: &SecretObjectiveId, _: i32) -> Option<Option<i32>> {
        None
    }
    pub fn score_one(
        _: &mut GameState,
        _: &ti4_content::ContentStore,
        _: ti4_model::content_types::SourceSet,
        _: &Choice,
        _: ScoreArgs,
        _: &Recording,
    ) -> Option<CoreWindow<(bool, Option<ObjectiveId>)>> {
        None
    }
    pub fn hand_limit(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
}
