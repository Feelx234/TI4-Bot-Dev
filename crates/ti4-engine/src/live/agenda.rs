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
        defense_act: r.code("defense_act"),
    }
}

/// The display facts of the questions of the agenda phase.
pub fn complete(core: &CoreState, choice: &CoreChoice, rendered: &mut Choice) {
    if let Ask::Agenda(AAsk::DiscardLaw { law: Agenda::CommitteeFormation, .. }) = choice.ask {
        // The prompt names the agenda the election is for: the one in play (revealed, not yet voted on).
        let alias = core.agenda().in_play.map_or("", |a| IdStr::as_str(a));
        rendered.prompt = rendered.prompt.replace("{agenda}", alias);
        super::turn::discard_law(Agenda::CommitteeFormation, COMMITTEE_EFFECT, rendered);
    }
}
