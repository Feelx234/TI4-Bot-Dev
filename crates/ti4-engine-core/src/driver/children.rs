//! The links between rule areas (package B6d, layering; docs/ENGINE_CORE_STATE.md 4.1): the one place that names the
//! windows an action card runs as children of its own.
//!
//! An L4 area may not import another, so the play window of an action card (`rules::action_cards`) asks the kernel's
//! [`AreaLinks`] for a payment, the hand limit, a research, an exploration reward or the ability of a strategy card. The
//! kernel only defines the shapes (`kernel::children`); this module is in the driver layer (L5), which may import every area,
//! and [`register`] fills the table. Whoever builds the [`AbilityTable`] of a play window installs it (the adapter's
//! `action_cards_view`); a play window built without it declines its run (`unsupported`: a counted bail, never a panic).

use crate::kernel::children::{AreaLinks, Erased, ErasedWindow, OneLink, Report, Spawn};
use crate::kernel::stage::AbilityTable;
use crate::driver::windows::hooked_draw::HookedDraw;
use crate::rules::combat::effect_hits::{Absorb, Destroy};
use crate::kernel::children::ExploreEnd;
use crate::rules::explore_windows::{Exploring, Reward};
use crate::rules::objective_windows::HandLimit;
use crate::rules::production::pay::PayBill;
use crate::rules::production::strategy_cards::{Construction, Leadership, Research, TitanBuild};
use crate::rules::production::home::HomeProduction;
use crate::rules::production::one::ProduceOne;
use crate::rules::production::window::ProductionWindow;
use crate::rules::tactical::fleet::{Enforce, Limits};
use crate::rules::strategy_primaries::{DMode, Diplomacy, ImperialPrimary, Politics, TeWarfare, WarfarePrimary};
use crate::rules::strategy_windows::{TradePrimary, TRADE_PRIMARY_GOODS};

/// Install the links of the areas whose windows an action card runs as children.
pub fn register(t: &mut AbilityTable) {
    // The extra link (a window of the faction crate) is the factions' to fill: it survives a registration of these.
    t.links = AreaLinks { spawn: Some(spawn), researchable: Some(Research::researchable_for), research: Some(Research::research_chosen), spawn_extra: t.links.spawn_extra, one: Some(one_link()) };
}

/// `ProduceOne` rebuilt from its plain state at every call (a host that keeps it in its payload: Freelancers inside an exploration).
pub fn one_link() -> OneLink {
    use crate::kernel::stage::Window as _;
    OneLink {
        facts: |core, k, roster, who| {
            let scope = crate::tables::Scope(u32::from(k.sources.0));
            let blanked = crate::query::reach::blanked(core);
            let buildable = crate::rules::production::home::buildable_for(core, k, scope, roster, who)?;
            let limit = i32::try_from(crate::rules::production::home::fleet_limit(core, k, blanked, who)).ok()?;
            Some((buildable, limit))
        },
        step: |run, sx| {
            let mut window = ProduceOne::resume(run);
            let next = window.step(sx);
            (window.run(), next)
        },
        ask: |run, s, k| ProduceOne::resume(*run).ask_of(s, k),
        answer: |run, sx, pick| {
            let mut window = ProduceOne::resume(run);
            window.answer(sx, pick)?;
            Ok(window.run())
        },
    }
}

/// The window a [`Spawn`] names, erased, with the function that reads what it did.
fn spawn(request: &Spawn) -> Option<Box<dyn ErasedWindow>> {
    Some(match request {
        Spawn::Bill { who, kind, cost } => Erased::boxed(PayBill::new(*who, *kind, *cost), |w| Report::Paid(w.outcome == Some(true))),
        Spawn::ExploreFrontier { who, system } => Erased::boxed(Exploring::frontier(*who, *system, false, true), |w| {
            Report::Explored(if w.unsupported().is_some() {
                ExploreEnd::Unsupported
            } else if w.empty() {
                ExploreEnd::Empty
            } else {
                ExploreEnd::Explored
            })
        }),
        Spawn::HandLimit { who } => Erased::boxed(HandLimit::new(*who), |_| Report::Nothing),
        Spawn::DrawCards { who, count, drop_unofficial } => Erased::boxed(HookedDraw::new(*who, *count, *drop_unofficial), |w| if w.unsupported { Report::Left } else { Report::Nothing }),
        Spawn::Reward { who, step, options, skip_single } => Erased::boxed(Reward::new(*who, *step, options.as_slice(), *skip_single), |w| Report::Chose(w.chosen)),
        Spawn::Research { who, primary, cost, scope, roster } => {
            Erased::boxed(Research::new(*who, *primary, *cost, *scope).with_roster(**roster), |w| if w.failed { Report::Left } else { Report::Nothing })
        }
        Spawn::RiderResearch { who, scope, roster } => {
            Erased::boxed(Research::rider(*who, *scope).with_roster(**roster), |w| if w.failed { Report::Left } else { Report::Nothing })
        }
        Spawn::Leadership { who, primary } => Erased::boxed(Leadership::new(*who, *primary), |_| Report::Nothing),
        Spawn::Construction { who, primary, facts } => Erased::boxed(Construction::new(*who, *primary, *facts), |_| Report::Nothing),
        Spawn::TitanBuild { who, facts, goods } => Erased::boxed(TitanBuild::new(*who, *facts, *goods), |w| Report::Built(w.built)),
        Spawn::Politics { who, primary, official } => Erased::boxed(Politics::new(*who, *primary, *official), |w| if w.unsupported { Report::Left } else { Report::Nothing }),
        Spawn::ConstructionTe { who, facts, uses: te } => {
            let uses: Vec<_> = te.uses().collect();
            Erased::boxed(Construction::new_te_producing(*who, *facts, te.source, &uses), |w| if w.unsupported { Report::Left } else { Report::Produced(w.produced.clone()) })
        }
        Spawn::TeWarfare { who, letnev_pool, free } => Erased::boxed(TeWarfare::new(*who, free.systems(), *letnev_pool, free.source), |w| Report::Free(w.chosen)),
        Spawn::Warfare { who, letnev_pool } => Erased::boxed(WarfarePrimary::new(*who, *letnev_pool), |_| Report::Nothing),
        Spawn::Diplomacy { who, card, primary, base_printing } => {
            let mode = if *primary { DMode::Primary { base_printing: *base_printing } } else { DMode::Secondary };
            Erased::boxed(Diplomacy::new(*who, *card, mode), |w| w.chosen().map_or(Report::Nothing, |system| Report::Placed(system, w.placed.clone())))
        }
        Spawn::Trade { who } => Erased::boxed(TradePrimary::new(*who), |w| Report::Goods(w.who, TRADE_PRIMARY_GOODS)),
        Spawn::Destroy { who, system, card, subtype, cause } => Erased::boxed(Destroy::new(*who, *system, *card, *subtype, *cause), |w| Report::Destroyed(w.destroyed)),
        Spawn::Absorb { who, producer, system, hits, texts } => Erased::boxed(Absorb::new(*who, *producer, *system, *hits, *texts), |_| Report::Nothing),
        Spawn::Produce { facts } => Erased::boxed(ProductionWindow::ability(facts), |w| Report::Made(w.made())),
        Spawn::Seeing { who, system, limits } => {
            let limits = Limits { enforce: limits.enforce, shown: limits.shown, halves: limits.halves };
            Erased::boxed(Enforce::new(*who, *system, limits), |w| Report::Removed(w.removed))
        }
        Spawn::ProduceOne { who, system, facts } => Erased::boxed(ProduceOne::new(*who, *system, *facts), |w| if w.unsupported { Report::Left } else { Report::Nothing }),
        Spawn::Home { facts } => Erased::boxed(HomeProduction::new(*facts), |w| if w.unsupported { Report::Left } else { Report::Made(w.made()) }),
        Spawn::HomeAt { who, system, roster, agent_rules } => Erased::boxed(HomeProduction::at(*who, *system, roster, *agent_rules), |w| if w.unsupported { Report::Left } else { Report::Made(w.made()) }),
        Spawn::Imperial { who, facts } => {
            let scoreable = facts.scoreable.iter().flatten().copied().collect();
            Erased::boxed(ImperialPrimary::new(*who, scoreable).with_price(facts.env), |w| if w.unsupported { Report::Left } else { Report::Scored(w.scored) })
        }
        // A window of the faction crate: the table's extra link starts it.
        Spawn::SelfAssembly { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Seat;
    use crate::query::economy::Spend;

    use crate::driver::{Driver, Status};
    use crate::kernel::children::ErasedChild;
    use crate::kernel::stage::{Illegal, Sx, Window};
    use crate::kernel::{Kernel, Next, Resolver};
    use crate::state::{CoreState, Player, Players};

    /// A parent that asks the table for `request`, runs the child without answering and keeps its report.
    #[derive(Debug, Clone)]
    struct Run {
        request: Spawn,
        child: ErasedChild,
        started: bool,
        report: Option<Report>,
    }

    impl Window for Run {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.started
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.started {
                self.started = true;
                let Some(window) = sx.table.links.start(&self.request) else { return Next::Done };
                self.child.start(window);
            }
            match self.child.step(sx) {
                Some(crate::kernel::stage::Ran::Done(report)) => {
                    self.report = Some(report);
                    Next::Done
                }
                Some(crate::kernel::stage::Ran::Asks(ask)) => Next::Continue(ask),
                Some(crate::kernel::stage::Ran::Pass(next)) => next,
                None => Next::Done,
            }
        }
        fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<crate::choice::Choice> {
            self.child.ask_of(s, k)
        }
        fn answer(&mut self, sx: &mut Sx<'_>, pick: crate::choice::Pick) -> Result<(), Illegal> {
            self.child.answer(sx, pick)
        }
    }

    /// One seat (an Arborec) with `goods` trade goods; the child `request` runs to its end on the registered table and its report
    /// comes back with the position after it.
    fn run(request: Spawn, goods: i16) -> (Option<Report>, CoreState) {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), crate::generated::Faction::TheArborec)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = goods;
        s.seats = crate::state::Seats::new(&[Seat(0)], &[Seat(0)], 1);
        let mut table = AbilityTable::default();
        register(&mut table);
        let mut d = Driver::new(Run { request, child: ErasedChild::idle(), started: false, report: None }, Resolver::new(&[Seat(0)], Some(Seat(0))), table, 1);
        assert!(matches!(d.run(&mut s, &Kernel::plain()).unwrap(), Status::Done), "the child asks nothing here");
        (d.window.report, s)
    }

    /// What a finished child reports is read off the window by the registered function.
    #[test]
    fn a_finished_child_reports_what_its_window_did() {
        let who = Seat(0);
        // A bill of nothing is paid; a bill nobody can pay is not.
        assert_eq!(run(Spawn::Bill { who, kind: Spend::Influence, cost: 0 }, 0).0, Some(Report::Paid(true)));
        assert_eq!(run(Spawn::Bill { who, kind: Spend::Influence, cost: 9 }, 0).0, Some(Report::Paid(false)));
        // A reward with one option takes it unasked.
        let options = crate::containers::SmallList::from_slice(&[crate::choice::explore::Opt::Convert]);
        assert_eq!(
            run(Spawn::Reward { who, step: crate::choice::explore::Step::MerchantStation, options: options.clone(), skip_single: true }, 0).0,
            Some(Report::Chose(Some(crate::choice::explore::Opt::Convert)))
        );
        // The Trade primary gains its goods and reports them for the parent to announce.
        let (report, after) = run(Spawn::Trade { who }, 1);
        assert_eq!(report, Some(Report::Goods(who, TRADE_PRIMARY_GOODS)));
        assert_eq!(after.players().by_seat(who).unwrap().econ.trade_goods, 1 + TRADE_PRIMARY_GOODS as i16, "the window itself gains them");
        // Windows with nothing to tell report nothing.
        assert_eq!(run(Spawn::HandLimit { who }, 0).0, Some(Report::Nothing));
    }

    /// Every request the kernel can put names a window of the registered table.
    #[test]
    fn every_request_starts_a_window() {
        let mut table = AbilityTable::default();
        register(&mut table);
        assert!(table.links.complete());
        let who = Seat(0);
        let requests = [
            Spawn::Bill { who, kind: Spend::Influence, cost: 3 },
            Spawn::HandLimit { who },
            Spawn::DrawCards { who, count: 2, drop_unofficial: true },
            Spawn::Reward { who, step: crate::choice::explore::Step::MerchantStation, options: crate::containers::SmallList::new(), skip_single: true },
            Spawn::Research { who, primary: false, cost: 4, scope: crate::tables::Scope(u32::MAX), roster: Box::leak(Box::default()) },
            Spawn::RiderResearch { who, scope: crate::tables::Scope(u32::MAX), roster: Box::leak(Box::default()) },
            Spawn::Leadership { who, primary: true },
            Spawn::Construction { who, primary: false, facts: crate::kernel::children::StructureFacts { pds: None, dock: None, source: crate::state::Code(0), alt: None } },
            Spawn::Politics { who, primary: true, official: true },
            Spawn::Warfare { who, letnev_pool: false },
            Spawn::Diplomacy { who, card: crate::generated::StrategyCard::DiplomacyBase, primary: false, base_printing: false },
            Spawn::Trade { who },
            Spawn::Destroy { who, system: crate::generated::System::ALL[0], card: crate::generated::ActionCard::CourageousToTheEnd, subtype: crate::state::Code(0), cause: crate::state::Code(0) },
            Spawn::Absorb { who, producer: who, system: crate::generated::System::ALL[0], hits: 1, texts: crate::kernel::children::HitTexts::default() },
            Spawn::ProduceOne { who, system: crate::generated::System::ALL[0], facts: crate::kernel::children::OneFacts { buildable: [None; 9], fleet_limit: 8, name: None, alias: None } },
            Spawn::Produce { facts: crate::kernel::children::ProduceFacts { who, system: crate::generated::System::ALL[0], limit: 2, max_unit_cost: Some(4), small_exemptions: 0, discount: 0, fleet_limit: 8, armada: 0, flagship_free: false, buildable: [None; 9] } },
        ];
        for request in &requests {
            assert!(table.links.start(request).is_some(), "{request:?}");
        }
    }
}
