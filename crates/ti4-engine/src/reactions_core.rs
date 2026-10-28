//! The adapter half of the core ability table (package `core-reactions`; docs/ENGINE_REACTIONS.md): which string
//! ability ids the core table has a rule for, the registry the core builds from it, and `follow`, which sets up
//! a core window to follow a string run that had nested activity (reaction windows, ability effects).
//!
//! * [`table`]: the core `AbilityTable` with every ported rule.
//! * [`slot_of`] / [`text_of`]: the mapping between a registered string id
//!   (`technology:hacan:inf2:GROUND_FORCE_DESTROYED:after`) and the owner-independent [`Slot`].
//! * [`Recording::unported`]: the first ability the string run found eligible that the table lacks.
//! * [`follow`]: the resolver (configured as the string one was) and the table for a nested window.

use ti4_engine_core::choice::reaction::{AbilityId, AbilityKind};
use ti4_engine_core::kernel::ability::{AbilityTable, Slot, Spec};
use ti4_engine_core::kernel::Resolver;
use ti4_engine_core::state::CoreState;
use ti4_engine_core::Seat;
use ti4_model::state::GameState;

use crate::dual_window::{Recording, ResolverCfg};
use crate::kernel_sync::text::{ability_id, event_text, relation_text};
use crate::render::Render;

/// One ported ability rule, named: the registered id of an owner is `{kind}:{owner}:{card}:{EVENT}:{rel}`; the
/// event and the relation are the spec's.
struct Named {
    slot: Slot,
    kind: &'static str,
    card: &'static str,
}

/// Every ported rule, in table order (the per-owner registration order of `reactions::arm`).
const NAMED: &[Named] = &[Named { slot: ti4_engine_core::rules::reactions::INFANTRY_II_ROLLS, kind: "technology", card: "inf2" }];

/// The core table: every ported ability rule.
#[must_use]
pub fn table() -> AbilityTable {
    static TABLE: std::sync::OnceLock<AbilityTable> = std::sync::OnceLock::new();
    TABLE
        .get_or_init(|| {
            let mut t = AbilityTable::default();
            t.bars = Some(ti4_engine_core::rules::agenda::political_secret_bars);
            ti4_engine_core::rules::reactions::register(&mut t);
            t
        })
        .clone()
}

fn specs() -> Vec<Spec> {
    table().specs().to_vec()
}

#[allow(dead_code)]
fn kind_text(kind: AbilityKind) -> &'static str {
    match kind {
        AbilityKind::Ability => "ability",
        AbilityKind::Leader => "leader",
        AbilityKind::Breakthrough => "breakthrough",
        AbilityKind::Technology => "technology",
        AbilityKind::Unit => "unit",
        AbilityKind::Promissory => "promissory",
        AbilityKind::Reaction => "reaction",
        AbilityKind::Other => "other",
    }
}

/// The slot of a registered string id (`kind:owner:card:EVENT:rel`), if the table has a rule for it.
#[must_use]
pub fn slot_of(id: &str) -> Option<Slot> {
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() < 5 {
        return None;
    }
    let (kind, rest) = (parts[0], &parts[2..]);
    let (event, rel) = (rest[rest.len() - 2], rest[rest.len() - 1]);
    let card = rest[..rest.len() - 2].join(":");
    let named = NAMED.iter().find(|n| n.kind == kind && n.card == card)?;
    let spec_event_rel = specs().into_iter().find(|s| s.slot == named.slot)?;
    let ok = event_name(&spec_event_rel) == event && relation_name(&spec_event_rel) == rel;
    ok.then_some(named.slot)
}

fn event_name(s: &Spec) -> String {
    event_text(&Render::new(Vec::new()), s.event)
}

fn relation_name(s: &Spec) -> &'static str {
    relation_text(s.relation)
}

impl Recording {
    /// The first ability the string run found eligible (or ran) that the core table has no rule for, anonymised
    /// (`technology:*:inf2:GROUND_FORCE_DESTROYED:after`), or the reason the nested activity cannot be followed.
    #[must_use]
    pub fn unported(&self, table: &AbilityTable) -> Option<String> {
        if self.foreign {
            return Some("nested outside an ability".to_owned());
        }
        for id in &self.eligible {
            if slot_of(id).is_some_and(|s| table.of_slot(s).is_some()) {
                continue;
            }
            let mut parts: Vec<&str> = id.split(':').collect();
            if parts.len() > 2 {
                parts[1] = "*";
            }
            return Some(format!("unported ability {}", parts.join(":")));
        }
        None
    }
}

/// What a core window needs to follow nested activity: the table, and the resolver configured as the string one.
pub struct Follow {
    pub table: AbilityTable,
    pub resolver: Resolver,
}

/// The resolver and table for a window that ran with `script`; `None` for a run without nested activity (the
/// window keeps its own minimal resolver and an empty table).
///
/// # Errors
/// The reason the nested activity cannot be followed (the adapter bails with it).
pub fn follow(r: &Render, script: &Recording) -> Result<Option<Follow>, String> {
    if !script.nested {
        return Ok(None);
    }
    let table = table();
    if let Some(why) = script.unported(&table) {
        return Err(why);
    }
    let Some(cfg) = script.cfg.as_ref() else {
        return Err("nested activity without a resolver".to_owned());
    };
    let resolver = resolver_of(r, cfg).ok_or_else(|| "a resolver seat the tables do not know".to_owned())?;
    Ok(Some(Follow { table, resolver }))
}

fn phase_of(p: ti4_model::state::Phase) -> ti4_engine_core::state::Phase {
    crate::render::generic::core_phase(p)
}

/// A core resolver configured as the string one was (no registry).
#[must_use]
pub fn resolver_of(r: &Render, cfg: &ResolverCfg) -> Option<Resolver> {
    use ti4_engine_core::kernel::resolver::UsedScope;
    let seats = |v: &[String]| -> Option<Vec<Seat>> { v.iter().map(|s| r.seat(s)).collect() };
    let mut core = Resolver::new(&seats(&cfg.initiative)?, cfg.active.as_deref().and_then(|s| r.seat(s)));
    core.set_phase(phase_of(cfg.phase));
    core.set_seating_order(&seats(&cfg.seating)?);
    core.set_speaker(cfg.speaker.as_deref().and_then(|s| r.seat(s)));
    core.set_counters(cfg.round, cfg.turn);
    core.set_maximum_depth(cfg.max_depth);
    for (id, scope) in &cfg.used {
        let Some((kind, rest)) = scope.split_once('(') else { continue };
        let n: u64 = rest.trim_end_matches(')').parse().unwrap_or(0);
        let kind = match kind {
            "Trigger" => UsedScope::Trigger,
            "Turn" => UsedScope::Turn,
            _ => UsedScope::Round,
        };
        core.spend(ability_id(r, id), kind, n);
    }
    Some(core)
}

impl Follow {
    /// Register the ported abilities of every seat in the resolver, in the string order. Call once, with the
    /// core state of the window, before the first `run`.
    pub fn arm(&mut self, r: &Render, gs: &GameState, core: &CoreState) {
        let order: Vec<Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
        let owner_name = |who: Seat| crate::promissory::faction_name(gs, &ti4_model::id::PlayerId::new(r.seat_text(who)));
        let mut name = |spec: &Spec, who: Seat| -> AbilityId {
            let n = NAMED.iter().find(|n| n.slot == spec.slot).expect("every spec is named");
            let text = format!(
                "{}:{}:{}:{}:{}",
                n.kind,
                owner_name(who),
                n.card,
                event_text(r, spec.event),
                relation_text(spec.relation)
            );
            ability_id(r, &text)
        };
        let defs = self.table.registry(core, &order, &mut name);
        self.resolver.register(defs);
    }
}
