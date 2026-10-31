//! saar: Captain Mendosa, the agent (lane M6c). "When a player activates a system: you may exhaust this card to increase the move value of 1
//! of that player's ships to match the move value of the ship on the game board that has the highest move value." The boost is remembered
//! against the activation (`SaarAgentBoost`, read by `movement::agent_move_bonus`). The Ssruu copy is the same offer with Ssruu exhausted.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::generated::{Faction, Leader, System, Unit};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, Applies, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};
use ti4_engine_core::tables::Scope;

/// Captain Mendosa (`leader:*:saaragent:SYSTEM_ACTIVATED:after`).
pub const MENDOSA: Slot = Slot(24001);
/// Ssruu copying Captain Mendosa (`leader:{lender faction}:{lender}:yssarilagent:{borrower}:saaragent:SYSTEM_ACTIVATED:after`).
pub const MENDOSA_COPY: Slot = Slot(24002);

const AGENT: Leader = Leader::CaptainMendosa;
/// `ti4_model::state::RULES_MENDOSA_SHIP`.
const RULES_MENDOSA_SHIP: u32 = 2;

/// One ship Mendosa may raise: where it is, which of the activator's ships in the space area it is, its type and damage, and the move values.
struct Candidate {
    system: System,
    index: usize,
    unit: Unit,
    damaged: bool,
    own: u8,
    top: u8,
}

fn scope(k: &ti4_engine_core::kernel::Kernel) -> Scope {
    crate::common::scope(k)
}

/// `highest_move`: the highest printed move value of any ship on the board.
fn highest_move(s: &CoreState, scope: Scope) -> u8 {
    s.board().systems().flat_map(|(_, cell)| cell.units.as_slice().iter()).filter_map(|u| scope.unit(u.kind())).filter(|d| d.ship).map(|d| d.movement.unwrap_or(0)).max().unwrap_or(0)
}

/// `agent_ships`: the activator's ships slower than the fastest on the board (equal ships are one choice from the modern rules on).
fn candidates(s: &CoreState, scope: Scope, activator: Seat) -> Vec<Candidate> {
    let top = highest_move(s, scope);
    let modern = s.meta().rules_version >= RULES_MENDOSA_SHIP;
    let mut found: Vec<Candidate> = Vec::new();
    for (system, cell) in s.board().systems() {
        let ships = cell.units.as_slice().iter().filter(|u| u.owner() == activator && scope.unit(u.kind()).is_some_and(|d| d.ship));
        for (index, unit) in ships.enumerate() {
            let own = scope.unit(unit.kind()).and_then(|d| d.movement).unwrap_or(0);
            if own < top {
                if modern && found.iter().any(|f| f.system == system && f.unit == unit.kind() && f.damaged == unit.sustained()) {
                    continue;
                }
                found.push(Candidate { system, index, unit: unit.kind(), damaged: unit.sustained(), own, top });
            }
        }
    }
    found
}

fn ready(s: &CoreState, who: Seat, card: Leader) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.leaders.status.get(&card) == Some(&LeaderStatus::Readied))
}

fn when(c: &CondCx<'_>) -> bool {
    let Some(activator) = c.event.payload.seat(Key::Player) else { return false };
    ready(c.state, c.def.owner, AGENT) && !candidates(c.state, scope(c.kernel), activator).is_empty()
}

fn copy_when(c: &CondCx<'_>) -> bool {
    let Some(activator) = c.event.payload.seat(Key::Player) else { return false };
    crate::cd_registry::yssaril::ssruu::borrowable(c.state, c.def.owner, c.def.pair_source(), AGENT) && !candidates(c.state, scope(c.kernel), activator).is_empty()
}

/// `store_boost`: the move value is remembered against this activation, per the rules version.
fn store(s: &mut CoreState, activator: Seat, found: &Candidate) {
    let seq = s.meta().activation_seq;
    let bonus = u32::from(found.top - found.own);
    let value = if s.meta().rules_version >= RULES_MENDOSA_SHIP {
        let count = s
            .board()
            .sys(found.system)
            .units
            .as_slice()
            .iter()
            .filter(|u| u.owner() == activator && u.kind() == found.unit && u.sustained() == found.damaged && u.kind().data().ship)
            .count();
        MarkVal::Rec(vec![
            MarkVal::Atom(Atom::Num(seq)),
            MarkVal::Atom(Atom::System(found.system)),
            MarkVal::Atom(Atom::Unit(found.unit)),
            MarkVal::Atom(Atom::Num(u32::from(found.damaged))),
            MarkVal::Atom(Atom::Num(u32::try_from(count).unwrap_or(u32::MAX))),
            MarkVal::Atom(Atom::Num(bonus)),
        ])
    } else {
        MarkVal::Rec(vec![MarkVal::Atom(Atom::Num(seq)), MarkVal::Atom(Atom::System(found.system)), MarkVal::Atom(Atom::Num(u32::try_from(found.index).unwrap_or(u32::MAX))), MarkVal::Atom(Atom::Num(bonus))])
    };
    s.marks_mut().tail.edit().insert(MarkKey::SaarAgentBoost(activator), value);
}

/// Ask which ship, exhaust `card` (the agent, or Ssruu), remember the boost.
fn offer(eff: &mut Eff<'_, '_>, card: Leader, copy: bool) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(activator) = eff.event.payload.seat(Key::Player) else { return Ok(()) };
    let list = {
        let (state, kernel) = eff.read();
        candidates(state, scope(kernel), activator)
    };
    if list.is_empty() {
        return Ok(());
    }
    let mut opts: Opts = list
        .iter()
        .enumerate()
        .map(|(n, c)| Opt::Reaction(ROpt::Boost { n: u8::try_from(n).unwrap_or(u8::MAX), system: c.system, unit: c.unit, own: c.own, top: c.top }))
        .collect();
    opts.push(Opt::Decline);
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::AgentShip { copy }),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Other(eff.codes().activation.saar_agent)),
            subtype: Subtype::Reaction(RSub::AgentShip),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    let Pick::Opt(Opt::Reaction(ROpt::Boost { n, .. })) = eff.ask(&choice) else { return Ok(()) };
    let Some(chosen) = list.get(usize::from(n)) else { return Ok(()) };
    if !ready(eff.state(), me, card) {
        return Ok(());
    }
    let (state, kernel) = eff.state_and_kernel();
    ti4_engine_core::kernel::fx_strategy::exhaust_agent(state, kernel, me, card);
    store(eff.state(), activator, chosen);
    Ok(())
}

fn mendosa(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if !ready(eff.state(), me, AGENT) {
        return Ok(());
    }
    offer(eff, AGENT, false)
}

fn mendosa_copy(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let (me, lender) = (eff.owner(), eff.def.pair_source());
    if !crate::cd_registry::yssaril::ssruu::borrowable(eff.state(), me, lender, AGENT) {
        return Ok(());
    }
    offer(eff, Leader::Ssruu, true)
}

/// The Saar source and every other seat as a borrower (the string's static roster).
fn others(_: &CoreState, borrower: Seat, lender: Seat) -> bool {
    borrower != lender
}

/// The two abilities of the activation window (`activation::register`).
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(MENDOSA, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, mendosa).optional().when(when));
    t.add(Spec::new(MENDOSA_COPY, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, mendosa_copy).applies(Applies::Faction(Faction::TheClanOfSaar)).pair(others).optional().when(copy_when));
}
