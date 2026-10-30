//! The rule bot on the typed core: package D3 of `docs/todo/D3-bots.md`.
//!
//! This crate is the home of the typed bot. It is a separate crate on purpose (user decision, 2026-10-30): the
//! string bot `ti4-rule-bot` is not touched and stays as the oracle, the fallback for every decision group that
//! is not ported yet, and the bot of the learned-policy tools.
//!
//! State of the port: D3-0 only. [`choose_typed_explained`] is the *self-check*: the typed entry point answers
//! with the string bot, so the shadow harness (`ti4-engine-core-bridge`, `bot_shadow`) can be proven to compare
//! and to catch a difference before any typed code exists. The `Q` wrapper, the `Verdict` router and the
//! per-group modules (`tactical`, `menu`, `production`, `strategy`, `combat`, `agenda`, `objectives`, `trade`,
//! `reactions`, `agents`) arrive with D3-2 (after D1b).
//!
//! Neutrality contract (docs/todo/D3-bots.md section 5): for the same choice, seed, seat place, decision index
//! and tuning, the answer is the triple `(picked, smart, rng)` of [`ti4_rule_bot::choose_explained`], including
//! the position of the random stream.

use rand_chacha::ChaCha8Rng;
use ti4_engine::choice::{Choice, SeatObservation};

/// The typed counterpart of [`ti4_rule_bot::choose_explained`]: the index of the picked option, whether the
/// bot's own opinion decided (`false`: the random pick stood), and the generator after the draws.
///
/// D3-0: the self-check, answered by the string bot. Every later package replaces one decision group by a
/// typed implementation that must return the same triple.
#[must_use]
pub fn choose_typed_explained(
    choice: &Choice,
    seen: Option<&SeatObservation<'_>>,
    game_seed: u64,
    seat: usize,
    index: usize,
) -> (usize, bool, ChaCha8Rng) {
    ti4_rule_bot::choose_explained(choice, seen, game_seed, seat, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine::choice::ChoiceOption;

    fn ti4_model_id(s: &str) -> ti4_model::id::PlayerId {
        ti4_model::id::PlayerId::new(s)
    }

    #[test]
    fn self_check_is_the_string_bot() {
        let choice = Choice::new(
            ti4_model_id("a"),
            "pick one",
            vec![
                ChoiceOption::labelled("x", "pick", "x"),
                ChoiceOption::labelled("y", "pick", "y"),
                ChoiceOption::labelled("z", "pick", "z"),
            ],
        );
        for index in 0..20 {
            let (p, s, r) = choose_typed_explained(&choice, None, 7, 1, index);
            let (p2, s2, r2) = ti4_rule_bot::choose_explained(&choice, None, 7, 1, index);
            assert_eq!((p, s), (p2, s2));
            assert_eq!(r.get_word_pos(), r2.get_word_pos());
            assert!(!s, "a random bot never has an opinion");
        }
    }
}
