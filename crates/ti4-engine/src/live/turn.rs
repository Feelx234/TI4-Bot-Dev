//! The turn flow's share of the live loop (package KP, lane B): the texts the core names by code, and the display facts of the questions
//! the turn-flow jobs ask that only the state and the content store know.

use ti4_content::ContentStore;
use ti4_model::ContentType;
use ti4_engine_core::choice::agenda::Ask as AAsk;
use ti4_engine_core::choice::technology::{Ask as TAsk, StartRule};
use ti4_engine_core::choice::turn::Ask as TurnAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::generated::{Agenda, Faction};
use ti4_engine_core::kernel::turn_codes::TurnCodes;
use ti4_engine_core::state::CoreState;

use crate::choice::{Choice, ChoiceOption};
use crate::generated::IdStr;
use crate::render::Render;

/// The prompt of the Minister of War's offer to discard its law.
pub const MINISTER_PROMPT: &str = "Minister of War: discard to retrieve a command token and act again";

/// What the offer says discarding does (`Game::minister_of_war_text`'s `DiscardPresentation`).
const MINISTER_EFFECT: &str = "Retrieve a command token and take another action";

/// The prompt of Imperial Arbiter's offer to discard its law (`Game::imperial_arbiter`).
pub const ARBITER_PROMPT: &str = "Imperial Arbiter: discard to swap a strategy card";

/// What the Imperial Arbiter's offer says discarding does.
const ARBITER_EFFECT: &str = "Swap a strategy card with another player";

/// What the Minister of Peace's offer says discarding does (`Game::minister_of_peace`).
const PEACE_EFFECT: &str = "Ends the active player's turn at once";

/// The interned texts of the turn flow (`reactions_core::set_table_codes`).
#[must_use]
pub fn codes(r: &Render) -> TurnCodes {
    TurnCodes {
        psychoarchaeology: r.code("psychoarchaeology"),
        legendary: r.code("legendary"),
        minister_prompt: r.code(MINISTER_PROMPT),
        minister_source: r.code("minister_war"),
        military_support: r.code("ms"),
        military_support_reason: r.code("military_support"),
        spec_ops: r.code("spec ops II"),
        strategy_card: r.code("strategy_card"),
        starting_technology: r.code("starting_technology"),
        transaction: r.code("transaction"),
        dark_pact: r.code("dark_pact"),
        tcs: r.code("tcs"),
        strategy_secondary: r.code("strategy_secondary"),
        trade_primary: r.code("trade_primary"),
        place_structure: r.code("place_structure"),
        te6warfare: r.code("te6warfare"),
        te4construction: r.code("te4construction"),
        production: r.code("production"),
        units: r.code("units"),
        arbiter_prompt: r.code(ARBITER_PROMPT),
        arbiter: r.code("arbiter"),
        quantum_datahub: r.code("quantum_datahub"),
    }
}

/// The display facts of the questions of the turn-flow jobs.
pub fn complete(core: &CoreState, choice: &CoreChoice, rendered: &mut Choice) {
    match choice.ask {
        // Predictive Intelligence: the pools as they stand now (`technology_dual::drive_valued`'s closure).
        Ask::Technology(TAsk::Restack) => {
            let letnev = core.players().by_seat(choice.actor).is_some_and(|p| p.faction == Faction::TheBaronyOfLetnev);
            rendered.details.extend(crate::strategy_cards_core::pool_details(core, choice.actor, letnev, "restack", None));
        }
        // Imperial Arbiter's offer (`Game::imperial_arbiter` through `laws::offer_discard`).
        Ask::Agenda(AAsk::DiscardLaw { law: Agenda::ImperialArbiter, .. }) => discard_law(Agenda::ImperialArbiter, ARBITER_EFFECT, Vec::new(), rendered),
        // The two questions of the swap are asked without a context (`Game::imperial_arbiter`).
        Ask::Strategy(ti4_engine_core::choice::strategy::Ask::ArbiterGive | ti4_engine_core::choice::strategy::Ask::ArbiterTake) => rendered.context = None,
        // "Which command token comes back": the string asks it without a context (`minister_of_war_text`).
        Ask::Turn(TurnAsk::MinisterToken) => rendered.context = None,
        // The fleet sweep's "remove a unit" question: the string asks it without a context (`fleet::enforce_seeing`).
        Ask::Tactical(ti4_engine_core::choice::tactical::Ask::Remove { .. }) => rendered.context = None,
        // The starting technology: the pick, the rule, the faction, what the seat holds; per option the colour, the other owners and
        // the number of prerequisites (`starting_tech::choice`).
        Ask::Technology(TAsk::StartingTech { number, of, rule }) => starting_tech(core, choice, number, of, rule, rendered),
        // Chaos Mapping's single unit (`produce_one_from` with no source): the string asks the unit and the place without a context.
        Ask::Production(ti4_engine_core::choice::production::Ask::One(one)) if one.alias.is_none() => rendered.context = None,
        Ask::Production(ti4_engine_core::choice::production::Ask::OnePlace { .. }) => rendered.context = None,
        // Military Support: the offer card of the placement.
        Ask::Promissory(_) => crate::render::promissory::complete_place_units(choice, rendered),
        // The Minister of War's offer (`laws::offer_discard`): the law as printed and what each answer does.
        Ask::Agenda(AAsk::DiscardLaw { law: Agenda::MinisterOfWar, .. }) => discard_law(Agenda::MinisterOfWar, MINISTER_EFFECT, Vec::new(), rendered),
        // The Minister of Peace's offer after an activation (`Game::minister_of_peace`): what the law does, the system and the seat that activated it.
        Ask::Agenda(AAsk::DiscardLaw { law: Agenda::MinisterOfPeace, .. }) => {
            let mut facts = Vec::new();
            if let Some(system) = core.meta().active_system {
                facts.push(crate::choice::offer_fact_system("Activated system", IdStr::as_str(system)));
            }
            if let Some(text) = core.meta().active.and_then(|s| core.owners.id(s)) {
                facts.push(crate::choice::offer_fact_seat("Activated by", &String::from_utf8_lossy(text)));
            }
            discard_law(Agenda::MinisterOfPeace, PEACE_EFFECT, facts, rendered);
        }
        _ => {}
    }
}

/// `starting_tech::choice`'s display facts.
fn starting_tech(core: &CoreState, choice: &CoreChoice, number: u8, of: u8, rule: StartRule, rendered: &mut Choice) {
    let content = ContentStore::embedded();
    let Some(me) = core.players().by_seat(choice.actor) else { return };
    let rule = match rule {
        StartRule::Listed => "listed",
        StartRule::OwnedByOthers => "owned_by_others",
        StartRule::Researchable => "researchable",
    };
    let held: Vec<String> = me.tech.owned.iter().map(|t| IdStr::as_str(t).to_owned()).collect();
    rendered.details.insert("pick".to_owned(), u64::from(number).into());
    rendered.details.insert("of".to_owned(), u64::from(of).into());
    rendered.details.insert("rule".to_owned(), rule.into());
    rendered.details.insert("faction".to_owned(), IdStr::as_str(me.faction).into());
    rendered.details.insert("held".to_owned(), held.into());
    for option in &mut rendered.options {
        let Some(tech) = ti4_engine_core::generated::Technology::from_id(&option.id) else { continue };
        let id = ti4_model::id::TechnologyId::new(option.id.clone());
        if let Some(colour) = crate::technology::colour_type(content, &id) {
            *option = std::mem::replace(option, ChoiceOption::decline()).with("colour", colour);
        }
        let owners: Vec<String> = core
            .players()
            .iter()
            .filter(|p| p.seat != choice.actor && p.tech.owned.contains(tech))
            .filter_map(|p| core.owners.id(p.seat).map(|b| String::from_utf8_lossy(b).into_owned()))
            .collect();
        if !owners.is_empty() {
            *option = std::mem::replace(option, ChoiceOption::decline()).with("owners", owners);
        }
        let prerequisites: usize = crate::technology::prerequisites(content, &id).values().sum();
        *option = std::mem::replace(option, ChoiceOption::decline()).with("prerequisites", prerequisites as u64);
    }
}

/// `laws::offer_discard_text`'s display facts for `law`.
pub(super) fn discard_law(law: Agenda, effect: &str, facts: Vec<serde_json::Value>, rendered: &mut Choice) {
    let alias = IdStr::as_str(law);
    let content = ContentStore::embedded();
    let record = content.get(ContentType::Agendas, alias);
    let text_of = |record: &ti4_content::Record, key: &str| record.text(key).filter(|text| !text.is_empty()).map(str::to_owned);
    let name = record.as_ref().and_then(|record| record.text("name")).map_or_else(|| alias.to_owned(), str::to_owned);
    let text = record.as_ref().and_then(|record| text_of(record, "mapText").or_else(|| text_of(record, "text1")));
    rendered.details.extend(crate::choice::offer_details(
        crate::choice::offer_card(&name, "law", None, text.as_deref()),
        facts,
        &[("discard", crate::choice::offer_caption(&format!("Discard {name}"), Some(effect))), ("decline", crate::choice::offer_caption(&format!("Keep {name}"), None))],
    ));
}
