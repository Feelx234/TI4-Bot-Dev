//! The live `Env` (package KF2): the ability registry and table of a game, built from the CORE position and the render tables
//! alone. The replay path builds the same thing from the string `GameState` (`reactions_core::Follow::armed` + `arm`); this is its
//! live counterpart, and the two share `reactions_core::{table, set_table_codes, id_text, ability_id}`, so the ids agree.
//!
//! The PLAY FACTS of an action card played inside a window (`AbilityTable::facts.plays`: the unit a seat places, Reveal Prototype's
//! research set, the Mahact pools, the blanked laws) are position-dependent, so they are not kept here: [`env_with_plays`] hangs the
//! provider of `live::plays` on the table and the core's `driver::live` computes them from the position at every call (on the first
//! read of the call, from a snapshot of the position it started from: most calls read none). [`env_for`]
//! alone leaves them out: a card played inside such an `Env`'s window stops the step with `LiveError::Unknown(PlayFacts)`, which the
//! playthrough counts as a bail (never a silent skip).

use std::sync::atomic::{AtomicBool, Ordering};

use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::Env;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::stage::{AbilityTable, Spec};
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::state::CoreState;

use crate::kernel_sync::text::ability_id;
use crate::ties::reactions_core::{id_text, set_table_codes, table};
use crate::render::Render;

/// The environment of a live game at `core`: every seat's abilities in the string order, the table with its codes. Built once per game (the registry is derived from the position; call again when the seats' abilities change: a faction
/// swap, a copied ability).
#[must_use]
pub fn env_for<'a>(r: &Render, core: &CoreState, kernel: &'a Kernel) -> Env<'a> {
    let order: Vec<Seat> = (0..core.players().len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
    let faction = |who: Seat| core.players().by_seat(who).map_or_else(String::new, |p| crate::generated::IdStr::as_str(p.faction).to_owned());
    let mut table = table();
    set_table_codes(&mut table, r);
    // The string abilities that have no rule yet: a native game stops (a counted bail) where a seat holds one, it never plays on without it.
    ti4_engine_core_factions::silent::gaps::register_live(&mut table);
    table.facts.staging = false;
    // The faction units (the form an Assimilate seat places on a planet it takes).
    table.facts.roster = crate::ties::technology_core::roster_for(ti4_content::ContentStore::embedded(), ti4_model::content_types::FULL);
    // An exploration (the invasion's first claim): the event texts, the words of its log row and the printed names of the action cards.
    table.codes.explore = crate::ties::exploring_dual::codes(r);
    table.facts.explore_words = Some(crate::ties::exploring_dual::words);
    let faction_of = |who: Seat| core.players().by_seat(who).map(|p| p.faction);
    let mut name = |spec: &Spec, who: Seat, lender: Option<Seat>| {
        let key = crate::render::ability_key(spec.slot.0, who.0, lender.map(|l| l.0), faction_of(who), lender.and_then(faction_of));
        r.ability_id_memo(key, || ability_id(r, &id_text(r, spec, &faction(who), r.seat_text(who), lender.map(|l| (faction(l), r.seat_text(l).to_owned())).as_ref())))
    };
    let defs = table.registry(core, &order, &mut name);
    let mut resolver = Resolver::new(&order, None);
    resolver.register(defs);
    Env { kernel, registry: Cow::new(resolver.registry().clone()), table }
}

/// [`env_for`] with the play facts: a card played from inside a live window finds the facts of the position it is played in
/// (`live::plays`), computed from the core alone. `sources` is the game's content scope (the unit roster of the forms).
///
/// The ability registry and the table are kept in the [`Render`] between calls ([`Template`]) and reused while [`Key`] holds; see there for what the
/// registry reads. [`env_with_plays_uncached`] builds the same thing from scratch (the definition); with [`verify_cache`] on, every reuse is checked
/// against it.
#[must_use]
pub fn env_with_plays<'a>(r: &Render, core: &CoreState, kernel: &'a Kernel, sources: ti4_model::content_types::SourceSet) -> Env<'a> {
    let key = Key::of(r, core, kernel, sources);
    let hit = r.env_template.borrow().as_ref().filter(|t| t.key == key).map(|t| (t.registry.clone(), t.table.clone()));
    if let Some((registry, table)) = hit {
        let env = Env { kernel, registry, table };
        if VERIFY.load(Ordering::Relaxed) {
            let fresh = env_with_plays_uncached(r, core, kernel, sources);
            assert_same(&env, &fresh, core, kernel);
        }
        return env;
    }
    let env = env_with_plays_uncached(r, core, kernel, sources);
    *r.env_template.borrow_mut() = Some(Template { key, registry: env.registry.clone(), table: env.table.clone() });
    env
}

/// [`env_with_plays`] built from scratch, the cache ignored and left as it was.
#[must_use]
pub fn env_with_plays_uncached<'a>(r: &Render, core: &CoreState, kernel: &'a Kernel, sources: ti4_model::content_types::SourceSet) -> Env<'a> {
    let mut env = env_for(r, core, kernel);
    env.table.facts.plays_provider = Some(super::plays::provider(r, sources));
    // The faction units of the game's own scope: a game without Thunder's Edge places the Naalu mech, not its reprint (`env_for` names the full scope,
    // which the buildable units of a production step and the form an Assimilate seat places would follow).
    env.table.facts.roster = crate::ties::technology_core::roster_for(ti4_content::ContentStore::embedded(), sources);
    // The provider reads the position, this kernel and this roster only: with a live kernel the facts of a call are computed when first read
    // (`AdapterFacts::arm_plays`); a kernel built by hand keeps the facts computed at once.
    env.table.facts.plays_source = super::kernel::exact_static(kernel).map(|kernel| ti4_engine_core::kernel::ability::PlaysSource { kernel, roster: env.table.facts.roster });
    // Losses are announced when some seat's faction has a module (`supply::staging_enabled`): fixed by the seating, set once.
    env.table.facts.staging = super::plays::staging_enabled(core);
    env
}

/// What the registry and the table of [`env_with_plays`] are a function of, besides the `Render` they are kept in (whose code table only grows and
/// whose texts for a seat do not change): the registry holds, per seat, the specs whose `Applies` holds for it and the pairs a lender lends to a
/// borrower, and the table holds the facts of the scope. The specs read (audit of every `Applies::Held`, `Applies::Faction` and `Spec::pair` of the
/// core and the faction modules): a seat's **faction**, the **leader cards a seat holds** (`leaders.status`: the key, and the status where a pair reads it)
/// and the rules version. Nothing else of the position: not the board, the cards in hand, the techs, the laws. So the key is the number of seats, the
/// faction and the `(leader, status)` list of each, the rules version, the kernel, the scope and a hash of the render's seat and faction texts.
/// A test (`perf_kernel` with `PERF_KERNEL_CHECK=1`) rebuilds the `Env` from scratch at every step of many games and compares it with the reused one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    seats: Vec<u32>,
    rules_version: u32,
    kernel: usize,
    sources: u32,
    texts: u64,
}

impl Key {
    fn of(r: &Render, core: &CoreState, kernel: &Kernel, sources: ti4_model::content_types::SourceSet) -> Self {
        let mut seats = Vec::with_capacity(core.players().len() * 6 + 1);
        for p in core.players().iter() {
            seats.push(p.faction.index() as u32);
            seats.push(p.leaders.status.len() as u32);
            for (leader, status) in p.leaders.status.iter() {
                seats.push((leader.index() as u32) << 8 | *status as u32);
            }
        }
        // (FNV-1a over the render's seat and faction texts: set when the game is seated, kept in the key so that a change would show.)
        let mut texts = 0xcbf2_9ce4_8422_2325_u64;
        for text in r.seats.iter().chain(r.factions.iter()) {
            for b in text.bytes().chain(std::iter::once(0xff)) {
                texts = (texts ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        Self { seats, rules_version: core.meta().rules_version, kernel: std::ptr::from_ref(kernel) as usize, sources: sources.as_u32(), texts }
    }
}

/// The `Env` pieces kept between calls (see [`Key`]).
pub struct Template {
    key: Key,
    registry: Cow<Registry>,
    table: AbilityTable,
}

impl std::fmt::Debug for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Template").field("key", &self.key).finish_non_exhaustive()
    }
}

static VERIFY: AtomicBool = AtomicBool::new(false);

/// Check every reuse of the kept `Env` against one built from scratch (a test mode: panics with the difference). Off by default.
pub fn verify_cache(on: bool) {
    VERIFY.store(on, Ordering::Relaxed);
}

/// Two `Env`s of one position are the same: the registry row by row, the table's slots, codes and facts, the play facts of the position.
fn assert_same(cached: &Env<'_>, fresh: &Env<'_>, core: &CoreState, kernel: &Kernel) {
    assert_eq!(format!("{:?}", cached.registry), format!("{:?}", fresh.registry), "the reused registry differs from a fresh one");
    let slots = |e: &Env<'_>| e.table.specs().iter().map(|s| s.slot).collect::<Vec<_>>();
    assert_eq!(slots(cached), slots(fresh), "the reused table has other specs");
    assert_eq!(format!("{:?}", cached.table.codes), format!("{:?}", fresh.table.codes), "the reused table has other codes");
    assert_eq!(format!("{:?}", cached.table.facts), format!("{:?}", fresh.table.facts), "the reused table has other facts");
    let facts = |e: &Env<'_>| e.table.facts.plays_provider.as_ref().map(|p| format!("{:?}", (p.0)(core, kernel)));
    assert_eq!(facts(cached), facts(fresh), "the reused provider answers other play facts");
}
