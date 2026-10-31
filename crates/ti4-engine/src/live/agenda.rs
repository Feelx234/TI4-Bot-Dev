//! The agenda phase's share of the live loop (package KV, lane D): the texts the core names by code, and the display facts of the
//! questions of the agenda phase that only the state and the content store know.

use ti4_engine_core::choice::agenda::Ask as AAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::generated::Agenda;
use ti4_engine_core::kernel::agenda_codes::AgendaCodes;
use ti4_engine_core::state::CoreState;

use crate::choice::Choice;
use crate::generated::IdStr;
use crate::render::Render;

/// The prompt of Committee Formation's discard offer; `{agenda}` is the alias of the agenda being revealed.
pub const COMMITTEE_PROMPT: &str = "Committee Formation: discard to choose who {agenda} elects";

/// What discarding Committee Formation does (`Game::committee_formation`'s `DiscardPresentation`).
const COMMITTEE_EFFECT: &str = "Choose who the agenda elects";

/// The interned texts of the agenda phase (`reactions_core::set_table_codes`).
#[must_use]
pub fn codes(r: &Render) -> AgendaCodes {
    AgendaCodes {
        quash: r.code("quash"),
        favor: r.code("political_favor"),
        secret: r.code("ps"),
        trade_rider: r.code("trade_rider"),
        keleres_rider: r.code("keleres_rider"),
        committee: r.code("committee"),
        committee_prompt: r.code(COMMITTEE_PROMPT),
        agenda: r.code("agenda"),
        fleet_pool: r.code("fleet"),
        defense_act: r.code("defense_act"),
        tech_corpus: |a, b| corpus_rank(a).cmp(&corpus_rank(b)),
        redistribution: r.code("redistribution"),
        grant: r.code("grant_reallocation"),
        ground_cause: std::array::from_fn(|i| ti4_engine_core::generated::Agenda::ALL.get(i).map_or(Default::default(), |a| r.code(&format!("agenda:{}", IdStr::as_str(*a))))),
    }
}

/// The whole ledger of a vote with more outcomes than `Tally` holds (`details.tally`): every outcome offered, in order, with its votes.
fn full_tally(core: &CoreState, rendered: &mut Choice) {
    use ti4_engine_core::choice::agenda::Tally;
    use ti4_engine_core::keys::Outcome;
    let Some(ti4_engine_core::state::Window::Voting(v)) = core.pending().window(ti4_engine_core::state::WindowKind::Voting) else { return };
    if v.vote.choices.len() <= Tally::MAX {
        return;
    }
    let text = |o: Outcome| -> String {
        match o {
            Outcome::Empty => String::new(),
            Outcome::For => "for".to_owned(),
            Outcome::Against => "against".to_owned(),
            Outcome::Seat(s) => core.owners.id(s).map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default(),
            Outcome::Planet(p) => IdStr::as_str(p).to_owned(),
            Outcome::StrategyCard(c) => IdStr::as_str(c).to_owned(),
            Outcome::Law(a) => IdStr::as_str(a).to_owned(),
            Outcome::Secret(s) => IdStr::as_str(s).to_owned(),
        }
    };
    let entries: Vec<serde_json::Value> = v
        .vote
        .choices
        .as_slice()
        .iter()
        .map(|o| serde_json::json!([text(*o), v.vote.counts.as_slice().iter().find(|(c, _)| c == o).map_or(0, |(_, n)| *n)]))
        .collect();
    rendered.details.insert("tally".to_owned(), serde_json::Value::Array(entries));
}

/// Where a technology stands in the corpus (the order of `technologies.json`).
fn corpus_rank(t: ti4_engine_core::generated::Technology) -> usize {
    use ti4_engine_core::generated::Technology;
    static RANKS: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
    let ranks = RANKS.get_or_init(|| {
        let mut ranks = vec![usize::MAX; Technology::COUNT];
        for (at, record) in ti4_content::ContentStore::embedded().records(ti4_model::content_types::ContentType::Technologies).iter().enumerate() {
            if let Some(tech) = record.text("alias").and_then(|alias| <Technology as IdStr>::from_id(alias)) {
                ranks[tech as usize] = at;
            }
        }
        ranks
    });
    ranks[t as usize]
}

/// The display facts of the questions of the agenda phase.
pub fn complete(core: &CoreState, choice: &CoreChoice, rendered: &mut Choice) {
    if let Ask::Agenda(AAsk::Planets { .. }) = choice.ask {
        full_tally(core, rendered);
    }
    // The Technology Rider's research: the options are named by the technology's printed name.
    if matches!(choice.ask, Ask::Strategy(ti4_engine_core::choice::strategy::Ask::ResearchTechnology { .. })) {
        let content = ti4_content::ContentStore::embedded();
        for (option, opt) in rendered.options.iter_mut().zip(choice.opts.iter()) {
            if let ti4_engine_core::choice::Opt::Strategy(ti4_engine_core::choice::strategy::Opt::Research(t)) = opt {
                option.label = crate::technology::name(content, &ti4_model::id::TechnologyId::new(IdStr::as_str(*t)));
            }
        }
    }
    // Committee Formation's election: the string asks it without a decision context (`Live::committee_elect`).
    if let Ask::Agenda(AAsk::Committee { .. }) = choice.ask {
        rendered.context = None;
    }
    if let Ask::Agenda(AAsk::DiscardLaw { law: Agenda::CommitteeFormation, .. }) = choice.ask {
        // The prompt names the agenda the election is for: the one in play (revealed, not yet voted on).
        let alias = core.agenda().in_play.map_or("", |a| IdStr::as_str(a));
        rendered.prompt = rendered.prompt.replace("{agenda}", alias);
        super::turn::discard_law(Agenda::CommitteeFormation, COMMITTEE_EFFECT, rendered);
    }
}
