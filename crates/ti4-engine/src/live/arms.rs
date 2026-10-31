//! The names of the arms (`TI4_NATIVE_ARMS`, the reports and the census labels use the same words) and the set of native arms a
//! process runs with. Text lives here, not in the core.

use ti4_engine_core::driver::session::Arm;
use ti4_engine_core::driver::step::NativeArms;

/// The name of `arm`.
#[must_use]
pub const fn name(arm: Arm) -> &'static str {
    match arm {
        Arm::Finished => "finished",
        Arm::Blocked => "blocked",
        Arm::LeaderFollowers => "leader_followers",
        Arm::Secondary => "secondary",
        Arm::EventScoring => "event_scoring",
        Arm::Scoring => "scoring",
        Arm::Tokens => "tokens",
        Arm::Voting => "voting",
        Arm::Diplomacy => "diplomacy",
        Arm::Aftermath => "aftermath",
        Arm::Invasion => "invasion",
        Arm::AgendaTalks => "agenda_talks",
        Arm::Trade => "trade",
        Arm::Tactical => "tactical",
        Arm::TurnPrelude => "turn_prelude",
        Arm::Status => "status",
        Arm::Agenda => "agenda",
        Arm::StrategyAnnounce => "strategy_announce",
        Arm::Ask => "ask",
        Arm::Phase => "phase",
    }
}

/// `"status,agenda"`: the names of [`name`]; `all` is the arms that have a live function ([`NativeArms::registered`]); `none` or
/// an empty text is the empty set.
///
/// # Errors
/// The first word that names no arm.
pub fn parse(text: &str) -> Result<NativeArms, String> {
    let mut arms: Vec<Arm> = Vec::new();
    let mut all = false;
    for word in text.split(',').map(str::trim).filter(|w| !w.is_empty() && *w != "none") {
        if word == "all" {
            all = true;
        } else {
            arms.push(*Arm::ALL.iter().find(|a| name(**a) == word).ok_or_else(|| format!("no arm is named {word:?}"))?);
        }
    }
    let named = NativeArms::of(&arms);
    Ok(if all { NativeArms::of(&Arm::ALL.iter().copied().filter(|a| named.has(*a) || NativeArms::registered().has(*a)).collect::<Vec<_>>()) } else { named })
}

/// The names of the arms in the set, in arm order.
#[must_use]
pub fn names(set: NativeArms) -> Vec<&'static str> {
    Arm::ALL.iter().filter(|a| set.has(**a)).map(|a| name(*a)).collect()
}

/// `TI4_NATIVE_ARMS` (unset or unreadable: the empty set, i.e. today's behaviour). Test and gate binaries read it; the product does not.
#[must_use]
pub fn from_env() -> NativeArms {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("TI4_NATIVE_ARMS").ok().and_then(|t| parse(&t).ok()).unwrap_or(NativeArms::NONE)
    }
    #[cfg(target_arch = "wasm32")]
    {
        NativeArms::NONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_are_distinct_and_read_back() {
        let mut seen: Vec<_> = Arm::ALL.iter().map(|a| name(*a)).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 20);
        for a in Arm::ALL {
            assert_eq!(parse(name(a)).unwrap(), NativeArms::of(&[a]));
        }
    }

    #[test]
    fn the_arm_set_reads_its_names() {
        assert_eq!(names(parse("tokens, agenda").unwrap()), ["tokens", "agenda"], "arm order, not text order");
        assert_eq!(parse("").unwrap(), NativeArms::NONE);
        assert_eq!(parse("none").unwrap(), NativeArms::NONE);
        assert_eq!(parse("all").unwrap(), NativeArms::registered());
        assert!(parse("tokens,nope").unwrap_err().contains("nope"));
    }
}
