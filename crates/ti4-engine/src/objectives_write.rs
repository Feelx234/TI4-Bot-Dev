//! `Area::Objectives` (package A4), the mutation half: the writes of `objectives.rs`, `secrets.rs` and
//! `GameState::reveal_objective` (a victory point moved, a public objective revealed, a secret drawn, a score
//! recorded) as `ti4_engine_core::rules::objectives` functions over the slices they touch, and the two objective
//! questions (`rules::objective_windows`: score an objective, return a secret over the hand limit) as lockstep
//! windows.
//!
//! Every function answers `None` when the position is outside what the core holds (more than 16 owners, an id
//! the tables do not know, a deck reserve of a turn redo, a value the codecs refuse): the caller keeps the string
//! write. The string callers keep what the core cannot do (the leaders a score unlocks, the staged event of Yin's breakthrough, the log, who is asked next in a scoring window); see
//! `docs/ENGINE_KNOWN_ISSUES.md`, A4.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

/// The scoring window's arguments that the core window needs and the string window keeps private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreArgs {
    /// The status window (publics and secrets in one list); an event window offers secrets only.
    pub status: bool,
    /// The feat occurrence the one-per-player cap records the score under.
    pub occurrence: Option<u64>,
    /// The window's bookkeeping before the step: who is asked, the caps, the plot card to place.
    pub book: crate::objectives::Book,
}

/// The label the scoring step's bails are counted under.
pub const SCORE_LABEL: &str = "scoring::step";

/// What a scoring step returns: whether the window finished, what was scored, the window's book afterwards.
pub type ScoreValue = (bool, Option<ti4_model::id::ObjectiveId>, crate::objectives::Book);

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::{PublicObjective, SecretObjective};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::driver::DriverError;
    use ti4_engine_core::kernel::fx_pay::PayEnv;
    use ti4_engine_core::kernel::Kernel;
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::query::objectives as q;
    use ti4_engine_core::choice::objectives::Offer;
    use ti4_engine_core::containers::{SeatSet, SmallList};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::rules::objective_windows::{HandLimit, PlaceCard, ScoreOne};
    use ti4_engine_core::rules::scoring_book::{Asked, Offers, Outcome, ScoringBook};
    use ti4_engine_core::rules::objectives as rules;
    use ti4_engine_core::state::{ObjectiveRef, Slice};
    use ti4_model::id::{ObjectiveId, PlayerId, SecretObjectiveId};
    use ti4_model::state::GameState;

    use super::{ScoreArgs, ScoreValue};
    use crate::choice::Choice;
    use crate::objectives::Book;
    use crate::render::Render;
    use crate::core_sync::keys::{parse_ledger, render_ledger};
    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::{Driven, Recording, lend};
    use crate::generated::IdStr;

    /// A victory point move and its ledger row.
    const ADJUST: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Ledger], &[Slice::Players, Slice::Ledger]);
    /// The objective deck and the revealed list.
    const REVEAL: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Decks, Slice::Objectives], &[Slice::Decks, Slice::Objectives]);
    /// A secret drawn into a hand.
    const DRAW: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks], &[Slice::Players, Slice::Decks]);
    /// A secret made public: the revealed list.
    const MAKE_PUBLIC: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Objectives], &[Slice::Objectives]);
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
    /// The scoring question of an offer with a price (a bought objective, Destroy Heretical Works, Form a Spy
    /// Network): the price spends trade goods, commodities, tokens, fragments and cards and exhausts planets, and
    /// the hooks it reads (Mentak, Keleres, the ocean cards) read the marks and the board, and the value of a planet the laws in play.
    const SCORE_PRICED_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Ledger, Slice::Pending, Slice::Board, Slice::Marks, Slice::Agenda],
        &[Slice::Meta, Slice::Players, Slice::Objectives, Slice::Ledger, Slice::Board],
    );
    /// The plot card question (Plots Within Plots, second step): the plot cards in every hand.
    const PLOT_WINDOW: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);
    /// The hand limit question: the hand and the secret deck.
    const HAND_WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Objectives, Slice::Decks, Slice::Pending],
        &[Slice::Players, Slice::Decks],
    );

    /// The secrets `secrets::pay_for` charges for.
    const PRICED_SECRETS: [&str; 2] = ["dhw", "fsn"];

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

    /// A secret leaves the hand for the bottom of the deck (`secrets::enforce_hand_limit`'s hand-back, the
    /// Galactic Council's discard). The outer `None` is a bail.
    pub fn return_secret(gs: &mut GameState, player: &PlayerId, alias: &SecretObjectiveId) -> Option<()> {
        if gs.deck_reserve.is_some() {
            return None;
        }
        let secret = SecretObjective::from_id(alias.as_str())?;
        lend(gs, DRAW, Some(player), |core, who| ti4_engine_core::kernel::fx::return_secret(core, who.expect("a seated call"), secret))
    }

    /// Classified Document Leaks: the secret becomes a public objective. `Some(false)` when it already was.
    pub fn make_public(gs: &mut GameState, alias: &ObjectiveId) -> Option<bool> {
        let secret = SecretObjective::from_id(alias.as_str())?;
        lend(gs, MAKE_PUBLIC, None, |core, _| ti4_engine_core::kernel::fx::make_public(core, secret))
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

    /// A bail of the scoring step, counted with its reason in the census.
    fn bail<T>(why: &str) -> Option<T> {
        crate::dual::note_bail(super::SCORE_LABEL, why);
        None
    }

    /// What a scoring window cannot follow: diplomacy promises a favour keeps and a nested emission. A battle
    /// or an invasion in progress is followed (an invasion is named in the question's context, see `complete`).
    fn outside_scoring(gs: &GameState, script: &Recording) -> bool {
        script.nested || gs.diplomacy.enabled
    }

    /// The display fact only the state knows: `ask_seeing` names the invasion a question is asked in.
    fn complete(gs: &GameState) -> impl Fn(&ti4_engine_core::choice::Choice, &ti4_engine_core::state::CoreState, &mut Choice) + '_ {
        move |_, _, rendered| {
            if let Some(ctx) = rendered.context.take() {
                rendered.context = Some(ctx.about_invasion(gs));
            }
        }
    }

    /// The lists the string engine computes for a scoring window, given to the core book as facts.
    struct StringOffers<'a> {
        gs: &'a GameState,
        content: &'a ti4_content::ContentStore,
        sources: ti4_model::content_types::SourceSet,
        galaxy: Option<&'a ti4_content::galaxy::Galaxy>,
        book: &'a Book,
        r: &'a Render,
        /// Something the core cannot name (an unknown id, an unseated player): the step bails.
        unmapped: bool,
    }

    impl StringOffers<'_> {
        fn player(&self, seat: Seat) -> PlayerId {
            PlayerId::new(self.r.seat_text(seat))
        }
    }

    impl Offers for StringOffers<'_> {
        fn publics(&mut self, seat: Seat) -> SmallList<ObjectiveRef, 24> {
            let who = self.player(seat);
            let mut out = SmallList::new();
            for alias in crate::objectives::scoreable_on(self.gs, self.content, self.sources, &who, self.galaxy) {
                match objective(alias.as_str()) {
                    Some(o) if out.len() < 24 => out.push(o),
                    _ => self.unmapped = true,
                }
            }
            out
        }

        fn secrets(&mut self, seat: Seat) -> SmallList<SecretObjective, 8> {
            let who = self.player(seat);
            let mut out = SmallList::new();
            let found = crate::objectives::window_secrets(self.gs, self.content, self.sources, &who, self.book.timing, self.book.occurrence, self.galaxy);
            for alias in found {
                match SecretObjective::from_id(alias.as_str()) {
                    Some(s) if out.len() < 8 => out.push(s),
                    _ => self.unmapped = true,
                }
            }
            out
        }

        fn plots(&mut self, seat: Seat) -> SmallList<(SecretObjective, Seat), 8> {
            let who = self.player(seat);
            let mut out = SmallList::new();
            let found = crate::factions::firmament::plot_options(self.gs, self.content, self.sources, &who, self.book.timing, self.book.occurrence, self.galaxy);
            for (secret, token) in found {
                match (SecretObjective::from_id(secret.as_str()), self.r.seat(token.as_str())) {
                    (Some(s), Some(t)) if out.len() < 8 => out.push((s, t)),
                    _ => self.unmapped = true,
                }
            }
            out
        }

        fn scored_at_occurrence(&mut self, seat: Seat) -> bool {
            let who = self.player(seat);
            self.book.occurrence.is_some_and(|o| self.gs.scored_at_occurrence(&who, o))
        }
    }

    /// The string window's book on the core's seats; `None` for a player that has no seat.
    fn to_core(book: &Book, r: &Render) -> Option<ScoringBook> {
        let seat = |p: &PlayerId| r.seat(p.as_str());
        let set = |l: &[PlayerId]| l.iter().map(&seat).collect::<Option<SeatSet>>();
        let pending: Vec<Seat> = book.pending.iter().map(&seat).collect::<Option<_>>()?;
        let plot_card = match &book.plot_card {
            Some((owner, token)) => Some((seat(owner)?, seat(token)?)),
            None => None,
        };
        Some(ScoringBook::restore(
            &pending,
            set(&book.public_used)?,
            set(&book.secret_used)?,
            plot_card,
            book.timing == crate::secrets::Timing::Status,
            book.one_per_player,
            book.occurrence.is_some(),
        ))
    }

    /// The core's book in the string window's form.
    fn to_string_form(b: &ScoringBook, like: &Book, r: &Render) -> Book {
        let id = |s: Seat| PlayerId::new(r.seat_text(s));
        let sorted = |set: SeatSet| {
            let mut v: Vec<PlayerId> = set.iter().map(id).collect();
            v.sort();
            v
        };
        Book {
            pending: b.pending().iter().map(|s| id(*s)).collect(),
            public_used: sorted(b.public_used()),
            secret_used: sorted(b.secret_used()),
            plot_card: b.plot_card().map(|(o, t)| (id(o), id(t))),
            timing: like.timing,
            occurrence: like.occurrence,
            one_per_player: like.one_per_player,
        }
    }

    fn is_priced(o: ObjectiveRef) -> bool {
        matches!(o, ObjectiveRef::Public(p) if q::cost_of(p).is_some()) || matches!(o, ObjectiveRef::Secret(s) if PRICED_SECRETS.contains(&s.as_str()))
    }

    /// The core half of `scoring_step`: one player's turn in a scoring window, and the window's bookkeeping.
    /// The core book works out who is asked and what they may score from the lists the string engine computes
    /// (`StringOffers`), puts the question, takes the answer and updates the book from the position the
    /// answer left; `asked` is the question the string window put, against which the core's is compared.
    /// `None` where the core cannot follow (each reason is counted): a nested emission, diplomacy, a bought
    /// objective for a payer holding ocean cards, a position the codecs refuse.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn score_one(
        gs: &mut GameState,
        content: &ti4_content::ContentStore,
        sources: ti4_model::content_types::SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        _asked: &Choice,
        args: &ScoreArgs,
        script: &Recording,
    ) -> Option<CoreWindow<ScoreValue>> {
        if outside_scoring(gs, script) {
            return bail(if script.nested { "nested emission" } else { "diplomacy" });
        }
        let Some((mut cx, r)) = crate::dual_window::tables(gs) else { return bail("a position the codecs refuse (more than 16 owners, a faction without a typed variant)") };
        let Some(mut book) = to_core(&args.book, &r) else { return bail("a player without a seat in the book") };
        let before = gs.clone();
        let mut pre = StringOffers { gs: &before, content, sources, galaxy, book: &args.book, r: &r, unmapped: false };
        // The step is the plot card placement, or one player's turn.
        let step = match book.plot_card() {
            Some((owner, token)) => Step::Card { owner, token },
            None => match book.asked(&mut pre) {
                Some(next) => Step::Score(next),
                None => {
                    // The core finds nobody to ask where the string asked: the questions differ.
                    let value = (false, None, to_string_form(&book, &args.book, &r));
                    return Some(CoreWindow { run: Run::new(value), questions: Vec::new() });
                }
            },
        };
        if pre.unmapped {
            return bail("an objective or secret the core has no id for");
        }
        match &step {
            Step::Card { owner, token } => {
                let source = r.code(crate::factions::firmament::PLOTS);
                let kernel = Kernel::plain();
                let mut driver = crate::window_driver::driver_for(PlaceCard::new(*owner, *token, source), Resolver::new(&[*owner], Some(*owner)), AbilityTable::default(), 1);
                let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), PLOT_WINDOW, |core, _| {
                    crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &complete(&before))
                })
                .ok()?;
                let Ok(driven) = driven else { return bail("the core refused the plot card") };
                if driven.finished {
                    book.card_placed();
                }
                let value = (driven.finished, None, to_string_form(&book, &args.book, &r));
                Some(CoreWindow { run: Run::new(value), questions: driven.rendered })
            }
            Step::Score(next) => {
                let who = PlayerId::new(r.seat_text(next.seat));
                let offers: Vec<Offer> = next.offers.iter().copied().collect();
                let priced = offers.iter().any(|o| matches!(o, Offer::Objective(x) if is_priced(*x)));
                if priced && price_is_string(gs, &who) {
                    return bail("a priced offer for a payer holding ocean cards");
                }
                // The production kernel: its hooks answer what a trade good pays, which commodities a payment
                // may spend and the ocean cards (`fx_pay::pay_facts`).
                let kernel = crate::production_view::kernel();
                let env = PayEnv { scope: crate::core_adapter::scope(sources), blanked: crate::laws::blanked(gs) };
                let mut driver = crate::window_driver::driver_for(
                    ScoreOne::new(next.seat, args.status, &offers, args.occurrence).with_price(env),
                    Resolver::new(&[next.seat], Some(next.seat)),
                    AbilityTable::default(),
                    1,
                );
                // The string was answered with an offer that has a price and the core refused the answer.
                let answered_priced = script.questions.first().and_then(|q| q.answer.as_deref()).and_then(objective).is_some_and(is_priced);
                let footprint = if priced { SCORE_PRICED_WINDOW } else { SCORE_WINDOW };
                let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), footprint, |core, _| {
                    crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &complete(&before))
                })
                .ok()?;
                let driven = match driven {
                    Ok(driven) => driven,
                    // The string took a price the core refuses: the facts differ. Report it (a window that never
                    // finished, nothing scored) instead of bailing silently.
                    Err(DriverError::Illegal) if answered_priced => Driven { questions: Vec::new(), rendered: Vec::new(), finished: false },
                    Err(_) => return bail("the core refused the answer (a stale window)"),
                };
                let w = &driver.window;
                let scored = w.scored;
                let outcome = if !driven.finished {
                    None
                } else if let Some((_, token, placed)) = w.plotted {
                    Some(Outcome::PlotScored { token, placed })
                } else if let Some(o) = scored {
                    Some(Outcome::Scored { secret: matches!(o, ObjectiveRef::Secret(_)) && !before_revealed(&before, o) })
                } else {
                    Some(Outcome::Declined)
                };
                let events: Vec<_> = driver.emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
                // What the core does not do: the leaders a public score unlocks and Yin's staged event.
                let public_now = scored.is_some_and(|o| before_revealed(&before, o) || matches!(o, ObjectiveRef::Public(_)));
                if let (Some(obj), true) = (scored, public_now) {
                    crate::objectives::award_tail(gs, content, sources, who, &ObjectiveId::new(crate::render::objectives::objective_text(obj)));
                }
                finish_score(gs, content, sources, galaxy, args, &r, book, next, outcome, driven.finished, scored, driven.rendered, events)
            }
        }
    }

    /// The step that asks a player to score something, or the placement of a plot card.
    enum Step {
        Score(Asked),
        Card { owner: Seat, token: Seat },
    }

    /// The book after a scoring turn, once the position it reads is the one the answer left.
    #[allow(clippy::too_many_arguments)]
    fn finish_score(
        gs: &GameState,
        content: &ti4_content::ContentStore,
        sources: ti4_model::content_types::SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        args: &ScoreArgs,
        r: &Render,
        mut book: ScoringBook,
        next: &Asked,
        outcome: Option<Outcome>,
        finished: bool,
        scored: Option<ObjectiveRef>,
        questions: Vec<Choice>,
        events: Vec<crate::event::Event>,
    ) -> Option<CoreWindow<ScoreValue>> {
        if let Some(outcome) = outcome {
            let mut post = StringOffers { gs, content, sources, galaxy, book: &args.book, r, unmapped: false };
            book.after(next, outcome, &mut post);
            if post.unmapped {
                return bail("an objective or secret the core has no id for");
            }
        }
        let value = (finished, scored.map(|o| ObjectiveId::new(crate::render::objectives::objective_text(o))), to_string_form(&book, &args.book, r));
        Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions })
    }

    /// Where the price stays with the string: a payer holding Deepwrought ocean cards (planet cards off the map the core has
    /// no planets for). The Last Bastion's Helios bonus is in the payment facts, the Crimson has no economy hook.
    fn price_is_string(gs: &GameState, payer: &PlayerId) -> bool {
        !crate::factions::deepwrought::spendable_oceans(gs, payer).is_empty()
    }

    fn before_revealed(gs: &GameState, obj: ObjectiveRef) -> bool {
        let text = crate::render::objectives::objective_text(obj);
        gs.revealed_objectives.iter().any(|o| o.as_str() == text)
    }

    /// The core half of `secrets::enforce_hand_limit`.
    pub fn hand_limit(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside_scoring(gs, script) {
            return bail(if script.nested { "nested emission" } else { "diplomacy" });
        }
        let (mut cx, r) = crate::dual_window::tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = crate::window_driver::driver_for(HandLimit::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), HAND_WINDOW, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &complete(&before))
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

    use super::{ScoreArgs, ScoreValue};
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
    pub fn return_secret(_: &mut GameState, _: &PlayerId, _: &SecretObjectiveId) -> Option<()> {
        None
    }
    pub fn make_public(_: &mut GameState, _: &ObjectiveId) -> Option<bool> {
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
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &Choice,
        _: &ScoreArgs,
        _: &Recording,
    ) -> Option<CoreWindow<ScoreValue>> {
        None
    }
    pub fn hand_limit(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
}
