//! The draw of action cards an ability of this lane makes (`action_cards::draw`): the core's own window (`rules::action_cards::draw::DrawCards`: the bonus card and the discard of Yssaril's
//! Scheming, the discard over the hand limit) stepped inside the effect, its questions going to the tape like any question an ability asks. Where the adapter gave the window no printed-name
//! groups (it could not ask) the draw is a LOUD gap (`Unknown::AbilityGap` of the rule's own slot) and nothing more is done.

use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{Eff, Runner, Slot, Unknown};
use ti4_engine_core::kernel::stage::Window;
use ti4_engine_core::kernel::Next;
use ti4_engine_core::rules::action_cards::draw::DrawCards;

/// Draw `count` cards for `who`, asking what the draw asks (Scheming's discard, the hand limit). (From rules version 30 the unofficial cards leave the deck before a draw; only the
/// latest version is supported.)
pub fn draw(eff: &mut Eff<'_, '_>, who: Seat, count: usize, slot: Slot) {
    draw_with(eff, who, count, true, slot);
}

/// [`draw`] with the choice of dropping the two unofficial cards from the deck first: `false` for a window that only enforces the hand limit (`action_cards::enforce_hand_limit` touches no deck).
pub fn draw_with(eff: &mut Eff<'_, '_>, who: Seat, count: usize, drop_unofficial: bool, slot: Slot) {
    let unsupported = eff.inline().with_sx(|sx| {
        let mut window = DrawCards::new(who, count, drop_unofficial);
        let mut next = window.step(sx);
        // (A draw asks at most a Scheming discard and a few over the limit; the bound only keeps a broken table from looping.)
        for _ in 0..64 { // mutants:skip (the bound only keeps a broken table from looping)
            match next {
                Next::Continue(_) => {
                    let Some(question) = window.ask_of(sx.state, sx.kernel) else { break };
                    let pick = sx.ask(&question);
                    if window.answer(sx, pick).is_err() { // mutants:skip (a draw asks at most a Scheming discard before it is done; the answer is replayed from the tape)
                        break;
                    }
                    next = window.step(sx);
                }
                _ => break,
            }
        }
        window.unsupported
    });
    if unsupported {
        eff.unknown(Unknown::AbilityGap(slot));
    }
}
