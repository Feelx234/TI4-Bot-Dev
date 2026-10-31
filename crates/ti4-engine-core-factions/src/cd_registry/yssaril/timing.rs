//! yssaril: So Ata, the commander (lane M6d). "After another player activates a system that contains your units: you may look at that player's
//! action cards, promissory notes, or secret objectives." The look is a reveal to the owner alone (`MarkKey::CardsReveal`, scope `action`, source
//! `yssarilcommander`), ended when the action is over: the clear is an `ACTION_COMPLETED` ability too (`yssarilcommander_clear`), as in the string.
//! (The string also tells the undo tracker that hidden information was seen; the core holds no tracker: it is the host's.)

use ti4_engine_core::choice::activation::{Ask as MAsk, Look, Opt as MOpt, Subtype as MSub};
use ti4_engine_core::choice::generic::Content;
use ti4_engine_core::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use ti4_engine_core::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use ti4_engine_core::containers::SmallList;
use ti4_engine_core::generated::{Leader, System};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec};
use ti4_engine_core::kernel::resolver::TimingError;
use ti4_engine_core::keys::{Atom, MarkKey, MarkVal, Word};
use ti4_engine_core::query::{board, leaders};
use ti4_engine_core::state::{CoreState, EventKind, Key};

/// So Ata (`leader:*:yssarilcommander:SYSTEM_ACTIVATED:after`).
pub const SO_ATA: Slot = Slot(50_020);
/// The end of the look (`leader:*:yssarilcommander_clear:ACTION_COMPLETED:after`).
pub const SO_ATA_CLEAR: Slot = Slot(50_021);

/// How the adapter sorts note ids (their text), when it says.
type NoteOrder = Option<fn(ti4_engine_core::state::NoteKey, ti4_engine_core::state::NoteKey) -> core::cmp::Ordering>;

fn order_of(facts: &ti4_engine_core::kernel::ability::AdapterFacts) -> NoteOrder {
    facts.plays.as_ref().map(|p| p.note_order)
}

/// The hidden promissory notes `who` holds (faceup notes are public), in the table's order.
fn hidden_notes(s: &CoreState, who: Seat, order: NoteOrder) -> Vec<ti4_engine_core::state::NoteKey> {
    let mut notes: Vec<_> = s.cards().promissory.iter().filter(|(key, holder)| **holder == who && !s.cards().promissory_faceup.contains_key(key)).map(|(key, _)| *key).collect();
    // The string lists a hand by the text of the note ids; the adapter says how they sort (the typed order is by enum).
    if let Some(order) = order {
        notes.sort_by(|a, b| order(*a, *b));
    }
    notes
}

/// `lookable`: the kinds with something in them.
fn lookable(s: &CoreState, order: NoteOrder, who: Seat) -> Vec<Look> {
    let mut kinds = Vec::new();
    let Some(player) = s.players().by_seat(who) else { return kinds };
    if !player.cards.hand.is_empty() {
        kinds.push(Look::ActionCards);
    }
    if !hidden_notes(s, who, order).is_empty() {
        kinds.push(Look::PromissoryNotes);
    }
    if !player.cards.secrets.is_empty() {
        kinds.push(Look::SecretObjectives);
    }
    kinds
}

/// `commander_target`: the activating player, when the seat has the commander's ability, another player activated a system holding its units
/// and there is something to look at.
fn target(s: &CoreState, facts: &ti4_engine_core::kernel::ability::AdapterFacts, seat: Seat, activator: Seat, system: System) -> Option<(Seat, Vec<Look>)> {
    if !leaders::has_commander_ability_in(s, seat, Leader::SoAta) || activator == seat {
        return None;
    }
    let has_units = board::space_units(s, system).iter().any(|p| p.owner() == seat) || board::planets_of(s, system).any(|planet| board::planet_units(s, planet).iter().any(|p| p.owner() == seat));
    if !has_units {
        return None;
    }
    let kinds = lookable(s, order_of(facts), activator);
    (!kinds.is_empty()).then_some((activator, kinds))
}

fn payload_target(s: &CoreState, facts: &ti4_engine_core::kernel::ability::AdapterFacts, seat: Seat, p: &ti4_engine_core::state::Payload) -> Option<(Seat, Vec<Look>)> {
    target(s, facts, seat, p.seat(Key::Player)?, p.system(Key::System)?)
}

fn look_when(c: &CondCx<'_>) -> bool {
    payload_target(c.state, c.facts, c.def.owner, &c.event.payload).is_some()
}

fn deck_word(look: Look) -> Word {
    match look {
        Look::ActionCards => Word::ActionCards,
        Look::PromissoryNotes => Word::PromissoryNotes,
        Look::SecretObjectives => Word::SecretObjectives,
    }
}

/// The ids shown: the target's cards of the kind, in the order the string lists them.
fn shown(s: &CoreState, order: NoteOrder, who: Seat, look: Look) -> Vec<MarkVal> {
    let Some(player) = s.players().by_seat(who) else { return Vec::new() };
    match look {
        Look::ActionCards => player.cards.hand.as_slice().iter().map(|c| MarkVal::Atom(Atom::Card(*c))).collect(),
        Look::SecretObjectives => player.cards.secrets.as_slice().iter().map(|c| MarkVal::Atom(Atom::Secret(*c))).collect(),
        Look::PromissoryNotes => hidden_notes(s, who, order).into_iter().map(|k| MarkVal::Rec(vec![MarkVal::Atom(Atom::Note(k.note)), MarkVal::Atom(Atom::Faction(k.faction))])).collect(),
    }
}

fn so_ata(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let found = {
        let (s, _) = eff.read();
        payload_target(s, eff.adapter_facts(), me, &eff.event.payload)
    };
    let Some((target, kinds)) = found else { return Ok(()) };
    let look = if let [only] = kinds[..] {
        only
    } else {
        let opts: Opts = kinds.iter().map(|k| Opt::Reaction(ROpt::Act(MOpt::Look(*k)))).collect();
        let choice = Choice {
            actor: me,
            ask: Ask::Reaction(RAsk::Act(MAsk::SoAta { target })),
            ctx: Ctx {
                origin: Origin::FactionAbility(Content::Other(eff.codes().activation.yssaril_commander)),
                subtype: Subtype::Reaction(RSub::Act(MSub::SoAta)),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts,
        };
        let Pick::Opt(Opt::Reaction(ROpt::Act(MOpt::Look(look)))) = eff.ask(&choice) else { return Ok(()) };
        if !kinds.contains(&look) {
            return Ok(());
        }
        look
    };
    let order = order_of(eff.adapter_facts());
    let s = eff.state();
    let mut ids = shown(s, order, target, look);
    let key = MarkKey::CardsReveal(Word::Action, Word::YssarilCommander, me, target, deck_word(look));
    // Revealing again merges into the same row: the ids already shown stay, the new ones follow.
    if let Some(MarkVal::List(old)) = s.marks().tail.get(&key) {
        let mut merged = old.clone();
        merged.extend(ids.drain(..).filter(|id| !old.contains(id)));
        ids = merged;
    }
    s.marks_mut().tail.edit().insert(key, MarkVal::List(ids));
    Ok(())
}

/// `commander_reveals_stand`: a So Ata reveal row of the seat is still standing.
fn stands(s: &CoreState, viewer: Seat) -> bool {
    s.marks().tail.iter().any(|(key, _)| matches!(key, MarkKey::CardsReveal(_, Word::YssarilCommander, from, ..) if *from == viewer))
}

fn clear_when(c: &CondCx<'_>) -> bool {
    stands(c.state, c.def.owner)
}

/// `hooks_cards::clear_reveals_from("yssarilcommander")`: every reveal made by So Ata, whoever looked.
fn clear(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let s = eff.state();
    let doomed: Vec<MarkKey> = s.marks().tail.iter().filter(|(key, _)| matches!(key, MarkKey::CardsReveal(_, Word::YssarilCommander, ..))).map(|(key, _)| *key).collect();
    for key in doomed {
        s.marks_mut().tail.edit().remove(&key);
    }
    Ok(())
}

/// The abilities of the activation window (`activation::register`): So Ata, the last module of the string's order.
pub fn register_activation(t: &mut AbilityTable) {
    t.add(Spec::new(SO_ATA, AbilityKind::Leader, EventKind::SystemActivated, Relation::After, so_ata).optional().when(look_when));
}

/// The end of the look (registered with the other `ACTION_COMPLETED` abilities).
pub fn register_abilities(t: &mut AbilityTable) {
    t.add(Spec::new(SO_ATA_CLEAR, AbilityKind::Leader, EventKind::ActionCompleted, Relation::After, clear).when(clear_when));
}
