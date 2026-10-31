//! ghost: Creuss IFF, the promissory note of the Creuss (`factions::ghost::iff`): "At the start of your turn during the action phase: place or move a Creuss wormhole token into either a system that contains a
//! planet you control or a non-home system that does not contain another player's ships. Then, return this card to the Creuss player." Optional (the window's own use / decline is the "may"); the question is the
//! destination: every Creuss token (alpha, then beta) into every such system it is not in, the systems in map order (the hex order of the placement). Registered for every seat
//! (`promissory:*:iff:TURN_BEGAN:after`); the condition names the holder at the start of its own turn in the action phase, a map and a destination.

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, PromissoryNote, System, Wormhole};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::{fx, fx_explore};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::query::board;
use ti4_engine_core::state::{CoreState, EventKind, Key, NoteKey, Phase};

/// Creuss IFF (`promissory:*:iff:TURN_BEGAN:after`).
pub const IFF: Slot = Slot(42_018);

const NOTE: NoteKey = NoteKey { note: PromissoryNote::CreussIff, faction: Faction::TheGhostsOfCreuss };

/// `CREUSS_TOKENS`: the two tokens a Creuss player owns, in the string's order.
const TOKENS: [Wormhole; 2] = [Wormhole::Alpha, Wormhole::Beta];

/// `held`: the note is in the hands of `holder`, who is no Creuss, in the action phase.
fn held(s: &CoreState, holder: Seat) -> bool {
    s.meta().phase == Phase::Action && s.cards().promissory.get(&NOTE) == Some(&holder) && s.players().by_seat(holder).is_some_and(|p| p.faction != Faction::TheGhostsOfCreuss)
}

/// `wormhole_generator_destinations`: the systems a token may go into, in map order, with each token not yet there. Empty without a map.
pub fn destinations(s: &CoreState, who: Seat) -> Vec<(Wormhole, System)> {
    let Some(galaxy) = s.galaxy() else { return Vec::new() };
    let mut placed: Vec<(ti4_engine_core::hex::Hex, System)> = galaxy.in_play().iter().filter_map(|ix| System::from_index(ix)).filter(|sys| !galaxy.is_hyperlane(*sys)).filter_map(|sys| galaxy.coord_of(sys).map(|hex| (hex, sys))).collect();
    placed.sort_by_key(|(hex, _)| *hex);
    let systems: Vec<System> = placed
        .into_iter()
        .map(|(_, sys)| sys)
        .filter(|sys| {
            let controls = board::planets_of(s, *sys).any(|p| s.board().planet(p).control == Some(who));
            let others_ships = board::space_units(s, *sys).iter().any(|u| u.owner() != who && u.kind().data().ship);
            controls || (!sys.data().home_system && !others_ships)
        })
        .collect();
    let mut found = Vec::new();
    for kind in TOKENS {
        for sys in &systems {
            if s.explore().wormhole_tokens.get(&kind) != Some(sys) {
                found.push((kind, *sys));
            }
        }
    }
    found
}

fn condition(c: &CondCx<'_>) -> bool {
    c.event.payload.seat(Key::Player) == Some(c.def.owner) && held(c.state, c.def.owner) && !destinations(c.state, c.def.owner).is_empty()
}

fn place(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    if eff.event.payload.seat(Key::Player) != Some(me) || !held(eff.read().0, me) {
        return Ok(());
    }
    let found = destinations(eff.read().0, me);
    if found.is_empty() {
        return Ok(());
    }
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::Iff))),
        ctx: Ctx {
            origin: Origin::FactionAbility(Content::Promissory(PromissoryNote::CreussIff)),
            subtype: Subtype::Reaction(RSub::G(GSub::T(TSub::Iff))),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: found.iter().map(|(kind, system)| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Token { kind: *kind, system: *system })))).collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Token { kind, system })))) = eff.ask(&choice) else { return Ok(()) };
    if !found.contains(&(kind, system)) {
        return Ok(());
    }
    fx_explore::place_wormhole_token(eff.state(), kind, system);
    fx::give_back_note(eff.state(), NOTE);
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(IFF, AbilityKind::Promissory, EventKind::TurnBegan, Relation::After, place).optional().when(condition));
}
