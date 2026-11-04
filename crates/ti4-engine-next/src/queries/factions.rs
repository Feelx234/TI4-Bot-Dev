//! The faction readers of the surface (`factions::mahact`, `factions::firmament`, `factions::crimson`, `factions::bastion_units`,
//! `fleet::limit`, `faction_abilities::fleet_supply`): the hooks of the core kernel, or a plain read of the marks the factions write.

use ti4_content::ContentStore;
use ti4_engine_core::query::fleet as core_fleet;
use ti4_engine_core::rules::abilities as core_abilities;
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;

use super::view::Full;

/// The owners of the other players' command tokens in `mahact`'s fleet pool, one entry per token, sorted (Edict). Public information.
#[must_use]
pub fn mahact_fleet_pool_owners(state: &GameState, mahact: &PlayerId) -> Vec<PlayerId> {
    Full::of(state)
        .and_then(|v| {
            let owners = v.kernel().hooks.direct.mahact_fleet_pool_owners(v.core(), v.seat(mahact)?);
            v.players(owners.iter().copied())
        })
        .unwrap_or_default()
}

/// How many foreign command tokens `player`'s fleet pool holds (0 unless they are the Mahact).
#[must_use]
pub fn mahact_foreign_tokens(state: &GameState, player: &PlayerId) -> i32 {
    // Only the Mahact's fleet pool holds foreign tokens (the hook answers 0 for every other faction, and for a player the position does not
    // seat). Asking first spares loading the whole position into a core view for every seat sheet a decider reads (`Observed::seat`:
    // 70 percent of a rule-bot game, docs/PERF_AB.md).
    if !state.player(player).is_some_and(|seat| seat.faction.as_str() == "mahact") {
        return 0;
    }
    Full::of(state).and_then(|v| Some(v.kernel().hooks.direct.mahact_foreign_tokens(v.core(), v.seat(player)?))).unwrap_or(0)
}

/// The players whose control tokens are on any of `owner`'s plot cards (the Firmament's puppets).
#[must_use]
pub fn firmament_puppeted(state: &GameState, owner: &PlayerId) -> std::collections::BTreeSet<PlayerId> {
    Full::of(state)
        .and_then(|v| {
            let seats = v.kernel().hooks.direct.firmament_puppeted(v.core(), v.seat(owner)?);
            v.player_set(seats.iter().copied())
        })
        .unwrap_or_default()
}

/// Every system holding an active breach (the Crimson Rebellion), in id order.
#[must_use]
pub fn crimson_active_breaches(state: &GameState) -> Vec<SystemId> {
    state.breach_tokens.iter().filter(|system| state.faction_marks.contains_key(&format!("crimson:breach:active:{system}"))).cloned().collect()
}

/// Non-home systems that contain a planet `player` controls. A space station is not a planet.
#[must_use]
pub fn non_home_systems_with_planets(state: &GameState, content: &ContentStore, sources: SourceSet, player: &PlayerId) -> usize {
    let homes: std::collections::BTreeSet<&SystemId> = state.players.iter().filter_map(|seat| seat.home_system.as_ref()).collect();
    state
        .board
        .iter()
        .filter(|(system, board)| {
            !homes.contains(system)
                && board.planet_control.iter().any(|(planet, owner)| owner == player && !ti4_content::galaxy::is_space_station(content, planet.as_str(), sources))
        })
        .count()
}

/// How many non-fighter ships this player may keep in one system (`fleet::limit`): the fleet pool (its tokens, the foreign ones of the
/// Mahact, capped by Fleet Regulations), then lifted by the abilities (Letnev's Armada, +2) and the faction modules.
#[must_use]
pub fn fleet_limit(state: &GameState, content: &ContentStore, player: &PlayerId) -> i32 {
    let Some(v) = Full::of(state) else {
        // A position the core cannot hold (a table with no faction seated, an id the tables do not know) has no faction module to ask: the
        // pool of the seat, capped by the law, lifted by the printed ability.
        let tokens = state.player(player).map_or(0, |seat| seat.fleet_tokens);
        let capped = super::laws::fleet_pool_cap(state, tokens);
        let armada = state
            .player(player)
            .and_then(|seat| ti4_content::factions::get(content, seat.faction.as_str()))
            .is_some_and(|faction| faction.abilities().contains(&"armada"));
        return capped + if armada { 2 } else { 0 };
    };
    let Some(seat) = v.seat(player) else { return 0 };
    let foreign = v.kernel().hooks.direct.mahact_foreign_tokens(v.core(), seat);
    let capped = super::laws::fleet_pool_cap(state, core_fleet::pool_base(v.core(), seat, foreign));
    let env = ti4_engine_core::driver::kernel_env::KernelEnv::new(v.core(), v.kernel());
    core_abilities::fleet_supply(&core_abilities::Cx { core: v.core(), env: &env }, seat, capped)
}

/// The owners of the other players' command tokens in `mahact`'s fleet pool, one entry per token, sorted (`factions::mahact::fleet_pool_owners`).
/// A plain read of the public state's marks, so it answers for every position, a redacted view included.
#[must_use]
pub fn fleet_pool_owners(state: &GameState, mahact: &PlayerId) -> Vec<PlayerId> {
    state
        .faction_marks
        .get(&format!("mahact:fleet:{mahact}"))
        .map(|list| list.split(',').filter(|id| !id.is_empty()).map(PlayerId::new).collect())
        .unwrap_or_default()
}

/// Combined-cost reduction for production in `system` (`factions::hooks_economy::production_cost_reduction`): the sum of the faction modules'
/// answers, never negative. Zero when the core cannot hold the position.
#[must_use]
pub fn production_cost_reduction(state: &GameState, player: &PlayerId, system: &SystemId) -> i64 {
    use ti4_engine_core::state::{Slice, SliceSet};
    let Some(view) = super::view::Loaded::of(state, None, ti4_model::content_types::FULL, SliceSet::of_without(SliceSet::ALL, Slice::Map)) else { return 0 };
    let (Some(seat), Some(system)) = (view.seat(player), crate::ties::core_adapter::typed::<ti4_engine_core::generated::System>(system.as_str())) else { return 0 };
    view.kernel().hooks.economy.production_cost_reduction(view.core(), seat, system).max(0)
}

/// The kind of a faction-specific asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetKind {
    /// A printed faction ability.
    Ability,
    /// A faction technology.
    Technology,
    /// A faction unit with printed ability text.
    Unit,
    /// A promissory note.
    Promissory,
    /// An agent, commander or hero.
    Leader,
    /// A Thunder's Edge breakthrough.
    Breakthrough,
}

/// One faction-specific card or component.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Asset {
    /// What it is.
    pub kind: AssetKind,
    /// Its content id.
    pub id: String,
}

use crate::builders::factions::tag_belongs_to;

/// Every faction-specific asset a faction has in `sources`, from the corpus (abilities, the technologies its sheet lists, units with their
/// upgrade chain, promissory notes, leaders, breakthroughs), sorted.
#[must_use]
pub fn assets(content: &ContentStore, sources: SourceSet, alias: &str) -> Vec<Asset> {
    use ti4_model::content_types::ContentType;
    let mut found = std::collections::BTreeSet::new();
    let Some(faction) = ti4_content::factions::get(content, alias) else {
        return Vec::new();
    };
    let mut add = |kind, id: &str| {
        found.insert(Asset { kind, id: id.to_owned() });
    };
    for id in faction.abilities() {
        add(AssetKind::Ability, id);
    }
    // Only the technologies the faction sheet lists (which includes the unit upgrades) count, plus official Thunder's Edge reprints.
    let sheet_technologies: std::collections::BTreeSet<&str> = faction.record().strings("factionTech").into_iter().collect();
    for record in content.from_sources(ContentType::Technologies, sources) {
        if record.text("faction").is_some_and(|f| tag_belongs_to(f, alias, false))
            && let Some(id) = record.text("alias")
            && (sheet_technologies.contains(id) || record.text("source") == Some("thunders_edge"))
        {
            add(AssetKind::Technology, id);
        }
    }
    // Units: the sheet's own, their upgrade chain, and extra upgrades a later source adds.
    let units: Vec<&ti4_content::Record> = content.from_sources(ContentType::Units, sources).filter(|record| record.text("faction").is_some_and(|f| tag_belongs_to(f, alias, true))).collect();
    let id_of = |record: &ti4_content::Record| record.text("id").or_else(|| record.text("alias")).map(ToOwned::to_owned);
    let mut sheet: std::collections::BTreeSet<String> = faction.record().strings("units").into_iter().map(ToOwned::to_owned).collect();
    loop {
        let before = sheet.len();
        for record in &units {
            let Some(id) = id_of(record) else { continue };
            let upgrades_listed = record.text("upgradesFromUnitId").is_some_and(|from| sheet.contains(from));
            let extra = record.flag("isUpgrade") && record.text("upgradesFromUnitId").is_none() && record.text("homebrewReplacesID").is_none();
            if upgrades_listed || extra {
                sheet.insert(id);
            }
        }
        if sheet.len() == before {
            break;
        }
    }
    for record in &units {
        if let Some(id) = id_of(record)
            && sheet.contains(&id)
        {
            add(AssetKind::Unit, &id);
        }
    }
    for record in content.from_sources(ContentType::PromissoryNotes, sources) {
        if record.text("faction").is_some_and(|f| f.eq_ignore_ascii_case(alias))
            && let Some(id) = record.text("alias").or_else(|| record.text("id"))
        {
            add(AssetKind::Promissory, id);
        }
    }
    for leader in crate::builders::leaders::for_faction(content, sources, alias) {
        add(AssetKind::Leader, leader.as_str());
    }
    for record in content.from_sources(ContentType::Breakthroughs, sources) {
        if record.text("faction").is_some_and(|f| tag_belongs_to(f, alias, false))
            && let Some(id) = record.text("alias")
        {
            add(AssetKind::Breakthrough, id);
        }
    }
    found.into_iter().collect()
}

/// The flagship `faction` brings (`<faction>_flagship`; the three Keleres share `keleres_flagship`).
fn flagship_of(faction: &str) -> String {
    if faction.starts_with("keleres") { "keleres_flagship".to_owned() } else { format!("{faction}_flagship") }
}

/// Whether the unit `unit_type` of `owner` has the text of flagship `flagship_id` (`factions::flagship_has_text`): it is that flagship, or it is
/// a Nekro flagship whose owner has placed the Z token (Valefar Assimilator Z) on that faction. Only text abilities follow; stats never do.
#[must_use]
pub fn flagship_has_text(state: &GameState, owner: &PlayerId, unit_type: &str, flagship_id: &str) -> bool {
    if unit_type == flagship_id {
        return true;
    }
    if unit_type != "nekro_flagship" {
        return false;
    }
    let nekro = state.player(owner).is_some_and(|seat| seat.faction.as_str() == "nekro");
    let holds = state.player(owner).and_then(|seat| seat.breakthrough.as_ref()).is_some_and(|held| held.as_str() == "nekrobt");
    nekro
        && holds
        && state
            .faction_marks
            .get(&format!("nekro:token:{owner}:Z"))
            .is_some_and(|marks| marks.split(',').filter(|faction| !faction.is_empty()).any(|faction| flagship_of(faction) == flagship_id))
}
