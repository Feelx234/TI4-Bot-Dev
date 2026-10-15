//! Tunable numbers of the rule bot, for optimisation runs.
//!
//! The defaults are the crate's constants, so without an override the bot decides exactly as it
//! always did. An override is **per thread** ([`set`], [`with`]): a self-play worker installs
//! the configuration of the bot it plays before each game, which lets a baseline and a candidate
//! share one process. The server and the browser never call [`set`] and so always run the
//! defaults. Keys are the lower-case names of the constants (see [`KEYS`]); [`Tuning::set_key`]
//! parses `key=value`.

use std::cell::Cell;

macro_rules! tuning {
    ($( $field:ident : $ty:ty = $default:expr, $doc:literal; )*) => {
        /// The tunable numbers. `Default` is the shipped bot.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Tuning { $( #[doc = $doc] pub $field: $ty, )* }

        impl Default for Tuning {
            fn default() -> Self { Self { $( $field: $default, )* } }
        }

        /// Every key accepted by [`Tuning::set_key`], with its default and meaning.
        pub const KEYS: &[(&str, &str)] = &[ $( (stringify!($field), $doc), )* ];

        impl Tuning {
            /// Set one value from text.
            ///
            /// # Errors
            /// A message for an unknown key or a value that does not parse.
            pub fn set_key(&mut self, key: &str, value: &str) -> Result<(), String> {
                match key {
                    $( stringify!($field) => {
                        self.$field = value.trim().parse().map_err(|e| format!("{key}={value}: {e}"))?;
                        Ok(())
                    } )*
                    _ => Err(format!("unknown tuning key '{key}' (known: {})",
                        KEYS.iter().map(|k| k.0).collect::<Vec<_>>().join(", "))),
                }
            }

            /// `(key, value)` of every field, as text.
            #[must_use]
            pub fn entries(&self) -> Vec<(&'static str, String)> {
                vec![ $( (stringify!($field), self.$field.to_string()), )* ]
            }
        }

        $( #[doc = $doc] #[must_use] pub fn $field() -> $ty { CURRENT.with(|c| c.get().$field) } )*
    };
}

tuning! {
    win_margin: f64 = crate::WIN_MARGIN, "ln strength ratio a fight must reach to be taken";
    retreat_margin: f64 = crate::RETREAT_MARGIN, "matchup below which a space combat is retreated from";
    mecatol_bonus: f64 = crate::MECATOL_BONUS, "extra worth of Mecatol Rex while the custodians token is on it";
    activation_threshold: f64 = crate::ACTIVATION_THRESHOLD, "least worth of a system to steer activation";
    eager_worth: f64 = crate::EAGER_WORTH, "worth that always gets a tactical action before the strategic one";
    accept_net: f64 = crate::ACCEPT_NET, "least engine-estimated value for accepting a trade";
    min_idle_ground: i64 = crate::MIN_IDLE_GROUND, "ground forces kept waiting at docks";
    rich_resources: f64 = crate::RICH_RESOURCES, "spendable resources from which production is slot-limited";
    fleet_tokens_wanted: i32 = crate::FLEET_TOKENS_WANTED, "fleet pool size kept before tokens go elsewhere";
    expansion_rounds: u32 = crate::EXPANSION_ROUNDS, "opening rounds played for planets above all";
    plenty_resources: i64 = crate::PLENTY_RESOURCES, "spendable resources from which a dock is worth a token";
    enough_tokens: i32 = crate::ENOUGH_TOKENS, "tactic plus strategy tokens beyond which Leadership buys no more";
    objective_round: u32 = crate::objectives::OBJECTIVE_ROUND, "round from which the objective layer plays";
    complete_value: f64 = crate::objectives::COMPLETE_VALUE, "worth per point of an objective completed by an action";
    progress_value: f64 = crate::objectives::PROGRESS_VALUE, "worth of progress toward an objective";
    deny_value: f64 = crate::objectives::DENY_VALUE, "worth of denying a rival about to win";
    deny_within: i32 = crate::objectives::DENY_WITHIN, "points from winning at which a rival is denied";
    objectives_enabled: bool = true, "false: the objective layer (round 3 on) is switched off for every seat";
    faction_plans_enabled: bool = true, "false: the faction plans (research order, production weights, preferred card) are off";
    short_bonus: f64 = crate::faction_plans::SHORT_BONUS, "production weight of a unit the faction plan is short of";
    upgraded_bonus: f64 = crate::faction_plans::UPGRADED_BONUS, "production weight of an upgraded unit";
    vote_rule: bool = false, "v2: agenda votes, Politics' speaker and agenda placement by rule (answers.rs)";
    casualty_rule: bool = false, "v2: sustain damage whenever possible, least valuable casualty first";
    ground_rule: bool = false, "v2: troops only for free lift; no fight that takes nothing in rounds 1-2";
    mecatol_rival_bonus: f64 = 0.0, "v2: extra worth of Mecatol Rex while a rival holds it";
    planner_weight: f64 = 0.0, "v2: production weight per point of an objective within planner_gap that more of a unit meets";
    planner_gap: f64 = 3.0, "v2: steps short within which the planner weighs an objective";
    progress_gap: f64 = 2.0, "steps short within which progress toward an objective is worth activating for";
    nebula_margin: f64 = 0.0, "v2: extra matchup needed to attack a fleet in a nebula (defender +1)";
    home_guard: bool = false, "v2: no warship leaves home when the rest could not hold it against a fleet a rival can bring";
    misc_rule: bool = false, "v2: ready the best planet, retreat to own systems, Trade replenishes nobody";
}

thread_local! {
    static CURRENT: Cell<Tuning> = Cell::new(Tuning::default());
}

/// Install `tuning` for this thread.
pub fn set(tuning: Tuning) {
    CURRENT.with(|c| c.set(tuning));
}

/// The tuning of this thread.
#[must_use]
pub fn get() -> Tuning {
    CURRENT.with(Cell::get)
}

/// Run `f` with `tuning` installed, then restore the previous one.
pub fn with<T>(tuning: Tuning, f: impl FnOnce() -> T) -> T {
    let before = get();
    set(tuning);
    let out = f();
    set(before);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_constants_and_keys_parse() {
        assert!((win_margin() - crate::WIN_MARGIN).abs() < f64::EPSILON);
        let mut t = Tuning::default();
        t.set_key("win_margin", "0.9").unwrap();
        assert!(t.set_key("nope", "1").is_err());
        with(t, || assert!((win_margin() - 0.9).abs() < f64::EPSILON));
        assert!((win_margin() - crate::WIN_MARGIN).abs() < f64::EPSILON);
    }
}
