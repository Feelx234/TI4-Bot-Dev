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
    /// `strategy_secondary`: the reason of the strategy token a follower spends (lane E).
    pub strategy_secondary: Code,
    /// `trade_primary`: the source of the three trade goods the Trade primary gains; `place_structure`: the source of Construction's
    /// structure question; `te6warfare` and `te4construction`: the sources of the Thunder's Edge primaries' questions.
    pub trade_primary: Code,
    pub place_structure: Code,
    pub te6warfare: Code,
    pub te4construction: Code,
    /// `production` (the source of `UNITS_PRODUCED`) and `units` (the key of its list).
    pub production: Code,
    pub units: Code,
    /// `extreme_duress`: the source of the trade goods an Extreme Duress hands the holder.
    pub extreme_duress: Code,
    /// `orbital_drop`: the source of Orbital Drop's questions and of its token event.
    pub orbital_drop: Code,
    /// `nekroagent`: the source of the trade goods Nekro Malleon gains.
    pub nekroagent: Code,
    /// `star_forge` (the source of Star Forge's question and the reason of its token event), `muaat_mech` (the source of the Ember
    /// Colossus question) and `inferno` (the reason of The Inferno's token event).
    pub star_forge: Code,
    pub muaat_mech: Code,
    pub inferno: Code,
    /// `production_biomes`: the reason of the token Production Biomes spends and the source of the goods it gives.
    pub production_biomes: Code,
    /// `fabrication` and `bmf`: the origins of the Naaz Fabrication and Black Market Forgery questions.
    pub fabrication: Code,
    pub bmf: Code,
    /// `stall_tactics` and `yssaril_mech`: the sources of the Yssaril Stall Tactics question and of the mech it places.
    pub stall_tactics: Code,
    pub yssaril_mech: Code,
    /// `relic`: the source of the trade goods a relic gains.
    pub relic: Code,
    /// `silver_flame`: the purpose of the Silver Flame's die.
    pub silver_flame: Code,
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
    /// A commander of a faction whose unlock condition no hook states could unlock. Never raised since lane F2 filled the hook slots of every in-scope
    /// faction (`driver::turn::start` judges every locked commander); kept for the dropped factions' hook-less commanders.
    CommanderUnlock,
    /// The seat chose an option of the closing menu that is not "end your turn" (a free leader's component action): the dispatch of a menu option is the
    /// string's (`Dispatch::Lend`, KM2).
    MenuDispatch,
    /// A free-form deal names a card, note, planet or fragment kind the typed tables do not know (the string answers with a rejection).
    TradeTerms,
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
    /// The action menu could not be listed or run natively (package KM): see [`MenuGap`].
    Menu(MenuGap),
    /// The ability of a strategy card could not begin at this table (`fx_perform::Began::Unsupported`: a table without the unit roster, Doctor Sucaban's offer in a
    /// research, a placement a faction effect changes, a scoreable objective the core cannot name).
    StrategyAbility,
    /// The table has no link to the window of an ability (`AbilityTable::links`).
    NoLink,
    /// A use of PRODUCTION made units and their `UNITS_PRODUCED` announcement is the production lane's (M2): the core's payload cannot carry
    /// the list yet.
    ProductionAnnounce,
    /// Xxcha's Peace Accords could annex a planet after a Diplomacy ability (the annexation window is not ported yet).
    PeaceAccords,
    /// A hero's strategy primary or Thunder's Edge Warfare's free tactical action needs the tactical action of the tactical lane.
    FreeTactical,
    /// A strategic action was cancelled (Coup d'Etat) and ends the turn.
    StrategicCancelled,
    /// The follower window of a card nobody holds, resolved by a hero (`exhausts_card` false), is not run by this arm.
    ForeignWindow,
}

/// Why the action menu, or the action chosen from it, is not native for a position (a counted gap, never a silent skip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuGap {
    /// An action card the core cannot judge playable (Signal Jamming's systems, or a bill whose facts the core does not hold).
    ActionCard,
    /// A faction component action of a module whose listing is not in the core (`ComponentAction::module`).
    Faction(crate::generated::Faction),
    /// The option chosen has no native body yet: the kind (`driver::turn::action::gap_code`) and the option's key (`Opt::key`), which names it
    /// exactly.
    Unrouted(u8, u32),
    /// An option the live menu never lists was answered.
    Refused,
}
