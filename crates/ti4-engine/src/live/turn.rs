//! The turn flow's share of the live loop (package KP, lane B): the texts the core names by code, and the display facts of the questions
//! the turn-flow jobs ask that only the state and the content store know.

use ti4_content::ContentStore;
use ti4_model::ContentType;
use ti4_engine_core::choice::agenda::Ask as AAsk;
use ti4_engine_core::choice::technology::Ask as TAsk;
use ti4_engine_core::choice::turn::Ask as TurnAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice};
use ti4_engine_core::generated::{Agenda, Faction};
use ti4_engine_core::kernel::turn_codes::TurnCodes;
use ti4_engine_core::state::CoreState;

use crate::choice::Choice;
use crate::generated::IdStr;
use crate::render::Render;

/// The prompt of the Minister of War's offer to discard its law.
pub const MINISTER_PROMPT: &str = "Minister of War: discard to retrieve a command token and act again";

/// What the offer says discarding does (`Game::minister_of_war_text`'s `DiscardPresentation`).
const MINISTER_EFFECT: &str = "Retrieve a command token and take another action";

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
        // "Which command token comes back": the string asks it without a context (`minister_of_war_text`).
        Ask::Turn(TurnAsk::MinisterToken) => rendered.context = None,
        // Military Support: the offer card of the placement.
        Ask::Promissory(_) => crate::render::promissory::complete_place_units(choice, rendered),
        // The Minister of War's offer (`laws::offer_discard`): the law as printed and what each answer does.
        Ask::Agenda(AAsk::DiscardLaw { law: Agenda::MinisterOfWar, .. }) => discard_law(Agenda::MinisterOfWar, MINISTER_EFFECT, rendered),
        _ => {}
    }
}

/// `laws::offer_discard_text`'s display facts for `law`.
fn discard_law(law: Agenda, effect: &str, rendered: &mut Choice) {
    let alias = IdStr::as_str(law);
    let content = ContentStore::embedded();
    let record = content.get(ContentType::Agendas, alias);
    let text_of = |record: &ti4_content::Record, key: &str| record.text(key).filter(|text| !text.is_empty()).map(str::to_owned);
    let name = record.as_ref().and_then(|record| record.text("name")).map_or_else(|| alias.to_owned(), str::to_owned);
    let text = record.as_ref().and_then(|record| text_of(record, "mapText").or_else(|| text_of(record, "text1")));
    rendered.details.extend(crate::choice::offer_details(
        crate::choice::offer_card(&name, "law", None, text.as_deref()),
        Vec::new(),
        &[("discard", crate::choice::offer_caption(&format!("Discard {name}"), Some(effect))), ("decline", crate::choice::offer_caption(&format!("Keep {name}"), None))],
    ));
}
