//! The string engine's view of the new engine's multi-asks (`docs/MULTIASK1_RESUME.md`): where the core asks ONE question for what the
//! string asks as a sequence (a token gain of several tokens, a restack, a payment), the shadow ([`crate::native_play::shadow_play`]) answers
//! the string's questions with the expansion of the core's answer ([`legacy_answers`]) and compares the string's questions of that
//! expansion loosely ([`loose_match`]): same seat, and every answer one the string offered.

use std::collections::VecDeque;

use crate::oracle::Scripted;

/// The string pool ids of a token gain, in the canonical order of the core (tactic, fleet, strategic).
const POOL_IDS: [&str; 3] = ["tactic_tokens", "fleet_tokens", "strategic_tokens"];

/// The prompt of the string's pool question (one token).
pub const GAIN_PROMPT: &str = "gain a command token into which pool";

/// The `{a}|{b}|{c}` counts of a split id.
#[must_use]
pub fn split_counts(id: &str) -> Option<[u8; 3]> {
    let mut parts = id.split('|');
    let mut next = || parts.next()?.parse::<u8>().ok();
    let counts = [next()?, next()?, next()?];
    parts.next().is_none().then_some(counts)
}

/// The string pool answers of a gain split, one per token.
#[must_use]
pub fn gain_answers(split: [u8; 3]) -> VecDeque<String> {
    POOL_IDS.iter().zip(split).flat_map(|(id, n)| std::iter::repeat_n((*id).to_owned(), usize::from(n))).collect()
}

/// The pool names of the string's restack moves (`{from}|{to}`).
const RESTACK_POOLS: [&str; 3] = ["tactic", "fleet", "strategy"];

/// The string's Predictive Intelligence answers that turn the pools `now` into `target`: the fewest single-token moves (every source has a
/// surplus, so each move is offered when its turn comes), then `done` unless the string stops by itself (as many moves as tokens).
#[must_use]
pub fn restack_answers(now: [u8; 3], target: [u8; 3]) -> VecDeque<String> {
    let surplus = (0..3).flat_map(|p| std::iter::repeat_n(p, usize::from(now[p].saturating_sub(target[p]))));
    let deficit: Vec<usize> = (0..3).flat_map(|p| std::iter::repeat_n(p, usize::from(target[p].saturating_sub(now[p])))).collect();
    let mut out: VecDeque<String> = surplus.zip(deficit).map(|(from, to)| format!("{}|{}", RESTACK_POOLS[from], RESTACK_POOLS[to])).collect();
    let total: usize = now.iter().map(|n| usize::from(*n)).sum();
    if out.len() < total {
        out.push_back("done".to_owned());
    }
    out
}

/// The string's two Politics answers (`top` / `bottom`, the first agenda first) that give the core's order `chosen` (`{top}/{bottom}`):
/// `None` for the two orders the string cannot reach (the first agenda above the second on top, or below it at the bottom: the string
/// places one agenda at a time in the order they were drawn).
#[must_use]
pub fn politics_answers(first: &str, second: &str, chosen: &str) -> Option<VecDeque<String>> {
    let answers = if chosen == format!("{first}/{second}") {
        ["top", "bottom"]
    } else if chosen == format!("{second}/{first}") {
        ["bottom", "top"]
    } else if chosen == format!("{second},{first}/") {
        ["top", "top"]
    } else if chosen == format!("/{first},{second}") {
        ["bottom", "bottom"]
    } else {
        return None;
    };
    Some(answers.into_iter().map(str::to_owned).collect())
}

/// For a Politics answer the string cannot reach ([`politics_answers`] `None`): the order of the same piles it can (both on top or both on
/// the bottom, the other way round), when the question offers it.
#[must_use]
pub fn reachable_politics_order(rendered: &ti4_engine::choice::Choice, chosen: &str) -> Option<String> {
    let agendas = rendered.details.get("agendas")?.as_array()?;
    let id = |i: usize| agendas.get(i)?.get("id")?.as_str();
    let (first, second) = (id(0)?, id(1)?);
    if politics_answers(first, second, chosen).is_some() {
        return None;
    }
    let other = if chosen == format!("{first},{second}/") { format!("{second},{first}/") } else { format!("/{first},{second}") };
    rendered.options.iter().any(|o| o.id == other).then_some(other)
}

/// The pools of a token question's display facts (`details.pools`).
fn pools_of(details: &serde_json::Map<String, serde_json::Value>) -> Option<[u8; 3]> {
    let pools = details.get("pools")?;
    let n = |k: &str| pools.get(k)?.as_u64().and_then(|v| u8::try_from(v).ok());
    Some([n("tactic")?, n("fleet")?, n("strategic")?])
}

/// How the string engine asks what the core asked (`rendered`) and answered with `chosen`: `None` when the string asks the same single
/// question (the answer is `chosen` itself).
#[must_use]
pub fn legacy_answers(rendered: &ti4_engine::choice::Choice, chosen: &str) -> Option<Scripted> {
    // A removal answered with one option: the core removes it first and fills up with the first options; the string asks per unit.
    if rendered.prompt.starts_with("remove a unit: over") && !chosen.contains('+') {
        let lead = rendered.options.iter().find(|o| o.id == chosen).map(|o| o.label.clone());
        return Some(Scripted::Removal { prompt: rendered.prompt.clone(), lead });
    }
    if let Some(multi) = &rendered.multi_pick {
        return Some(multi_answers(rendered, &multi.kind, multi.buy.is_some_and(|[min, _]| min > 0), chosen));
    }
    expand(rendered.context.as_ref().map(|c| c.subtype.as_str()), &rendered.details, chosen)
}

/// The string's answers to a multi-pick: a payment's atoms as a [`Scripted::Set`] (smallest worth first, so the largest is taken last, as
/// the closed form needs); a purchase as a [`Scripted::Leadership`] (`secondary`: the follower's yes was the first purchase).
#[must_use]
pub fn multi_answers(rendered: &ti4_engine::choice::Choice, kind: &str, secondary: bool, chosen: &str) -> Scripted {
    let worth = |id: &str| rendered.options.iter().find(|o| o.id == id).and_then(|o| o.payload.get("worth")).and_then(serde_json::Value::as_i64).unwrap_or(0);
    let mut buys = 0u8;
    let mut atoms: Vec<String> = Vec::new();
    for part in chosen.split(ti4_engine::wire::choice::MULTI_PICK_SEPARATOR) {
        match part.strip_prefix(ti4_engine::wire::choice::MULTI_PICK_BUY) {
            Some(n) => buys = n.parse().unwrap_or(0),
            // The primary purchase's decline: nothing bought (the string's "no").
            None if part == ti4_engine::wire::choice::DECLINE_ID => {}
            None => atoms.push(part.to_owned()),
        }
    }
    if kind == "discard" {
        let label = |id: &str| rendered.options.iter().find(|o| o.id == id).map_or_else(|| id.to_owned(), |o| o.label.clone());
        return Scripted::Labels(atoms.iter().map(|id| label(id)).collect());
    }
    atoms.sort_by_key(|id| worth(id));
    if kind == "purchase" { Scripted::Leadership { buys, bought: u8::from(secondary), atoms } } else { Scripted::Set(atoms) }
}

/// [`legacy_answers`] of a question of `subtype` with the display facts `details`.
#[must_use]
pub fn expand(subtype: Option<&str>, details: &serde_json::Map<String, serde_json::Value>, chosen: &str) -> Option<Scripted> {
    match subtype? {
        "gain_command_token" => split_counts(chosen).map(|split| Scripted::Seq(gain_answers(split))),
        "predictive_intelligence_redistribute" => Some(Scripted::Seq(restack_answers(pools_of(details)?, split_counts(chosen)?))),
        "politics_place_agenda" => {
            let agendas = details.get("agendas")?.as_array()?;
            let id = |i: usize| agendas.get(i)?.get("id")?.as_str();
            politics_answers(id(0)?, id(1)?, chosen).map(Scripted::Seq)
        }
        _ => None,
    }
}

/// The string pool question the status phase's token gain still asks (the core asks none: the token goes to the canonical pool and the
/// redistribution follows), as `(prompt, options, answer)`.
#[must_use]
pub fn status_gain_question() -> (&'static str, Vec<String>, String) {
    (GAIN_PROMPT, POOL_IDS.iter().map(|s| (*s).to_owned()).collect(), POOL_IDS[0].to_owned())
}

/// Whether the string's questions `records` (player, chosen, offered) answer one multi-ask of `player` the way its expansion `entry` says:
/// the same seat asked each, and each answer was offered and is one of the expansion.
#[must_use]
pub fn loose_match(player: &str, entry: &Scripted, records: &[(&str, &str, &[String])]) -> bool {
    let allowed: Vec<&str> = match entry {
        Scripted::One(id) => vec![id.as_str()],
        Scripted::Seq(ids) => ids.iter().map(String::as_str).collect(),
        Scripted::Set(ids) => ids.iter().map(String::as_str).collect(),
        Scripted::Leadership { atoms, .. } => atoms.iter().map(String::as_str).chain(["yes", "no"]).collect(),
        // (The string's ids of a discard are places in the hand: only the seat and that the answer was offered are compared.)
        Scripted::Removal { .. } => return records.iter().all(|(p, chosen, offered)| *p == player && offered.iter().any(|o| o == chosen)),
        Scripted::Labels(_) => return records.iter().all(|(p, chosen, offered)| *p == player && offered.iter().any(|o| o == chosen)),
    };
    records.iter().all(|(p, chosen, offered)| *p == player && allowed.contains(chosen) && offered.iter().any(|o| o == chosen))
}

/// The string's questions of one step, as the core asks them (the reverse of [`legacy_answers`], for the `NativeSubject` oracle that lets the
/// string lead): the answers the core takes, and per core question the string's question as JSON (`player`, `prompt`, `options`), or for a
/// question the core asks as one multi-ask a loose stand-in `{player, answer}` ([`settle_loose`]).
#[derive(Debug, Default)]
pub struct Collapsed {
    pub answers: Vec<String>,
    pub questions: Vec<serde_json::Value>,
}

/// The prompt of the string's Predictive Intelligence question (one move per question).
pub const RESTACK_PROMPT: &str = "Predictive Intelligence: redistribute command tokens";

/// Collapse the string's questions `records` (answered with `answers`): a run of pool questions of one seat becomes ONE gain split; with
/// `drop_gain` (the status phase's token arm: the core asks no gain there) they are dropped. A run of Predictive Intelligence moves becomes the
/// final split, from the seat's pools before the step (`pools`).
#[must_use]
pub fn collapse(records: &[ti4_engine::choice::DecisionRecord], answers: Vec<String>, drop_gain: bool, pools: &dyn Fn(&str) -> Option<[u8; 3]>) -> Collapsed {
    let is_gain = |r: &ti4_engine::choice::DecisionRecord| r.prompt == GAIN_PROMPT && r.offered.iter().map(String::as_str).eq(POOL_IDS);
    let mut out = Collapsed::default();
    let mut i = 0;
    while i < records.len() {
        let r = &records[i];
        if r.prompt == RESTACK_PROMPT
            && let Some(mut at) = pools(r.player.as_str())
        {
            let mut j = i;
            while j < records.len() && records[j].prompt == RESTACK_PROMPT && records[j].player == r.player {
                let chosen = records[j].chosen.as_str();
                j += 1;
                let Some((from, to)) = chosen.split_once('|') else { break };
                if let (Some(f), Some(t)) = (RESTACK_POOLS.iter().position(|p| *p == from), RESTACK_POOLS.iter().position(|p| *p == to)) {
                    at[f] = at[f].saturating_sub(1);
                    at[t] += 1;
                }
            }
            let answer = format!("{}|{}|{}", at[0], at[1], at[2]);
            out.questions.push(serde_json::json!({ "player": r.player.as_str(), "answer": answer }));
            out.answers.push(answer);
            i = j;
            continue;
        }
        // Politics: the string's two "place {agenda} where" questions of one seat become the core's one order.
        if let (Some(first), Some(next)) = (politics_agenda(&r.prompt), records.get(i + 1))
            && let Some(second) = politics_agenda(&next.prompt)
            && next.player == r.player
        {
            let answer = match (r.chosen.as_str(), next.chosen.as_str()) {
                ("top", "bottom") => format!("{first}/{second}"),
                ("bottom", "top") => format!("{second}/{first}"),
                ("top", "top") => format!("{second},{first}/"),
                _ => format!("/{first},{second}"),
            };
            out.questions.push(serde_json::json!({ "player": r.player.as_str(), "answer": answer }));
            out.answers.push(answer);
            i += 2;
            continue;
        }
        if is_gain(r) {
            let mut split = [0u8; 3];
            let mut j = i;
            while j < records.len() && is_gain(&records[j]) && records[j].player == r.player {
                if let Some(at) = POOL_IDS.iter().position(|p| *p == records[j].chosen) {
                    split[at] += 1;
                }
                j += 1;
            }
            if !drop_gain {
                let answer = format!("{}|{}|{}", split[0], split[1], split[2]);
                out.questions.push(serde_json::json!({ "player": r.player.as_str(), "answer": answer }));
                out.answers.push(answer);
            }
            i = j;
            continue;
        }
        out.questions.push(serde_json::json!({ "player": r.player.as_str(), "prompt": r.prompt, "options": r.offered }));
        out.answers.push(answers.get(i).cloned().unwrap_or_else(|| r.chosen.clone()));
        i += 1;
    }
    out.answers.extend(answers.into_iter().skip(records.len()));
    out
}

/// The agenda of the string's Politics question `place {agenda} where`.
fn politics_agenda(prompt: &str) -> Option<&str> {
    prompt.strip_prefix("place ")?.strip_suffix(" where")
}

/// Replace each loose stand-in of `legacy` ([`collapse`]) by the core's question at its place when that question is the same seat's and offers
/// the collapsed answer.
pub fn settle_loose(legacy: &mut [serde_json::Value], native: &[serde_json::Value]) {
    for (l, n) in legacy.iter_mut().zip(native) {
        let Some(answer) = l.get("answer").and_then(serde_json::Value::as_str) else { continue };
        let offered = n.get("options").and_then(serde_json::Value::as_array).is_some_and(|o| o.iter().any(|x| x.as_str() == Some(answer)));
        if offered && n.get("player") == l.get("player") {
            l.clone_from(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gain_split_expands_to_one_pool_answer_per_token_in_the_canonical_order() {
        let want: VecDeque<String> = ["tactic_tokens", "strategic_tokens", "strategic_tokens"].into_iter().map(str::to_owned).collect();
        assert_eq!(gain_answers([1, 0, 2]), want);
        let none = serde_json::Map::new();
        assert_eq!(expand(Some("gain_command_token"), &none, "1|0|2"), Some(Scripted::Seq(want)));
        assert_eq!(expand(Some("gain_command_token"), &none, "tactic_tokens"), None, "a string pool id is no split");
        assert_eq!(expand(Some("status_redistribute_tokens"), &none, "1|0|2"), None, "a redistribution is one question in both engines");
        let pools = serde_json::json!({ "pools": { "tactic": 2, "fleet": 1, "strategic": 0 } });
        assert_eq!(expand(Some("predictive_intelligence_redistribute"), pools.as_object().unwrap(), "2|1|0"), Some(Scripted::Seq(["done".to_owned()].into())));
    }

    #[test]
    fn a_restack_split_expands_to_the_fewest_moves_and_done_unless_every_token_moved() {
        let ids = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<VecDeque<String>>();
        assert_eq!(restack_answers([2, 1, 0], [0, 1, 2]), ids(&["tactic|strategy", "tactic|strategy", "done"]));
        assert_eq!(restack_answers([2, 1, 0], [2, 1, 0]), ids(&["done"]), "kept");
        assert_eq!(restack_answers([1, 0, 0], [0, 1, 0]), ids(&["tactic|fleet"]), "one token, one move: the string stops by itself");
        assert_eq!(restack_answers([0, 2, 1], [1, 0, 2]), ids(&["fleet|tactic", "fleet|strategy", "done"]), "two moves of three tokens");
        assert_eq!(restack_answers([1, 1, 0], [0, 0, 2]), ids(&["tactic|strategy", "fleet|strategy"]), "every token moved");
    }

    #[test]
    fn a_politics_order_expands_to_the_two_answers_the_string_reaches_it_with() {
        let ids = |v: [&str; 2]| v.iter().map(|s| (*s).to_owned()).collect::<VecDeque<String>>();
        assert_eq!(politics_answers("a", "b", "a/b"), Some(ids(["top", "bottom"])));
        assert_eq!(politics_answers("a", "b", "b/a"), Some(ids(["bottom", "top"])));
        assert_eq!(politics_answers("a", "b", "b,a/"), Some(ids(["top", "top"])), "both on top: the second answer lands above the first");
        assert_eq!(politics_answers("a", "b", "/a,b"), Some(ids(["bottom", "bottom"])));
        assert_eq!(politics_answers("a", "b", "a,b/"), None, "the string cannot put the first agenda above the second");
        assert_eq!(politics_answers("a", "b", "/b,a"), None, "nor below it at the bottom");
    }

    #[test]
    fn a_loose_match_wants_the_same_seat_and_answers_of_the_expansion() {
        let entry = Scripted::Seq(gain_answers([1, 1, 0]));
        let offered: Vec<String> = POOL_IDS.iter().map(|s| (*s).to_owned()).collect();
        assert!(loose_match("a", &entry, &[("a", "tactic_tokens", &offered), ("a", "fleet_tokens", &offered)]));
        assert!(!loose_match("a", &entry, &[("b", "tactic_tokens", &offered)]), "another seat");
        assert!(!loose_match("a", &entry, &[("a", "strategic_tokens", &offered)]), "not in the expansion");
    }

    /// Leadership's PRIMARY purchase answered with its decline (or `buy:0`): nothing bought, no atom, so the string hears its "no".
    #[test]
    fn the_primary_purchase_declined_expands_to_the_strings_no() {
        let choice = ti4_engine::choice::Choice::new(
            ti4_model::id::PlayerId::new("a"),
            "buy command tokens for 3 influence each (up to 1)",
            vec![ti4_engine::choice::ChoiceOption::decline(), ti4_engine::choice::ChoiceOption::labelled("trade_good", "pay", "spend a trade good")],
        );
        for chosen in [ti4_engine::wire::choice::DECLINE_ID, "buy:0"] {
            let Scripted::Leadership { buys, bought, atoms } = multi_answers(&choice, "purchase", false, chosen) else { panic!("a purchase") };
            assert_eq!((buys, bought, atoms.len()), (0, 0, 0), "{chosen}");
        }
    }
}
