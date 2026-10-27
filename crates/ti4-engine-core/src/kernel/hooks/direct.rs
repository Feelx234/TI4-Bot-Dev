//! The direct calls: shared modules of the string engine that call a function of one faction module
//! (`crate::factions::keleres::offer_agent`, `crimson::abilities_lost`, `bastion::extra_die`, ...).
//! They are not in `factions::Hooks` today; in the core there are no such imports (layer rule), so each
//! distinct function becomes a named slot here. Inventory at `201f6714`: 91 functions and constants called
//! from 24 shared modules (`combat` 38 sites, `invasion` 56, `production` 49, `game` 24, `objectives` 13, ...).
//!
//! Naming: `<faction>_<function>`. Kind `call` takes the single provider (the faction module registers its
//! own function; with no provider the default value is returned, which is the neutral value of every
//! slot below: `false`, `0`, empty). Two modules never provide the same slot.
//!
//! Faction-specific memory (the `faction_marks` of the string state) is read and written inside the
//! providing function through `state.marks`; the shared module never sees it.
//!
//! Constants (`keleres::VARIANTS`, `obsidian::NOT_CHOOSABLE_DURING_SETUP`, `crimson::MECH`,
//! `firmament_plots::PRIMARY_INSTEAD_ID`, `nomad::AGENT_EXHAUSTED`, `bastion_units::ROUND_BEGAN`) are not
//! hooks: they become generated ids / tables (X3, C shards) and `EventKind` variants.

use super::{CombatCtx, hook_group};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Faction, Leader, Planet, SecretObjective, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::{Cx, Kernel};
use crate::state::{CoreState, Piece};

/// A short list of planets.
pub type Planets = SmallList<Planet, 4>;
/// A short list of systems.
pub type Systems = SmallList<System, 4>;
/// A short list of seats.
pub type Seats = SmallList<Seat, 4>;
/// Pieces on a planet or in space.
pub type Pieces = SmallList<Piece, 4>;
/// `(system, planet)` plot choices of the Firmament/Obsidian plots: the secret and the token owner.
pub type PlotOptions = SmallList<(SecretObjective, Seat), 4>;

/// Which research technologies a window around research counts (Carrina: `any` or `unit_upgrades`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechFilter {
    Any,
    UnitUpgrades,
}

/// A booked-in Keleres agent grant probe: the state is evaluated with the agent's grant applied
/// (`keleres::with_agent_granted(state, payer, probe)`): the core clones the state (cheap), applies the
/// grant to the clone and runs the query on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentGrant {
    pub payer: Seat,
}

hook_group! {
    /// The direct faction calls of the shared modules.
    DirectHooks {
        // ---- Arborec / Argent / Cabal / Empyrean / Winnu (small) ---------------------------------------
        /// Argent agent: forget the production destinations of the agent. String: `production` (8 sites).
        each argent_clear_agent_production_destinations(cx: &mut Cx<'_>, who: Seat, system: System) -> ();
        /// Cabal: a unit was replenished (Devour bookkeeping). String: `promissory`.
        each cabal_note_replenished(cx: &mut Cx<'_>, who: Seat) -> ();
        /// Empyrean: deal the Blood Pact notes at seating. String: `seating`.
        each empyrean_deal_notes(cx: &mut Cx<'_>, who: Seat) -> ();
        /// Winnu agent is ready for production. String: `production`.
        call winnu_agent_ready(s: &CoreState, who: Seat) -> bool;
        /// Winnu agent exhausted for production; true if it was. String: `production`.
        call winnu_exhaust_agent_for_production(cx: &mut Cx<'_>, who: Seat) -> bool;

        // ---- Bastion ------------------------------------------------------------------------------------
        /// Extra dice a Bastion unit rolls. String: `combat` (3), `invasion` (3).
        call bastion_extra_die(s: &CoreState, piece: Piece) -> usize;
        /// A ship was lost in `system`. String: `combat`.
        each bastion_note_ship_lost(cx: &mut Cx<'_>, system: System, piece: Piece) -> ();
        /// Take the ship-lost mark of `owner` in `system`; true if one existed. String: `combat` (2).
        call bastion_take_ship_lost(cx: &mut Cx<'_>, system: System, owner: Seat, unit: Unit) -> bool;
        /// Ships retreated from `system` to `destination`. String: `combat`.
        each bastion_note_retreat(cx: &mut Cx<'_>, system: System, destination: System, owner: Seat, ships: &[Piece]) -> ();
        /// Clear the retreat marks of `system`. String: `combat`.
        each bastion_clear_retreated(cx: &mut Cx<'_>, system: System) -> ();
        /// Adjust the spots where produced ships may be placed (the list is edited in place). String: `production`.
        each bastion_adjust_ship_spots(s: &CoreState, k: &Kernel, who: Seat, producing: System, produced: &[Unit], spots: &mut Systems) -> ();
        /// A ship was placed by production. String: `production`.
        each bastion_ship_placed(cx: &mut Cx<'_>, who: Seat, producing: System, target: System, unit: Unit) -> ();
        /// Resource bonus of a Bastion planet. String: `production` (2).
        call bastion_units_resource_bonus(s: &CoreState, planet: Planet) -> i64;
        /// Bombardment hits cancelled on a planet. String: `invasion` (2).
        call bastion_units_cancel_bombardment_hits(s: &CoreState, victim: Seat, system: System, planet: Planet, hits: usize) -> usize;
        /// Hit groups cancelled (edited in place). String: `invasion` (2).
        each bastion_units_cancel_in_groups(s: &CoreState, victim: Seat, system: System, planet: Planet, groups: &mut SmallList<usize, 8>) -> ();
        /// Whether Bastion watches this ground round. String: `invasion`.
        call bastion_units_watches_ground_round(s: &CoreState, sides: &[Seat]) -> bool;
        /// Non-home systems with planets the player controls (Bastion objective count). String: `choice`.
        call bastion_units_non_home_systems_with_planets(s: &CoreState, k: &Kernel, who: Seat) -> usize;

        // ---- Borrowed agents -------------------------------------------------------------------------
        /// Extra dice from a borrowed round agent. String: `combat`, `invasion`.
        call borrowed_round_agents_extra_die_for(s: &CoreState, k: &Kernel, system: System, planet: Option<Planet>, who: Seat, unit: Unit, unit_index: usize, round_seq: u32) -> i64;
        /// A copy of the round agent is held. String: `combat`, `invasion` (2 each).
        call borrowed_round_agents_has_round_copy(s: &CoreState, k: &Kernel, source_agent: Leader) -> bool;
        /// The owner holds a round agent usable in space. String: `combat`.
        call borrowed_round_agents_has_owner_space_round_agent(s: &CoreState) -> bool;
        /// The owner holds a round agent. String: `invasion`.
        call borrowed_round_agents_has_owner_round_agent(s: &CoreState) -> bool;
        /// Timing abilities of the borrowed round agents. String: `reactions`.
        concat borrowed_round_agents_abilities(s: &CoreState) -> super::AbilityList;
        /// Planets of the Firmament a borrowed commander counts. String: `secrets`.
        concat borrowed_commanders_b_firmament_planets(s: &CoreState, k: &Kernel, who: Seat) -> Planets;

        // ---- Crimson ---------------------------------------------------------------------------------
        /// Sorrow breaches currently active. String: `choice` (map facts).
        concat crimson_active_breaches(s: &CoreState) -> Systems;
        /// The owner's abilities are lost in `system` (breach). String: `combat` (4), `invasion` (4), `production`.
        call crimson_abilities_lost(s: &CoreState, owner: Seat, system: System) -> bool;
        /// Enforce the sundered state after a change. String: `game`.
        each crimson_enforce_sundered(cx: &mut Cx<'_>) -> ();
        /// The invader may deploy a mech to a planet. String: `game`, `invasion`.
        call crimson_can_deploy(s: &CoreState, k: &Kernel, invader: Seat, system: System) -> bool;
        /// Planets the invader may deploy on, among the landable ones. String: `invasion` (2).
        concat crimson_deploy_planets(s: &CoreState, k: &Kernel, invader: Seat, system: System, landable: &[Planet]) -> Planets;
        /// The invader deployed. String: `invasion`.
        each crimson_note_deployed(cx: &mut Cx<'_>, invader: Seat) -> ();
        /// Place the Sorrow breach at setup. String: `seating`.
        each crimson_place_sorrow_breach(cx: &mut Cx<'_>) -> ();
        /// Offer the card spot among the placement spots. String: `production`.
        each crimson_cards_offer_card_spot(s: &CoreState, who: Seat, spots: &mut Systems) -> ();
        /// Put a unit on the Crimson card. String: `production`.
        each crimson_cards_put_on_card(cx: &mut Cx<'_>, who: Seat, unit: Unit) -> ();

        // ---- Deepwrought -------------------------------------------------------------------------------
        /// Oceans the player holds. String: `action_cards`.
        concat deepwrought_oceans(s: &CoreState, who: Seat) -> Planets;
        /// Oceans spendable as payment. String: `production`.
        concat deepwrought_spendable_oceans(s: &CoreState, who: Seat) -> Planets;
        /// Coexistence began for a player. String: `action_cards`.
        each deepwrought_note_coexistence_began(cx: &mut Cx<'_>, who: Seat) -> ();
        /// Begin coexisting on a planet; false if refused. String: `invasion` (2).
        call deepwrought_begin_coexisting(cx: &mut Cx<'_>, who: Seat, system: System, planet: Planet, taker: Option<Seat>) -> bool;
        /// The research-team window is open for the invader. String: `invasion`.
        call deepwrought_research_team_open(s: &CoreState, k: &Kernel, invader: Seat, system: System, planet: Planet) -> bool;
        /// The research-team window is open for the defender. String: `invasion` (2).
        call deepwrought_research_team_defender_open(s: &CoreState, k: &Kernel, defender: Seat, invader: Seat, system: System, planet: Planet) -> bool;
        /// Open the Carrina research window around a research (`deepwrought_research::open`); the
        /// returned code identifies the window for `settle`. String: `strategy_cards`.
        call deepwrought_research_open(cx: &mut Cx<'_>, researcher: Seat, filter: TechFilter) -> u32;
        /// Settle the window opened above. String: `strategy_cards`.
        each deepwrought_research_settle(cx: &mut Cx<'_>, researcher: Seat, window: u32, finished_cleanly: bool) -> ();
        /// Whether a technology counts for the Carrina window. String: `action_cards` (3), `strategy_cards`.
        call deepwrought_research_counts(k: &Kernel, tech: Technology, filter: TechFilter) -> bool;

        // ---- Firmament / Obsidian ------------------------------------------------------------------------
        /// Seats whose pieces are puppeted by `owner`. String: `choice`.
        concat firmament_puppeted(s: &CoreState, owner: Seat) -> Seats;
        /// Space cannon is silenced for the active player in `system`. String: `combat`.
        call firmament_space_cannon_silenced(s: &CoreState, k: &Kernel, active: Seat, system: System) -> bool;
        /// Viper opens the landing for the invader. String: `invasion` (2).
        call firmament_viper_open(s: &CoreState, invader: Seat, system: System, planet: Planet) -> bool;
        /// Viper opens the landing for the defender. String: `invasion`.
        call firmament_viper_defender_open(s: &CoreState, defender: Seat, invader: Seat, system: System, planet: Planet) -> bool;
        /// Assail bonus in space. String: `combat` (2).
        call firmament_plots_assail_space(s: &CoreState, k: &Kernel, who: Seat, system: Option<System>) -> i64;
        /// Assail bonus against the targets. String: `combat`, `invasion`.
        call firmament_plots_assail_against(s: &CoreState, owner: Seat, targets: &[Seat]) -> i64;
        /// Assail bonus on the ground. String: `invasion`.
        call firmament_plots_assail_ground(s: &CoreState, who: Seat, system: System, planet: Planet) -> i64;
        /// Plot cards still available. String: `objectives`.
        call firmament_remaining_cards(s: &CoreState) -> u32;
        /// Place a plot card; true if placed. String: `objectives` (2).
        call firmament_place_card(cx: &mut Cx<'_>, owner: Seat, card: u16, token: Seat) -> bool;
        /// The plot options for a scoring moment. String: `objectives` (2).
        concat firmament_plot_options(s: &CoreState, k: &Kernel, who: Seat) -> PlotOptions;
        /// Score a secret as a plot; true if scored. String: `objectives`.
        call firmament_score_as_plot(cx: &mut Cx<'_>, who: Seat, secret: SecretObjective, token: Seat) -> bool;

        // ---- Keleres --------------------------------------------------------------------------------------
        /// Run `probe` conceptually on the state with the agent's grant applied: the core exposes the
        /// grant as a pure transformation of a state clone. String: `production` (3), `strategy_cards` (2), `action_cards`.
        call keleres_with_agent_granted(s: &CoreState, grant: AgentGrant) -> Option<CoreState>;
        /// Whether the agent could grant. String: `game`.
        call keleres_agent_could_grant(s: &CoreState, payer: Seat) -> bool;
        /// Offer the agent in a payment window; true if used. String: `production` (3), `strategy_cards`, `game`.
        call keleres_offer_agent(cx: &mut Cx<'_>, payer: Seat) -> bool;
        /// Close the agent window. String: `production` (6), `strategy_cards`, `game`.
        each keleres_close_agent_window(cx: &mut Cx<'_>, payer: Seat) -> ();
        /// Commodities spendable as trade goods. String: `payment` (2), `production` (3).
        call keleres_spendable_commodities(s: &CoreState, who: Seat) -> i64;
        /// Reconcile the Keleres variant after changes. String: `game`.
        each keleres_reconcile(cx: &mut Cx<'_>) -> ();
        /// Take the executive-order slot; the taker. String: `game`.
        call keleres_take_executive_order(cx: &mut Cx<'_>) -> Option<Seat>;
        /// The custodian space cannon on a planet: `(owner, count, hit_on)`. String: `planets`.
        call keleres_custodian_cannon(s: &CoreState, planet: Planet, owner: Seat) -> Option<(Seat, u32, u8)>;
        /// ASN ready / mark bookkeeping. String: `production` (2).
        call keleres_asn_ready(s: &CoreState, who: Seat) -> bool;
        /// Mark ASN used.
        each keleres_asn_mark(cx: &mut Cx<'_>, who: Seat) -> ();
        /// The Keleres units may not be committed to the planet. String: `invasion`.
        call keleres_units_commit_blocked(s: &CoreState, k: &Kernel, invader: Seat, system: System, planet: Planet) -> bool;
        /// Pay the commit toll; false if refused (asks). String: `invasion` (3).
        call keleres_units_pay_commit_toll(cx: &mut Cx<'_>, invader: Seat, system: System, planet: Planet) -> bool;

        // ---- Mahact ---------------------------------------------------------------------------------------
        /// Owners whose fleet pools Mahact draws on. String: `choice`, `game` (2), `invasion`.
        concat mahact_fleet_pool_owners(s: &CoreState, mahact: Seat) -> Seats;
        /// Foreign command tokens in the Mahact pool. String: `choice`, `fleet`.
        call mahact_foreign_tokens(s: &CoreState, who: Seat) -> i32;
        /// Imperia grants a commander ability. String: `promissory`.
        call mahact_imperia_grants(s: &CoreState, who: Seat, commander: Leader) -> bool;
        /// Whether the player is Mahact. String: `promissory`.
        call mahact_is_mahact(s: &CoreState, who: Seat) -> bool;
        /// Whether the faction's promissory purges the alliance. String: `promissory`.
        call mahact_purges_alliance(faction: Faction) -> bool;
        /// Flagship roll bonus. String: `combat`.
        call mahact_units_flagship_roll_bonus(s: &CoreState, k: &Kernel, who: Seat, unit: Unit) -> i64;
        /// Ships may not move while the Starlancer bars them. String: `combat` (2), `game`.
        call mahact_units_ship_movement_barred(s: &CoreState) -> bool;
        /// Offer the Starlancer; the seat that took it. String: `game`.
        call mahact_units_offer_starlancer(cx: &mut Cx<'_>, system: System, active: Seat) -> Option<Seat>;

        // ---- Naalu / Nekro / Nomad / Ralnel ----------------------------------------------------------------
        /// Naalu commander lets the player peek. String: `choice`.
        call naalu_commander_has_peek(s: &CoreState, who: Seat) -> bool;
        /// Nekro units that Alastor covers on the planet. String: `combat` (2).
        concat nekro_units_alastor_participants(s: &CoreState, who: Seat, system: System) -> SmallList<(Planet, u16), 4>;
        /// Alastor covers the unit. String: `combat`.
        call nekro_units_alastor_covers(s: &CoreState, system: System, owner: Seat, unit: Unit) -> bool;
        /// Alastor spent a cover on a planet. String: `combat`.
        each nekro_units_alastor_decrement(cx: &mut Cx<'_>, system: System, owner: Seat, planet: Planet, unit: Unit) -> ();
        /// Alastor covers cleared. String: `combat` (2).
        each nekro_units_alastor_clear(cx: &mut Cx<'_>, system: System, who: Option<Seat>) -> ();
        /// Mech roll bonus. String: `combat`, `invasion`.
        call nekro_units_mech_roll_bonus(s: &CoreState, k: &Kernel, who: Seat, unit: Unit, system: Option<System>, planet: Option<Planet>) -> i64;
        /// Players waiting on a Nekro propagation. String: `game`.
        concat nekro_pending_propagation(s: &CoreState) -> Seats;
        /// Whether the Nomad wants the agent-exhaustion event. String: `leaders`.
        call nomad_watches_agent_exhaustion(s: &CoreState) -> bool;
        /// A Nomad agent was readied. String: `leaders` (2).
        each nomad_agents_agent_readied(cx: &mut Cx<'_>, who: Seat, leader: Leader) -> ();
        /// Nomad agent watches the roll step. String: `combat`, `invasion`.
        call nomad_agents_watches_roll_step(s: &CoreState) -> bool;
        /// Ralnel silenced a unit in space. String: `combat` (2), `production`.
        call ralnel_silenced_in_space(s: &CoreState, k: &Kernel, piece: Piece) -> bool;
        /// Ralnel linkship guns firing in the system; those that may fire. String: `combat`.
        concat ralnel_linkship_guns(s: &CoreState, k: &Kernel, system: System) -> Pieces;
        /// A Ralnel flagship is among the arrived ships. String: `combat`.
        call ralnel_flagship_among(s: &CoreState, who: Seat, arrived: &[Unit]) -> bool;
        /// Ralnel alarum is on for a planet. String: `invasion`.
        call ralnel_alarum_on(s: &CoreState, system: System, planet: Planet) -> bool;
        /// Ralnel skimmer takes the discarded card. String: `action_cards`.
        call ralnel_cards_skimmer_takes(cx: &mut Cx<'_>, discarder: Seat, card: ActionCard) -> bool;
        /// Who listens for the end of a tactical action (state only / with content). String: `game` (2).
        call obsidian_listens_for_tactical_end(s: &CoreState) -> bool;
        /// Ralnel listens for the end of a tactical action.
        call ralnel_listens_for_tactical_end(s: &CoreState, k: &Kernel) -> bool;

        // ---- generic, combat context ------------------------------------------------------------------------
        /// Whether the combat context (space or ground) is silenced for a unit. Reserved for C shards.
        call context_silenced(s: &CoreState, k: &Kernel, piece: Piece, ctx: CombatCtx) -> bool;
    }
}
