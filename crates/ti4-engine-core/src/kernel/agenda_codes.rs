//! The texts the agenda phase (lane D, `driver::agenda`) names that the core holds as interned codes, and the closed list of the places
//! the live agenda phase cannot follow the string yet. The adapter interns the texts once per game (`reactions_core::set_table_codes`).

use crate::state::Code;

/// The interned texts of the agenda phase.
#[derive(Debug, Clone, Copy)]
pub struct AgendaCodes {
    /// The `reason` of the staged `STRATEGY_TOKEN_SPENT` events of Quash (`quash`) and Political Favor (`political_favor`).
    pub quash: Code,
    pub favor: Code,
    /// The source of Political Secret's question (the renderer's text `ps`).
    pub secret: Code,
    /// The sources of the staged `TRADE_GOODS_GAINED` events of the Trade and Keleres Riders.
    pub trade_rider: Code,
    pub keleres_rider: Code,
    /// `committee`: the source of Committee Formation's election question and of its discard offer.
    pub committee: Code,
    /// The prompt of Committee Formation's discard offer (the renderer completes it with the agenda: `committee_prompt`).
    pub committee_prompt: Code,
    /// `agenda`, the source of a trade-good gain of an agenda effect.
    pub agenda: Code,
    /// `fleet`: the pool of the command tokens a Diplomacy Rider places.
    pub fleet_pool: Code,
    /// `defense_act`, the content Homeland Defense Act's question cites.
    pub defense_act: Code,
    /// `redistribution` and `grant_reallocation`: the sources of the questions of those two effects.
    pub redistribution: Code,
    pub grant: Code,
    /// The order of the technologies in the corpus (the order a "gain which technology" question lists them in); the core's own order is
    /// by id text.
    pub tech_corpus: fn(crate::generated::Technology, crate::generated::Technology) -> core::cmp::Ordering,
    /// The cause `agenda:<alias>` of a ground force an agenda's effect destroyed, by the agenda's index in `Agenda::ALL`.
    pub ground_cause: [Code; GROUND_CAUSES],
}

/// Room for every agenda (`Agenda::COUNT` is 63).
pub const GROUND_CAUSES: usize = 64;

impl Default for AgendaCodes {
    fn default() -> Self {
        Self {
            quash: Code(0),
            favor: Code(0),
            secret: Code(0),
            trade_rider: Code(0),
            keleres_rider: Code(0),
            committee: Code(0),
            committee_prompt: Code(0),
            agenda: Code(0),
            fleet_pool: Code(0),
            defense_act: Code(0),
            redistribution: Code(0),
            grant: Code(0),
            tech_corpus: |a, b| a.cmp(&b),
            ground_cause: [Code(0); GROUND_CAUSES],
        }
    }
}

/// Why the live agenda phase stops (a counted gap, never a silent skip): what the phase needs and the core does not hold yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgendaGap {
    /// The phase was entered outside the agenda phase, or before the custodians were removed (the string reports an error).
    PhaseNotReady,
    /// The reveal chain failed (the typed emission of `AGENDA_REVEALED` was refused).
    RevealFailed,
    /// The close of a vote failed (the typed emission of `AGENDA_RESOLVED` was refused).
    CloseFailed,
    /// The close met a position it does not carry (a rider's payoff, a draw a hook changes, a rules version before the pool question).
    CloseUnsupported,
    /// The agenda's effect has no window in the core (the agenda is named by `effect`'s index in `Agenda::ALL`).
    NoEffect(u8),
    /// The effect met a position it does not carry (a draw a hook changes, a table without the printed-name groups).
    EffectUnsupported(u8),
    /// A child window asked the driver to open another window (`Next::Open`): the job runs its children to their end inside one step.
    OpenedWindow,
    /// The resolver refused an emission (a defect of the position, never a rule).
    EmissionRefused,
    /// A replayed answer was refused by the window it was given to (the question changed between the replays).
    ReplayDiverged,
    /// The unit roster or the printed-name groups of the table are missing.
    NoRoster,
    /// The play facts a window needs are missing.
    NoFacts,
    /// A position of a feature the product refuses (a deck reserve, the Executive Order, structured diplomacy).
    Refused,
}
