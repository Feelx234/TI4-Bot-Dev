//! mahact: Scepter of Dominion, the promissory note (`factions::mahact::scepter`): "At the start of the strategy phase: choose 1 non-home system that contains your units; each other player who has a token on the
//! Mahact player's command sheet places a token from their reinforcements in that system. Then, return this card to the Mahact player." Mandatory (a holder with no system still returns the card); the holder
//! picks the system (asked with several), the payers are the owners of the tokens in the Mahact fleet pool other than the holder, without a token in the system and with one in reinforcements, in the pool's order.
//! Registered for every seat (`promissory:*:scepter:STRATEGY_PHASE_BEGAN:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, PromissoryNote, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::{board, economy};
use ti4_engine_core::state::{CoreState, EventKind, NoteKey};

use crate::cc_ground::mahact::query::fleet_pool_owners;

/// Scepter of Dominion (`promissory:*:scepter:STRATEGY_PHASE_BEGAN:after`).
pub const SCEPTER: Slot = Slot(42_017);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::ScepterOfDominion, faction: Faction::TheMahactGeneSorcerers };

/// `scepter_owner`: the Mahact seat whose Scepter `holder` holds.
fn mahact_of(s: &CoreState, holder: Seat) -> Option<Seat> {
    let mahact = s.players().iter().find(|p| p.faction == Faction::TheMahactGeneSorcerers)?.seat;
    (mahact != holder && s.cards().promissory.get(&NOTE) == Some(&holder)).then_some(mahact)
}

/// `scepter_systems`: the non-home systems (of a seated player) that contain the holder's units, in the string's (id) order.
fn systems(s: &CoreState, holder: Seat) -> Vec<System> {
    let homes: Vec<System> = s.players().iter().filter_map(|p| p.home_system).collect();
    System::ALL
        .iter()
        .copied()
        .filter(|system| !homes.contains(system))
        .filter(|system| board::space_units(s, *system).iter().any(|u| u.owner() == holder) || board::planets_of(s, *system).any(|p| board::planet_units(s, p).iter().any(|u| u.owner() == holder)))
        .collect()
}

/// `scepter_payers`: who places a token in `system`.
fn payers(s: &CoreState, mahact: Seat, holder: Seat, system: System) -> Vec<Seat> {
    let mut owners: Vec<Seat> = fleet_pool_owners(s, mahact).as_slice().iter().copied().filter(|payer| *payer != holder).collect();
    owners.dedup();
    owners.retain(|payer| !board::has_token(s, system, *payer) && economy::tokens_in_reinforcements(s, *payer, fx::tokens_held_by_mahact(s, *payer)) > 0);
    owners
}

/// The factions of `seats`, as the option shows them (at most five other players).
fn factions(s: &CoreState, seats: &[Seat]) -> [Option<Faction>; 5] {
    let mut out = [None; 5];
    for (slot, seat) in out.iter_mut().zip(seats) {
        *slot = s.players().by_seat(*seat).map(|p| p.faction);
    }
    out
}

fn condition(c: &CondCx<'_>) -> bool {
    mahact_of(c.state, c.def.owner).is_some()
}

fn place(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(mahact) = mahact_of(eff.read().0, me) else { return Ok(()) };
    let found = systems(eff.read().0, me);
    let system = match found.as_slice() {
        [] => None,
        [only] => Some(*only),
        many => {
            let opts: Opts = many
                .iter()
                .map(|system| {
                    let who = payers(eff.read().0, mahact, me, *system);
                    Opt::Reaction(ROpt::G(GOpt::T(TOpt::Scepter { system: *system, payers: factions(eff.read().0, &who) })))
                })
                .collect();
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::Scepter))),
                ctx: Ctx {
                    origin: Origin::FactionAbility(Content::Promissory(PromissoryNote::ScepterOfDominion)),
                    subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::Scepter))),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            };
            match eff.ask(&choice) {
                Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Scepter { system, .. })))) => many.iter().copied().find(|s| *s == system),
                _ => None,
            }
        }
    };
    if let Some(system) = system {
        // Placing one token never changes another payer's reinforcements or tokens here: deciding them all first is the same as checking each in turn.
        for payer in payers(eff.read().0, mahact, me, system) {
            fx::place_command_token(eff.state(), payer, system);
        }
    }
    fx::give_back_note(eff.state(), NOTE);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SCEPTER, AbilityKind::Promissory, EventKind::StrategyPhaseBegan, Relation::After, place).when(condition));
}
