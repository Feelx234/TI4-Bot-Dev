//! `Area::ExplorationRelics` (package W16), the legendary planet abilities of `legendary.rs` as lockstep windows:
//! `legendary::end_turn` (the abilities used at the end of the holder's turn) and `legendary::pass` (the abilities used as the
//! holder passes), both the core window `ti4_engine_core::rules::legendary::Legendary`; and Styx's victory point
//! (`legendary::settle_control_points`).
//!
//! Each core half answers `None` (the string result stands, counted as a bail with its reason) where the position holds something
//! the core does not: see `docs/ENGINE_KNOWN_ISSUES.md`, W16.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_content::ContentStore;
    use ti4_engine_core::choice::legendary::When;
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::generated::{Planet, System};
    use ti4_engine_core::kernel::Resolver;
    use ti4_engine_core::rules::legendary::Legendary;
    use ti4_engine_core::state::Slice;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::core_sync::{with_core, Footprint, Sources};
    use crate::dual::{CoreWindow, Run, Why};
    use crate::dual_window::{apply_staged, drive_with, Recording};
    use crate::generated::IdStr;

    /// The slices the abilities read (the placement ban and the reinforcements, the hand limit of a draw, the decks) and write.
    const LEGENDARY: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending],
        &[Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Transient, Slice::Pending],
    );

    /// Styx's victory point: the slices `rules::legendary::settle_styx` reads and writes (the holder is in the meta, the points in the
    /// players and the ledger).
    const STYX: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board], &[Slice::Meta, Slice::Players, Slice::Ledger]);

    /// `legendary::settle_control_points` as one core write. `None`: the position is outside what the core holds.
    pub fn settle_styx(gs: &mut GameState) -> Option<()> {
        crate::dual_window::lend(gs, STYX, None, |core, _| ti4_engine_core::rules::legendary::settle_styx(core))
    }

    /// The slices the salvage yard's purge reads (who holds Garbozia) and writes.
    const PURGE: Footprint = Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards], &[Slice::Cards]);

    /// `legendary::purge_salvaged` as one core write: whether the card was purged. `None`: outside what the core holds.
    pub fn purge_salvaged(gs: &mut GameState, player: &PlayerId, card: &str) -> Option<bool> {
        let card = ti4_engine_core::generated::ActionCard::from_id(card).why(crate::dual::reason::UNKNOWN_ID)?;
        crate::dual_window::lend(gs, PURGE, Some(player), |core, who| ti4_engine_core::rules::legendary::purge_salvaged(core, who.expect("a seated call"), card))
    }

    /// The prompt of the question the Maxis Central Control offers (a planet named in its answer ends the core's run).
    const MAXIS_PROMPT: &str = "Maxis Central Control: gain control of which planet";

    /// The core half of `legendary::end_turn` and `legendary::pass`. `None` where the position holds something the core does not:
    /// a nested activity, a battle, a planet Maxis Central Control takes (its control chain and exploration), a card drawn through a
    /// faction hook or a hand over the limit the table cannot judge, the Galactic Council with a hand of secrets over the limit,
    /// Artuno readied by The Acropolis.
    #[allow(clippy::too_many_arguments)]
    pub fn window(
        gs: &mut GameState,
        content: &ContentStore,
        sources: SourceSet,
        galaxy: Option<&ti4_content::galaxy::Galaxy>,
        player: &PlayerId,
        when: When,
        script: &Recording,
    ) -> Option<CoreWindow<()>> {
        let answered = |prompt: &str, f: &dyn Fn(&str) -> bool| script.questions.iter().any(|q| q.choice.prompt.starts_with(prompt) && q.answer.as_deref().is_some_and(f));
        if answered(MAXIS_PROMPT, &|a| a != crate::choice::DECLINE_ID) {
            crate::dual::note_bail_here("Maxis Central Control takes a planet (the control chain and the exploration after it)");
            return None;
        }
        if let Some(why) = crate::dual_window::outside_why(gs, script) {
            crate::dual::note_bail_here(why);
            return None;
        }
        if gs.deck_reserve.is_some() {
            crate::dual::note_bail_here("deck reserve (a turn redo replays the draws)");
            return None;
        }
        if answered("Imperial Arms Vault", &|a| a == "card") {
            if let Some(why) = crate::strategy_cards_core::draw_hook(gs, content, player, 1) {
                crate::dual::note_bail_here(why);
                return None;
            }
        }
        if answered("The Galactic Council", &|_| true) && crate::secrets::held_count(gs, content, player) > crate::secrets::HAND_LIMIT + crate::relics::secret_objective_bonus(gs, player) {
            crate::dual::note_bail_here("the Galactic Council with a hand of secrets over the limit (the string asks which to return)");
            return None;
        }
        if answered("The Acropolis", &|a| a == "leader|nomadagentartuno") {
            crate::dual::note_bail_here("The Acropolis readies Artuno the Betrayer (its trade goods are the string's)");
            return None;
        }
        let (mut cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
        let roster = crate::technology_core::roster_for(content, sources).why("the unit tables of the sources")?;
        let scope = crate::core_adapter::scope(sources);
        let mut window = Legendary::new(seat, when, r.code("legendary"), scope, roster, gs.rules_version >= ti4_model::state::RULES_OFFICIAL_ACTION_DECK);
        if when == When::Pass && crate::legendary::available_text(gs, player, &ti4_model::id::PlanetId::new("faunus")) {
            let candidates = crate::legendary::maxis_candidates(gs, content, sources, player, galaxy);
            let typed: Option<Vec<(System, Planet)>> = candidates
                .iter()
                .map(|(system, planet)| Some((System::from_id(system.as_str())?, Planet::from_id(planet.as_str())?)))
                .collect();
            window = window.with_maxis(typed.why(crate::dual::reason::UNKNOWN_ID)?);
        }
        let before = gs.clone();
        let kernel = crate::production_view::kernel();
        let mut driver = Driver::new(window, Resolver::new(&[seat], Some(seat)), crate::reactions_core::linked_table(), 1);
        let mut staged = Vec::new();
        let driven = with_core(gs, &mut cx, Sources::default(), LEGENDARY, |core, _| {
            let driven = drive_with(&mut driver, core, kernel, &r, &before, script, &|_, _, _| {});
            staged = std::mem::take(&mut core.pending_mut().staged);
            driven
        })
        .why(crate::dual::reason::NOT_LENDABLE)?
        .why(crate::dual::reason::DRIVER_REFUSED)?;
        if driver.window.unsupported {
            crate::dual::note_bail_here("a draw the table could not judge (no printed names for the hand limit)");
            return None;
        }
        apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
        Some(CoreWindow { run: Run::new(()), questions: driven.rendered })
    }

}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_content::ContentStore;
    use ti4_model::content_types::SourceSet;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn settle_styx(_: &mut GameState) -> Option<()> {
        None
    }

    pub fn purge_salvaged(_: &mut GameState, _: &PlayerId, _: &str) -> Option<bool> {
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub fn window(
        _: &mut GameState,
        _: &ContentStore,
        _: SourceSet,
        _: Option<&ti4_content::galaxy::Galaxy>,
        _: &PlayerId,
        _: ti4_engine_core::choice::legendary::When,
        _: &Recording,
    ) -> Option<CoreWindow<()>> {
        None
    }
}
