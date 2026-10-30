//! nomad: the timing abilities (the reaction windows), docs/ENGINE_REACTIONS.md. Ported: The Cavalry, a promissory note of
//! the Nomad: played by its holder at the start of a space combat (the holder picks the non-fighter ship type that takes
//! the flagship's abilities; one type is not a question; the note stays faceup with them), and returned to the Nomad
//! when the combat ends. The Thundarian (agent, package W2): "after the Roll Dice step of combat: you may exhaust this card; if you
//! do, return to the start of that step" (the replay mark the roll step reads). Not ported: Artuno, Mercer, Temporal Command Suite.

use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::{fx, fx_strategy};
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
use ti4_engine_core::query::reach;
use ti4_engine_core::rules::combat;
use ti4_engine_core::state::{CoreState, EventKind, Key, LeaderStatus};

use super::query::{cavalry_ship_type, CAVALRY};

/// The Cavalry, played (`promissory:*:cavalry:SPACE_COMBAT_STARTED:after`).
pub const CAVALRY_PLAY: Slot = Slot(10_001);
/// The Cavalry, returned (`promissory:*:cavalry:SPACE_COMBAT_ENDED:after`).
pub const CAVALRY_RETURN: Slot = Slot(10_002);

/// The non-fighter ship types `who` has in the combat's system, in id order (the order of the enum is the order of the ids).
fn ship_types(s: &CoreState, k: &ti4_engine_core::kernel::Kernel, who: Seat, system: System) -> Vec<Unit> {
    let rd = Rd::new(s, k).with_blanked(reach::blanked(s));
    let mut kinds: Vec<Unit> = combat::ships_of(rd, who, system)
        .into_iter()
        .filter(|p| rd.data(p.kind()).is_some_and(|d| d.ship && d.base_type != UnitBaseType::Fighter))
        .map(|p| p.kind())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

/// `cavalry_combat`: the system of a space combat `holder` may play the note at the start of.
fn playable_at(c: &CondCx<'_>) -> Option<System> {
    let (s, holder) = (c.state, c.def.owner);
    if s.cards().promissory.get(&CAVALRY) != Some(&holder) || s.cards().promissory_faceup.contains_key(&CAVALRY) {
        return None;
    }
    let nomad = |who: Seat| s.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheNomad);
    if nomad(holder) {
        return None;
    }
    let p = &c.event.payload;
    let (attacker, defender) = (p.seat(Key::Attacker)?, p.seat(Key::Defender)?);
    let opponent = if holder == attacker {
        defender
    } else if holder == defender {
        attacker
    } else {
        return None;
    };
    if nomad(opponent) || s.players().by_seat(opponent).is_none() {
        return None;
    }
    let system = p.system(Key::System)?;
    (!ship_types(s, c.kernel, holder, system).is_empty()).then_some(system)
}

fn play_condition(c: &CondCx<'_>) -> bool {
    playable_at(c).is_some()
}

fn play(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let holder = eff.owner();
    let Some(system) = eff.event.payload.system(Key::System) else { return Ok(()) };
    let kinds = {
        let (s, k) = eff.read();
        ship_types(s, k, holder, system)
    };
    let Some(&first) = kinds.first() else { return Ok(()) };
    let kind = if kinds.len() == 1 {
        first
    } else {
        let source = eff.codes().cavalry;
        let choice = Choice {
            actor: holder,
            ask: Ask::Reaction(RAsk::Cavalry),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Other(source)),
                subtype: Subtype::Reaction(RSub::Cavalry),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: kinds.iter().map(|u| Opt::Reaction(ROpt::Ship(*u))).collect::<Opts>(),
        };
        match eff.ask(&choice) {
            Pick::Opt(Opt::Reaction(ROpt::Ship(u))) if kinds.contains(&u) => u,
            _ => first,
        }
    };
    eff.state().cards_mut().promissory_faceup.edit().insert(CAVALRY, ());
    eff.state().marks_mut().tail.edit().insert(MarkKey::NomadCavalry(holder), MarkVal::Rec(vec![MarkVal::Atom(Atom::System(system)), MarkVal::Atom(Atom::Unit(kind))]));
    Ok(())
}

fn return_condition(c: &CondCx<'_>) -> bool {
    c.event.payload.system(Key::System).is_some_and(|system| cavalry_ship_type(c.state, c.def.owner, system).is_some())
}

fn give_back(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let holder = eff.owner();
    eff.state().marks_mut().tail.edit().remove(&MarkKey::NomadCavalry(holder));
    fx::give_back_note(eff.state(), CAVALRY);
    Ok(())
}

/// The Thundarian at the end of the "Roll Dice" step of a space combat (`leader:*:nomadagentthundarian:SPACE_COMBAT_ROLL_STEP_ENDED:after`).
pub const THUNDARIAN_SPACE: Slot = Slot(10_003);
/// ... and of a ground combat (`GROUND_COMBAT_ROLL_STEP_ENDED`).
pub const THUNDARIAN_GROUND: Slot = Slot(10_004);

/// A readied The Thundarian in hand, and no replay already asked for in this step.
fn thundarian_condition(c: &CondCx<'_>) -> bool {
    c.state.players().by_seat(c.def.owner).is_some_and(|p| p.leaders.status.get(&Leader::TheThundarian) == Some(&LeaderStatus::Readied))
        && !c.state.marks().tail.contains_key(&MarkKey::PrivateCombatReplayRoll())
}

/// Exhaust the card and ask for the step to be played again (`combat::request_roll_replay`). A Nomad that hears every agent exhausted
/// (Temporal Command Suite) is told by an event the core does not emit: that position is the string's.
fn thundarian(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let owner = eff.owner();
    let (s, k) = eff.read();
    if k.hooks.direct.nomad_watches_agent_exhaustion(s) {
        eff.unknown(ti4_engine_core::kernel::ability::Unknown::NomadAgentExhaustion);
    }
    if fx_strategy::exhaust_leader(eff.state(), owner, Leader::TheThundarian) {
        eff.state().marks_mut().tail.edit().insert(MarkKey::PrivateCombatReplayRoll(), MarkVal::Atom(Atom::Absent));
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(THUNDARIAN_SPACE, AbilityKind::Leader, EventKind::SpaceCombatRollStepEnded, Relation::After, thundarian).when(thundarian_condition).optional());
    t.add(Spec::new(THUNDARIAN_GROUND, AbilityKind::Leader, EventKind::GroundCombatRollStepEnded, Relation::After, thundarian).when(thundarian_condition).optional());
    t.add(Spec::new(CAVALRY_PLAY, AbilityKind::Promissory, EventKind::SpaceCombatStarted, Relation::After, play).when(play_condition).optional());
    t.add(Spec::new(CAVALRY_RETURN, AbilityKind::Promissory, EventKind::SpaceCombatEnded, Relation::After, give_back).when(return_condition));
}
