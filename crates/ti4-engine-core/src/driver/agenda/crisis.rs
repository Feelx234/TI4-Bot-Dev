//! Galactic Crisis Pact (lane E2; string `agenda_effects::effect_crisis`): each player, from the speaker clockwise, may perform the secondary
//! ability of the elected strategy card without spending a command token. A driver-level composition: the question is the window of
//! `rules::crisis_pact`, the ability is the card's own child window (`strategic::perform::run`, the plain secondary: no faction substitutes the
//! primary here, `strategy_cards::secondary`), and nothing follows (`strategy_cards::secondary` does not resolve faction effects).

use crate::driver::kernel_env::KernelEnv;
use crate::driver::turn::inline::{run, Flow};
use crate::driver::windows::strategic::perform;
use crate::generated::{StrategyCard, StrategyCardKind};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::query::seats;
use crate::rules::crisis_pact::CrisisPact;
use crate::rules::strategy::{self, Env};

/// Whether `who` would be asked (`strategy::crisis_pact_choice` is `Some`): Leadership needs the influence of one command token, every other
/// secondary something to do (`secondary_can_do_something`).
fn eligible(sx: &Sx<'_>, who: Seat, card: StrategyCard) -> bool {
    let env = KernelEnv::new(sx.state, sx.kernel);
    let cx = strategy::Cx { core: sx.state, kernel: sx.kernel, env: &env };
    let kind = card.data().kind;
    if kind == StrategyCardKind::Leadership {
        return env.leadership_affordable(who);
    }
    sx.state.players().by_seat(who).is_some_and(|p| strategy::can_do_something(&cx, who, p, kind))
}

/// The effect of the agenda for the elected `card`.
pub fn run_pact(sx: &mut Sx<'_>, card: StrategyCard) -> Flow {
    for who in seats::speaker_order(sx.state).iter().copied() {
        let mut window = CrisisPact::new(who, card, eligible(sx, who, card));
        if run(sx, &mut window) == Flow::Halt {
            return Flow::Halt;
        }
        if window.accepted {
            let mut free = None;
            if perform::run(sx, who, card, false, &mut free) == Flow::Halt {
                return Flow::Halt;
            }
        }
    }
    Flow::Done
}
