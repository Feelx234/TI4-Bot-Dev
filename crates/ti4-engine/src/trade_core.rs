//! `Area::TradeDiplomacy` (package B4): the adapter between the string engine's `transactions`
//! call sites and `ti4_engine_core::query::trade` / `rules::trade`.
//!
//! A [`TradeView`] is the typed view of one position (the Meta, Seats, Players, Board and Trade
//! slices, loaded once) plus the string engine's answers to what the core cannot see yet (faction
//! hooks, the promissory-note and planet-card legs of a deal): the core's `Env`. The view belongs
//! to one `&GameState` and is lent to the string functions through a thread-local scope
//! ([`scoped`]); a call made outside a scope, with another state or galaxy, or with an id the core
//! has no variant for, answers from the string engine. Nothing here decides a rule.
//!
//! Entry points that ask many questions of one position (`available_actions`, `why_illegal`,
//! `partners`) open the scope; a single question ([`leaf`]) builds a view only in the dual run, so
//! the `Core` backend never pays a load for one answer.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::galaxy::Galaxy;
    use ti4_model::state::GameState;

    /// Without `typed-core` there is no view.
    pub fn scoped<T>(_: &GameState, _: &ti4_content::ContentStore, _: &Galaxy, f: impl FnOnce() -> T) -> T {
        f()
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use std::cell::RefCell;
    use std::collections::BTreeSet;
    use std::rc::Rc;

    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_engine_core::bits::SysSet;
    use ti4_engine_core::galaxy::Adjacency;
    use ti4_engine_core::generated::{ActionCard, ExploreType, SecretObjective, System};
    use ti4_engine_core::ids::Seat;
    use ti4_engine_core::query::trade as q;
    use ti4_engine_core::rules::trade as rules;
    use ti4_engine_core::state::{CoreState, Slice, SliceSet};
    use ti4_model::id::{PlayerId, SystemId};
    use ti4_model::state::GameState;

    use crate::core_sync::{CoreView, Ctx, Sources};
    use crate::generated::IdStr;
    use crate::transactions::{Offer, OfferError, Terms};

    /// How often the typed mutators answered and how often they handed the call back to the string
    /// engine (a deal with a note or a planet card, an unmapped id): reported by `trade_dual`.
    pub static TYPED_WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    pub static STRING_WRITES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn counted(done: Option<()>) -> Option<()> {
        let c = if done.is_some() { &TYPED_WRITES } else { &STRING_WRITES };
        c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        done
    }

    /// What the trade queries read.
    pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Trade]);

    /// The typed view of one position.
    #[derive(Debug)]
    pub struct TradeView {
        state_addr: usize,
        galaxy_addr: usize,
        view: CoreView,
        cx: Ctx,
        adj: std::sync::Arc<Adjacency>,
        mecatol: System,
    }

    thread_local! {
        static AMBIENT: RefCell<Option<Rc<TradeView>>> = const { RefCell::new(None) };
    }

    fn ambient() -> Option<Rc<TradeView>> {
        AMBIENT.with(|a| a.borrow().clone())
    }

    struct Restore(Option<Rc<TradeView>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            AMBIENT.with(|a| *a.borrow_mut() = self.0.take());
        }
    }

    /// Run `f` with a view of this position lent to every trade call on this thread, unless the
    /// trade area is on the string path, a view of exactly this position is lent already, or the
    /// position cannot be expressed (the string engine answers then).
    pub fn scoped<T>(state: &GameState, content: &ContentStore, galaxy: &Galaxy, f: impl FnOnce() -> T) -> T {
        if crate::dual::backend(crate::dual::Area::TradeDiplomacy) == crate::dual::Backend::String {
            return f();
        }
        if ambient().is_some_and(|v| v.matches(state, galaxy)) {
            return f();
        }
        let Some(view) = TradeView::build(state, content, galaxy) else { return f() };
        let _restore = Restore(AMBIENT.with(|a| a.borrow_mut().replace(view)));
        f()
    }

    /// The view lent for exactly this position.
    #[must_use]
    pub fn lent(state: &GameState, galaxy: &Galaxy) -> Option<Rc<TradeView>> {
        ambient().filter(|v| v.matches(state, galaxy))
    }

    /// The view lent for this state, whatever the galaxy (questions that never read the map).
    #[must_use]
    pub fn lent_state(state: &GameState) -> Option<Rc<TradeView>> {
        ambient().filter(|v| v.state_addr == std::ptr::from_ref(state) as usize)
    }

    /// The view for a single question: the lent one, or (dual run only) a fresh one, so the
    /// `Core` backend never builds a view for one answer.
    #[must_use]
    pub fn leaf(state: &GameState, content: &ContentStore, galaxy: &Galaxy) -> Option<Rc<TradeView>> {
        lent(state, galaxy).or_else(|| {
            (crate::dual::backend(crate::dual::Area::TradeDiplomacy) == crate::dual::Backend::Dual)
                .then(|| TradeView::build(state, content, galaxy))
                .flatten()
        })
    }

    fn typed<T: IdStr>(id: &str) -> Option<T> {
        T::from_id(id)
    }

    impl TradeView {
        /// `None` when the typed state cannot hold the position (an unknown id, more than 16
        /// owners), the content is not the compiled-in corpus or the galaxy is not mapped.
        #[must_use]
        pub fn build(state: &GameState, content: &ContentStore, galaxy: &Galaxy) -> Option<Rc<Self>> {
            if !content.is_embedded() {
                return None;
            }
            let adj = crate::core_adapter::adjacency_for(galaxy)?;
            let mecatol = typed::<System>(crate::seating::mecatol_in_galaxy(galaxy))?;
            let mut cx = Ctx::of(state).ok()?;
            let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
            Some(Rc::new(Self {
                state_addr: std::ptr::from_ref(state) as usize,
                galaxy_addr: std::ptr::from_ref(galaxy) as usize,
                view,
                cx,
                adj,
                mecatol,
            }))
        }

        fn matches(&self, state: &GameState, galaxy: &Galaxy) -> bool {
            self.state_addr == std::ptr::from_ref(state) as usize && self.galaxy_addr == std::ptr::from_ref(galaxy) as usize
        }

        /// The typed state of the position.
        #[must_use]
        pub fn core(&self) -> &CoreState {
            &self.view
        }

        fn seat(&self, p: &PlayerId) -> Option<Seat> {
            self.cx.owners.seat(p.as_str())
        }

        fn pid(&self, s: Seat) -> Option<PlayerId> {
            self.cx.owners.id(s).map(PlayerId::new)
        }

        fn players(&self, seats: &q::SeatList) -> Option<Vec<PlayerId>> {
            seats.as_slice().iter().map(|s| self.pid(*s)).collect()
        }

        fn env<'a>(&'a self, state: &'a GameState) -> Bound<'a> {
            Bound { state, content: ContentStore::embedded(), cx: &self.cx }
        }

        fn run<T>(&self, state: &GameState, f: impl FnOnce(&q::Cx<'_>) -> T) -> T {
            let env = self.env(state);
            let cx = q::Cx {
                core: &self.view,
                adj: &self.adj,
                mecatol: self.mecatol,
                limits: q::Limits {
                    per_action: ti4_model::state::MAX_NEGOTIATIONS_PER_ACTION,
                    per_round: ti4_model::state::MAX_NEGOTIATIONS_PER_ROUND,
                },
                env: &env,
            };
            f(&cx)
        }

        // -- the answers, in the string engine's types ---------------------------------------

        /// `transactions::presence`.
        #[must_use]
        pub fn presence(&self, player: &PlayerId) -> Option<BTreeSet<SystemId>> {
            let seat = self.seat(player)?;
            let set: SysSet = q::presence(&self.view, seat);
            Some(set.iter().map(|i| SystemId::new(ti4_engine_core::ids::system_at(i).as_str())).collect())
        }

        /// `transactions::are_neighbours`.
        #[must_use]
        pub fn are_neighbours(&self, state: &GameState, a: &PlayerId, b: &PlayerId) -> Option<bool> {
            let (a, b) = (self.seat(a)?, self.seat(b)?);
            Some(self.run(state, |cx| q::are_neighbours(cx, a, b)))
        }

        /// `transactions::neighbours`.
        #[must_use]
        pub fn neighbours(&self, state: &GameState, player: &PlayerId) -> Option<Vec<PlayerId>> {
            let seat = self.seat(player)?;
            self.players(&self.run(state, |cx| q::neighbours(cx, seat)))
        }

        /// `transactions::neighbours_who_transacted`.
        #[must_use]
        pub fn neighbours_who_transacted(&self, state: &GameState, player: &PlayerId) -> Option<Vec<PlayerId>> {
            let seat = self.seat(player)?;
            self.players(&self.run(state, |cx| q::neighbours_who_transacted(cx, seat)))
        }

        /// `transactions::partners`.
        #[must_use]
        pub fn partners(&self, state: &GameState, player: &PlayerId) -> Option<Vec<PlayerId>> {
            let seat = self.seat(player)?;
            self.players(&self.run(state, |cx| q::partners(cx, seat)))
        }

        /// `transactions::may_transact`.
        #[must_use]
        pub fn may_transact(&self, state: &GameState, proposer: &PlayerId, partner: &PlayerId) -> Option<bool> {
            let (a, b) = (self.seat(proposer)?, self.seat(partner)?);
            Some(self.run(state, |cx| q::may_transact(cx, a, b)))
        }

        /// `transactions::may_open_again`.
        #[must_use]
        pub fn may_open_again(&self, state: &GameState, proposer: &PlayerId, partner: &PlayerId) -> Option<bool> {
            let (a, b) = (self.seat(proposer)?, self.seat(partner)?);
            Some(self.run(state, |cx| q::may_open_again(cx, a, b)))
        }

        /// The partners `transactions::available_actions` opens a transaction with.
        #[must_use]
        pub fn open_partners(&self, state: &GameState, player: &PlayerId) -> Option<Vec<PlayerId>> {
            let seat = self.seat(player)?;
            self.players(&self.run(state, |cx| q::open_partners(cx, seat)))
        }

        /// The typed deal. `None` when a term has no typed form (an unknown card or fragment kind).
        fn offer(&self, state: &GameState, content: &ContentStore, offer: &Offer) -> Option<q::Offer> {
            Some(q::Offer {
                proposer: self.seat(&offer.proposer)?,
                partner: self.seat(&offer.partner)?,
                given: terms(state, content, &offer.proposer, &offer.partner, &offer.given)?,
                received: terms(state, content, &offer.partner, &offer.proposer, &offer.received)?,
            })
        }

        /// `transactions::can_pay`.
        #[must_use]
        pub fn can_pay(&self, state: &GameState, content: &ContentStore, player: &PlayerId, terms_: &Terms) -> Option<bool> {
            let seat = self.seat(player)?;
            // The receiver only matters for `receivable`, which `can_pay` does not read.
            let t = terms(state, content, player, player, terms_)?;
            Some(q::can_pay(&self.view, seat, &t))
        }

        /// `transactions::why_illegal`.
        #[must_use]
        pub fn why_illegal(&self, state: &GameState, content: &ContentStore, offer: &Offer) -> Option<Option<OfferError>> {
            let typed_offer = self.offer(state, content, offer)?;
            let found = self.run(state, |cx| q::why_illegal(cx, &typed_offer));
            let Some(found) = found else { return Some(None) };
            Some(Some(match found {
                q::Illegal::SamePlayer => OfferError::SamePlayer,
                q::Illegal::PlayerMissing(s) => OfferError::PlayerMissing(self.pid(s)?),
                q::Illegal::Empty => OfferError::Empty,
                q::Illegal::NotNeighbours(a, b) => OfferError::NotNeighbours(self.pid(a)?, self.pid(b)?),
                q::Illegal::CannotPay(s) => OfferError::CannotPay(self.pid(s)?),
                q::Illegal::NoteRefused(s, leg) => {
                    let note = match leg {
                        q::Leg::Given => offer.given.promissory.clone(),
                        q::Leg::Received => offer.received.promissory.clone(),
                    };
                    OfferError::NoteRefused(self.pid(s)?, note?)
                }
                q::Illegal::ActionCardsNotTradeable => OfferError::ActionCardsNotTradeable,
                q::Illegal::MissingActionCard(s, card) => OfferError::MissingActionCard(self.pid(s)?, card.as_str().to_owned()),
            }))
        }
    }

    /// The string engine's answers to what the core cannot see, for one position.
    struct Bound<'a> {
        state: &'a GameState,
        content: &'a ContentStore,
        cx: &'a Ctx,
    }

    impl Bound<'_> {
        fn pid(&self, s: Seat) -> PlayerId {
            PlayerId::new(self.cx.owners.id(s).unwrap_or_default())
        }
    }

    impl q::Env for Bound<'_> {
        fn has_iihq(&self, who: Seat) -> bool {
            crate::factions::keleres::has_iihq(self.state, &self.pid(who))
        }
        fn wide(&self, who: Seat) -> bool {
            let p = self.pid(who);
            crate::faction_abilities::ignores_neighbours(self.state, self.content, &p) || crate::promissory::reaches_anyone(self.state, &p)
        }
        fn transaction_reach(&self, a: Seat, b: Seat) -> bool {
            crate::factions::hooks_cards::transaction_reach(self.state, self.content, &self.pid(a), &self.pid(b))
        }
        fn exempt_from_limit(&self, a: Seat, b: Seat) -> bool {
            crate::factions::hooks_cards::transaction_exempt_from_limit(self.state, self.content, &self.pid(a), &self.pid(b))
        }
        fn trades_action_cards(&self, who: Seat) -> bool {
            crate::transactions::trades_action_cards(self.state, self.content, &self.pid(who))
        }
        fn black_market(&self) -> bool {
            self.state.transient_flags.has(ti4_model::state::TransientFlags::BLACK_MARKET)
        }
    }

    fn kind_of(trait_name: &str) -> Option<ExploreType> {
        match crate::transactions::fragment_key(trait_name).as_str() {
            "CULTURAL" => Some(ExploreType::Cultural),
            "HAZARDOUS" => Some(ExploreType::Hazardous),
            "INDUSTRIAL" => Some(ExploreType::Industrial),
            "FRONTIER" => Some(ExploreType::Frontier),
            _ => None,
        }
    }

    /// One side of a deal in typed form; the note and planet legs are the string engine's facts for
    /// this giver and receiver.
    fn terms(state: &GameState, content: &ContentStore, giver: &PlayerId, receiver: &PlayerId, t: &Terms) -> Option<q::Terms> {
        let mut fragments = [0_i32; 4];
        for f in &t.fragments {
            fragments[kind_of(f)? as usize] += 1;
        }
        Some(q::Terms {
            trade_goods: t.trade_goods,
            commodities: t.commodities,
            fragments,
            note: t.promissory.as_ref().map(|note| q::NoteFacts {
                payable: crate::transactions::note_payable(state, content, giver, note),
                receivable: crate::promissory::may_receive(state, receiver, note),
            }),
            action_card: match &t.action_card {
                Some(c) => Some(typed::<ActionCard>(c.as_str())?),
                None => None,
            },
            secret: match &t.secret {
                Some(s) => Some(typed::<SecretObjective>(s.as_str())?),
                None => None,
            },
            planet: t.planet.as_ref().map(|p| q::PlanetFacts { payable: crate::transactions::planet_term_payable(state, giver, p) }),
        })
    }

    // -- the decision model -----------------------------------------------------------------

    /// The typed form of a transaction-window question (`TradeWindow::pending_choice`), or `None`
    /// for any other question or a seat the table does not know.
    #[must_use]
    pub fn typed_choice(state: &GameState, choice: &crate::choice::Choice, r: &crate::render::Render) -> Option<ti4_engine_core::choice::Choice> {
        use ti4_engine_core::choice::generic::Rule;
        use ti4_engine_core::choice::trade::{Ask, Opt, Subtype};
        use ti4_engine_core::choice::{Ctx, Opts, Origin, Subtype as Sub, Target};
        let context = choice.context.as_ref()?;
        let actor = r.seat(choice.player.as_str())?;
        let crate::decision_context::DecisionTarget::Player(other) = context.target.clone()? else { return None };
        let other = r.seat(other.as_str())?;
        let (ask, subtype, opts) = match context.subtype.as_str() {
            "propose_transaction" => {
                let name = choice.prompt.strip_prefix("transaction with ")?;
                let mut opts = Opts::default();
                for o in &choice.options {
                    opts.push(if o.is_decline() {
                        ti4_engine_core::choice::Opt::Decline
                    } else {
                        ti4_engine_core::choice::Opt::Trade(Opt::Deal(r.shape(o)))
                    });
                }
                (Ask::Propose { proposer: actor, partner: other, name: r.code(name) }, Subtype::Propose, opts)
            }
            "answer_transaction" => {
                let said = choice.prompt.strip_suffix(" -- accept?")?;
                let net = choice.options.iter().find(|o| o.id == "accept")?.payload.get("net")?.as_f64()?;
                let mut opts = Opts::default();
                for o in &choice.options {
                    opts.push(ti4_engine_core::choice::Opt::Trade(match o.id.as_str() {
                        "accept" => Opt::Accept,
                        "refuse" => Opt::Refuse,
                        "counter" => Opt::Counter,
                        _ => return None,
                    }));
                }
                (Ask::Answer { responder: actor, proposer: other, said: r.code(said), net: r.code(&net.to_string()) }, Subtype::Answer, opts)
            }
            _ => return None,
        };
        let _ = state;
        Some(ti4_engine_core::choice::Choice {
            actor,
            ask: ti4_engine_core::choice::Ask::Trade(ask),
            ctx: Ctx {
                origin: Origin::Rule(Rule::R60),
                subtype: Sub::Trade(subtype),
                optional: context.optional,
                target: Some(Target::Seat(other)),
                outstanding: ti4_engine_core::containers::SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        })
    }

    /// The question rendered from its typed form: what the string window built, byte for byte.
    #[must_use]
    pub fn rendered(state: &GameState, choice: &crate::choice::Choice) -> Option<crate::choice::Choice> {
        let cx = Ctx::of(state).ok()?;
        let r = crate::render::Render::new(cx.owners.ids().to_vec());
        let typed = typed_choice(state, choice, &r)?;
        let phase = crate::render::generic::core_phase(state.phase);
        Some(crate::render::render_choice(&r, &typed, phase, state.round))
    }

    // -- mutators -----------------------------------------------------------------------------

    use crate::core_sync::{Footprint, with_core};

    const GOODS: Footprint = Footprint::new(&[Slice::Players], &[Slice::Players]);
    const TRADE: Footprint = Footprint::new(&[Slice::Trade], &[Slice::Trade]);

    /// The goods half of `transactions::resolve` on the typed state. `None` when the deal has a
    /// term the core does not move (a note or a planet card) or the position is not expressible.
    pub fn exchange(state: &mut GameState, content: &ContentStore, offer: &Offer) -> Option<()> {
        if !content.is_embedded()
            || offer.given.promissory.is_some()
            || offer.received.promissory.is_some()
            || offer.given.planet.is_some()
            || offer.received.planet.is_some()
        {
            return counted(None);
        }
        let mut cx = Ctx::of(state).ok()?;
        let seat = |p: &PlayerId| cx.owners.seat(p.as_str());
        let typed_offer = q::Offer {
            proposer: seat(&offer.proposer)?,
            partner: seat(&offer.partner)?,
            given: terms(state, content, &offer.proposer, &offer.partner, &offer.given)?,
            received: terms(state, content, &offer.partner, &offer.proposer, &offer.received)?,
        };
        counted(with_core(state, &mut cx, Sources { galaxy: None, rng: None }, GOODS, |core, _| rules::exchange(core, &typed_offer)).ok())
    }

    /// `GameState::record_transaction` on the typed state.
    pub fn record_transaction(state: &mut GameState, a: &PlayerId, b: &PlayerId) -> Option<()> {
        let mut cx = Ctx::of(state).ok()?;
        let (x, y) = (cx.owners.seat(a.as_str())?, cx.owners.seat(b.as_str())?);
        with_core(state, &mut cx, Sources { galaxy: None, rng: None }, TRADE, |core, _| rules::record_transaction(core, x, y)).ok()
    }

    /// `GameState::note_negotiation` on the typed state.
    pub fn note_negotiation(state: &mut GameState, player: &PlayerId) -> Option<()> {
        let mut cx = Ctx::of(state).ok()?;
        let seat = cx.owners.seat(player.as_str())?;
        with_core(state, &mut cx, Sources { galaxy: None, rng: None }, TRADE, |core, _| rules::note_negotiation(core, seat)).ok()
    }

    /// `GameState.transactions_this_round.push` on the typed state.
    pub fn record_resolved(state: &mut GameState, proposer: &PlayerId, partner: &PlayerId) -> Option<()> {
        let mut cx = Ctx::of(state).ok()?;
        let (x, y) = (cx.owners.seat(proposer.as_str())?, cx.owners.seat(partner.as_str())?);
        with_core(state, &mut cx, Sources { galaxy: None, rng: None }, TRADE, |core, _| rules::record_resolved(core, x, y)).ok()
    }
}
