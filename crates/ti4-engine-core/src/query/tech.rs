//! Technology queries (`technology.rs`, `synergy.rs`, `starting_tech.rs`, `entropic_scars.rs`).
//!
//! Package A3. Two layers:
//!
//! * **Printed data**, no state: colour, unit upgrade, faction, replaced upgrades, prerequisites,
//!   the active deck. These replace the content-store lookups by alias string of the string engine
//!   (`colour_type`, `is_unit_upgrade`, `faction_of`, `replaced_for_faction`, `prerequisites`,
//!   `active_aliases`) with a table row of the generated [`Technology`] data.
//! * **Position queries** over `&CoreState`: owned colours, specialties, synergy, prerequisites met,
//!   `can_research`, `researchable`, the unit form a seat is entitled to, scars, faction technologies.
//!
//! What the core cannot see is passed in, like in A1: the source [`Scope`], the "laws are blank"
//! flag, and the answers of faction hooks ([`ResearchHooks`]); the hook table (A5) replaces the
//! trait implementations. Hook points still on the string side: `deepwrought::technology_purged`
//! (a faction mark), `faction_abilities::waived_prerequisites`,
//! `hooks_strategy::extra_prerequisite_colours` (Yin commander), `payment::affordable` for
//! Inheritance Systems, `hooks_strategy::research_waiver_offers`, `unit_form_override` (Mentak).

use crate::containers::Set;
use crate::generated::{
    Agenda, Attachment, Faction, FactionTag, Planet, Relic, System, TechType, Technology, Unit, UnitBaseType,
    Requirement, Source,
};
use crate::ids::Seat;
use crate::state::TechSet;
use crate::state::CoreState;
use crate::tables::{Scope, source_bit};

use super::{board, laws};

/// `ti4_model::state::RULES_TECH_EFFECTS` (checked equal by a bridge test).
pub const RULES_TECH_EFFECTS: u32 = 15;

/// The four research tracks, in the order of `technology::COLOURS` (`BIOTIC`, `CYBERNETIC`,
/// `PROPULSION`, `WARFARE`; also the key order of the string engine's colour maps).
pub const COLOURS: [TechType; 4] = [TechType::Biotic, TechType::Cybernetic, TechType::Propulsion, TechType::Warfare];

/// A count per colour, indexed like [`COLOURS`] (the string `BTreeMap<&str, usize>`, zeros absent).
pub type Colours = [u16; 4];

/// A set of colours, indexed like [`COLOURS`].
pub type ColourSet = [bool; 4];

/// The index of a research colour; `None` for unit upgrades.
#[must_use]
pub const fn colour_index(t: TechType) -> Option<usize> {
    match t {
        TechType::Biotic => Some(0),
        TechType::Cybernetic => Some(1),
        TechType::Propulsion => Some(2),
        TechType::Warfare => Some(3),
        TechType::Unitupgrade => None,
    }
}

// --- printed data ------------------------------------------------------------------------------

/// `technology::colour_type`: the first track among the technology's types.
#[must_use]
pub fn colour_type(tech: Technology) -> Option<TechType> {
    let types = tech.data().types;
    COLOURS.into_iter().find(|c| types.contains(c))
}

/// `technology::is_unit_upgrade`.
#[must_use]
pub fn is_unit_upgrade(tech: Technology) -> bool {
    tech.data().types.contains(&TechType::Unitupgrade)
}

/// `technology::faction_of`.
#[must_use]
pub fn faction_of(tech: Technology) -> Option<FactionTag> {
    tech.data().faction
}

/// Whether the tag is the id of exactly this faction (the string engine compares `faction == id`;
/// a family tag such as `keleres` is never equal to a faction id).
#[must_use]
pub fn tag_is(tag: FactionTag, faction: Faction) -> bool {
    matches!(tag.factions(), [only] if *only == faction)
}

/// `technology::replaced_for_faction`: the faction owns an upgrade whose `baseUpgrade` is `tech`.
#[must_use]
pub fn replaced_for_faction(faction: Faction, tech: Technology) -> bool {
    Technology::ALL.iter().any(|t| {
        let d = t.data();
        d.base_upgrade == Some(tech) && d.faction.is_some_and(|tag| tag_is(tag, faction))
    })
}

/// `technology::prerequisites`: the counts per colour.
#[must_use]
pub fn prerequisites(tech: Technology) -> Colours {
    let p = tech.data().prerequisites;
    [u16::from(p.green), u16::from(p.yellow), u16::from(p.blue), u16::from(p.red)]
}

/// The cards `techs_pok_c4` leaves out although they are not Thunder's Edge cards: the replaced
/// printings of Magen Defense Grid and X-89, and the two Nekro cards of the old printing. (A bridge
/// test compares the result with the deck.)
const REPLACED_PRINTINGS: [Technology; 6] = [
    Technology::MagenDefenseGrid,
    Technology::MagenDefenseGridOmega,
    Technology::ERRORERROR,
    Technology::NULLREFERENCE,
    Technology::X89BacterialWeaponOmega,
    Technology::X89BacterialWeapon,
];

/// Whether the PoK + Codex deck (`techs_pok_c4`) holds the card: every card but the replaced
/// printings and the Thunder's Edge faction cards (Executive Order is printed as one but is in it).
#[must_use]
pub fn in_deck(tech: Technology) -> bool {
    !REPLACED_PRINTINGS.contains(&tech) && (tech.data().source != Source::ThundersEdge || tech == Technology::ExecutiveOrder)
}

/// `technology::active_aliases`: the authoritative deck, plus the Thunder's Edge cards when that
/// expansion is in the game.
#[must_use]
pub fn active(scope: Scope) -> TechSet {
    let te = scope.0 & source_bit(Source::ThundersEdge) != 0;
    Technology::ALL.into_iter().filter(|t| in_deck(*t) || (te && t.data().source == Source::ThundersEdge)).collect()
}

/// The technology that is not in the deck of any seat once played out: the War Sun technology
/// (`laws::is_war_sun_technology`).
const WAR_SUN: Technology = Technology::WarSun;

// --- state: ownership --------------------------------------------------------------------------

/// `technology::owns_technology`.
#[must_use]
pub fn owns(core: &CoreState, seat: Seat, tech: Technology) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(tech))
}

/// The seat's owned technologies.
#[must_use]
pub fn owned(core: &CoreState, seat: Seat) -> TechSet {
    core.players().by_seat(seat).map(|p| p.tech.owned).unwrap_or_default()
}

/// Owned and not exhausted (the "ready" test of `inheritance_systems_ready`, `aida_waiver`).
#[must_use]
pub fn ready(core: &CoreState, seat: Seat, tech: Technology) -> bool {
    core.players().by_seat(seat).is_some_and(|p| p.tech.owned.contains(tech) && !p.tech.exhausted.contains(tech))
}

/// `technology::owned_colours`.
#[must_use]
pub fn owned_colours(core: &CoreState, seat: Seat) -> Colours {
    colours_of(&owned(core, seat))
}

/// The colour counts of a set of technologies.
#[must_use]
pub fn colours_of(set: &TechSet) -> Colours {
    let mut held = [0u16; 4];
    for t in set.iter() {
        if let Some(i) = colour_type(t).and_then(colour_index) {
            held[i] += 1;
        }
    }
    held
}

/// `technology::psychoarchaeology_frees_specialties`.
#[must_use]
pub fn psychoarchaeology_frees_specialties(core: &CoreState, seat: Seat) -> bool {
    core.meta().rules_at_least(RULES_TECH_EFFECTS) && owns(core, seat, Technology::Psychoarchaeology)
}

// --- state: specialties and synergy ------------------------------------------------------------

/// `planets::tech_specialties_now`: the specialties printed on the planet (when its source is in
/// scope) and those its attachments give, each once, printed first.
#[must_use]
pub fn specialties_now(core: &CoreState, scope: Scope, planet: Planet) -> crate::containers::SmallList<TechType, 4> {
    specialties_of(scope, planet, board::attachments(core, planet).as_slice().iter().copied())
}

/// [`specialties_now`] for a caller that holds the planet's attachments itself.
#[must_use]
pub fn specialties_of(
    scope: Scope,
    planet: Planet,
    attachments: impl IntoIterator<Item = Attachment>,
) -> crate::containers::SmallList<TechType, 4> {
    let mut found = crate::containers::SmallList::new();
    let data = planet.data();
    if scope.0 & source_bit(data.source) != 0 {
        for s in data.specialties {
            if !found.as_slice().contains(s) {
                found.push(*s);
            }
        }
    }
    for a in attachments {
        for s in a.data().specialties {
            if !found.as_slice().contains(s) {
                found.push(*s);
            }
        }
    }
    found
}

/// `technology::specialties`: specialties on the planets the seat controls, by colour. An exhausted
/// planet counts only when Psychoarchaeology frees the specialties.
#[must_use]
pub fn specialties(core: &CoreState, scope: Scope, seat: Seat) -> Colours {
    let mut found = [0u16; 4];
    let free = psychoarchaeology_frees_specialties(core, seat);
    for (_, planet) in board::controlled_planets(core, seat).as_slice() {
        if !free && board::exhausted(core, *planet) {
            continue;
        }
        for s in specialties_now(core, scope, *planet).as_slice() {
            if let Some(i) = colour_index(*s) {
                found[i] += 1;
            }
        }
    }
    found
}

/// `breakthroughs::specialty_research_planets` without the held-breakthrough test: the readied
/// controlled planets with a specialty, one entry per colour, in controlled order.
#[must_use]
pub fn readied_specialty_planets(
    core: &CoreState,
    scope: Scope,
    seat: Seat,
) -> crate::containers::SmallList<(Planet, TechType), 8> {
    let mut found = crate::containers::SmallList::new();
    for (_, planet) in board::controlled_planets(core, seat).as_slice() {
        if board::exhausted(core, *planet) {
            continue;
        }
        for s in specialties_now(core, scope, *planet).as_slice() {
            if colour_index(*s).is_some() {
                found.push((*planet, *s));
            }
        }
    }
    found
}

/// `synergy::joined`: the colours the seat's synergy pools (the Quantumcore: all four; else the two
/// colours of the breakthrough, when its source is in scope).
#[must_use]
pub fn synergy_joined(core: &CoreState, scope: Scope, seat: Seat) -> ColourSet {
    let mut joined = [false; 4];
    let Some(p) = core.players().by_seat(seat) else {
        return joined;
    };
    if p.cards.relics.as_slice().contains(&Relic::TheQuantumcore) {
        return [true; 4];
    }
    if let Some(b) = p.cards.breakthrough {
        let d = b.data();
        if scope.0 & source_bit(d.source) != 0
            && let [first, second] = d.synergy
        {
            for c in [first, second] {
                if let Some(i) = colour_index(*c) {
                    joined[i] = true;
                }
            }
        }
    }
    joined
}

/// `synergy::satisfies`: whether `holdings` meet `needs` with the `joined` colours pooled
/// (rules 2, 3, 4, 6) and `waivable` prerequisites to ignore across the whole requirement.
#[must_use]
pub fn satisfies(needs: Colours, holdings: Colours, joined: ColourSet, mut waivable: usize) -> bool {
    if joined.iter().any(|j| *j) {
        let wanted: usize = (0..4).filter(|i| joined[*i] && needs[*i] > 0).map(|i| usize::from(needs[i])).sum();
        if wanted > 0 {
            let pooled: usize = (0..4).filter(|i| joined[*i]).map(|i| usize::from(holdings[i])).sum();
            if pooled < wanted {
                let short = wanted - pooled;
                if waivable < short {
                    return false;
                }
                waivable -= short;
            }
        }
    }
    for i in 0..4 {
        let need = usize::from(needs[i]);
        if need == 0 || joined[i] {
            continue;
        }
        let have = usize::from(holdings[i]);
        if have >= need {
            continue;
        }
        let short = need - have;
        if waivable >= short {
            waivable -= short;
        } else {
            return false;
        }
    }
    true
}

// --- state: research ---------------------------------------------------------------------------

/// The answers of the faction hooks to the research questions. The string engine implements them
/// from its faction modules; the hook table (A5) replaces that. Every method is a pure question of
/// the position the trait object was built for.
pub trait ResearchHooks {
    /// `deepwrought::technology_purged`: Ta Zern purged the card from every deck.
    fn purged(&self, tech: Technology) -> bool;
    /// `faction_abilities::waived_prerequisites`: prerequisite slots a faction ability ignores.
    fn waived_prerequisites(&self, tech: Technology) -> usize;
    /// `hooks_strategy::extra_prerequisite_colours`: holdings a faction effect adds (Yin commander).
    fn extra_colours(&self) -> Colours;
    /// Whether 2 resources are affordable (`payment::affordable`, Inheritance Systems).
    fn inheritance_affordable(&self) -> bool;
    /// `hooks_strategy::research_waiver_offers` is non-empty for the technology.
    fn waiver_offer(&self, tech: Technology) -> bool;
}

/// No faction hook speaks (a position without faction effects; tests).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoHooks;

impl ResearchHooks for NoHooks {
    fn purged(&self, _: Technology) -> bool {
        false
    }
    fn waived_prerequisites(&self, _: Technology) -> usize {
        0
    }
    fn extra_colours(&self) -> Colours {
        [0; 4]
    }
    fn inheritance_affordable(&self) -> bool {
        false
    }
    fn waiver_offer(&self, _: Technology) -> bool {
        false
    }
}

/// What Inheritance Systems charges to ignore a technology's prerequisites.
pub const INHERITANCE_SYSTEMS_COST: i64 = 2;

/// `laws::war_sun_prerequisites_waived`: Publicize Weapon Schematics is in play, the technology is
/// the war sun and somebody owns it.
#[must_use]
pub fn war_sun_prerequisites_waived(core: &CoreState, blanked: bool, tech: Technology) -> bool {
    tech == WAR_SUN
        && laws::active(core, blanked, Agenda::PublicizeWeaponSchematics)
        && core.players().iter().any(|p| p.tech.owned.contains(WAR_SUN))
}

/// `technology::aida_waiver`.
#[must_use]
pub fn aida_waiver(core: &CoreState, seat: Seat, tech: Technology) -> usize {
    usize::from(
        core.meta().rules_at_least(RULES_TECH_EFFECTS)
            && is_unit_upgrade(tech)
            && ready(core, seat, Technology::AIDevelopmentAlgorithm),
    )
}

/// `relics::prerequisite_waivers`: the Prophet's Tears.
#[must_use]
pub fn relic_waivers(core: &CoreState, seat: Seat) -> usize {
    usize::from(
        core.players().by_seat(seat).is_some_and(|p| p.cards.relics.as_slice().contains(&Relic::TheProphetsTears)),
    )
}

/// `technology::inheritance_systems_ready`.
#[must_use]
pub fn inheritance_ready(core: &CoreState, seat: Seat, hooks: &dyn ResearchHooks) -> bool {
    ready(core, seat, Technology::InheritanceSystems) && hooks.inheritance_affordable()
}

/// Everything the research questions ask of a seat that does not depend on the technology asked
/// about, computed once per seat: `researchable` asks about a hundred technologies, and the string
/// engine recomputed owned colours, specialties and waivers for each one.
///
/// The fields are public so that a caller that has the same facts in another shape (the string
/// engine's `GameState`, until the flip) can build it without a [`CoreState`].
#[derive(Debug, Clone, Copy)]
pub struct ResearchCtx {
    pub faction: Faction,
    pub owned: TechSet,
    /// Owned colours + controlled specialties + the hooks' extra holdings.
    pub holdings: Colours,
    /// Research Team laws + the Prophet's Tears: waivers that do not depend on the technology.
    pub waivable: usize,
    pub joined: ColourSet,
    /// AI Development Algorithm is owned, ready, and its rules version has begun.
    pub aida_ready: bool,
    /// Inheritance Systems is owned and ready (its 2 resources are asked of the hooks when needed).
    pub inheritance_card: bool,
    /// Publicize Weapon Schematics is in play and somebody owns the war sun technology.
    pub war_sun_waived: bool,
}

impl ResearchCtx {
    /// The context of `seat` in `core`; `None` for a seat without a player.
    #[must_use]
    pub fn of(core: &CoreState, scope: Scope, blanked: bool, seat: Seat, hooks: &dyn ResearchHooks) -> Option<Self> {
        let p = core.players().by_seat(seat)?;
        let held = owned_colours(core, seat);
        let spec = specialties(core, scope, seat);
        let extra = hooks.extra_colours();
        let mut holdings = [0u16; 4];
        for i in 0..4 {
            holdings[i] = held[i] + spec[i] + extra[i];
        }
        let mut waivable = relic_waivers(core, seat);
        for colour in COLOURS {
            waivable += laws::research_team_waivers(core, blanked, seat, colour);
        }
        Some(Self {
            faction: p.faction,
            owned: p.tech.owned,
            holdings,
            waivable,
            joined: synergy_joined(core, scope, seat),
            aida_ready: core.meta().rules_at_least(RULES_TECH_EFFECTS) && ready(core, seat, Technology::AIDevelopmentAlgorithm),
            inheritance_card: ready(core, seat, Technology::InheritanceSystems),
            war_sun_waived: war_sun_prerequisites_waived(core, blanked, WAR_SUN),
        })
    }

    /// `technology::prerequisites_met_with`.
    #[must_use]
    pub fn prerequisites_met(&self, tech: Technology, hooks: &dyn ResearchHooks, with_aida: bool) -> bool {
        let mut waivable = hooks.waived_prerequisites(tech) + self.waivable;
        if with_aida && self.aida_ready && is_unit_upgrade(tech) {
            waivable += 1;
        }
        if tech == WAR_SUN && self.war_sun_waived {
            return true;
        }
        satisfies(prerequisites(tech), self.holdings, self.joined, waivable)
    }

    /// `technology::can_research`.
    #[must_use]
    pub fn can_research(&self, tech: Technology, hooks: &dyn ResearchHooks) -> bool {
        if self.owned.contains(tech) || hooks.purged(tech) || !open_to_faction(self.faction, tech) {
            return false;
        }
        self.prerequisites_met(tech, hooks, true)
            || (self.inheritance_card && hooks.inheritance_affordable())
            || hooks.waiver_offer(tech)
    }

    /// `technology::researchable`: the active technologies this seat can research, in id order.
    #[must_use]
    pub fn researchable(&self, scope: Scope, hooks: &dyn ResearchHooks) -> TechSet {
        active(scope).iter().filter(|t| self.can_research(*t, hooks)).collect()
    }

    /// `technology::researchable_by_prerequisites`.
    #[must_use]
    pub fn researchable_by_prerequisites(&self, scope: Scope, hooks: &dyn ResearchHooks) -> TechSet {
        active(scope)
            .iter()
            .filter(|t| self.can_research(*t, hooks) && self.prerequisites_met(*t, hooks, true))
            .collect()
    }
}

/// `technology::prerequisites_met` (every standing waiver counted).
#[must_use]
pub fn prerequisites_met(
    core: &CoreState,
    scope: Scope,
    blanked: bool,
    seat: Seat,
    tech: Technology,
    hooks: &dyn ResearchHooks,
) -> bool {
    ResearchCtx::of(core, scope, blanked, seat, hooks).is_some_and(|c| c.prerequisites_met(tech, hooks, true))
}

/// `technology::prerequisites_met_with(.., false)`: AI Development Algorithm's waiver left out.
#[must_use]
pub fn prerequisites_met_without_aida(
    core: &CoreState,
    scope: Scope,
    blanked: bool,
    seat: Seat,
    tech: Technology,
    hooks: &dyn ResearchHooks,
) -> bool {
    ResearchCtx::of(core, scope, blanked, seat, hooks).is_some_and(|c| c.prerequisites_met(tech, hooks, false))
}

/// The part of `technology::can_research` that depends on no position: researchable by its text,
/// the faction's own if it is a faction technology, and not replaced by one of the faction's.
#[must_use]
pub fn open_to_faction(faction: Faction, tech: Technology) -> bool {
    let d = tech.data();
    d.researchable && d.faction.is_none_or(|tag| tag_is(tag, faction)) && !replaced_for_faction(faction, tech)
}

/// `technology::can_research`.
#[must_use]
pub fn can_research(
    core: &CoreState,
    scope: Scope,
    blanked: bool,
    seat: Seat,
    tech: Technology,
    hooks: &dyn ResearchHooks,
) -> bool {
    ResearchCtx::of(core, scope, blanked, seat, hooks).is_some_and(|c| c.can_research(tech, hooks))
}

/// `technology::researchable`: every active technology the seat can research, in id order.
#[must_use]
pub fn researchable(core: &CoreState, scope: Scope, blanked: bool, seat: Seat, hooks: &dyn ResearchHooks) -> TechSet {
    ResearchCtx::of(core, scope, blanked, seat, hooks).map(|c| c.researchable(scope, hooks)).unwrap_or_default()
}

/// `technology::researchable_by_prerequisites`.
#[must_use]
pub fn researchable_by_prerequisites(
    core: &CoreState,
    scope: Scope,
    blanked: bool,
    seat: Seat,
    hooks: &dyn ResearchHooks,
) -> TechSet {
    ResearchCtx::of(core, scope, blanked, seat, hooks)
        .map(|c| c.researchable_by_prerequisites(scope, hooks))
        .unwrap_or_default()
}

// --- unit upgrades -----------------------------------------------------------------------------

fn unit_in_scope(scope: Scope, unit: Unit) -> Option<&'static crate::generated::UnitData> {
    scope.unit(unit)
}

/// `ti4_content::units::unlocked_upgrade`: the first unit (in id order) of the base type, generic
/// or this faction's own, whose required technology is held.
#[must_use]
pub fn unlocked_upgrade(scope: Scope, base: UnitBaseType, faction: Faction, held: &TechSet) -> Option<Unit> {
    Unit::ALL.into_iter().find(|u| {
        unit_in_scope(scope, *u).is_some_and(|d| {
            d.base_type == base
                && d.faction.is_none_or(|tag| tag_is(tag, faction))
                && matches!(d.required_technology, Requirement::Known(t) if held.contains(t))
        })
    })
}

/// A faction's own unit per base type as the source scope resolves it (`ti4_content::units::
/// faction_unit`). The roster ids go through the store's reprint rules (`resolve_id`: a newer
/// printing in scope wins, an out-of-scope one falls back to the id without its suffix), which
/// work on id text; the adapter resolves them once per scope and hands the result in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitRoster {
    forms: [[Option<Unit>; UnitBaseType::COUNT]; Faction::COUNT],
}

impl Default for UnitRoster {
    fn default() -> Self {
        Self { forms: [[None; UnitBaseType::COUNT]; Faction::COUNT] }
    }
}

impl UnitRoster {
    pub fn set(&mut self, faction: Faction, base: UnitBaseType, unit: Option<Unit>) {
        self.forms[faction.index()][base as usize] = unit;
    }

    #[must_use]
    pub fn get(&self, faction: Faction, base: UnitBaseType) -> Option<Unit> {
        self.forms[faction.index()][base as usize]
    }
}

/// `ti4_content::units::faction_unit`.
#[must_use]
pub fn faction_unit(roster: &UnitRoster, faction: Faction, base: UnitBaseType) -> Option<Unit> {
    roster.get(faction, base)
}

/// The plain unit of the base type: the one whose corpus id is the id of the base type
/// (`catalogue.get(base_type)`; a bridge test checks the table against the ids).
#[must_use]
pub fn generic_unit(scope: Scope, base: UnitBaseType) -> Option<Unit> {
    let unit = match base {
        UnitBaseType::Carrier => Unit::CarrierI,
        UnitBaseType::Cruiser => Unit::CruiserI,
        UnitBaseType::Destroyer => Unit::DestroyerI,
        UnitBaseType::Dreadnought => Unit::DreadnoughtI,
        UnitBaseType::Fighter => Unit::FighterI,
        UnitBaseType::Flagship => Unit::FactionlessFlagship,
        UnitBaseType::Infantry => Unit::InfantryI,
        UnitBaseType::Mech => Unit::FactionlessMech,
        UnitBaseType::Pds => Unit::PDSI,
        UnitBaseType::Spacedock => Unit::SpaceDockI,
        UnitBaseType::Warsun => Unit::WarSun,
    };
    unit_in_scope(scope, unit).map(|_| unit)
}

/// `action_cards::placed_unit_id` without the faction override (`unit_form_override`): the form a
/// seat is entitled to for a base type: the upgrade it owns, else its faction's version, else the
/// plain one.
#[must_use]
pub fn unit_form(core: &CoreState, scope: Scope, roster: &UnitRoster, seat: Seat, base: UnitBaseType) -> Option<Unit> {
    let p = core.players().by_seat(seat)?;
    unlocked_upgrade(scope, base, p.faction, &p.tech.owned)
        .or_else(|| faction_unit(roster, p.faction, base))
        .or_else(|| generic_unit(scope, base))
}

// --- scars, faction technologies, starting technologies ----------------------------------------

/// `entropic_scars::is_scar`.
#[must_use]
pub fn is_scar(scope: Scope, system: System) -> bool {
    scope.system(system).is_some_and(|d| d.anomalies.scar)
}

/// `entropic_scars::unowned_faction_technologies` (a set; the string list is in file order).
#[must_use]
pub fn unowned_faction_technologies(core: &CoreState, scope: Scope, seat: Seat) -> TechSet {
    core.players().by_seat(seat).map(|p| unowned_faction_technologies_of(scope, p.faction, &p.tech.owned)).unwrap_or_default()
}

/// [`unowned_faction_technologies`] for a caller that holds the faction and the owned set itself.
#[must_use]
pub fn unowned_faction_technologies_of(scope: Scope, faction: Faction, owned: &TechSet) -> TechSet {
    Technology::ALL
        .into_iter()
        .filter(|t| {
            let d = t.data();
            scope.0 & source_bit(d.source) != 0 && d.faction.is_some_and(|tag| tag_is(tag, faction)) && !owned.contains(*t)
        })
        .collect()
}
