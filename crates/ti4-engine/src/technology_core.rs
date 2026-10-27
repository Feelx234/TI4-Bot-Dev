//! The adapter half of package A3 (docs/ENGINE_CORE_STATE.md 4.4): the typed technology queries
//! of `ti4_engine_core::query::tech` for the string engine's callers.
//!
//! * **Printed data** (colour, unit upgrade, faction, prerequisites, the active deck): a table row
//!   of the generated [`Technology`] data instead of a content-store lookup by alias string.
//! * **Position queries** (`can_research`, `researchable`, prerequisites): the seat's facts are read
//!   from the `GameState` once into a [`ResearchCtx`]; the technology-dependent part is the typed
//!   core code. The faction hooks stay string code behind [`StringHooks`] until the hook table (A5).
//! * [`View`]: the same questions answered by the core over a lent `CoreState` (one per position),
//!   which is what `Dual` compares in the bridge gate (`core_equivalence tech`).
//!
//! Every function returns `None` when the core cannot express the question (an id the tables do
//! not know); the caller answers with the string engine, exactly as `core_adapter` does.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_engine_core::generated::{
    Attachment, Breakthrough, Faction, FactionTag, Planet, Source as CoreSource, TechType, Technology, UnitBaseType,
};
use ti4_engine_core::query::tech::{self as q, Colours, ResearchCtx, ResearchHooks};
use ti4_engine_core::state::TechSet;
use ti4_engine_core::tables::{Scope, source_bit};
use ti4_model::content_types::{Source, SourceSet};
use ti4_model::id::{PlayerId, TechnologyId};
use ti4_model::state::GameState;

use crate::generated::IdStr;

/// The typed technology of an alias.
#[inline]
#[must_use]
pub fn tech(alias: &str) -> Option<Technology> {
    <Technology as IdStr>::from_id(alias)
}

/// The scope mask of a source set (`core_adapter::scope` without the text round trip).
#[must_use]
pub fn scope(sources: SourceSet) -> Scope {
    Scope(sources.iter().fold(0, |mask, s| {
        mask | source_bit(match s {
            Source::Base => CoreSource::Base,
            Source::Codex1 => CoreSource::Codex1,
            Source::Codex2 => CoreSource::Codex2,
            Source::Codex3 => CoreSource::Codex3,
            Source::Codex4 => CoreSource::Codex4,
            Source::Pok => CoreSource::Pok,
            Source::ThundersEdge => CoreSource::ThundersEdge,
        })
    }))
}

/// The corpus spelling of a research track (`technology::COLOURS`).
#[must_use]
pub fn colour_name(t: TechType) -> &'static str {
    <TechType as IdStr>::as_str(t)
}

/// The colour map of the string engine (`BTreeMap<&str, usize>`, zeros absent).
#[must_use]
pub fn colour_map(counts: Colours) -> BTreeMap<&'static str, usize> {
    q::COLOURS
        .into_iter()
        .zip(counts)
        .filter(|(_, n)| *n > 0)
        .map(|(c, n)| (colour_name(c), usize::from(n)))
        .collect()
}

/// A colour name of the string engine (`BIOTIC` ...) as a track.
#[must_use]
pub fn track(name: &str) -> Option<usize> {
    <TechType as IdStr>::from_id(name).and_then(q::colour_index)
}

/// Ids of a typed set, in id order (the order of the string engine's sorted `Vec<TechnologyId>`).
#[must_use]
pub fn ids(set: &TechSet) -> Vec<TechnologyId> {
    set.iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- printed data ------------------------------------------------------------------------------

#[must_use]
pub fn colour_type(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::colour_type(tech(alias.as_str())?).map(colour_name))
}

#[must_use]
pub fn is_unit_upgrade(alias: &TechnologyId) -> Option<bool> {
    Some(q::is_unit_upgrade(tech(alias.as_str())?))
}

#[must_use]
pub fn faction_of(alias: &TechnologyId) -> Option<Option<&'static str>> {
    Some(q::faction_of(tech(alias.as_str())?).map(<FactionTag as IdStr>::as_str))
}

#[must_use]
pub fn replaced_for_faction(faction: &str, alias: &TechnologyId) -> Option<bool> {
    let faction = <Faction as IdStr>::from_id(faction)?;
    Some(q::replaced_for_faction(faction, tech(alias.as_str())?))
}

#[must_use]
pub fn prerequisites(alias: &TechnologyId) -> Option<BTreeMap<&'static str, usize>> {
    Some(colour_map(q::prerequisites(tech(alias.as_str())?)))
}

#[must_use]
pub fn active_aliases(sources: SourceSet) -> std::collections::BTreeSet<TechnologyId> {
    q::active(scope(sources)).iter().map(|t| TechnologyId::new(<Technology as IdStr>::as_str(t))).collect()
}

// --- the string state as a research context ----------------------------------------------------

/// The faction hooks of the research questions, answered by the string engine's faction modules.
pub struct StringHooks<'a> {
    pub state: &'a GameState,
    pub content: &'a ContentStore,
    pub sources: SourceSet,
    pub player: &'a PlayerId,
}

impl ResearchHooks for StringHooks<'_> {
    fn purged(&self, t: Technology) -> bool {
        crate::factions::deepwrought::technology_purged(self.state, &TechnologyId::new(<Technology as IdStr>::as_str(t)))
    }

    fn waived_prerequisites(&self, t: Technology) -> usize {
        crate::faction_abilities::waived_prerequisites(
            self.state,
            self.content,
            self.sources,
            self.player,
            <Technology as IdStr>::as_str(t),
        )
    }

    fn extra_colours(&self) -> Colours {
        let mut out = [0u16; 4];
        for (name, count) in crate::factions::hooks_strategy::extra_prerequisite_colours(self.state, self.content, self.player) {
            if let Some(i) = crate::technology::track_index(&name) {
                out[i] += u16::try_from(count).unwrap_or(u16::MAX);
            }
        }
        out
    }

    fn inheritance_affordable(&self) -> bool {
        crate::payment::affordable(
            self.state,
            self.content,
            self.sources,
            self.player,
            q::INHERITANCE_SYSTEMS_COST,
            crate::production::Spend::Resources,
        )
    }

    fn waiver_offer(&self, t: Technology) -> bool {
        !crate::technology::research_waiver_offers(
            self.state,
            self.content,
            self.player,
            &TechnologyId::new(<Technology as IdStr>::as_str(t)),
        )
        .is_empty()
    }
}

/// The set of owned technologies of a string player; `None` when one is not in the tables.
fn owned_set(seat: &ti4_model::state::Player) -> Option<TechSet> {
    let mut set = TechSet::new();
    for t in &seat.technologies {
        set.insert(tech(t.as_str())?);
    }
    Some(set)
}

/// `technology::specialties` read from the string state, as the typed counts.
fn specialties(state: &GameState, sc: Scope, player: &PlayerId) -> Option<Colours> {
    let free = state.rules_version >= q::RULES_TECH_EFFECTS && crate::technology::owns_technology(state, player, "pa");
    let mut found = [0u16; 4];
    for (_, planet) in state.controlled_planets(player) {
        if !free && state.exhausted_planets.contains(planet) {
            continue;
        }
        let typed = <Planet as IdStr>::from_id(planet.as_str())?;
        let mut attached = Vec::new();
        for id in state.planet_attachments.get(planet).into_iter().flatten() {
            attached.push(<Attachment as IdStr>::from_id(id)?);
        }
        for s in q::specialties_of(sc, typed, attached).as_slice() {
            if let Some(i) = q::colour_index(*s) {
                found[i] += 1;
            }
        }
    }
    Some(found)
}

/// The synergy pool of a string player (`synergy::joined`).
#[must_use]
pub fn synergy_joined(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<q::ColourSet> {
    let seat = state.player(player)?;
    if seat.relics.iter().any(|r| r.as_str() == "quantumcore") {
        return Some([true; 4]);
    }
    let mut joined = [false; 4];
    if let Some(b) = &seat.breakthrough {
        let d = <Breakthrough as IdStr>::from_id(b.as_str())?.data();
        if scope(sources).0 & source_bit(d.source) != 0
            && let [first, second] = d.synergy
        {
            for c in [first, second] {
                if let Some(i) = q::colour_index(*c) {
                    joined[i] = true;
                }
            }
        }
    }
    Some(joined)
}

/// The research context of a string player; `None` when the core cannot express the position.
fn ctx(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, hooks: &StringHooks<'_>) -> Option<ResearchCtx> {
    let seat = state.player(player)?;
    let sc = scope(sources);
    let faction = <Faction as IdStr>::from_id(seat.faction.as_str())?;
    let owned = owned_set(seat)?;
    let held = q::colours_of(&owned);
    let spec = specialties(state, sc, player)?;
    let extra = hooks.extra_colours();
    let mut holdings = [0u16; 4];
    for i in 0..4 {
        holdings[i] = held[i] + spec[i] + extra[i];
    }
    let mut waivable = crate::relics::prerequisite_waivers(state, player);
    for colour in q::COLOURS {
        waivable += crate::laws::research_team_waivers(state, player, colour_name(colour));
    }
    let ready = |t: Technology| owned.contains(t) && !seat.exhausted_technologies.contains(ti4_model::id::text(<Technology as IdStr>::as_str(t)));
    Some(ResearchCtx {
        faction,
        owned,
        holdings,
        waivable,
        joined: synergy_joined(state, sources, player)?,
        aida_ready: state.rules_version >= q::RULES_TECH_EFFECTS && ready(Technology::AIDevelopmentAlgorithm),
        inheritance_card: ready(Technology::InheritanceSystems),
        war_sun_waived: crate::laws::war_sun_prerequisites_waived(state, content, &TechnologyId::new("ws")),
    })
}

#[must_use]
pub fn can_research(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, alias: &TechnologyId) -> Option<bool> {
    let t = tech(alias.as_str())?;
    let hooks = StringHooks { state, content, sources, player };
    Some(ctx(state, content, sources, player, &hooks)?.can_research(t, &hooks))
}

#[must_use]
pub fn prerequisites_met(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    alias: &TechnologyId,
    with_aida: bool,
) -> Option<bool> {
    let t = tech(alias.as_str())?;
    let hooks = StringHooks { state, content, sources, player };
    Some(ctx(state, content, sources, player, &hooks)?.prerequisites_met(t, &hooks, with_aida))
}

#[must_use]
pub fn researchable(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> Option<Vec<TechnologyId>> {
    let hooks = StringHooks { state, content, sources, player };
    let c = ctx(state, content, sources, player, &hooks)?;
    Some(ids(&c.researchable(scope(sources), &hooks)))
}

#[must_use]
pub fn researchable_by_prerequisites(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Option<Vec<TechnologyId>> {
    let hooks = StringHooks { state, content, sources, player };
    let c = ctx(state, content, sources, player, &hooks)?;
    Some(ids(&c.researchable_by_prerequisites(scope(sources), &hooks)))
}

#[must_use]
pub fn owned_colours(state: &GameState, player: &PlayerId) -> Option<BTreeMap<&'static str, usize>> {
    let Some(seat) = state.player(player) else {
        return Some(BTreeMap::new());
    };
    Some(colour_map(q::colours_of(&owned_set(seat)?)))
}

#[must_use]
pub fn specialties_map(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<BTreeMap<&'static str, usize>> {
    Some(colour_map(specialties(state, scope(sources), player)?))
}

/// `synergy::satisfies` over the typed arrays.
#[must_use]
pub fn satisfies(
    needs: &BTreeMap<&'static str, usize>,
    holdings: &BTreeMap<&'static str, usize>,
    joined: &[String],
    waivable: usize,
) -> Option<bool> {
    let arr = |m: &BTreeMap<&'static str, usize>| -> Option<Colours> {
        let mut out = [0u16; 4];
        for (k, v) in m {
            out[track(k)?] = u16::try_from(*v).ok()?;
        }
        Some(out)
    };
    let mut set = [false; 4];
    for j in joined {
        set[track(j)?] = true;
    }
    Some(q::satisfies(arr(needs)?, arr(holdings)?, set, waivable))
}

/// The faction units of a source scope, resolved by the store's reprint rules; built once per scope
/// (`None`: a store that is not the embedded one, whose catalogue the cache cannot share).
pub fn roster_for(content: &ContentStore, sources: SourceSet) -> Option<&'static q::UnitRoster> {
    static ROSTERS: [std::sync::OnceLock<Option<q::UnitRoster>>; 128] = [const { std::sync::OnceLock::new() }; 128];
    if !content.is_embedded() {
        return None;
    }
    let slot = ROSTERS.get(usize::try_from(sources.as_u32()).ok()?)?;
    slot.get_or_init(|| {
        let embedded = ContentStore::embedded();
        let mut roster = q::UnitRoster::default();
        for faction in Faction::ALL {
            for base in UnitBaseType::ALL {
                let found = ti4_content::units::faction_unit(
                    embedded,
                    <Faction as IdStr>::as_str(faction),
                    <UnitBaseType as IdStr>::as_str(base),
                    sources,
                );
                let unit = match found {
                    Some(u) => Some(<ti4_engine_core::generated::Unit as IdStr>::from_id(u.id())?),
                    None => None,
                };
                roster.set(faction, base, unit);
            }
        }
        Some(roster)
    })
    .as_ref()
}

/// The unit type id a seat's units of `base` must have, before any faction override.
#[must_use]
pub fn unit_form(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId, base: &str) -> Option<&'static str> {
    let seat = state.player(player)?;
    let faction = <Faction as IdStr>::from_id(seat.faction.as_str())?;
    let base = <UnitBaseType as IdStr>::from_id(base)?;
    let owned = owned_set(seat)?;
    let sc = scope(sources);
    let roster = roster_for(content, sources)?;
    q::unlocked_upgrade(sc, base, faction, &owned)
        .or_else(|| q::faction_unit(roster, faction, base))
        .or_else(|| q::generic_unit(sc, base))
        .map(<ti4_engine_core::generated::Unit as IdStr>::as_str)
}

/// The scars of a scope, as the typed answer of `entropic_scars::is_scar`.
#[must_use]
pub fn is_scar(sources: SourceSet, system: &str) -> Option<bool> {
    let system = <ti4_engine_core::generated::System as IdStr>::from_id(system)?;
    Some(q::is_scar(scope(sources), system))
}

/// `entropic_scars::unowned_faction_technologies(..).len()`.
#[must_use]
pub fn unowned_faction_technologies_count(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<usize> {
    let seat = state.player(player)?;
    let faction = <Faction as IdStr>::from_id(seat.faction.as_str())?;
    Some(q::unowned_faction_technologies_of(scope(sources), faction, &owned_set(seat)?).len())
}

/// `synergy::pair`: the two colours of the breakthrough's synergy, in the record's order.
#[must_use]
pub fn synergy_pair(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<Option<[String; 2]>> {
    let seat = state.player(player)?;
    let Some(b) = &seat.breakthrough else {
        return Some(None);
    };
    let d = <Breakthrough as IdStr>::from_id(b.as_str())?.data();
    if scope(sources).0 & source_bit(d.source) == 0 {
        return Some(None);
    }
    Some(match d.synergy {
        [first, second] => Some([colour_text(*first), colour_text(*second)]),
        _ => None,
    })
}

fn colour_text(t: TechType) -> String {
    colour_name(t).to_owned()
}

/// `synergy::joined`: the colour names this player's synergy pools.
#[must_use]
pub fn synergy_joined_names(state: &GameState, sources: SourceSet, player: &PlayerId) -> Option<Vec<String>> {
    let seat = state.player(player)?;
    if seat.relics.iter().any(|r| r.as_str() == "quantumcore") {
        return Some(q::COLOURS.into_iter().map(colour_text).collect());
    }
    Some(synergy_pair(state, sources, player)?.map_or_else(Vec::new, |pair| pair.to_vec()))
}

// --- the lent view ----------------------------------------------------------------------------

use crate::core_sync::{Ctx, CoreView, Footprint, Slice, Sources};

/// The slices the technology queries read.
pub const FOOTPRINT: Footprint = Footprint::reads(&[Slice::Meta, Slice::Players, Slice::Board, Slice::Agenda]);

/// One typed view of a position for the technology queries (`None`: a value the core cannot hold).
#[must_use]
pub fn view(state: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>) -> Option<(CoreView, Ctx)> {
    let mut cx = Ctx::of(state).ok()?;
    let view = CoreView::of(state, &mut cx, Sources { galaxy, rng: None }, FOOTPRINT.loads()).ok()?;
    Some((view, cx))
}

/// A unit form with the faction override of the string engine applied (`placed_unit_id`).
#[must_use]
pub fn overridden_form(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
    base: &str,
    form: &str,
) -> ti4_model::id::UnitTypeId {
    crate::factions::hooks_strategy::unit_form_override(state, content, sources, player, base, form)
        .unwrap_or_else(|| ti4_model::id::UnitTypeId::new(form.to_owned()))
}
