//! `Area::TradeDiplomacy` (package B4d): the structured negotiation window (`diplomacy::window::DiplomacyWindow`)
//! as a lockstep window.
//!
//! As the transaction window (`trade_window_dual`), the unit of the dual is one step of the string window
//! (`DiplomacyWindow::answer_with`: the question, the table's answer, the window's `apply`): [`step`] builds the core
//! `rules::diplomacy_window::DiplomacyWindow` at the position the string window was in, runs it to its first
//! question, compares the question with the one the string put, answers it with the string's answer and compares the
//! state and the window afterwards ([`Snap`]).
//!
//! What stays with the string engine (a `None` here keeps the window on the string, counted as a bail): a nested
//! emission or a window inside a battle, a faction two seats play, a seat or term the codecs refuse (a system or
//! leader the core has no variant for), a map the window has none of, a step the core refuses (the string reports it
//! as an illegal choice), and, after a settled deal, the staged `TRADE_GOODS_GAINED` events and Dark Pact's gains
//! (the same as in the transaction window).

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_model::state::GameState;

    use crate::diplomacy::window::{DiplomacyWindow, Snap};
    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn step(_: &mut GameState, _: &ContentStore, _: Option<&Galaxy>, _: &DiplomacyWindow, _: &Recording) -> Option<CoreWindow<Snap>> {
        None
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_engine_core::containers::SmallList;
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{Agenda, Leader, System};
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::query::diplomacy_items::{DipFacts, Draft, Scope};
    use ti4_engine_core::rules::diplomacy_window::{DStage, DiplomacyWindow as Core};
    use ti4_engine_core::state::{DealId, Slice};
    use ti4_model::state::GameState;

    use crate::core_adapter::typed;
    use crate::core_sync::codec::{agenda, trade as codec};
    use crate::core_sync::{Ctx, Footprint, Sources, with_core};
    use crate::diplomacy::builder::{Counted as SCounted, Pending};
    use crate::diplomacy::window::{DiplomacyStage, DiplomacyWindow, LABEL_ANSWER, Snap};
    use crate::dual::{CoreWindow, Run, note_bail};
    use crate::dual_window::{Recording, drive, tables};
    use crate::generated::IdStr;
    use crate::render::diplomacy::{View, bundle_value, dip_note_id, string_draft, with_view};

    /// What one answer reads and writes: goods, fragments, cards and secrets (Players), the notes (Cards), the book
    /// and the Support loans (Trade), the points (Ledger), Black Market (Transient); the ballot is read (Agenda).
    const WINDOW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger, Slice::Transient, Slice::Agenda],
        &[Slice::Players, Slice::Cards, Slice::Trade, Slice::Ledger],
    );

    /// `builder::FAVOUR_AGENTS`.
    const AGENTS: [&str; 4] = ["hacanagent", "jolnaragent", "l1z1xagent", "xxchaagent"];

    fn bail<T>(why: &str) -> Option<T> {
        note_bail(LABEL_ANSWER, why);
        None
    }

    fn counted(k: SCounted) -> ti4_engine_core::choice::diplomacy::Counted {
        use ti4_engine_core::choice::diplomacy::Counted as C;
        match k {
            SCounted::TradeGoods => C::TradeGoods,
            SCounted::Commodities => C::Commodities,
            SCounted::Cultural => C::Cultural,
            SCounted::Hazardous => C::Hazardous,
            SCounted::Industrial => C::Industrial,
            SCounted::Unknown => C::Unknown,
        }
    }

    fn scope(cx: &mut Ctx, s: &crate::diplomacy::builder::ContactScope) -> Option<Scope> {
        let mut out = Scope { physical: s.physical, ..Scope::default() };
        for p in &s.attack_targets {
            out.attack_targets.push(cx.owners.seat(p.as_str())?);
        }
        for system in &s.systems {
            out.systems.push(typed::<System>(system.as_str())?);
        }
        if let Some((a, outcomes)) = &s.agenda {
            let mut list = SmallList::new();
            for o in outcomes {
                list.push(agenda::outcome(cx, Slice::Trade, o).ok()?);
            }
            out.agenda = Some((typed::<Agenda>(a)?, list));
        }
        Some(out)
    }

    fn draft(cx: &mut Ctx, d: &crate::diplomacy::builder::Draft) -> Option<Draft> {
        let mut out = Draft::new(cx.owners.seat(d.builder.as_str())?, cx.owners.seat(d.other.as_str())?);
        for t in &d.give {
            out.give.push(codec::term(cx, t).ok()?);
        }
        for t in &d.take {
            out.take.push(codec::term(cx, t).ok()?);
        }
        out.asking = d.asking;
        out.reviewing = d.reviewing;
        out.pending = d.pending.map(|Pending { kind, later }| (counted(kind), later));
        out.steps = d.steps;
        Some(out)
    }

    /// The core half of one step of the negotiation window (see the module docs).
    pub fn step(gs: &mut GameState, content: &ContentStore, galaxy: Option<&Galaxy>, at: &DiplomacyWindow, script: &Recording) -> Option<CoreWindow<Snap>> {
        if crate::dual_window::outside_battle(gs, script) || !content.is_embedded() {
            return None;
        }
        let Some(galaxy) = galaxy else {
            crate::product::unreachable_here("L19 (trade)", "a diplomacy window with no map");
            return bail("a position with no map");
        };
        let mut factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        factions.sort_unstable();
        if factions.windows(2).any(|w| w[0] == w[1]) {
            crate::product::unreachable_here("L19 (trade)", "a faction played by two seats");
            return bail("a faction played by two seats");
        }
        let Some((mut cx, r)) = tables(gs) else { return bail("a position the codecs refuse") };
        let (Some(sp), Some(sr)) = (r.seat(at.proposer.as_str()), r.seat(at.recipient.as_str())) else { return bail("a seat the table does not know") };
        let Some(deal_facts) = crate::trade_window_dual::facts_of(gs, content, galaxy, &r, &at.proposer, &at.recipient) else { return bail("a seat the table does not know") };
        let mut agents = [Leader::from_index(0)?; 4];
        for (slot, id) in agents.iter_mut().zip(AGENTS) {
            *slot = typed::<Leader>(id)?;
        }
        let mut deal_facts = deal_facts;
        deal_facts.note_order = |a, b| dip_note_id(a).cmp(&dip_note_id(b));
        let facts = DipFacts { deal: deal_facts, agents };
        let Some(scope) = scope(&mut cx, &at.scope) else { return bail("a scope the core has no ids for") };
        let mut signals = SmallList::new();
        for s in &at.signals {
            let Ok(statement) = codec::statement(&mut cx, s) else { return bail("a signal the core has no ids for") };
            signals.push(statement);
        }
        let core_draft = match &at.draft {
            Some(d) => match draft(&mut cx, d) {
                Some(d) => Some(d),
                None => return bail("a draft the core has no ids for"),
            },
            None => None,
        };
        let current = match &at.current {
            Some(c) => match codec::revision(&mut cx, &c.revision) {
                Ok(rev) => Some(rev),
                Err(_) => return bail("a revision the core has no ids for"),
            },
            None => None,
        };
        let stage = match at.stage {
            DiplomacyStage::Building => DStage::Building,
            DiplomacyStage::Responding => DStage::Responding,
            DiplomacyStage::Done => DStage::Done,
        };
        let window = Core::at(
            sp,
            sr,
            stage,
            scope,
            signals,
            core_draft.clone(),
            current.clone(),
            at.deal_id.map(|d| DealId(d.0)),
            at.responses,
            at.counter_abandoned,
            facts,
        )
        .single();
        let before = gs.clone();
        let round = before.round;
        let kernel = Kernel::plain();
        let names = cx.clone();
        let driven = with_core(gs, &mut cx, Sources::default(), WINDOW, |core, _| {
            // What the giver of the side being edited holds, and the draft and bundle as the string writes them.
            let mut held = [0_u8; 6];
            if let Some(d) = &core_draft {
                let giver = d.side().0;
                for (slot, kind) in held.iter_mut().zip(ti4_engine_core::choice::diplomacy::Counted::ALL) {
                    *slot = ti4_engine_core::query::diplomacy_items::held(core, kind, giver);
                }
            }
            let view = View {
                round,
                held,
                cx: names.clone(),
                draft: core_draft.as_ref().and_then(|d| string_draft(&names, d).ok()),
                bundle: current.as_ref().and_then(|c| codec::revision_back(&names, c).ok()).map(bundle_value),
                signals: at.signals.clone(),
                proposer: sp,
            };
            if view.draft.is_some() != core_draft.is_some() || view.bundle.is_some() != current.is_some() {
                return Err("a draft or revision the string types cannot hold");
            }
            let mut driver = Driver::new(window, Resolver::new(&[sp, sr], Some(sp)), AbilityTable::default(), 1);
            let trades = core.trade().this_round.len();
            let ran = with_view(view, || drive(&mut driver, core, &kernel, &r, &before, script));
            match ran {
                Ok(driven) => Ok((driven, driver.window, core.trade().this_round.len() > trades)),
                Err(_) => Err("a run the driver refused"),
            }
        });
        let (driven, window, traded) = match driven {
            Ok(Ok(done)) => done,
            Ok(Err(why)) => return bail(why),
            Err(_) => return bail("a position the codecs refuse"),
        };
        if !driven.finished && driven.rendered.is_empty() {
            return bail("a question nobody answered");
        }
        if !window.stepped() {
            // The string's answer is not one of the core question's options (it refuses it as an illegal choice).
            return bail("an answer the core's question does not offer");
        }
        if traded {
            // What the core does not do of `transactions::resolve`: the staged gains of both sides and Dark Pact.
            let deal = before.diplomacy.active_deals.get(&at.deal_id?)?;
            let latest = deal.latest();
            let given = crate::diplomacy::transfers::immediate_terms(&latest.proposer_terms).ok()?;
            let received = crate::diplomacy::transfers::immediate_terms(&latest.recipient_terms).ok()?;
            let offer = crate::transactions::Offer { proposer: deal.proposer.clone(), partner: deal.recipient.clone(), given, received };
            crate::supply::note_trade_goods_gained(gs, &offer.partner, offer.given.trade_goods + offer.given.commodities, "transaction");
            crate::supply::note_trade_goods_gained(gs, &offer.proposer, offer.received.trade_goods + offer.received.commodities, "transaction");
            crate::factions::empyrean_units::dark_pact_gains(gs, content, &offer);
        }
        let cx = names;
        let snap = Snap {
            stage: match window.stage {
                DStage::Building => DiplomacyStage::Building,
                DStage::Responding => DiplomacyStage::Responding,
                DStage::Done => DiplomacyStage::Done,
            },
            draft: match &window.draft {
                Some(d) => Some(string_draft(&cx, d).ok()?),
                None => None,
            },
            revision: match &window.current {
                Some(c) => Some(codec::revision_back(&cx, c).ok()?),
                None => None,
            },
            deal_id: window.deal_id.map(|d| ti4_model::DealId(d.0)),
            responses: window.responses,
            counter_abandoned: window.counter_abandoned,
            failed: false,
        };
        Some(CoreWindow { run: Run::new(snap).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}
