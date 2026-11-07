//! The lockstep duals over the core's multi-asks (`docs/MULTIASK1_RESUME.md`, section 8): where the core asks ONE question for what the
//! string asks as a sequence (a token gain of several tokens, a restack, Politics' two agendas, a payment, Leadership's purchase loop, the
//! hand-limit discard, a fleet removal), the string's recorded questions are collapsed into the core's one answer ([`collapse`]) and the
//! core reads that answer back ([`pick`]). The collapsed question is compared loosely: the same seat, and a core question of the same kind
//! ([`Kind::of_rendered`]); the position, the events and the dice are compared as before, so a set the core takes differently from the
//! string's sequence still shows.
//!
//! The answers it builds (never ids a client sends; read only by [`pick`]):
//! * a gain: the core's own option id `{tactic}|{fleet}|{strategic}`;
//! * a restack: the final split (the core's option id), from the pools of the string's first move question;
//! * Politics: the core's own option id `{top}/{bottom}`;
//! * a payment: `pay:` + the atom ids joined by `+`; a purchase: `purchase:{n}:` + the atom ids;
//! * a discard: `discard:` + the printed names joined by `+`; a removal: `remove:` + the option labels joined by `+`.
//!
//! The string pays a lone remaining atom without asking (a trade good when nothing else is left), so a payment's set is the SMALLEST legal
//! set holding every atom the string was asked for ([`complete_set`]).

use ti4_engine_core::choice::production::{self, Opt as POpt, PayAsk, PaySet};
use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Opt, Pick};

use crate::choice::Choice;
use crate::render::{self, Render};

/// The string pool ids of a token gain, in the canonical order of the core (tactic, fleet, strategic).
pub const POOL_IDS: [&str; 3] = ["tactic_tokens", "fleet_tokens", "strategic_tokens"];
/// The prompt of the string's pool question (one token).
pub const GAIN_PROMPT: &str = "gain a command token into which pool";
/// The prompt of the string's Predictive Intelligence question (one move per question).
pub const RESTACK_PROMPT: &str = "Predictive Intelligence: redistribute command tokens";
const RESTACK_POOLS: [&str; 3] = ["tactic", "fleet", "strategy"];

/// What one core question of a multi-ask is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gain,
    Restack,
    Politics,
    Pay,
    Purchase,
    Discard,
    Removal,
}

impl Kind {
    /// The kind of a core question (the typed ask).
    #[must_use]
    pub fn of_core(choice: &CoreChoice) -> Option<Self> {
        use ti4_engine_core::choice::{cards, strategy, tactical, technology};
        Some(match choice.ask {
            Ask::Strategy(strategy::Ask::GainPool { .. }) => Self::Gain,
            Ask::Strategy(strategy::Ask::PoliticsAgendas { .. }) => Self::Politics,
            Ask::Technology(technology::Ask::Restack) => Self::Restack,
            Ask::Production(production::Ask::Pay(_)) => Self::Pay,
            Ask::Production(production::Ask::Purchase(_)) => Self::Purchase,
            Ask::Cards(cards::Ask::HandLimit { .. }) => Self::Discard,
            Ask::Tactical(tactical::Ask::Remove { .. }) => Self::Removal,
            _ => return None,
        })
    }

    /// The kind of a core question as the string engine renders it (what a window dual compares).
    #[must_use]
    pub fn of_rendered(c: &Choice) -> Option<Self> {
        let p = c.prompt.as_str();
        let gain = p == GAIN_PROMPT || (p.starts_with("gain ") && p.ends_with(" command tokens into which pools"));
        Some(if gain && c.options.iter().all(|o| split_counts(&o.id).is_some()) {
            Self::Gain
        } else if p == RESTACK_PROMPT {
            Self::Restack
        } else if p.starts_with("place ") && p.ends_with(" on the top or the bottom of the agenda deck") {
            Self::Politics
        } else if p.starts_with("pay ") {
            Self::Pay
        } else if p.starts_with("buy command tokens for ") {
            Self::Purchase
        } else if p.starts_with("over the hand limit") {
            Self::Discard
        } else if p.starts_with("remove a unit: over") {
            Self::Removal
        } else {
            return None;
        })
    }

    /// Whether a core question of kind `core` answers a group of this kind: the same kind, or a payment and Leadership's purchase (the string's
    /// follower pays its first token without a purchase question, so a lone token's payment looks like a plain one).
    #[must_use]
    pub fn matches(self, core: Option<Self>) -> bool {
        core == Some(self) || matches!((self, core), (Self::Pay, Some(Self::Purchase)) | (Self::Purchase, Some(Self::Pay)))
    }

    fn name(self) -> &'static str {
        match self {
            Self::Gain => "gain",
            Self::Restack => "restack",
            Self::Politics => "politics",
            Self::Pay => "payment",
            Self::Purchase => "purchase",
            Self::Discard => "discard",
            Self::Removal => "removal",
        }
    }
}

/// The `{a}|{b}|{c}` counts of a split id.
#[must_use]
pub fn split_counts(id: &str) -> Option<[u8; 3]> {
    let mut parts = id.split('|');
    let mut next = || parts.next()?.parse::<u8>().ok();
    let counts = [next()?, next()?, next()?];
    parts.next().is_none().then_some(counts)
}

/// The string questions (in recording order) that make one core question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// `None`: one string question the core asks alike (compared as usual).
    pub kind: Option<Kind>,
    /// The places of the string questions in the recording.
    pub at: Vec<usize>,
    /// The answer the core gets (`None`: the string's question was not answered).
    pub answer: Option<String>,
}

impl Group {
    /// A stand-in for the string side of a collapsed question (a window dual compares it with [`loose_of`] of the core's).
    #[must_use]
    pub fn stand_in(&self, first: &Choice) -> Choice {
        let kind = self.kind.map_or("", Kind::name);
        Choice::new(first.player.clone(), format!("multi-ask {kind}"), Vec::new())
    }
}

/// The stand-in of a core question that answers a collapsed group of `kind` (equal to [`Group::stand_in`] when the seat and the kind agree);
/// the rendered question itself when it is no question of that kind.
#[must_use]
pub fn loose_of(kind: Kind, rendered: &Choice) -> Choice {
    if kind.matches(Kind::of_rendered(rendered)) {
        Choice::new(rendered.player.clone(), format!("multi-ask {}", kind.name()), Vec::new())
    } else {
        rendered.clone()
    }
}

fn is_gain(c: &Choice) -> bool {
    c.prompt == GAIN_PROMPT && !c.options.is_empty() && c.options.iter().all(|o| POOL_IDS.contains(&o.id.as_str()))
}

/// Whether the payment question `next` asks the rest of the bill the payment `prev` (answered) paid an atom of: what it owes is what `prev`
/// owed less the atom's worth (without the worth, less at all).
fn next_atom(prev: (&Choice, Option<&str>, bool), next: &Choice) -> bool {
    let (Some(before), Some(now)) = (owed_of(prev.0), owed_of(next)) else { return false };
    let worth = prev.1.and_then(|a| prev.0.options.iter().find(|o| o.id == a)).and_then(|o| o.payload.get("worth")).and_then(serde_json::Value::as_i64);
    match worth {
        Some(w) => now == before - w,
        None => now < before,
    }
}

/// Whether the pool question `next` places the next token of the gain `prev` placed one of (`details.tokens_to_place` one lower; without the
/// count, any pool question of the seat).
fn next_token(prev: &Choice, next: &Choice) -> bool {
    let left = |c: &Choice| c.details.get("tokens_to_place").and_then(serde_json::Value::as_u64);
    match (left(prev), left(next)) {
        (Some(p), Some(n)) => n + 1 == p,
        _ => true,
    }
}

fn is_pay(c: &Choice) -> bool {
    c.prompt.starts_with("pay ") && c.prompt.contains(" more ") && c.options.iter().any(|o| o.kind == crate::production::PAY_KIND)
}

/// `pay N more X`: N.
fn owed_of(c: &Choice) -> Option<i64> {
    c.prompt.strip_prefix("pay ")?.split(' ').next()?.parse().ok()
}

fn currency_of(c: &Choice) -> &str {
    c.prompt.rsplit(' ').next().unwrap_or("")
}

/// Leadership's purchase question of the string ("spend 3 influence for a command token"), the follower's question (`details.kind`
/// `strategy_secondary`) excepted: that one is the core's follow question.
fn is_purchase(c: &Choice) -> bool {
    c.prompt.starts_with("spend ") && c.prompt.ends_with(" influence for a command token") && c.details.get("kind").and_then(|k| k.as_str()) != Some("strategy_secondary")
}

fn politics_agenda(prompt: &str) -> Option<&str> {
    prompt.strip_prefix("place ")?.strip_suffix(" where")
}

fn pools_of(c: &Choice) -> Option<[u8; 3]> {
    let pools = c.details.get("pools")?;
    let n = |k: &str| pools.get(k)?.as_u64().and_then(|v| u8::try_from(v).ok());
    Some([n("tactic")?, n("fleet")?, n("strategic")?])
}

/// Collapse the string's questions `qs` (each with its answer and whether it was asked inside an emission) into the core's: a run of one
/// seat's pool questions is one gain; Predictive Intelligence's moves one split; Politics' two placements one order; one bill's payment
/// questions one set; Leadership's purchase loop one purchase (the pools of the bought tokens one gain after it); the hand limit's
/// discards one set; one stage's removals one set.
#[must_use]
pub fn collapse(qs: &[(&Choice, Option<&str>, bool)]) -> Vec<Group> {
    let mut out = Vec::new();
    let mut i = 0;
    let same = |a: usize, b: usize| qs[a].0.player == qs[b].0.player && qs[a].2 == qs[b].2;
    let answered = |k: usize| qs[k].1;
    while i < qs.len() {
        let (c, answer, _) = qs[i];
        // A purchase loop: from its question, or from a payment that a purchase question of the seat follows (the follower's first token,
        // or a primary's purchase whose question came before).
        if is_purchase(c) || (is_pay(c) && purchase_follows(qs, i)) {
            let implied = u8::from(!is_purchase(c));
            let (mut buys, mut atoms, mut pools, mut at, mut gain_at) = (implied, Vec::new(), Vec::new(), Vec::new(), Vec::new());
            let mut j = i;
            while j < qs.len() && same(i, j) {
                let (q, a, _) = qs[j];
                if is_purchase(q) {
                    at.push(j);
                    j += 1;
                    match a {
                        Some("yes") => buys += 1,
                        _ => break,
                    }
                } else if is_pay(q) {
                    at.push(j);
                    atoms.extend(a.map(str::to_owned));
                    j += 1;
                } else if is_gain(q) {
                    gain_at.push(j);
                    pools.extend(a.map(str::to_owned));
                    j += 1;
                } else {
                    break;
                }
            }
            let answer = format!("purchase:{buys}:{}", atoms.join("+"));
            out.push(Group { kind: Some(Kind::Purchase), at, answer: Some(answer) });
            if !gain_at.is_empty() {
                out.push(Group { kind: Some(Kind::Gain), at: gain_at, answer: Some(gain_split(pools.iter().map(String::as_str))) });
            }
            i = j;
            continue;
        }
        if is_gain(c) {
            let mut j = i;
            // (One gain counts its tokens down, `tokens_to_place`; a count that does not go down by one is the next gain.)
            while j < qs.len() && is_gain(qs[j].0) && same(i, j) && (j == i || next_token(qs[j - 1].0, qs[j].0)) {
                j += 1;
            }
            let answer = (i..j).all(|k| answered(k).is_some()).then(|| gain_split((i..j).filter_map(answered)));
            out.push(Group { kind: Some(Kind::Gain), at: (i..j).collect(), answer });
            i = j;
            continue;
        }
        if c.prompt == RESTACK_PROMPT {
            let mut j = i;
            let mut at = pools_of(c);
            while j < qs.len() && qs[j].0.prompt == RESTACK_PROMPT && same(i, j) {
                let chosen = answered(j);
                j += 1;
                match chosen.and_then(|m| m.split_once('|')) {
                    Some((from, to)) => {
                        if let (Some(pools), Some(f), Some(t)) = (at.as_mut(), RESTACK_POOLS.iter().position(|p| *p == from), RESTACK_POOLS.iter().position(|p| *p == to)) {
                            pools[f] = pools[f].saturating_sub(1);
                            pools[t] += 1;
                        }
                    }
                    None => break,
                }
            }
            let answer = at.map(|p| format!("{}|{}|{}", p[0], p[1], p[2]));
            out.push(Group { kind: Some(Kind::Restack), at: (i..j).collect(), answer });
            i = j;
            continue;
        }
        if let (Some(first), Some(next)) = (politics_agenda(&c.prompt), qs.get(i + 1))
            && let Some(second) = politics_agenda(&next.0.prompt)
            && same(i, i + 1)
        {
            let answer = match (answer, next.1) {
                (Some("top"), Some("bottom")) => Some(format!("{first}/{second}")),
                (Some("bottom"), Some("top")) => Some(format!("{second}/{first}")),
                (Some("top"), Some("top")) => Some(format!("{second},{first}/")),
                (Some("bottom"), Some("bottom")) => Some(format!("/{first},{second}")),
                _ => None,
            };
            out.push(Group { kind: Some(Kind::Politics), at: vec![i, i + 1], answer });
            i += 2;
            continue;
        }
        if is_pay(c) {
            let mut j = i + 1;
            while j < qs.len() && is_pay(qs[j].0) && same(i, j) && currency_of(qs[j].0) == currency_of(c) && next_atom(qs[j - 1], qs[j].0) {
                j += 1;
            }
            let atoms: Option<Vec<&str>> = (i..j).map(answered).collect();
            out.push(Group { kind: Some(Kind::Pay), at: (i..j).collect(), answer: atoms.map(|a| format!("pay:{}", a.join("+"))) });
            i = j;
            continue;
        }
        if c.prompt.starts_with("over the hand limit") {
            let mut j = i;
            let mut names = Vec::new();
            while j < qs.len() && qs[j].0.prompt.starts_with("over the hand limit") && same(i, j) {
                let (q, a, _) = qs[j];
                names.extend(a.and_then(|a| q.options.iter().find(|o| o.id == a)).map(|o| o.label.clone()));
                j += 1;
            }
            out.push(Group { kind: Some(Kind::Discard), at: (i..j).collect(), answer: Some(format!("discard:{}", names.join("+"))) });
            i = j;
            continue;
        }
        if c.prompt.starts_with("remove a unit: over") {
            let mut j = i;
            let mut labels = Vec::new();
            while j < qs.len() && qs[j].0.prompt == c.prompt && same(i, j) {
                let (q, a, _) = qs[j];
                labels.extend(a.and_then(|a| q.options.iter().find(|o| o.id == a)).map(|o| o.label.clone()));
                j += 1;
            }
            out.push(Group { kind: Some(Kind::Removal), at: (i..j).collect(), answer: Some(format!("remove:{}", labels.join("+"))) });
            i = j;
            continue;
        }
        out.push(Group { kind: None, at: vec![i], answer: answer.map(str::to_owned) });
        i += 1;
    }
    out
}

/// Whether the payment at `i` belongs to a purchase loop: the seat's payments and pools after it lead to its purchase question.
fn purchase_follows(qs: &[(&Choice, Option<&str>, bool)], i: usize) -> bool {
    let c = qs[i].0;
    if currency_of(c) != "influence" {
        return false;
    }
    qs[i..].iter().take_while(|(q, _, n)| q.player == c.player && *n == qs[i].2 && (is_pay(q) || is_gain(q) || is_purchase(q))).any(|(q, _, _)| is_purchase(q))
}

fn gain_split<'a>(pools: impl Iterator<Item = &'a str>) -> String {
    let mut split = [0u8; 3];
    for p in pools {
        if let Some(at) = POOL_IDS.iter().position(|id| *id == p) {
            split[at] += 1;
        }
    }
    format!("{}|{}|{}", split[0], split[1], split[2])
}

/// The core's answer an id of [`collapse`] gives `choice` (`None`: no such id, the caller reads it as usual).
#[must_use]
pub fn pick(r: &Render, choice: &CoreChoice, id: &str) -> Option<Pick> {
    // (Rendered only for the questions of an id of `collapse`: an option of another family may need the position.)
    let rendered = || -> Vec<String> { choice.opts.iter().map(|o| render::option(r, &choice.ask, o).id).collect() };
    if id == LONE {
        return lone_answer(choice, None);
    }
    if let Some(atoms) = id.strip_prefix("pay:") {
        // (The follower's one token paid without a purchase question: one token.)
        if let Ask::Production(production::Ask::Purchase(_)) = choice.ask {
            return pick(r, choice, &format!("purchase:1:{atoms}"));
        }
        let Ask::Production(production::Ask::Pay(ask)) = choice.ask else { return None };
        let set = complete_set(&ask, choice, &rendered(), &split_atoms(atoms), 0)?;
        return Some(Pick::Opt(Opt::Production(POpt::Set(set))));
    }
    if let Some(rest) = id.strip_prefix("purchase:") {
        let Ask::Production(production::Ask::Purchase(ask)) = choice.ask else { return None };
        let (n, atoms) = rest.split_once(':')?;
        let buy: u8 = n.parse().ok()?;
        let buy = buy.clamp(ask.min, ask.max);
        if buy == 0 {
            if choice.opts.iter().any(|o| matches!(o, Opt::Decline)) {
                return Some(Pick::Opt(Opt::Decline));
            }
            return Some(Pick::Opt(Opt::Production(POpt::Set(PaySet::default()))));
        }
        let bill = PayAsk { owed: i64::from(ask.each) * i64::from(buy), ..ask.pay };
        let set = complete_set(&bill, choice, &rendered(), &split_atoms(atoms), buy)?;
        return Some(Pick::Opt(Opt::Production(POpt::Set(set))));
    }
    if let Some(names) = id.strip_prefix("discard:") {
        use ti4_engine_core::choice::cards;
        let mut counts = 0u64;
        for name in names.split('+').filter(|n| !n.is_empty()) {
            let at = choice.opts.iter().enumerate().position(|(i, o)| {
                matches!(o, Opt::Cards(cards::Opt::Discard { .. })) && render::option(r, &choice.ask, o).label == name && {
                    let Opt::Cards(cards::Opt::Discard { copies, .. }) = o else { return false };
                    cards::discard_count(counts, i) < *copies
                }
            })?;
            counts = cards::with_discard_count(counts, at, cards::discard_count(counts, at) + 1);
        }
        return Some(Pick::Opt(Opt::Cards(cards::Opt::DiscardSet { counts: cards::Counts::of(counts) })));
    }
    if let Some(labels) = id.strip_prefix("remove:") {
        use ti4_engine_core::choice::{cards, tactical};
        let mut counts = 0u64;
        for label in labels.split('+').filter(|n| !n.is_empty()) {
            // (Each unit is an option of its own: the first option with the label that is not taken yet, else the first with the label.)
            let labelled: Vec<usize> = choice.opts.iter().enumerate().filter(|(_, o)| render::option(r, &choice.ask, o).label == label).map(|(i, _)| i).collect();
            let at = labelled.iter().copied().find(|i| cards::discard_count(counts, *i) == 0).or_else(|| labelled.first().copied())?;
            counts = cards::with_discard_count(counts, at, cards::discard_count(counts, at) + 1);
        }
        return Some(Pick::Opt(Opt::Tactical(tactical::Opt::RemoveSet { counts: cards::Counts::of(counts) })));
    }
    None
}

/// The answer [`Follow`] gives a payment with one legal answer ([`lone_answer`], read back by [`pick`]).
pub const LONE: &str = "multiask:lone";

/// The one legal answer of a core payment or purchase question that has only one (Leadership's follower buying its one token with the trade
/// goods it holds): the string pays such a bill without asking, so a lockstep whose string side asked nothing there answers it silently.
/// `recorded`: the kind of the string question at this place, if any (a question of the same kind is answered from the recording).
#[must_use]
pub fn lone_answer(choice: &CoreChoice, recorded: Option<Kind>) -> Option<Pick> {
    let kind = Kind::of_core(choice)?;
    if !matches!(kind, Kind::Pay | Kind::Purchase) || recorded.is_some_and(|k| k.matches(Some(kind))) {
        return None;
    }
    let answers = choice.answers(2);
    if answers.len() == 1 { answers.into_iter().next() } else { None }
}

/// [`pick`], else what the render layer reads back from `id`.
#[must_use]
pub fn pick_or_parse(r: &Render, choice: &CoreChoice, id: &str) -> Option<Pick> {
    pick(r, choice, id).or_else(|| render::parse_pick(r, &choice.ask, id))
}

/// The asker of a live lockstep (a native run that takes the answers the string was given, `live::Scripted` over the string's questions):
/// the string's questions collapsed ([`collapse`]), one group per core question. A multi-ask answered by a core question of the same kind
/// and seat is SEEN as the string's questions of the group (so the two lists compare equal there, and the position, events and dice are
/// compared as usual); a plain question is seen as the core rendered it, and so is a core question that is not the group's kind (the
/// lists then differ). The status phase's pool questions, which the core no longer asks (every owed token goes to the tactic pool, resume
/// 1'), are seen as they were and give no answer.
#[derive(Debug, Clone, Default)]
pub struct Follow {
    /// Each group with the string questions it is seen as.
    groups: std::collections::VecDeque<(Group, Vec<Choice>)>,
    /// The answers past the string's questions (a script longer than the run).
    rest: std::collections::VecDeque<String>,
}

impl Follow {
    /// `questions[i]` was answered with `answers[i]`; answers past the questions are given in order afterwards.
    #[must_use]
    pub fn new(questions: &[Choice], answers: &[String]) -> Self {
        let qs: Vec<(&Choice, Option<&str>, bool)> = questions.iter().enumerate().map(|(i, c)| (c, answers.get(i).map(String::as_str), false)).collect();
        let groups = collapse(&qs);
        let mut spans: Vec<Vec<usize>> = groups.iter().map(|g| g.at.clone()).collect();
        // A purchase and the gain of its tokens interleave in the string (a pool after each token): the purchase is seen as the whole stretch.
        for k in 0..groups.len().saturating_sub(1) {
            if groups[k].kind == Some(Kind::Purchase) && groups[k + 1].kind == Some(Kind::Gain) {
                let (lo, hi) = (spans[k][0], spans[k].iter().chain(&spans[k + 1]).copied().max().unwrap_or(0));
                if spans[k + 1].iter().all(|i| *i > lo && *i < hi) || spans[k + 1].iter().all(|i| *i > lo) {
                    spans[k] = (lo..=hi).collect();
                    spans[k + 1].clear();
                }
            }
        }
        let groups = groups.into_iter().zip(spans).map(|(g, span)| {
            let seen = span.iter().map(|i| questions[*i].clone()).collect();
            (g, seen)
        });
        Self { groups: groups.collect(), rest: answers.iter().skip(questions.len()).cloned().collect() }
    }

    /// Answers not given yet.
    #[must_use]
    pub fn left(&self) -> usize {
        // (A string run that stopped at a question it had no answer for recorded it: no answer to give.)
        self.groups.iter().filter(|(g, _)| g.answer.is_some()).count() + self.rest.len()
    }

    /// The answer to the core question `core` (rendered `rendered`) and what it is seen as.
    ///
    /// # Errors
    /// `Stop::Exhausted` when there is no answer left.
    pub fn answer(&mut self, core: &CoreChoice, rendered: &Choice, seen: &mut Vec<Choice>) -> Result<String, crate::live::Stop> {
        let kind = Kind::of_core(core);
        // A payment with one legal answer the string paid without asking: answered, not seen.
        if lone_answer(core, self.groups.front().and_then(|(g, _)| g.kind)).is_some() {
            return Ok(LONE.to_owned());
        }
        // The status phase's string gain: the core asks none.
        while let Some((g, _)) = self.groups.front()
            && g.kind == Some(Kind::Gain)
            && kind != Some(Kind::Gain)
            && self.groups.front().and_then(|(_, s)| s.first()).and_then(|c| c.context.as_ref()).is_some_and(|c| c.phase == ti4_model::state::Phase::Status)
        {
            let (_, strings) = self.groups.pop_front().unwrap_or_default();
            seen.extend(strings);
        }
        let Some((g, strings)) = self.groups.pop_front() else {
            seen.push(rendered.clone());
            return self.rest.pop_front().ok_or(crate::live::Stop::Exhausted);
        };
        let agrees = g.kind.is_some_and(|k| k.matches(kind)) && strings.first().is_none_or(|c| c.player == rendered.player);
        if agrees {
            seen.extend(strings);
        } else {
            seen.push(rendered.clone());
        }
        g.answer.ok_or(crate::live::Stop::Exhausted)
    }
}

/// The string's questions `string` (answered with `answers`) and the native run's `native` (rendered core questions) made comparable: a
/// string multi-ask group and the native question at its place, when that one is of the group's kind and seat, both become the group's
/// stand-in; the status phase's pool questions (the core asks none) are dropped from the string's list. Everything else is kept as is.
#[must_use]
pub fn align(string: &[Choice], answers: &[String], native: &[Choice]) -> (Vec<Choice>, Vec<Choice>) {
    let qs: Vec<(&Choice, Option<&str>, bool)> = string.iter().enumerate().map(|(i, c)| (c, answers.get(i).map(String::as_str), false)).collect();
    let (mut s_out, mut n_out) = (Vec::new(), Vec::new());
    let mut n = native.iter();
    for g in collapse(&qs) {
        let first = &string[g.at.first().copied().unwrap_or(0)];
        // A native payment or purchase where the string asked something else: one the string paid without asking (`lone_answer`).
        while let Some(c) = n.clone().next()
            && matches!(Kind::of_rendered(c), Some(Kind::Pay | Kind::Purchase))
            && !g.kind.is_some_and(|k| k.matches(Kind::of_rendered(c)))
        {
            n.next();
        }
        let Some(kind) = g.kind else {
            s_out.push(first.clone());
            n_out.extend(n.next().cloned());
            continue;
        };
        let status_gain = kind == Kind::Gain && first.context.as_ref().is_some_and(|c| c.phase == ti4_model::state::Phase::Status);
        let next = n.clone().next();
        if status_gain && next.is_none_or(|c| Kind::of_rendered(c) != Some(Kind::Gain)) {
            continue;
        }
        let stand_in = g.stand_in(first);
        s_out.push(stand_in.clone());
        match n.next() {
            Some(c) if kind.matches(Kind::of_rendered(c)) && c.player == first.player => n_out.push(stand_in),
            Some(c) => n_out.push(c.clone()),
            None => {}
        }
    }
    n_out.extend(n.cloned());
    (s_out, n_out)
}

/// The answers of the string's questions `string` (answered with `answers`, those past the questions kept at the end) as the core takes them:
/// one per [`collapse`] group; the status phase's pool questions give none (the core asks none, resume 1').
#[must_use]
pub fn core_script(string: &[Choice], answers: &[String]) -> Vec<String> {
    let qs: Vec<(&Choice, Option<&str>, bool)> = string.iter().enumerate().map(|(i, c)| (c, answers.get(i).map(String::as_str), false)).collect();
    let mut out: Vec<String> = collapse(&qs)
        .into_iter()
        .filter(|g| !(g.kind == Some(Kind::Gain) && string[g.at[0]].context.as_ref().is_some_and(|c| c.phase == ti4_model::state::Phase::Status)))
        .filter_map(|g| g.answer)
        .collect();
    out.extend(answers.iter().skip(string.len()).cloned());
    out
}

thread_local! {
    /// The questions of the string run a lockstep test made last ([`remember_string_questions`]).
    static STRING_RUN: std::cell::RefCell<Option<Vec<Choice>>> = const { std::cell::RefCell::new(None) };
}

/// A lockstep test's string run asked `questions` (in order): the next `live::Scripted` whose answers are theirs follows them ([`Follow`]).
pub fn remember_string_questions(questions: Vec<Choice>) {
    STRING_RUN.with(|s| *s.borrow_mut() = Some(questions));
}

/// The questions [`remember_string_questions`] kept, when `answers` answer them in order (every answer offered; at most the last question
/// unanswered); taken either way.
#[must_use]
pub fn take_string_questions(answers: &[String]) -> Option<Vec<Choice>> {
    let qs = STRING_RUN.with(|s| s.borrow_mut().take())?;
    // (The native side may start after the string's first questions: a menu the native job does not ask.)
    let fits = |qs: &[Choice]| qs.len() <= answers.len() + 1 && qs.iter().zip(answers).all(|(q, a)| q.options.iter().any(|o| o.id == *a));
    let skip = (0..qs.len().min(3)).find(|k| fits(&qs[*k..]))?;
    Some(qs[skip..].to_vec())
}

impl Default for Group {
    fn default() -> Self {
        Self { kind: None, at: Vec::new(), answer: None }
    }
}

fn split_atoms(atoms: &str) -> Vec<&str> {
    atoms.split('+').filter(|a| !a.is_empty()).collect()
}

/// The smallest set the question accepts that spends every atom of `atoms` (ids of `rendered`, a trade good or commodity once per one):
/// the string's asked atoms, plus what it paid without asking.
fn complete_set(bill: &PayAsk, choice: &CoreChoice, rendered: &[String], atoms: &[&str], buy: u8) -> Option<PaySet> {
    let atom_opts: Vec<POpt> = choice.opts.iter().filter_map(|o| if let Opt::Production(p) = o { Some(*p) } else { None }).collect();
    let face_ids: Vec<usize> = choice.opts.iter().enumerate().filter(|(_, o)| matches!(o, Opt::Production(POpt::Exhaust { .. }))).map(|(i, _)| i).collect();
    let mut base = PaySet { buy, ..PaySet::default() };
    for a in atoms {
        let at = rendered.iter().position(|id| id == a)?;
        match choice.opts.as_slice()[at] {
            Opt::Production(POpt::TradeGood { .. }) => base.goods += 1,
            Opt::Production(POpt::Commodity { .. }) => base.commodities += 1,
            Opt::Production(POpt::Exhaust { .. }) => base.faces |= 1 << face_ids.iter().position(|f| *f == at)?,
            _ => return None,
        }
    }
    let ok = |set: PaySet| {
        if buy > 0 {
            let Ask::Production(production::Ask::Purchase(ask)) = choice.ask else { return false };
            production::purchase_worth(&ask, &atom_opts, set).is_some()
        } else {
            production::set_worth(bill, &atom_opts, set).is_some()
        }
    };
    let faces = face_ids.len().min(16);
    let goods = u8::try_from(bill.goods.clamp(0, 15)).unwrap_or(0);
    let comms = u8::try_from(bill.commodities.clamp(0, 15)).unwrap_or(0);
    let mut best: Option<(u32, PaySet)> = None;
    for extra in 0u32..(1 << faces) {
        if extra & base.faces != 0 {
            continue;
        }
        for g in base.goods..=goods.max(base.goods) {
            for c in base.commodities..=comms.max(base.commodities) {
                let set = PaySet { faces: base.faces | extra, goods: g, commodities: c, buy };
                let size = extra.count_ones() + u32::from(g - base.goods) + u32::from(c - base.commodities);
                if best.is_some_and(|(s, _)| s <= size) || !ok(set) {
                    continue;
                }
                best = Some((size, set));
            }
        }
    }
    best.map(|(_, set)| set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::choice::ChoiceOption;
    use ti4_model::id::PlayerId;

    fn q(player: &str, prompt: &str, ids: &[&str]) -> Choice {
        Choice::new(PlayerId::new(player), prompt, ids.iter().map(|id| ChoiceOption::labelled(*id, "x", *id)).collect())
    }

    fn pay(player: &str, owed: i64, ids: &[&str]) -> Choice {
        Choice::new(
            PlayerId::new(player),
            format!("pay {owed} more influence"),
            ids.iter().map(|id| ChoiceOption::labelled(*id, crate::production::PAY_KIND, *id)).collect(),
        )
    }

    /// The groups' kinds and answers.
    fn shape(groups: &[Group]) -> Vec<(Option<Kind>, Vec<usize>, Option<String>)> {
        groups.iter().map(|g| (g.kind, g.at.clone(), g.answer.clone())).collect()
    }

    #[test]
    fn the_strings_pool_questions_of_one_seat_are_one_gain_and_a_plain_question_stays_one() {
        let gain = q("a", GAIN_PROMPT, &POOL_IDS);
        let other = q("a", "choose", &["x", "y"]);
        let qs = [(&gain, Some("tactic_tokens"), false), (&gain, Some("strategic_tokens"), false), (&gain, Some("tactic_tokens"), false), (&other, Some("y"), false)];
        assert_eq!(
            shape(&collapse(&qs)),
            vec![(Some(Kind::Gain), vec![0, 1, 2], Some("2|0|1".to_owned())), (None, vec![3], Some("y".to_owned()))]
        );
        // Another seat's pools are another gain.
        let gain_b = q("b", GAIN_PROMPT, &POOL_IDS);
        let qs = [(&gain, Some("tactic_tokens"), false), (&gain_b, Some("fleet_tokens"), false)];
        assert_eq!(collapse(&qs).len(), 2);
    }

    /// Leadership's primary as the string asks it (three free pools, then "yes", a payment and the bought token's pool, then "no"): the core's
    /// three questions, the free gain, the purchase and the gain of what was bought.
    #[test]
    fn leadership_s_loop_is_the_free_gain_the_purchase_and_the_bought_gain() {
        let gain = q("a", GAIN_PROMPT, &POOL_IDS);
        let buy = q("a", "spend 3 influence for a command token", &["no", "yes"]);
        let paid = pay("a", 3, &["exhaust|jord", "trade_good"]);
        let qs = [
            (&gain, Some("tactic_tokens"), false),
            (&gain, Some("tactic_tokens"), false),
            (&gain, Some("fleet_tokens"), false),
            (&buy, Some("yes"), false),
            (&paid, Some("exhaust|jord"), false),
            (&gain, Some("fleet_tokens"), false),
            (&buy, Some("yes"), false),
            (&gain, Some("strategic_tokens"), false),
            (&buy, Some("no"), false),
        ];
        assert_eq!(
            shape(&collapse(&qs)),
            vec![
                (Some(Kind::Gain), vec![0, 1, 2], Some("2|1|0".to_owned())),
                (Some(Kind::Purchase), vec![3, 4, 6, 8], Some("purchase:2:exhaust|jord".to_owned())),
                (Some(Kind::Gain), vec![5, 7], Some("0|1|1".to_owned())),
            ]
        );
        // The secondary: the follower's first token is paid before any purchase question.
        let qs = [(&paid, Some("trade_good"), false), (&gain, Some("fleet_tokens"), false), (&buy, Some("no"), false)];
        assert_eq!(
            shape(&collapse(&qs)),
            vec![(Some(Kind::Purchase), vec![0, 2], Some("purchase:1:trade_good".to_owned())), (Some(Kind::Gain), vec![1], Some("0|1|0".to_owned()))]
        );
    }

    #[test]
    fn one_bills_payments_are_one_set_and_a_new_bill_is_another() {
        let three = pay("a", 3, &["trade_good", "exhaust|jord"]);
        let two = pay("a", 2, &["trade_good", "exhaust|jord"]);
        let qs = [(&three, Some("trade_good"), false), (&two, Some("trade_good"), false), (&three, Some("exhaust|jord"), false)];
        assert_eq!(
            shape(&collapse(&qs)),
            vec![(Some(Kind::Pay), vec![0, 1], Some("pay:trade_good+trade_good".to_owned())), (Some(Kind::Pay), vec![2], Some("pay:exhaust|jord".to_owned()))]
        );
    }

    #[test]
    fn politics_two_placements_are_one_order() {
        let first = q("a", "place a1 where", &["top", "bottom"]);
        let second = q("a", "place a2 where", &["top", "bottom"]);
        for (answers, order) in [(["top", "bottom"], "a1/a2"), (["bottom", "top"], "a2/a1"), (["top", "top"], "a2,a1/"), (["bottom", "bottom"], "/a1,a2")] {
            let qs = [(&first, Some(answers[0]), false), (&second, Some(answers[1]), false)];
            assert_eq!(shape(&collapse(&qs)), vec![(Some(Kind::Politics), vec![0, 1], Some(order.to_owned()))]);
        }
    }

    #[test]
    fn the_discards_and_the_removals_name_what_went() {
        let mut hand = q("a", "over the hand limit — discard one of 8", &["0", "1"]);
        hand.options[0].label = "Sabotage".to_owned();
        hand.options[1].label = "Bribery".to_owned();
        let qs = [(&hand, Some("1"), false), (&hand, Some("0"), false)];
        assert_eq!(shape(&collapse(&qs)), vec![(Some(Kind::Discard), vec![0, 1], Some("discard:Bribery+Sabotage".to_owned()))]);
        let mut remove = q("a", "remove a unit: over fleet supply in 01", &["remove|0", "remove|1"]);
        remove.options[1].label = "remove cruiser".to_owned();
        let qs = [(&remove, Some("remove|1"), false), (&remove, Some("remove|1"), false)];
        assert_eq!(shape(&collapse(&qs)), vec![(Some(Kind::Removal), vec![0, 1], Some("remove:remove cruiser+remove cruiser".to_owned()))]);
    }
}
