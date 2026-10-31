//! The texts the turn flow (lane B, `driver::turn`) names that the core holds as interned codes, and the closed list of the places
//! the live turn flow cannot follow the string yet. The adapter interns the texts once per game (`reactions_core::set_table_codes`).

use crate::state::Code;

/// The interned texts of the turn flow.
#[derive(Debug, Clone, Copy, Default)]
pub struct TurnCodes {
    /// `psychoarchaeology`: the source of the trade good the card gains.
    pub psychoarchaeology: Code,
    /// `legendary`: the source of every legendary planet question.
    pub legendary: Code,
    /// The prompt of the Minister of War's offer, and the law's alias (`minister_war`).
    pub minister_prompt: Code,
    pub minister_source: Code,
    /// `ms`, the ability name of Military Support's placement question, and `military_support`, the reason of its strategy token.
    pub military_support: Code,
    pub military_support_reason: Code,
    /// `spec ops II`: the purpose of the survival roll.
    pub spec_ops: Code,
    /// `strategy_card`: the source of the trade goods a card gathered on the mat.
    pub strategy_card: Code,
    /// `starting_technology`: the source of a starting technology question.
    pub starting_technology: Code,
    /// `transaction`: the source of the trade goods a deal gives (the live trade window).
    pub transaction: Code,
    /// `dark_pact`: the source of the trade good Dark Pact gains.
    pub dark_pact: Code,
    /// `tcs`: the source of Temporal Command Suite's question.
    pub tcs: Code,
}

/// Why the live turn flow stops (a counted gap, never a silent skip): what the job needs and the core does not hold yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnGap {
    /// Chaos Mapping at the start of a turn: the production facts (`buildable`, the capacity of the seat's systems) are the production
    /// lane's.
    ChaosMapping,
    /// The play facts of the table are missing (`AbilityTable::facts.plays` is the unit roster and the scope of the sources).
    NoRoster,
    /// A child window asked the driver to open another window (`Next::Open`): the job runs its children to their end inside one step.
    OpenedWindow,
    /// The legendary planets' window left a case to the string (a draw the table could not judge).
    LegendaryUnsupported,
    /// The closing menu needs sources of the action menu the core does not hold yet.
    MenuSources,
    /// `Game::step_phase` ran in the action phase while a seat had not passed (the string reports `MissingActivePlayer`).
    PhaseNotReady,
    /// The diplomacy promise book could not settle at the round's end (a feature the creation refuses).
    PromiseBook,
    /// A question source (`AskArm as u8`) no lane has made native yet.
    AskArm(u8),
    /// Nekro's Propagation is announced (its reaction is the string's).
    Propagation,
    /// A tile a faction effect replaced or swapped has to be replayed onto the map.
    MapEdits,
    /// Imperial Arbiter (a ministry law) could swap strategy cards at the end of the draft.
    ImperialArbiter,
    /// Quantum Datahub Node could swap strategy cards at the end of the draft.
    QuantumDatahub,
    /// The event of a strategy card chosen was cancelled in its window (the string reports `TimingEventCancelled`).
    EventCancelled,
    /// The resolver refused an emission (a defect of the position, never a rule).
    EmissionRefused,
    /// A replayed answer was refused by the window it was given to (the question changed between the replays).
    ReplayDiverged,
    /// A free-form deal names a card, note, planet or fragment kind the typed tables do not know (the string answers with a rejection).
    TradeTerms,
    /// A commander of a faction whose unlock condition no hook states yet (`commander_unlocked`) could unlock.
    CommanderUnlock,
    /// Plots Within Plots left a plot card to place (the Firmament, a dropped faction).
    PlotCard,
    /// A ship stands in an Entropic Scar (Thunder's Edge: the faction technologies of the status phase).
    EntropicScar,
    /// A promissory note's faction is played by two seats, so the note has no one owner to go home to.
    NoteOwner,
    /// An event scoring window closed with the agendas of its agenda phase waiting on the stack: the vote is the agenda lane's.
    AgendaQueue,
    /// A secret objective the core cannot judge (Strengthen Bonds, Produce en Masse).
    ScoringSecret,
}
