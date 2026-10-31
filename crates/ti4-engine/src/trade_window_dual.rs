//! `Area::TradeDiplomacy` (package B4c): the transaction window (`transactions::TradeWindow`) as a lockstep window.
//!
//! The string window is driven one answer at a time (`Game::step_trade`: the question, the table's answer, the
//! window's `resolve`), so the unit of the dual is one such step: [`step`] builds the core
//! `rules::trade_window::TradeWindow` at the position the string window was in (`At`), runs it to its first
//! question, compares the question with the one the string put, answers it with the string's answer and
//! compares the state and the outcome afterwards. The core derives the option list and the prices
//! (`query::trade_deals`); what stays with the string engine is in `docs/ENGINE_KNOWN_ISSUES.md` (B4c): the
//! faction hooks (reach, Arbiters, an Alliance's commander), the staged `TRADE_GOODS_GAINED` events of an accepted
//! deal, Dark Pact's gains, the `TRANSACTION_RESOLVED` event, and every window with a free-form deal, a faction two
//! seats play, or diplomacy on.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

use crate::transactions::Offer;
use ti4_model::id::PlayerId;

/// Where the string window stands when a step begins.
#[derive(Debug, Clone)]
pub struct At {
    pub proposer: PlayerId,
    pub partner: PlayerId,
    /// Proposals still to be answered.
    pub rounds_left: u8,
    /// The deal on the table (the partner answers it) and the id of the option it came from.
    pub offer: Option<(Offer, String)>,
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use super::At;
    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;
    use crate::transactions::Traded;

    pub fn step(_: &mut GameState, _: &ContentStore, _: &Galaxy, _: &At, _: &Recording) -> Option<CoreWindow<Traded>> {
        None
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use core::cmp::Ordering;

    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_engine_core::choice::trade::Shape;
    use ti4_engine_core::generated::ActionCard;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::query::trade::Illegal as Why;
    use ti4_engine_core::query::trade_deals::{DealFacts, FreeDeal};
    use ti4_engine_core::rules::trade_window::{Outcome, TradeWindow};
    use ti4_engine_core::state::{NoteKey, Slice};
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use super::At;
    use crate::dual::{CoreWindow, Run, note_bail};
    use crate::dual_window::{Recording, drive, tables};
    use crate::core_sync::{Footprint, Sources, with_core};
    use crate::generated::IdStr;
    use crate::render::trade::{note_id, shape_of};
    use crate::transactions::{FREE_FORM_PREFIX, OfferError, Traded};

    const LABEL: &str = "transactions::answer";

    /// What one answer reads and writes: goods, fragments, cards and secrets (Players), the notes (Cards), the
    /// Support loans (Trade) with their point (Players, Ledger), and the Black Market flag (Transient).
    pub(crate) const WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger, Slice::Transient],
        &[Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger],
    );

    /// [`WINDOW`] with the board: a Pride of Kenara card moves its giver's units and its control.
    pub(crate) const WINDOW_BOARD: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger, Slice::Transient, Slice::Board],
        &[Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger, Slice::Board],
    );

    fn note_order(a: NoteKey, b: NoteKey) -> Ordering {
        note_id(a).cmp(&note_id(b))
    }

    fn card_order(a: ActionCard, b: ActionCard) -> Ordering {
        a.as_str().cmp(b.as_str())
    }

    /// The string window's error for a refusal of the core's.
    fn error_of(why: Why, r: &crate::render::Render, offer: &crate::transactions::Offer) -> Option<OfferError> {
        Some(match why {
            Why::SamePlayer => OfferError::SamePlayer,
            Why::PlayerMissing(s) => OfferError::PlayerMissing(r.player(s)),
            Why::Empty => OfferError::Empty,
            Why::NotNeighbours(a, b) => OfferError::NotNeighbours(r.player(a), r.player(b)),
            Why::CannotPay(s) => OfferError::CannotPay(r.player(s)),
            Why::NoteRefused(s, leg) => {
                let terms = match leg {
                    ti4_engine_core::query::trade::Leg::Given => &offer.given,
                    ti4_engine_core::query::trade::Leg::Received => &offer.received,
                };
                OfferError::NoteRefused(r.player(s), terms.promissory.clone()?)
            }
            Why::ActionCardsNotTradeable => OfferError::ActionCardsNotTradeable,
            Why::MissingActionCard(s, card) => OfferError::MissingActionCard(r.player(s), card.as_str().to_owned()),
        })
    }

    /// The string engine's answers a window needs (`DealFacts`), for the two chairs.
    pub(crate) fn facts_of(gs: &GameState, content: &ContentStore, galaxy: &Galaxy, r: &crate::render::Render, proposer: &PlayerId, partner: &PlayerId) -> Option<DealFacts> {
        let (sp, sq) = (r.seat(proposer.as_str())?, r.seat(partner.as_str())?);
        let mut unlocked = 0_u16;
        for p in &gs.players {
            if crate::promissory::commander_unlocked(gs, content, &p.id) {
                unlocked |= 1 << r.seat(p.id.as_str())?.0;
            }
        }
        Some(DealFacts {
            note_order,
            card_order,
            unlocked,
            pair: (sp, sq),
            may: [
                crate::transactions::may_transact(gs, content, galaxy, proposer, partner),
                crate::transactions::may_transact(gs, content, galaxy, partner, proposer),
            ],
            arbiters: [
                crate::transactions::trades_action_cards(gs, content, proposer),
                crate::transactions::trades_action_cards(gs, content, partner),
            ],
            planet_values: crate::render::trade::planet_values(),
            free: [None, None],
        })
    }

    /// The free-form deal `id` in typed form, registered in the renderer's table with the net the partner's accept option reports
    /// (the string's own number). `None` when the id is none the string reads (it offers nothing then) or a term has no typed form.
    fn register_free(gs: &GameState, content: &ContentStore, r: &crate::render::Render, id: &str, proposer: &PlayerId, partner: &PlayerId) -> Option<(u8, FreeDeal)> {
        let offer = crate::transactions::offer_from(gs, id, proposer, partner)?;
        let given = crate::trade_core::terms(gs, content, proposer, partner, &offer.given)?;
        let received = crate::trade_core::terms(gs, content, partner, proposer, &offer.received)?;
        let net = offer.given.worth_to_receiver(gs, content) - offer.received.cost_to_giver(gs, content);
        Some((r.register_free(id, net), FreeDeal { given, received }))
    }

    fn bail<T>(why: &str) -> Option<T> {
        note_bail(LABEL, why);
        None
    }

    /// The core half of one step of the transaction window (see the module docs). `None` where the core does not
    /// follow: diplomacy on or a nested emission, a free-form deal, a Pride of Kenara planet card, a faction two
    /// seats play, a question nobody answered, a position the codecs refuse.
    pub fn step(gs: &mut GameState, content: &ContentStore, galaxy: &Galaxy, at: &At, script: &Recording) -> Option<CoreWindow<Traded>> {
        if gs.active_space_combat.is_some() || gs.active_invasion.is_some() {
            crate::product::unreachable_here("L19 (trade)", "a transaction window inside a battle or an invasion");
        }
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| (!content.is_embedded()).then_some("content other than the embedded store")) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let answer = script.questions.first().and_then(|q| q.answer.clone())?;
        let pair = [&at.proposer, &at.partner];
        // A Pride of Kenara on either chair brings the board into the window (units move with the card).
        let kenara = pair.iter().any(|p| !crate::transactions::pride_of_kenara_planets(gs, p).is_empty());
        let mut factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        factions.sort_unstable();
        if factions.windows(2).any(|w| w[0] == w[1]) {
            crate::product::unreachable_here("L19 (trade)", "a faction played by two seats");
            note_bail(LABEL, "a faction played by two seats");
            return None;
        }
        let Some((mut cx, r)) = tables(gs) else { return bail("a position the codecs refuse") };
        let (Some(sp), Some(sq)) = (r.seat(at.proposer.as_str()), r.seat(at.partner.as_str())) else { return bail("a seat the table does not know") };
        let Some(mut facts) = facts_of(gs, content, galaxy, &r, &at.proposer, &at.partner) else { return bail("a seat the table does not know") };
        // The free-form deals of this step: the one on the table, then the one the proposer names (the string reads the text, the
        // core gets the terms). The same order the table registers them in: index 0 is the one on the table when there is one.
        let mut slot = 0;
        for id in at.offer.as_ref().map(|(_, id)| id.as_str()).into_iter().chain(std::iter::once(answer.as_str())).filter(|id| id.starts_with(FREE_FORM_PREFIX)) {
            if r.free_index(id).is_some() {
                continue;
            }
            let Some((index, deal)) = register_free(gs, content, &r, id, &at.proposer, &at.partner) else {
                return bail("a free-form deal the string does not read or the core tables do not name");
            };
            debug_assert_eq!(usize::from(index), slot);
            facts.free[slot] = Some(deal);
            slot += 1;
        }
        // The Support swap names the two factions; the id does not.
        let swap = {
            let faction = |p: &PlayerId| crate::core_adapter::typed::<ti4_engine_core::generated::Faction>(gs.player(p)?.faction.as_str());
            faction(&at.proposer).zip(faction(&at.partner))
        };
        let before = gs.clone();
        let kernel = Kernel::plain();
        let driven = with_core(gs, &mut cx, Sources::default(), if kenara { WINDOW_BOARD } else { WINDOW }, |core, _| {
            let deal: Option<Shape> = match &at.offer {
                Some((_, id)) if id.starts_with(FREE_FORM_PREFIX) => match r.free_index(id) {
                    Some(i) => Some(Shape::Free(i)),
                    None => return Err("a deal on the table the core has no shape for"),
                },
                Some((_, id)) => match shape_of(id, swap) {
                    Some(shape) => Some(shape),
                    None => return Err("a deal on the table the core has no shape for"),
                },
                None => None,
            };
            let window = TradeWindow::at(sp, sq, deal, at.rounds_left, facts).single();
            let mut driver = crate::window_driver::driver_for(window, Resolver::new(&[sp, sq], Some(sp)), AbilityTable::default(), 1);
            match drive(&mut driver, core, &kernel, &r, &before, script) {
                Ok(driven) => Ok((driven, driver.window.outcome)),
                Err(_) => Err("a run the driver refused"),
            }
        });
        let (driven, outcome) = match driven {
            Ok(Ok(done)) => done,
            Ok(Err(why)) => return bail(why),
            Err(_) => return bail("a position the codecs refuse"),
        };
        // The offer the refusal is about: the one on the table, or the one the proposer just named.
        let named = || match &at.offer {
            Some((offer, _)) => Some(offer.clone()),
            None => crate::transactions::offer_from(&before, &answer, &at.proposer, &at.partner),
        };
        let traded = match outcome? {
            Outcome::Resolved => Traded::Resolved,
            Outcome::NothingOffered => Traded::NothingOffered,
            Outcome::Refused => Traded::Refused,
            Outcome::Offered => Traded::Offered,
            Outcome::Countered => Traded::Countered,
            Outcome::Rejected(why) => Traded::Rejected(error_of(why, &r, &named()?)?),
        };
        if traded == Traded::Resolved
            && let Some((offer, _)) = &at.offer
        {
            // What the core does not do of `transactions::resolve`: the staged gains of both sides and Dark Pact.
            crate::supply::note_trade_goods_gained(gs, &offer.partner, offer.given.trade_goods + offer.given.commodities, "transaction");
            crate::supply::note_trade_goods_gained(gs, &offer.proposer, offer.received.trade_goods + offer.received.commodities, "transaction");
            crate::factions::empyrean_units::dark_pact_gains(gs, content, offer);
        }
        Some(CoreWindow { run: Run::new(traded).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}
