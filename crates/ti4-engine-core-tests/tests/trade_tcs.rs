//! Lane M1: Temporal Command Suite (`nomad::timing::tcs`) as a trade-window child: an agent becomes exhausted (the event is staged and
//! announced), the Nomad answers, readies the agent and may perform a transaction with the agent's owner, the negotiation running through the
//! questions of the effect with the reach the card grants while it lasts.

use std::rc::Rc;

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind, Opt as ROpt, Relation};
use ti4_engine_core::choice::technology::{Ask as TAsk, Opt as TOpt};
use ti4_engine_core::choice::trade::{Opt as TrOpt, Shape};
use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::{advance, open, Env, Live};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::stage::{AbilityTable, Held, Illegal, Sx};
use ti4_engine_core::kernel::{fx, Next, Resolver};
use ti4_engine_core::keys::MarkKey;
use ti4_engine_core::state::{Code, EventKind, Key, Payload, ProductionUse, Val, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, spiral};

/// A window that announces the exhaustion of seat 1's agent and ends (the stand-in for the site that exhausted it).
#[derive(Clone)]
struct Exhaust {
    done: bool,
}

impl Window for Exhaust {
    type Stage = bool;
    fn stage(&self) -> bool {
        self.done
    }
    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.done {
            self.done = true;
            let rec = fx::event(EventKind::AgentExhausted, Payload::new().with(Key::Player, Val::Seat(Seat(1))).with(Key::Leader, Val::Leader(Leader::BerekarBerekon)));
            let _ = sx.emit(rec);
        }
        Next::Done
    }
    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        None
    }
    fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
        Err(Illegal)
    }
}

impl Held for Exhaust {
    const KIND: WindowKind = WindowKind::Production;
    fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
        match core.pending().window(WindowKind::Production)? {
            OpenWindow::Production(p) => Some(Self { done: p.data[0] == 1 }),
            _ => None,
        }
    }
    fn to_pending(&self, core: &mut CoreState) {
        core.pending_mut().open(OpenWindow::Production(Box::new(ProductionUse { actor: None, data: [u32::from(self.done), 0, 0] })));
    }
}

fn position() -> (CoreState, Kernel) {
    let p = plain_systems();
    let ring = spiral(61);
    // Nomad (0) and Arborec (1) are strangers: only the card's reach lets them deal.
    Scenario::new(&[Faction::TheNomad, Faction::TheArborec])
        .map_hexes(&[(p[0], ring[0]), (p[1], ring[60])])
        .unit(p[0], None, Unit::CruiserI, 0, 1)
        .unit(p[1], None, Unit::CruiserI, 1, 1)
        .tech(0, Technology::TemporalCommandSuite)
        .goods(0, 2)
        .goods(1, 1)
        .commodities(1, 1)
        .build()
}

fn env<'a>(k: &'a Kernel, s: &CoreState) -> Env<'a> {
    let mut table = AbilityTable::default();
    ti4_engine_core_factions::ce_movement::register_late_abilities(&mut table);
    ti4_engine_core_factions::ce_movement::register_abilities(&mut table);
    table.codes.turn.tcs = Code(9);
    table.codes.turn.transaction = Code(7);
    table.facts.staging = true;
    let order = [Seat(0), Seat(1)];
    let mut name = |spec: &ti4_engine_core::kernel::stage::Spec, _: Seat, _: Option<Seat>| AbilityId { kind: spec.kind, key: Code(spec.slot.0) };
    let defs = table.registry(s, &order, &mut name);
    let mut resolver = Resolver::new(&order, Some(Seat(0)));
    resolver.register(defs);
    Env { kernel: k, registry: Cow::new(resolver.registry().clone()), table, seeds: Rc::new(NoSeeds) }
}

fn agent_exhausted(s: &mut CoreState) {
    s.players_mut().by_seat_mut(Seat(1)).unwrap().leaders.status.insert(Leader::BerekarBerekon, ti4_engine_core::state::LeaderStatus::Exhausted);
}

fn pick(live: Live) -> Choice {
    let Live::Asks(a) = live else { panic!("a question") };
    a.choice
}

#[test]
fn the_nomad_readies_the_agent_and_deals_with_a_stranger_through_the_card() {
    let (mut s, k) = position();
    agent_exhausted(&mut s);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    // The window of AGENT_EXHAUSTED: the Nomad's card.
    let q = pick(advance::<Exhaust>(&mut s, &e, None).unwrap().0);
    assert_eq!(q.actor, Seat(0));
    let ability = q.opts.iter().find_map(|o| if let Opt::Reaction(ROpt::Ability(a)) = o { Some(*a) } else { None }).expect("the card is offered");
    assert_eq!(ability.kind, AbilityKind::Technology);
    let q = pick(advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Reaction(ROpt::Ability(ability))))).unwrap().0);
    assert_eq!((q.actor, q.ask), (Seat(0), Ask::Technology(TAsk::TcsTransaction { partner: Seat(1) })));
    assert!(q.ctx.optional && !q.ctx.space_battle, "the transaction is the holder's \"may\"");
    let status = s.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::BerekarBerekon).copied();
    assert_eq!(status, Some(ti4_engine_core::state::LeaderStatus::Exhausted), "the replay of a suspended step has not applied anything");
    // Transact: the Nomad proposes to a stranger, which only the card allows.
    let q = pick(advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::Transact)))).unwrap().0);
    assert!(matches!(q.ask, Ask::Trade(_)), "{:?}", q.ask);
    assert_eq!(q.actor, Seat(0));
    let q = pick(advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Trade(TrOpt::Deal(Shape::Goods { give: 1, want: 1 }))))).unwrap().0);
    assert_eq!(q.actor, Seat(1), "the partner answers");
    let (live, out) = advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Trade(TrOpt::Accept)))).unwrap();
    // The reaction windows of the resolution (nobody holds a card), then the end of the event.
    assert_eq!(live, Live::Done);
    assert!(out.emitted.iter().any(|r| r.kind == EventKind::AgentExhausted));
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::BerekarBerekon).copied(), Some(ti4_engine_core::state::LeaderStatus::Readied), "the agent is readied");
    assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::TemporalCommandSuite), "the card was exhausted");
    assert!(ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)), "the pair's transaction is spent");
    assert!(s.marks().tail.get(&MarkKey::NomadTcsNegotiating(Seat(0), Seat(1))).is_none(), "the reach lasts only while the negotiation does");
    assert_eq!(s.trade().this_round.as_slice(), &[(Seat(0), Seat(1))]);
    assert!(s.ledger.events.iter().filter_map(|e| e.plain()).next().is_none(), "the string logs no plain event for this transaction");
    assert!(!s.ledger.events.iter().any(|e| matches!(e, ti4_engine_core::state::Emit::Typed(t) if t.kind == EventKind::TransactionOpened)), "no opening is announced for the card's transaction");
}

#[test]
fn declining_the_transaction_leaves_only_the_readied_agent_and_a_used_card_is_not_offered_again() {
    let (mut s, k) = position();
    agent_exhausted(&mut s);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    let q = pick(advance::<Exhaust>(&mut s, &e, None).unwrap().0);
    let ability = q.opts.iter().find_map(|o| if let Opt::Reaction(ROpt::Ability(a)) = o { Some(*a) } else { None }).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Reaction(ROpt::Ability(ability))))).unwrap();
    let (live, _) = advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert_eq!(live, Live::Done);
    assert_eq!(s.players().by_seat(Seat(1)).unwrap().leaders.status.get(&Leader::BerekarBerekon).copied(), Some(ti4_engine_core::state::LeaderStatus::Readied));
    assert!(!ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)));
    // Without the card (already exhausted) nothing is asked.
    let (mut s, k) = position();
    agent_exhausted(&mut s);
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::TemporalCommandSuite);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    assert_eq!(advance::<Exhaust>(&mut s, &e, None).unwrap().0, Live::Done);
}

#[test]
fn a_clone_in_the_middle_of_the_transaction_continues_identically() {
    let (mut s, k) = position();
    agent_exhausted(&mut s);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    let q = pick(advance::<Exhaust>(&mut s, &e, None).unwrap().0);
    let ability = q.opts.iter().find_map(|o| if let Opt::Reaction(ROpt::Ability(a)) = o { Some(*a) } else { None }).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Reaction(ROpt::Ability(ability))))).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::Transact)))).unwrap();
    let mut fork = s.clone();
    for state in [&mut s, &mut fork] {
        advance::<Exhaust>(state, &e, Some(Pick::Opt(Opt::Trade(TrOpt::Deal(Shape::Goods { give: 1, want: 0 }))))).unwrap();
        advance::<Exhaust>(state, &e, Some(Pick::Opt(Opt::Trade(TrOpt::Refuse)))).unwrap();
    }
    assert_eq!(s, fork);
}

#[test]
fn a_seat_that_is_not_the_nomad_is_not_offered_the_card_even_holding_it() {
    let (mut s, k) = position();
    agent_exhausted(&mut s);
    // Seat 1 (Arborec) holds the technology; the Nomad does not.
    s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(Technology::TemporalCommandSuite);
    s.players_mut().by_seat_mut(Seat(1)).unwrap().tech.owned.insert(Technology::TemporalCommandSuite);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    assert_eq!(advance::<Exhaust>(&mut s, &e, None).unwrap().0, Live::Done, "nobody is asked");
}

#[test]
fn a_module_that_exempts_the_pair_leaves_the_cards_transaction_unrecorded() {
    let (mut s, mut k) = position();
    k.hooks.cards.transaction_limit_exempt.push(|_, _, _, _| true);
    agent_exhausted(&mut s);
    let e = env(&k, &s);
    open(&mut s, &Exhaust { done: false });
    let q = pick(advance::<Exhaust>(&mut s, &e, None).unwrap().0);
    let ability = q.opts.iter().find_map(|o| if let Opt::Reaction(ROpt::Ability(a)) = o { Some(*a) } else { None }).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Reaction(ROpt::Ability(ability))))).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Technology(TOpt::Transact)))).unwrap();
    advance::<Exhaust>(&mut s, &e, Some(Pick::Opt(Opt::Decline))).unwrap();
    assert!(!ti4_engine_core::query::trade::transacted_with(&s, Seat(0), Seat(1)));
}
