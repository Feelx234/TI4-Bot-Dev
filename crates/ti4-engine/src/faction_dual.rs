//! `Area::Factions` (package W11 of lent-bodies lane 5): the core halves of the faction component actions the core owns
//! (`ti4_engine_core::rules::faction_component`), beside their string bodies in `factions/*.rs`.
//!
//! A core half answers `None` (the string result stands, counted as a bail) where the position holds more than the body: a
//! unit type the typed tables do not know, a question inside a battle or an invasion, diplomacy on, a nested emission.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::generated::{System, Unit};
    use ti4_engine_core::rules::faction_component as rules;
    use ti4_engine_core::state::Slice;
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::core_sync::{Footprint, Sources, with_core};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::Recording;

    /// What Stall Tactics reads (hand, planets, box, laws) and writes (hand, the staged discard mark, the units).
    const STALL: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Marks],
    );

    /// What the leaf bodies read (the cards, the planets, the box) and write (the exhausted card, the units).
    const LEAF: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Marks], &[Slice::Players, Slice::Board]);

    /// `scope` of the sources, as the core reads it.
    fn scope(sources: ti4_model::content_types::SourceSet) -> Option<ti4_engine_core::tables::Scope> {
        Some(ti4_engine_core::tables::Scope(u32::try_from(crate::core_adapter::scope(sources).0).ok()?))
    }

    /// The core half of Winnu's Lazax Gate Folding (`factions::winnu::perform_component`; the value: the action resolved).
    /// `form` is the infantry the string would place (`None`: nothing, or a faction rule forbids it).
    pub fn lazax_gate_folding(
        gs: &mut GameState,
        player: &PlayerId,
        mecatol: &str,
        form: Option<&UnitTypeId>,
        sources: ti4_model::content_types::SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let mecatol = crate::core_adapter::typed::<System>(mecatol).why(crate::dual::reason::UNKNOWN_ID)?;
        let form = match form {
            Some(id) => Some(crate::core_adapter::typed::<Unit>(id.as_str()).why("a unit form the typed tables do not know")?),
            None => None,
        };
        let scope = scope(sources)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let done = with_core(gs, &mut cx, Sources::default(), LEAF, |core, _| rules::lazax_gate_folding(core, scope, seat, mecatol, form))
            .ok()
            .why(crate::dual::reason::NOT_LENDABLE)?;
        Some(CoreWindow { run: Run::new(done).with_events_vec(Vec::new()), questions: Vec::new() })
    }

    /// The core half of Yssaril's Stall Tactics (`factions::yssaril::stall_tactics`; the value: the action resolved). `mech` is the form the
    /// seat would place (`None`: nothing, or a faction rule forbids it).
    pub fn stall_tactics(
        gs: &mut GameState,
        player: &PlayerId,
        mech: Option<&UnitTypeId>,
        sources: ti4_model::content_types::SourceSet,
        script: &Recording,
    ) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::driver::Driver;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::{Kernel, Resolver};
        use ti4_engine_core::rules::promissory_windows::StallTactics;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let unit = match mech {
            Some(id) => Some(crate::core_adapter::typed::<Unit>(id.as_str()).why("a unit form the typed tables do not know")?),
            None => None,
        };
        let scope = scope(sources)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut table = AbilityTable::default();
        table.facts.card_groups = crate::reactions_core::card_groups();
        let mut driver = Driver::new(StallTactics::new(seat, unit, scope, r.code("stall_tactics"), r.code("yssaril_mech")), Resolver::new(&seats, Some(seat)), table, 1);
        let driven = with_core(gs, &mut cx, Sources::default(), STALL, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|c, _, rendered| crate::render::promissory::complete_place_units(c, rendered))
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What the Nekro PDS reads (the planets, the structure cap, the box, the laws) and writes (the PDS and the exhausted card).
    const C4R_PLACE: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda, Slice::Marks, Slice::Pending],
        &[Slice::Players, Slice::Board],
    );

    /// The core half of Nekro's ???_ERROR_ERROR_??? PDS (`factions::nekro_units::place_pds`; the value: the PDS was placed and the card
    /// exhausted). `pds` is the form the seat places (`None`: nothing). A seat that carries the text on something else than its own card
    /// (a Valefar Assimilator) and a faction rule that forbids the placement are the string's.
    pub fn c4r_place_pds(gs: &mut GameState, player: &PlayerId, pds: Option<&UnitTypeId>, forbidden: bool, sources: ti4_model::content_types::SourceSet, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::driver::Driver;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::{Kernel, Resolver};
        use ti4_engine_core::rules::promissory_windows::{PlaceUnits, Then};
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if forbidden {
            crate::dual::note_bail_here("a faction rule forbids the placement of the PDS (the string asks the planet, then places nothing)");
            return None;
        }
        if !gs.player(player).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&"nekroc4r"))) {
            crate::dual::note_bail_here("???_ERROR_ERROR_??? carried by something other than the player's own card (a Valefar Assimilator)");
            return None;
        }
        let unit = match pds {
            Some(id) => Some(crate::core_adapter::typed::<Unit>(id.as_str()).why("a unit form the typed tables do not know")?),
            None => None,
        };
        let scope = scope(sources)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(PlaceUnits::new(seat, unit, 1, None, false, scope, r.code("nekroc4r"), Then::C4rPds), Resolver::new(&seats, Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), C4R_PLACE, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {})
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.placed > 0).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What the Nekro draw reads and writes: the hand and the deck, the staged discard mark, the laws that cap the hand.
    const C4R_DRAW: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Decks, Slice::Agenda, Slice::Marks, Slice::Cards, Slice::Pending],
        &[Slice::Players, Slice::Decks, Slice::Marks, Slice::Cards],
    );

    /// The core half of Nekro's ???_ERROR_ERROR_??? draw (`factions::nekro_units::discard_and_draw`; the value: the action resolved). A
    /// draw a faction hook or a law changes, a replayed turn and a seat that carries the text on a Valefar Assimilator are the string's.
    pub fn c4r_draw(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::driver::Driver;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::{Kernel, Resolver};
        use ti4_engine_core::rules::promissory_windows::C4rDraw;
        if let Some(why) = crate::dual_window::outside_why(gs, script).or_else(|| gs.deck_reserve.is_some().then_some("deck reserve (a turn redo replays the draws)")).or_else(|| crate::strategy_cards_core::draw_hook(gs, content, player, 1)) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if !gs.player(player).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&"nekroc4r"))) {
            crate::dual::note_bail_here("???_ERROR_ERROR_??? carried by something other than the player's own card (a Valefar Assimilator)");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut table = AbilityTable::default();
        table.facts.card_groups = crate::reactions_core::card_groups();
        ti4_engine_core::driver::children::register(&mut table);
        let mut driver = Driver::new(C4rDraw::new(seat, gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK), Resolver::new(&seats, Some(seat)), table, 1);
        let driven = with_core(gs, &mut cx, Sources::default(), C4R_DRAW, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {})
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the draw window or no printed-name groups");
            return None;
        }
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// The core half of Yssaril's Mageon Implants (`factions::yssaril::mageon_implants`; the value: the action resolved). A hand limit a
    /// faction hook changes is not carried (the child window reads the printed rule and the laws): the string's.
    pub fn mageon_implants(gs: &mut GameState, content: &ti4_content::ContentStore, player: &PlayerId, target: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::driver::Driver;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::{Kernel, Resolver};
        use ti4_engine_core::rules::promissory_windows::MageonImplants;
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if crate::laws::blanked(gs) {
            crate::dual::note_bail_here("a turn in which the laws are blank (Law's Order)");
            return None;
        }
        let plain = crate::laws::action_card_limit(gs, crate::action_cards::HAND_LIMIT);
        let limit = crate::factions::hooks_economy::action_card_limit(gs, content, player, plain);
        if limit != plain && limit != usize::MAX {
            crate::dual::note_bail_here("a faction changes the action card hand limit");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let target = r.seat(target.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut table = AbilityTable::default();
        table.facts.card_groups = crate::reactions_core::card_groups();
        ti4_engine_core::driver::children::register(&mut table);
        let mut driver = Driver::new(MageonImplants::new(seat, target, limit == plain), Resolver::new(&seats, Some(seat)), table, 1);
        let driven = with_core(gs, &mut cx, Sources::default(), C4R_DRAW, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {})
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("the table has no link to the hand limit window or no printed-name groups");
            return None;
        }
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What Fabrication and Black Market Forgery read (the fragments, the box of command tokens, the notes) and write (the fragments,
    /// the relic, the token, the note going home).
    const FABRICATION: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Ledger, Slice::Pending],
        &[Slice::Meta, Slice::Players, Slice::Decks, Slice::Cards, Slice::Ledger],
    );

    /// The core half of the Naaz Fabrication (`token`: the command token action; else the relic) and of Black Market Forgery (`bmf`)
    /// (`factions::naaz::perform_component`; the value: the action resolved). A turn redo's deck reserve and a command token in a Mahact
    /// fleet pool are the string's.
    pub fn naaz_fabrication(gs: &mut GameState, player: &PlayerId, token: bool, bmf: bool, script: &Recording) -> Option<CoreWindow<bool>> {
        use ti4_engine_core::driver::Driver;
        use ti4_engine_core::kernel::stage::AbilityTable;
        use ti4_engine_core::kernel::{Kernel, Resolver};
        use ti4_engine_core::rules::relic_windows::{FabMode, Fabrication};
        if let Some(why) = crate::dual_window::outside_why(gs, script)
            .or_else(|| gs.deck_reserve.is_some().then_some("deck reserve (a turn redo replays the draws)"))
            .or_else(|| (gs.tokens_held_by_mahact(player) > 0).then_some("a command token in a Mahact fleet pool (the reinforcements differ)"))
        {
            crate::dual::note_bail_here(why);
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let seats: Vec<ti4_engine_core::ids::Seat> = r.seats.iter().filter_map(|text| r.seat(text)).collect();
        let before = gs.clone();
        let kernel = Kernel::plain();
        let (mode, source) = if token { (FabMode::Token, "fabrication") } else if bmf { (FabMode::Relic { bmf: true }, "bmf") } else { (FabMode::Relic { bmf: false }, "fabrication") };
        let mut driver = Driver::new(Fabrication::new(seat, mode, r.code(source)), Resolver::new(&seats, Some(seat)), AbilityTable::default(), 1);
        let driven = with_core(gs, &mut cx, Sources::default(), FABRICATION, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, _, _| {})
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        Some(CoreWindow { run: Run::new(driver.window.done).with_events_vec(Vec::new()), questions: driven.rendered })
    }

    /// What Terraform reads and writes: the note (Cards), the planet and its attachment (Board).
    const TERRAFORM: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards],
        &[Slice::Board, Slice::Cards],
    );

    /// The core half of the Titans' Terraform (`factions::titans_leaders::perform_component`; the value: the action resolved). `home` is
    /// the string's read of whether the planet is anybody's home planet.
    pub fn terraform(gs: &mut GameState, player: &PlayerId, planet: &str, home: bool, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        let planet = crate::core_adapter::typed::<ti4_engine_core::generated::Planet>(planet).why(crate::dual::reason::UNKNOWN_ID)?;
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let done = with_core(gs, &mut cx, Sources::default(), TERRAFORM, |core, _| rules::terraform(core, seat, planet, home)).ok().why(crate::dual::reason::NOT_LENDABLE)?;
        Some(CoreWindow { run: Run::new(done).with_events_vec(Vec::new()), questions: Vec::new() })
    }

    /// The core half of Nekro's ???_ERROR_ERROR_??? repair (`factions::nekro_units::perform_component`; the value: the action resolved).
    /// A seat that carries the text on something else than its own card (a Valefar Assimilator) is the string's.
    pub fn c4r_repair(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if !gs.player(player).is_some_and(|p| p.technologies.contains(ti4_model::id::text(&"nekroc4r"))) {
            crate::dual::note_bail_here("???_ERROR_ERROR_??? carried by something other than the player's own card (a Valefar Assimilator)");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let done = with_core(gs, &mut cx, Sources::default(), LEAF, |core, _| rules::c4r_repair(core, seat)).ok().why(crate::dual::reason::NOT_LENDABLE)?;
        Some(CoreWindow { run: Run::new(done).with_events_vec(Vec::new()), questions: Vec::new() })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::{PlayerId, UnitTypeId};
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn lazax_gate_folding(
        _: &mut GameState,
        _: &PlayerId,
        _: &str,
        _: Option<&UnitTypeId>,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }

    pub fn c4r_repair(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn c4r_place_pds(_: &mut GameState, _: &PlayerId, _: Option<&UnitTypeId>, _: bool, _: ti4_model::content_types::SourceSet, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn terraform(_: &mut GameState, _: &PlayerId, _: &str, _: bool, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn naaz_fabrication(_: &mut GameState, _: &PlayerId, _: bool, _: bool, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn mageon_implants(_: &mut GameState, _: &ti4_content::ContentStore, _: &PlayerId, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn stall_tactics(
        _: &mut GameState,
        _: &PlayerId,
        _: Option<&UnitTypeId>,
        _: ti4_model::content_types::SourceSet,
        _: &Recording,
    ) -> Option<CoreWindow<bool>> {
        None
    }
    pub fn c4r_draw(_: &mut GameState, _: &ti4_content::ContentStore, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
}
