//! The live `Env` (package KF2): the ability registry and table of a game, built from the CORE position and the render tables
//! alone. The replay path builds the same thing from the string `GameState` (`reactions_core::Follow::armed` + `arm`); this is its
//! live counterpart, and the two share `reactions_core::{table, set_table_codes, id_text, ability_id}`, so the ids agree.
//!
//! The PLAY FACTS of an action card played inside a window (`AbilityTable::facts.plays`: the unit a seat places, Reveal Prototype's
//! research set, the Mahact pools, the blanked laws) are position-dependent, so they are not kept here: [`env_with_plays`] hangs the
//! provider of `live::plays` on the table and the core's `driver::live` computes them from the position at every call. [`env_for`]
//! alone leaves them out: a card played inside such an `Env`'s window stops the step with `LiveError::Unknown(PlayFacts)`, which the
//! playthrough counts as a bail (never a silent skip).

use std::rc::Rc;

use ti4_engine_core::containers::Cow;
use ti4_engine_core::driver::live::Env;
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::stage::Spec;
use ti4_engine_core::kernel::{Kernel, Resolver};
use ti4_engine_core::state::{CoreState, SeedSource};

use crate::kernel_sync::text::ability_id;
use crate::reactions_core::{id_text, set_table_codes, table};
use crate::render::Render;

/// The environment of a live game at `core`: every seat's abilities in the string order, the table with its codes, the game's
/// seeds. Built once per game (the registry is derived from the position; call again when the seats' abilities change: a faction
/// swap, a copied ability).
#[must_use]
pub fn env_for<'a>(r: &Render, core: &CoreState, kernel: &'a Kernel, seeds: Rc<dyn SeedSource>) -> Env<'a> {
    let order: Vec<Seat> = (0..core.players().len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
    let faction = |who: Seat| core.players().by_seat(who).map_or_else(String::new, |p| crate::generated::IdStr::as_str(p.faction).to_owned());
    let mut table = table();
    set_table_codes(&mut table, r);
    table.facts.staging = false;
    // The faction units (the form an Assimilate seat places on a planet it takes).
    table.facts.roster = crate::technology_core::roster_for(ti4_content::ContentStore::embedded(), ti4_model::content_types::FULL);
    // An exploration (the invasion's first claim): the event texts, the words of its log row and the printed names of the action cards.
    table.codes.explore = crate::exploring_dual::codes(r);
    table.facts.explore_words = Some(crate::exploring_dual::words);
    let mut name = |spec: &Spec, who: Seat, lender: Option<Seat>| {
        ability_id(r, &id_text(r, spec, &faction(who), r.seat_text(who), lender.map(|l| (faction(l), r.seat_text(l).to_owned())).as_ref()))
    };
    let defs = table.registry(core, &order, &mut name);
    let mut resolver = Resolver::new(&order, None);
    resolver.register(defs);
    Env { kernel, registry: Cow::new(resolver.registry().clone()), table, seeds }
}

/// [`env_for`] with the play facts: a card played from inside a live window finds the facts of the position it is played in
/// (`live::plays`), computed from the core alone. `sources` is the game's content scope (the unit roster of the forms).
#[must_use]
pub fn env_with_plays<'a>(r: &Render, core: &CoreState, kernel: &'a Kernel, seeds: Rc<dyn SeedSource>, sources: ti4_model::content_types::SourceSet) -> Env<'a> {
    let mut env = env_for(r, core, kernel, seeds);
    env.table.facts.plays_provider = Some(super::plays::provider(r, sources));
    env
}
