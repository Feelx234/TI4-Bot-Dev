//! `Area::Movement` windows (package A2b): the tactical action as a core window beside the string engine.
//!
//! The unit of the dual is one answer of the open `TacticalWindow` (`Game::step_tactical`): [`step`] builds the core
//! `rules::tactical::Machine` at the position the string window was in ([`At`]), runs it to its question, compares the
//! question with the string's (ids, labels, payload, context; the details and previews are the recorded question's: item
//! A2b-12), answers it with the string's answer and compares the position, the typed events, the plain events, the
//! rift dice and the streams afterwards. [`finish`] is the head of `finish_tactical_inner` and [`free`] the free
//! tactical action. What stays with the string engine is in `docs/ENGINE_KNOWN_ISSUES.md` (A2b-bail-*).

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

use ti4_model::id::SystemId;
use ti4_model::units::Unit;

/// Where the string window stands when a step begins.
#[derive(Debug, Clone)]
pub enum At {
    Activating,
    Moving,
    Loading { origin: SystemId, ship: Unit, path: Vec<String>, loaded: Vec<usize>, closed: bool },
}

/// The stage the string window is in afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Activating,
    Moving,
    Loading,
    /// The window is gone (a hook ended the turn): never a position the core follows.
    Closed,
}

/// What both sides produce for the comparison besides the state: the stage afterwards, the plain events the step wrote,
/// whether the string refused the answer, the faces of the rift dice and where the streams stand.
#[derive(Debug, Clone, PartialEq)]
pub struct Stepped {
    pub stage: Tag,
    pub plain: Vec<String>,
    pub failed: bool,
    pub rift_faces: Vec<u32>,
    pub rng: crate::rng::RngPositions,
}

/// What the head of the close of the movement step leaves besides the state: the system the rest belongs to, whether it
/// skips to the commit step, the plain events, whether the string refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closed {
    pub system: String,
    pub skip_to_commit: bool,
    pub plain: Vec<String>,
    pub failed: bool,
}

/// What a call site knows besides the state.
#[derive(Debug, Clone, Copy)]
pub struct Args<'a> {
    pub player: &'a ti4_model::id::PlayerId,
    /// The dice and streams are the ones the core can reproduce (no test faces, no pending restore).
    pub reproducible: bool,
    pub rng_before: &'a crate::rng::RngPositions,
    pub failed: bool,
}

/// The label of a step by its stage.
#[must_use]
pub const fn label(at: &At) -> &'static str {
    match at {
        At::Activating => "tactical::activate",
        At::Moving => "tactical::move",
        At::Loading { .. } => "tactical::load",
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::SystemId;
    use ti4_model::state::GameState;

    use super::{Args, At, Stepped};
    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn step(_: &mut GameState, _: &ContentStore, _: SourceSet, _: &Galaxy, _: &At, _: Args<'_>, _: &Recording) -> Option<CoreWindow<Stepped>> {
        None
    }

    pub fn free(_: &mut GameState, _: &ContentStore, _: SourceSet, _: &Galaxy, _: &SystemId, _: super::Args<'_>, _: &Recording) -> Option<CoreWindow<Stepped>> {
        None
    }

    pub fn enforce(_: &mut GameState, _: &ContentStore, _: SourceSet, _: &ti4_model::id::PlayerId, _: &SystemId, _: &Recording) -> Option<CoreWindow<Option<usize>>> {
        None
    }

    pub fn finish(_: &mut GameState, _: &ContentStore, _: SourceSet, _: &Galaxy, _: &ti4_model::id::PlayerId, _: bool, _: &Recording) -> Option<CoreWindow<super::Closed>> {
        None
    }
}

#[cfg(feature = "typed-core")]
mod real {
    use std::cell::Cell;

    use ti4_content::ContentStore;
    use ti4_content::galaxy::Galaxy;
    use ti4_engine_core::generated::{System, Unit as CUnit};
    use ti4_engine_core::kernel::stage::{AbilityTable, Window};
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::rules::tactical::{Facts, Machine, Plain, RoutePick, Stage};
    use ti4_engine_core::state::{Piece, Slice};
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::SystemId;
    use ti4_model::state::GameState;

    use super::{Args, At, Stepped, Tag, label};
    use crate::core_sync::{Footprint, Sources, with_core};
    use crate::dual::{CoreWindow, Run, note_bail};
    use crate::dual_window::{Recording, apply_staged, drive_with, event_of, nested_check, nested_fp, tables};
    use crate::generated::IdStr;

    /// What one answer reads and writes: the tokens, techs and cards (Players, Cards, Trade), the board, the map and the
    /// wormhole tokens the reach reads (Map, Explore, Agenda, Marks), the scratch (Transient), the active system and the
    /// activation count (Meta), the pending tag, the events and the dice (Ledger, Rng).
    pub(crate) const WINDOW: Footprint = Footprint::new(
        &[
            Slice::Meta,
            Slice::Seats,
            Slice::Players,
            Slice::Board,
            Slice::Map,
            Slice::Cards,
            Slice::Trade,
            Slice::Agenda,
            Slice::Explore,
            Slice::Marks,
            Slice::Transient,
            Slice::Pending,
            Slice::Rng,
            Slice::Ledger,
        ],
        &[Slice::Meta, Slice::Players, Slice::Board, Slice::Cards, Slice::Trade, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Rng, Slice::Ledger],
    );

    /// The kernel of a window run: the combat one (the string bridge for the reaction table) with the faction movement hooks
    /// of the scope, which the combat kernel does not register (Argent's free cargo, Gravleash, the wormhole limits...).
    #[must_use]
    pub(crate) fn env_of(gs: &GameState, cx: &crate::core_sync::Ctx, sources: SourceSet) -> Option<crate::core_combat::WindowEnv> {
        let mut env = crate::core_combat::window_env(gs, cx, sources)?;
        env.kernel.hooks.movement = crate::core_adapter::kernel_for(sources).hooks.movement.clone();
        Some(env)
    }

    fn bail<T>(label: &str, why: &str) -> Option<T> {
        note_bail(label, why);
        None
    }

    /// The factions the lobby can never seat (dropped by decision, like Bastion and Crimson): their pieces of the
    /// tactical action stay with the string engine.
    #[must_use]
    pub fn dropped_faction_at_the_table(gs: &GameState) -> bool {
        gs.players.iter().for_each(|p| crate::product::lobby_faction(p.faction.as_str()));
        gs.players.iter().any(|p| {
            let f = p.faction.as_str();
            crate::factions::keleres::is_keleres_faction(f)
                || matches!(
                    f,
                    crate::factions::deepwrought::FACTION
                        | crate::factions::firmament::FACTION
                        | crate::factions::obsidian::FACTION
                        | crate::factions::ralnel::FACTION
                        | crate::factions::bastion::FACTION
                        | crate::factions::crimson::FACTION
                )
        })
    }

    /// The hooks of an activation the core machine does not follow: a technology or an ability that asks, pays or ends
    /// the turn, a law, a relic, a note that denies the move. `None`: the activation is the plain one.
    #[must_use]
    pub fn activation_unported(gs: &GameState, player: &ti4_model::id::PlayerId, system: &SystemId) -> Option<&'static str> {
        for (alias, why) in [
            ("scc", "Spatial Conduit Cylinders held"),
            ("sdn", "Scanlink Drone Network held"),
            ("ers", "E-Res Siphons held"),
            ("nf", "Nullification Field held"),
        ] {
            if gs.players.iter().any(|p| p.technologies.contains(ti4_model::id::text(&alias))) {
                return Some(why);
            }
        }
        if gs.players.iter().any(|p| p.faction.as_str() == crate::factions::mahact::FACTION) || crate::promissory::has_commander_ability(gs, player, "mahactcommander") {
            return Some("Mahact at the table (Starlancer, the commander's re-activation)");
        }
        if gs.laws.contains_key("minister_peace") {
            return Some("Minister of Peace in play");
        }
        if crate::relics::holds(gs, player, &ti4_model::id::RelicId::new("dominusorb")) {
            return Some("Dominus Orb held");
        }
        if crate::promissory::denies_movement_into(gs, player, system) {
            return Some("Ceasefire denies the move");
        }
        None
    }

    /// What a run of the machine leaves besides the state.
    struct Ran {
        stage: Stage,
        plain: Vec<Plain>,
        dice: Vec<u32>,
        emitted: Vec<ti4_engine_core::state::EventRec>,
        bailed: Option<ti4_engine_core::rules::tactical::Bail>,
    }

    fn tag_of(stage: Stage) -> Tag {
        match stage {
            Stage::Activating => Tag::Activating,
            Stage::Moving => Tag::Moving,
            Stage::Loading => Tag::Loading,
            Stage::Done => Tag::Closed,
        }
    }

    fn plain_text(r: &crate::render::Render, p: Plain) -> String {
        match p {
            Plain::SupportReturned(owner) => format!("SUPPORT_FOR_THE_THRONE_RETURNED:{}", r.seat_text(owner)),
            Plain::SystemActivated(system) => format!("SYSTEM_ACTIVATED:{}", system.as_str()),
            Plain::FreeActivated => "SYSTEM_ACTIVATED".to_owned(),
            Plain::FreeTactical => "FREE_TACTICAL_ACTION".to_owned(),
            Plain::Typed(kind) => crate::kernel_sync::event_text(r, kind),
            Plain::ShipMoved => "SHIP_MOVED".to_owned(),
            Plain::ShipLost => "SHIP_LOST_TO_GRAVITY_RIFT".to_owned(),
        }
    }

    /// The route the answer `move|O|i|rN` names, from the routes the recorded question lists for the option (the
    /// string's `ship_routes`; the core has no list of its own yet): the path, with the boosts it spends.
    fn route_of(script: &Recording, base: &str, number: usize) -> Option<RoutePick> {
        let routes = script.questions.first()?.choice.details.get(crate::tactical::ROUTES_DETAIL)?.get(base)?;
        let list: Vec<crate::tactical::MoveRoute> = serde_json::from_value(routes.clone()).ok()?;
        let chosen = list.into_iter().nth(number)?;
        let path: Option<Vec<System>> = chosen.path.iter().map(|s| <System as IdStr>::from_id(s)).collect();
        Some(RoutePick { path: path?, gd: chosen.gravity_drive, ionian: chosen.ionian })
    }

    /// The core half of one answer of the tactical window (see the module docs). `None` where the core does not follow:
    /// a hook of the activation, diplomacy on, a dropped faction at the table, a route the question does not list, a
    /// nested activity the reaction table does not know, irreproducible dice, a position the codecs refuse.
    #[must_use]
    pub fn step(gs: &mut GameState, content: &ContentStore, sources: SourceSet, galaxy: &Galaxy, at: &At, args: Args<'_>, script: &Recording) -> Option<CoreWindow<Stepped>> {
        let label = label(at);
        if args.failed {
            return None;
        }
        if !content.is_embedded() || gs.diplomacy.enabled || gs.active_space_combat.is_some() || gs.active_invasion.is_some() {
            return None;
        }
        if dropped_faction_at_the_table(gs) {
            return bail(label, "a faction dropped by decision at the table");
        }
        let mut factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        factions.sort_unstable();
        if factions.windows(2).any(|w| w[0] == w[1]) {
            crate::product::unreachable_here("L19 (trade)", "a faction played by two seats");
            return bail(label, "a faction played by two seats");
        }
        if !args.reproducible {
            return bail(label, "irreproducible dice (test faces or a stream restore)");
        }
        nested_check(script)?;
        if script.questions.iter().filter(|q| !q.nested).count() != 1 {
            return bail(label, "a hook of the activation asked a question");
        }
        let answer = script.questions.first().and_then(|q| q.answer.clone())?;
        if matches!(at, At::Activating) {
            let system = SystemId::new(answer.clone());
            if let Some(why) = activation_unported(gs, args.player, &system) {
                return bail(label, why);
            }
        }
        let (mut cx, r) = tables(gs)?;
        let seat = r.seat(args.player.as_str())?;
        let facts = Facts { pool: r.code("tactic") };
        let before = gs.clone();
        let env = env_of(&before, &cx, sources)?;
        // The answer's route suffix is not an option id; the route is a fact of the machine.
        let mut script = script.clone();
        let mut route = None;
        if matches!(at, At::Moving)
            && let Some((base, number)) = crate::tactical::route_suffix(&answer)
        {
            route = route_of(&script, base, number);
            if route.is_none() {
                return bail(label, "a route the question does not list");
            }
            script.questions[0].answer = Some(base.to_owned());
        }
        let seeds = std::rc::Rc::new(crate::dual_window::DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?));
        let resolver = Resolver::new(&[seat], Some(seat));
        let src = Sources { galaxy: Some(galaxy), rng: Some(args.rng_before) };
        let index = Cell::new(0_usize);
        let recorded = script.questions.clone();
        let mut staged = Vec::new();
        let mut rng_after = None;
        let mut out: Option<Ran> = None;
        let driven = with_core(gs, &mut cx, src, nested_fp(WINDOW, &script), |core, _| {
            let mut machine = match at {
                At::Activating => Machine::activating(seat, facts),
                At::Moving => Machine::moving(seat, facts),
                At::Loading { origin, ship, path, loaded, closed } => {
                    let origin = <System as IdStr>::from_id(origin.as_str())?;
                    let kind = <CUnit as IdStr>::from_id(ship.type_id.as_str())?;
                    let piece = Piece::new(kind, seat).with_sustained(ship.sustained_damage).with_galvanized(ship.galvanized);
                    let path: Option<Vec<System>> = path.iter().map(|s| <System as IdStr>::from_id(s)).collect();
                    let loaded: Vec<u16> = loaded.iter().map(|i| u16::try_from(*i).ok()).collect::<Option<_>>()?;
                    Machine::loading(core, &env.kernel, seat, facts, origin, piece, path?, &loaded, *closed)
                }
            }
            .single();
            if let Some(route) = route.take() {
                machine = machine.with_route(route);
            }
            let mut driver = crate::window_driver::driver_for(machine, resolver, AbilityTable::default(), 1).with_seeds(seeds);
            // The details and previews are the recorded question's (A2b-12 produces them from the core).
            let complete = |_: &ti4_engine_core::choice::Choice, _: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
                let i = index.get();
                index.set(i + 1);
                if let Some(recorded) = recorded.get(i) {
                    rendered.details = recorded.choice.details.clone();
                    if rendered.options.len() == recorded.choice.options.len() {
                        for (mine, theirs) in rendered.options.iter_mut().zip(&recorded.choice.options) {
                            mine.preview = theirs.preview.clone();
                        }
                    }
                }
            };
            let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, &script, &complete);
            staged = std::mem::take(&mut core.pending_mut().staged);
            rng_after = Some(core.rng().clone());
            out = Some(Ran { stage: driver.window.stage(), plain: driver.window.plain.clone(), dice: driver.window.dice.clone(), emitted: driver.emitted.clone(), bailed: driver.window.bailed });
            Some(driven)
        });
        let driven = match driven {
            Ok(Some(Ok(d))) => d,
            Ok(Some(Err(_))) => return bail(label, "a run the driver refused"),
            Ok(None) => return bail(label, "a position the codecs refuse"),
            Err(_) => return bail(label, "a position the codecs refuse"),
        };
        let Ran { stage, plain, dice: rift_faces, emitted, bailed } = out?;
        if let Some(why) = bailed {
            return bail(label, &format!("the machine stopped following ({why:?})"));
        }
        apply_staged(gs, &r, &staged)?;
        let rng = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
        let events = emitted.iter().map(|e| event_of(&r, e)).collect::<Option<Vec<_>>>()?;
        let stepped = Stepped { stage: tag_of(stage), plain: if script.nested { Vec::new() } else { plain.into_iter().map(|p| plain_text(&r, p)).collect() }, failed: false, rift_faces, rng };
        Some(CoreWindow { run: Run::new(stepped).with_events_vec(events), questions: driven.rendered })
    }

    /// The core half of `begin_free_tactical` (TE Warfare, Z'eu): no token, no question.
    #[must_use]
    pub fn free(gs: &mut GameState, content: &ContentStore, sources: SourceSet, galaxy: &Galaxy, system: &SystemId, args: Args<'_>, script: &Recording) -> Option<CoreWindow<Stepped>> {
        const LABEL: &str = "tactical::free";
        if args.failed || !content.is_embedded() || gs.diplomacy.enabled || gs.active_space_combat.is_some() || gs.active_invasion.is_some() {
            return None;
        }
        if dropped_faction_at_the_table(gs) {
            return bail(LABEL, "a faction dropped by decision at the table");
        }
        let mut factions: Vec<&str> = gs.players.iter().map(|p| p.faction.as_str()).collect();
        factions.sort_unstable();
        if factions.windows(2).any(|w| w[0] == w[1]) {
            crate::product::unreachable_here("L19 (trade)", "a faction played by two seats");
            return bail(LABEL, "a faction played by two seats");
        }
        nested_check(script)?;
        if script.questions.iter().any(|q| !q.nested) {
            return bail(LABEL, "a free activation asked a question");
        }
        let (mut cx, r) = tables(gs)?;
        let seat = r.seat(args.player.as_str())?;
        let target = <System as IdStr>::from_id(system.as_str())?;
        let facts = Facts { pool: r.code("tactic") };
        let before = gs.clone();
        let env = env_of(&before, &cx, sources)?;
        let resolver = Resolver::new(&[seat], Some(seat));
        let src = Sources { galaxy: Some(galaxy), rng: Some(args.rng_before) };
        let mut staged = Vec::new();
        let mut rng_after = None;
        let mut out = None;
        let driven = with_core(gs, &mut cx, src, nested_fp(WINDOW, script), |core, _| {
            let mut driver = crate::window_driver::driver_for(Machine::free(seat, facts, target), resolver, AbilityTable::default(), 1);
            let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            rng_after = Some(core.rng().clone());
            out = Some((driver.window.stage(), driver.window.plain.clone(), driver.emitted.clone()));
            driven
        });
        let driven = match driven {
            Ok(Ok(d)) => d,
            _ => return bail(LABEL, "a position the codecs refuse"),
        };
        let (stage, plain, emitted) = out?;
        apply_staged(gs, &r, &staged)?;
        let rng = crate::core_sync::codec::rng::positions(&cx, &rng_after?).ok()?;
        let events = emitted.iter().map(|e| event_of(&r, e)).collect::<Option<Vec<_>>>()?;
        let stepped = Stepped { stage: tag_of(stage), plain: if script.nested { Vec::new() } else { plain.into_iter().map(|p| plain_text(&r, p)).collect() }, failed: false, rift_faces: Vec::new(), rng };
        Some(CoreWindow { run: Run::new(stepped).with_events_vec(events), questions: driven.rendered })
    }

    /// What fleet enforcement reads and writes: the units of the system (Board) with the scope and the rules version
    /// (Meta); the limits are facts of the string.
    pub(crate) const ENFORCE: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Board]);

    /// The string's limits for the enforcement of `player` in `system`: the one the enforcement judges by, the one the question
    /// shows, the half-weight fighters. `Err`: more than two half-weight fighter forms, or a unit the tables do not know.
    pub(crate) fn limits_of(gs: &GameState, content: &ContentStore, player: &ti4_model::id::PlayerId, system: &SystemId) -> Result<ti4_engine_core::rules::tactical::fleet::Limits, &'static str> {
        use ti4_engine_core::rules::tactical::fleet::Limits;
        let enforce = crate::fleet::limit_for_enforcement(gs, content, player);
        let shown = if crate::fleet::is_unlimited(gs, player) { crate::fleet::UNLIMITED_FLEET_BILL } else { i64::from(crate::fleet::limit(gs, content, player)).max(0) };
        let mut halves = [None, None];
        let here = gs.system_state(system);
        let mut types: Vec<&str> = here.units.iter().filter(|u| &u.owner == player).map(|u| u.type_id.as_str()).collect();
        types.sort_unstable();
        types.dedup();
        let mut found = 0;
        for id in types {
            if crate::factions::hooks_combat::fighter_fleet_weight_halves(gs, content, player, id) {
                if found >= halves.len() {
                    return Err("more than two half-weight fighter forms");
                }
                halves[found] = Some(<CUnit as IdStr>::from_id(id).ok_or("a unit the tables do not know")?);
                found += 1;
            }
        }
        Ok(Limits { enforce, shown, halves })
    }

    /// The core half of `fleet::enforce_seeing`: the units over the fleet supply, then over capacity, removed one question
    /// at a time. The value is the number of units removed (`None`: the string refused an answer).
    #[must_use]
    pub fn enforce(gs: &mut GameState, content: &ContentStore, sources: SourceSet, player: &ti4_model::id::PlayerId, system: &SystemId, script: &Recording) -> Option<CoreWindow<Option<usize>>> {
        use ti4_engine_core::rules::tactical::fleet::Enforce;
        const LABEL: &str = "tactical::enforce";
        if !content.is_embedded() || script.nested {
            return None;
        }
        if script.questions.iter().any(|q| q.answer.is_none()) {
            return None;
        }
        let (mut cx, r) = tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let at = <System as IdStr>::from_id(system.as_str())?;
        let limits = match limits_of(gs, content, player, system) {
            Ok(limits) => limits,
            Err(why) => return bail(LABEL, why),
        };
        let before = gs.clone();
        let kernel = crate::core_adapter::kernel_for(sources);
        let resolver = Resolver::new(&[seat], Some(seat));
        let mut out = None;
        let driven = with_core(gs, &mut cx, Sources::default(), ENFORCE, |core, _| {
            let mut driver = crate::window_driver::driver_for(Enforce::new(seat, at, limits), resolver, AbilityTable::default(), 1);
            let driven = drive_with(&mut driver, core, kernel, &r, &before, script, &|_, _, _| {});
            out = Some(driver.window.removed);
            driven
        });
        let driven = match driven {
            Ok(Ok(d)) => d,
            _ => return bail(LABEL, "a position the codecs refuse"),
        };
        let removed = out?;
        Some(CoreWindow { run: Run::new(Some(removed as usize)).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What the head of the close of the movement step holds that the machine does not follow: Nova Seed's map edits, the
    /// Sardakk hero (it asks), a Floating Factory in the active system (a blockade destroys it).
    #[must_use]
    pub fn finish_unported(gs: &GameState, content: &ContentStore, sources: SourceSet, system: &SystemId) -> Option<&'static str> {
        if gs.faction_marks.keys().any(|k| k.starts_with(crate::movement::MAP_EDIT_PREFIX)) {
            return Some("map edits (Nova Seed)");
        }
        if gs.players.iter().any(|p| p.leaders.get("sardakkhero") == Some(&ti4_model::state::LeaderStatus::Unlocked)) {
            return Some("the Sardakk hero is unlocked");
        }
        let types = ti4_content::units::catalogue(content, sources);
        if gs
            .system_state(system)
            .units
            .iter()
            .any(|u| types.get(u.type_id.as_str()).is_some_and(|k| k.is_space_only_structure() && k.has_production()))
        {
            return Some("a Floating Factory in the active system");
        }
        None
    }

    /// The core half of the head of `finish_tactical_inner`.
    #[must_use]
    pub fn finish(gs: &mut GameState, content: &ContentStore, sources: SourceSet, galaxy: &Galaxy, player: &ti4_model::id::PlayerId, failed: bool, script: &Recording) -> Option<CoreWindow<super::Closed>> {
        const LABEL: &str = "tactical::finish";
        if failed || !content.is_embedded() || gs.diplomacy.enabled || gs.active_space_combat.is_some() || gs.active_invasion.is_some() {
            return None;
        }
        if dropped_faction_at_the_table(gs) {
            return bail(LABEL, "a faction dropped by decision at the table");
        }
        let system = gs.active_system.clone()?;
        if let Some(why) = finish_unported(gs, content, sources, &system) {
            return bail(LABEL, why);
        }
        nested_check(script)?;
        if script.questions.iter().any(|q| !q.nested) {
            return bail(LABEL, "a hook of the close asked a question");
        }
        let (mut cx, r) = tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let facts = Facts { pool: r.code("tactic") };
        let before = gs.clone();
        let env = env_of(&before, &cx, sources)?;
        let resolver = Resolver::new(&[seat], Some(seat));
        let src = Sources { galaxy: Some(galaxy), rng: None };
        let mut staged = Vec::new();
        let mut out = None;
        let driven = with_core(gs, &mut cx, src, nested_fp(WINDOW, script), |core, _| {
            let mut driver = crate::window_driver::driver_for(Machine::closing(seat, facts), resolver, AbilityTable::default(), 1);
            let driven = drive_with(&mut driver, core, &env.kernel, &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            out = Some((driver.window.closed, driver.emitted.clone(), driver.window.plain.clone()));
            driven
        });
        let driven = match driven {
            Ok(Ok(d)) => d,
            _ => return bail(LABEL, "a position the codecs refuse"),
        };
        let (closed, emitted, plain) = out?;
        let closed = closed?;
        apply_staged(gs, &r, &staged)?;
        let events = emitted.iter().map(|e| event_of(&r, e)).collect::<Option<Vec<_>>>()?;
        let value = super::Closed { system: closed.system.as_str().to_owned(), skip_to_commit: closed.skip_to_commit, plain: if script.nested { Vec::new() } else { plain.into_iter().map(|p| plain_text(&r, p)).collect() }, failed: false };
        Some(CoreWindow { run: Run::new(value).with_events_vec(events), questions: driven.rendered })
    }
}
