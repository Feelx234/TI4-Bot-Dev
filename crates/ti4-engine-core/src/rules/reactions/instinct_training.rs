//! Instinct Training (Xxcha, `reactions::instinct_training`): "You may exhaust this card and spend 1 token from your
//! strategy pool when another player plays an action card: cancel that action card." The card is still spent (1.15),
//! as with Sabotage. Registered for every seat (a technology can be gained mid-game), the condition limits it to the
//! holder; WHEN `ACTION_CARD_PLAYED`.
//!
//! A card carried by a Nekro Valefar Assimilator is the string's (`Unknown`); the staged `STRATEGY_TOKEN_SPENT`
//! (Muaat Magmus listens) is announced by the window's adapter.

use crate::choice::reaction::{AbilityKind, Ask as RAsk, Opt as ROpt, Relation, Subtype as RSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Faction, Technology};
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, CondCx, Eff, Slot, Spec, Unknown};
use crate::kernel::fx;
use crate::kernel::resolver::TimingError;
use crate::state::{CoreState, EventKind, EventRec, Key, Payload, Val};

/// Instinct Training (`technology:*:it:ACTION_CARD_PLAYED:when`).
pub const INSTINCT_TRAINING: Slot = Slot(5);

const IT: Technology = Technology::InstinctTraining;

/// Held, not exhausted and a strategy token to spend. `Err`: the card may be carried by a Nekro Valefar Assimilator.
fn ready(s: &CoreState, who: Seat) -> Result<bool, Unknown> {
    let Some(p) = s.players().by_seat(who) else { return Ok(false) };
    if !p.tech.owned.contains(IT) {
        return if p.faction == Faction::TheNekroVirus { Err(Unknown::NekroCarriedCard) } else { Ok(false) };
    }
    Ok(!p.tech.exhausted.contains(IT) && p.econ.strategic_tokens > 0)
}

fn condition(c: &CondCx<'_>) -> bool {
    let Some(player) = c.event.payload.seat(Key::Player) else { return false };
    if player == c.def.owner {
        return false;
    }
    match ready(c.state, c.def.owner) {
        Ok(ok) => ok,
        Err(why) => c.unknown(why),
    }
}

fn cancel(eff: &mut Eff<'_, '_>) -> Result<(), TimingError> {
    let me = eff.owner();
    let Some(card) = eff.event.payload.action(Key::Card) else { return Ok(()) };
    let choice = Choice {
        actor: me,
        ask: Ask::Reaction(RAsk::InstinctTraining { card }),
        ctx: Ctx {
            origin: Origin::Content(Content::Technology(IT)),
            subtype: Subtype::Reaction(RSub::InstinctTraining),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: [Opt::Reaction(ROpt::Use), Opt::Decline].into_iter().collect::<Opts>(),
    };
    let Pick::Opt(Opt::Reaction(ROpt::Use)) = eff.ask(&choice) else { return Ok(()) };
    if !matches!(ready(eff.state(), me), Ok(true)) {
        return Ok(());
    }
    if let Some(p) = eff.state().players_mut().by_seat_mut(me) {
        p.econ.strategic_tokens -= 1;
        p.tech.exhausted.insert(IT);
    }
    let reason = eff.codes().instinct_training;
    fx::stage(
        eff.state(),
        EventRec { id: 0, kind: EventKind::StrategyTokenSpent, payload: Payload::new().with(Key::Player, Val::Seat(me)).with(Key::Reason, Val::Code(reason)), cancelled: false },
    );
    eff.cancel();
    Ok(())
}

pub fn register(t: &mut AbilityTable) {
    t.add(Spec::new(INSTINCT_TRAINING, AbilityKind::Technology, EventKind::ActionCardPlayed, Relation::When, cancel).optional().when(condition));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::reaction::{AbilityId, Opt as ROpt};
    use crate::driver::Status;
    use crate::generated::ActionCard;
    use crate::kernel::{fx, Kernel};
    use crate::rules::reactions::testkit::driver;
    use crate::state::{Code, Player, Players};

    fn game(tokens: i16, exhausted: bool) -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheXxchaKingdom), Player::new(Seat(1), Faction::TheVuilraithCabal)]);
        let p = s.players_mut().by_seat_mut(Seat(0)).unwrap();
        p.tech.owned.insert(IT);
        if exhausted {
            p.tech.exhausted.insert(IT);
        }
        p.econ.strategic_tokens = tokens;
        s
    }

    fn played_by(who: Seat) -> EventRec {
        fx::event(EventKind::ActionCardPlayed, Payload::new().with(Key::Player, Val::Seat(who)).with(Key::Card, Val::Action(ActionCard::Sabotage1)))
    }

    fn table() -> AbilityTable {
        let mut t = AbilityTable::default();
        register(&mut t);
        t
    }

    fn pick_ability() -> Pick {
        Pick::Opt(Opt::Reaction(ROpt::Ability(AbilityId { kind: AbilityKind::Other, key: Code(INSTINCT_TRAINING.0) })))
    }

    #[test]
    fn using_it_cancels_the_card_spends_a_token_exhausts_the_technology_and_stages_the_spend() {
        let (mut s, k) = (game(2, false), Kernel::plain());
        let mut d = driver(table(), &s, &[Seat(1), Seat(0)], played_by(Seat(1)));
        let Status::Asking(window) = d.run(&mut s, &k).unwrap() else { panic!("the window asks the holder") };
        assert_eq!(window.actor, Seat(0));
        let Status::Asking(q) = d.resume(&mut s, &k, pick_ability()).unwrap() else { panic!("the ability asks") };
        assert_eq!(q.ask, Ask::Reaction(RAsk::InstinctTraining { card: ActionCard::Sabotage1 }));
        assert_eq!(q.opts.as_slice(), &[Opt::Reaction(ROpt::Use), Opt::Decline]);
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Reaction(ROpt::Use))).unwrap(), Status::Done);
        let p = s.players().by_seat(Seat(0)).unwrap();
        assert_eq!(p.econ.strategic_tokens, 1);
        assert!(p.tech.exhausted.contains(IT));
        assert!(d.emitted[0].cancelled, "the card play is cancelled");
        assert_eq!(s.pending().staged.len(), 1);
        assert_eq!(s.pending().staged[0].kind, EventKind::StrategyTokenSpent);
    }

    #[test]
    fn declining_changes_nothing() {
        let (mut s, k) = (game(2, false), Kernel::plain());
        let mut d = driver(table(), &s, &[Seat(1), Seat(0)], played_by(Seat(1)));
        d.run(&mut s, &k).unwrap();
        d.resume(&mut s, &k, pick_ability()).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.strategic_tokens, 2);
        assert!(!d.emitted[0].cancelled && s.pending().staged.is_empty());
    }

    #[test]
    fn it_is_not_offered_for_its_holders_own_card_without_a_token_or_exhausted() {
        let k = Kernel::plain();
        for (mut s, who) in [(game(2, false), Seat(0)), (game(0, false), Seat(1)), (game(2, true), Seat(1))] {
            let mut d = driver(table(), &s, &[Seat(1), Seat(0)], played_by(who));
            assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done, "nothing to ask");
        }
    }

    #[test]
    fn a_nekro_seat_without_the_card_may_carry_it_and_the_window_is_unknown() {
        let (mut s, k) = (game(2, false), Kernel::plain());
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheNekroVirus), Player::new(Seat(1), Faction::TheVuilraithCabal)]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.strategic_tokens = 2;
        let mut d = driver(table(), &s, &[Seat(1), Seat(0)], played_by(Seat(1)));
        let _ = d.run(&mut s, &k).unwrap();
        assert_eq!(d.unknown(), Some(Unknown::NekroCarriedCard));
    }
}
