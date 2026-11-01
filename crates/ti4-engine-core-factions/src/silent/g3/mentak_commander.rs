//! mentak: S'ula Mentarion, the commander (`factions::mentak::commander`): "After you win a space combat: you may force your opponent to give you 1 promissory note from their hand." Optional. The opponents
//! come from the `SPACE_COMBAT_WON` event; one question of the commander's owner (which opponent, asked only with several that have a note to give), one of the opponent (which note, asked even
//! with a single one). The hand is the notes held and not faceup that the receiver may be given (Mahact's Hubris refuses an Alliance), in the string's order, then the opponent's own Support for the
//! Throne while it is home. The Support scores a point on arrival (`rules::trade::receive_support`); any other note is taken (`fx_strategy::take_note`: faceup if that is where the card lives).
//! Registered for every seat (`leader:*:mentakcommander:SPACE_COMBAT_WON:after`).

use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::silent::{Ask as GAsk, Opt as GOpt, Subtype as GSub};
use ti4_engine_core::choice::silent3::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Faction, Leader, PromissoryNote};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, AdapterFacts, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::fx_strategy;
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::{promissory, trade_deals};
use ti4_engine_core::rules::trade::receive_support;
use ti4_engine_core::state::{CoreState, EventKind, EventRec, Key, List, NoteKey};

use crate::cc_ground::borrowed::query::has_commander_ability;

/// S'ula Mentarion (`leader:*:mentakcommander:SPACE_COMBAT_WON:after`).
pub const SULA: Slot = Slot(42_013);

const COMMANDER: Leader = Leader::SulaMentarion;

/// One note an opponent could give: a note of the hand, or the opponent's Support for the Throne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gift {
    Note(NoteKey),
    Support(Faction),
}

/// `notes_in_hand`: the notes `player` holds that are not faceup and that `receiver` may be given, in the string's order, then the player's own Support while it is home.
pub fn notes_in_hand(s: &CoreState, facts: &AdapterFacts, player: Seat, receiver: Seat) -> Vec<Gift> {
    let mut held: Vec<NoteKey> = promissory::held_by(s, player)
        .into_iter()
        .filter(|key| !s.cards().promissory_faceup.contains_key(key))
        .filter(|key| promissory::may_receive(s, receiver, key.note))
        .collect();
    // The string's order is the notes' text (the adapter's `note_order`); the typed order is the enum's.
    match facts.plays.as_ref() {
        Some(plays) => held.sort_by(|a, b| (plays.note_order)(*a, *b)),
        None => held.sort(),
    }
    let mut gifts: Vec<Gift> = held.into_iter().map(Gift::Note).collect();
    gifts.extend(trade_deals::available_support(s, player).map(Gift::Support));
    gifts
}

/// `commander_targets`: the opponents of the winner (the event's) with a note to give.
fn targets(s: &CoreState, k: &Kernel, facts: &AdapterFacts, event: &EventRec, owner: Seat) -> Vec<Seat> {
    if event.payload.seat(Key::Player) != Some(owner) || !has_commander_ability(s, k, owner, COMMANDER) {
        return Vec::new();
    }
    match event.payload.list(Key::Other(facts.combat.opponents)) {
        Some(List::Seats(opponents)) => opponents.as_slice().iter().copied().filter(|o| *o != owner && !notes_in_hand(s, facts, *o, owner).is_empty()).collect(),
        _ => Vec::new(),
    }
}

fn condition(c: &CondCx<'_>) -> bool {
    !targets(c.state, c.kernel, c.facts, c.event, c.def.owner).is_empty()
}

fn ctx(subtype: TSub) -> Ctx {
    Ctx {
        origin: Origin::FactionAbility(Content::Leader(COMMANDER)),
        subtype: Subtype::Reaction(RSub::G(GSub::T(subtype))),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false, // mutants:skip (the adapter puts the combat into every nested context)
        invasion_seq: None,
    }
}

fn force(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let found = {
        let (s, k) = eff.read();
        targets(s, k, eff.adapter_facts(), eff.event, me)
    };
    let target = match found.as_slice() {
        [] => return Ok(()),
        [only] => *only,
        many => {
            let choice = Choice {
                actor: me,
                ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::SulaOpponent))),
                ctx: ctx(TSub::SulaOpponent),
                opts: many.iter().map(|who| Opt::Reaction(ROpt::G(GOpt::T(TOpt::Seat(*who))))).collect::<Opts>(),
            };
            let Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Seat(who))))) = eff.ask(&choice) else { return Ok(()) };
            who
        }
    };
    if !found.contains(&target) {
        return Ok(());
    }
    let hand = notes_in_hand(eff.read().0, eff.adapter_facts(), target, me);
    let choice = Choice {
        actor: target,
        ask: Ask::Reaction(RAsk::G(GAsk::T(TAsk::SulaGive { to: me }))),
        ctx: ctx(TSub::SulaGive),
        opts: hand
            .iter()
            .map(|gift| {
                Opt::Reaction(ROpt::G(GOpt::T(match gift {
                    Gift::Note(key) => TOpt::Note(*key),
                    Gift::Support(faction) => TOpt::Support(*faction),
                })))
            })
            .collect::<Opts>(),
    };
    let gift = match eff.ask(&choice) {
        Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Note(key))))) => Gift::Note(key),
        Pick::Opt(Opt::Reaction(ROpt::G(GOpt::T(TOpt::Support(faction))))) => Gift::Support(faction),
        _ => return Ok(()),
    };
    if !hand.contains(&gift) {
        return Ok(());
    }
    match gift {
        // Support scores a point on arrival: the same path transactions use.
        Gift::Support(faction) => {
            let _ = receive_support(eff.state(), me, faction);
        }
        Gift::Note(key) => fx_strategy::take_note(eff.state(), me, key),
    }
    Ok(())
}

pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SULA, AbilityKind::Leader, EventKind::SpaceCombatWon, Relation::After, force).optional().when(condition));
}

#[allow(dead_code, reason = "the note a gift names")]
const _: PromissoryNote = PromissoryNote::SupportForTheThrone;
