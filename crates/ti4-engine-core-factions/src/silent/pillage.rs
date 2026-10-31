//! mentak: Pillage (`factions::mentak::pillage`): "After 1 of your neighbors gains trade goods or resolves a transaction: if they have 3 or more trade goods, you
//! may take 1 of their trade goods or commodities." Optional; the string registers it for every seat on both events (`ability:*:pillage:TRADE_GOODS_GAINED:after`
//! and `...:TRANSACTION_RESOLVED:after`). The taken commodity becomes a trade good in the Mentak player's hands and the gain is staged (source `pillage`).
//!
//! Suffi An (the Mentak agent: "after the Pillage faction ability is used against another player: you may exhaust this card; if you do, you and that player each
//! draw 1 action card") and Ssruu's copy of it need the action-card draw as a child window: they are the loud gaps `gaps::SUFFI_AN` and `gaps::SUFFI_AN_COPY`; the take is
//! native and a position where the agent could answer stops the step right after it (`Unknown::AbilityGap`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::{fx, Kernel};
use ti4_engine_core::query::trade;
use ti4_engine_core::rules::trade::KernelEnv;
use ti4_engine_core::state::{Code, CoreState, EventKind, EventRec, Key, LeaderStatus, NoteKey};

use super::gaps;

/// Pillage after a gain of trade goods (`ability:*:pillage:TRADE_GOODS_GAINED:after`).
pub const PILLAGE_GAINED: Slot = Slot(40_008);
/// Pillage after a resolved transaction (`ability:*:pillage:TRANSACTION_RESOLVED:after`).
pub const PILLAGE_TRANSACTION: Slot = Slot(40_009);

fn is_mentak(s: &CoreState, who: Seat) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheMentakCoalition)
}

/// `transaction_gain_pending`: a `TRADE_GOODS_GAINED` from a transaction is staged for `who` and not yet announced.
fn transaction_gain_pending(s: &CoreState, who: Seat, transaction: Code) -> bool {
    s.pending().staged.iter().any(|e| e.kind == EventKind::TradeGoodsGained && e.payload.seat(Key::Player) == Some(who) && e.payload.code(Key::Source) == Some(transaction))
}

/// `protected`: `target` holds the Mentak's Promise of Protection faceup.
fn protected(s: &CoreState, target: Seat) -> bool {
    let note = NoteKey { note: PromissoryNote::PromiseOfProtection, faction: Faction::TheMentakCoalition };
    s.cards().promissory.get(&note) == Some(&target) && s.cards().promissory_faceup.contains_key(&note)
}

/// `pillage_targets`: the neighbours of `owner` the event concerns that may be pillaged now, in the string's order.
fn targets(s: &CoreState, k: &Kernel, event: &EventRec, owner: Seat, transaction: Code) -> Vec<Seat> {
    if !is_mentak(s, owner) {
        return Vec::new();
    }
    let p = &event.payload;
    let concerned: Vec<Seat> = match event.kind {
        EventKind::TradeGoodsGained => p.seat(Key::Player).into_iter().collect(),
        EventKind::TransactionResolved => [Key::Proposer, Key::Partner].into_iter().filter_map(|key| p.seat(key)).collect(),
        _ => Vec::new(),
    };
    let Some(adj) = s.map().adjacency() else { return Vec::new() };
    let env = KernelEnv { core: s, kernel: k };
    // The limits are not read by the neighbour queries.
    let cx = trade::Cx { core: s, adj, mecatol: ti4_engine_core::query::objectives::mecatol_in_galaxy(s), limits: trade::Limits { per_action: 0, per_round: 0 }, env: &env };
    concerned
        .into_iter()
        .filter(|target| *target != owner)
        .filter(|target| event.kind != EventKind::TransactionResolved || !transaction_gain_pending(s, *target, transaction))
        .filter(|target| trade::are_neighbours(&cx, owner, *target))
        .filter(|target| s.players().by_seat(*target).is_some_and(|p| p.econ.trade_goods >= 3))
        .filter(|target| !protected(s, *target))
        .collect()
}

fn condition(c: &CondCx<'_>) -> bool {
    !targets(c.state, c.kernel, c.event, c.def.owner, c.codes.not_from_supply[0]).is_empty()
}

fn pillage(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let found = {
        let (s, k) = eff.read();
        targets(s, k, eff.event, me, eff.codes().not_from_supply[0])
    };
    let mut options: Vec<(bool, Seat)> = Vec::new();
    for target in &found {
        options.push((false, *target));
        if eff.state().players().by_seat(*target).is_some_and(|p| p.econ.commodities > 0) {
            options.push((true, *target));
        }
    }
    let (commodity, target) = match options.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::Pillage)),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Other(eff.codes().not_from_supply[1])),
                    subtype: Subtype::Reaction(RSub::G(GSub::Pillage)),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts: many.iter().map(|(commodity, target)| Opt::Reaction(ROpt::G(GOpt::Pillage { commodity: *commodity, target: *target }))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::Pillage { commodity, target }))) = eff.ask(&choice) else { return Ok(()) };
            if !found.contains(&target) {
                return Ok(());
            }
            (commodity, target)
        }
    };
    let taken = {
        let Some(p) = eff.state().players_mut().by_seat_mut(target) else { return Ok(()) };
        if commodity && p.econ.commodities > 0 {
            p.econ.commodities -= 1;
            true
        } else if !commodity && p.econ.trade_goods > 0 {
            p.econ.trade_goods -= 1;
            true
        } else {
            false
        }
    };
    if !taken {
        return Ok(());
    }
    let source = eff.codes().not_from_supply[1];
    fx::gain_trade_goods_staged(eff.state(), me, 1, source);
    // Suffi An answers the use of Pillage (a draw of action cards for two seats, a child window); Ssruu's copy of it too.
    let (s, _) = eff.read();
    let agent = s.players().by_seat(me).is_some_and(|p| p.leaders.status.get(&Leader::SuffiAnMentakagent) == Some(&LeaderStatus::Readied));
    let copy = s.players().iter().any(|p| p.faction == Faction::TheYssarilTribes);
    if copy {
        eff.unknown(Unknown::AbilityGap(gaps::slot(gaps::SUFFI_AN_COPY)));
    } else if agent && !s.decks().action.is_empty() {
        eff.unknown(Unknown::AbilityGap(gaps::slot(gaps::SUFFI_AN)));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(PILLAGE_GAINED, AbilityKind::Ability, EventKind::TradeGoodsGained, Relation::After, pillage).optional().when(condition));
    t.add(Spec::new(PILLAGE_TRANSACTION, AbilityKind::Ability, EventKind::TransactionResolved, Relation::After, pillage).optional().when(condition));
}
