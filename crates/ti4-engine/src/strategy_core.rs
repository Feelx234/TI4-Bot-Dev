//! `Area::Strategy` (package B8), the question-asking card effects as lockstep windows: so far the Trade
//! primary (`strategy_cards::trade_primary`). The core half of each answers `None` (the string result stands,
//! counted as a bail) where the position holds something the core does not: see `docs/ENGINE_KNOWN_ISSUES.md`,
//! B8.

#[cfg(feature = "typed-core")]
pub use real::*;
#[cfg(not(feature = "typed-core"))]
pub use stub::*;

#[cfg(feature = "typed-core")]
mod real {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::{Kernel, Resolver};
    use ti4_engine_core::rules::strategy_windows::{TradePrimary, TRADE_PRIMARY_GOODS};
    use ti4_engine_core::state::Slice;
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::core_sync::{Footprint, Sources};
    use crate::dual::{CoreWindow, Run};
    use crate::dual_window::Recording;
    use crate::dual_window::outside;
    use crate::leaders_core::replenish_causes_events;

    /// The tables of a window whose options name seats by faction. `None`: more than 16 owners, or two seats
    /// of one faction (the option ids would clash).
    pub(crate) fn named_tables(gs: &GameState) -> Option<(crate::core_sync::Ctx, crate::render::Render)> {
        let (cx, r) = crate::dual_window::tables(gs)?;
        let factions: Vec<String> = r
            .seats
            .iter()
            .map(|id| gs.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned()))
            .collect();
        let named: Vec<&String> = factions.iter().filter(|f| !f.is_empty()).collect();
        if named.iter().enumerate().any(|(i, f)| named[..i].contains(f)) {
            return None;
        }
        Some((cx, r.with_factions(factions)))
    }

    /// The display facts of the Trade primary's question: the seat of each option and its commodities.
    pub(crate) fn complete_trade(r: &crate::render::Render, core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice) {
        let name_of = |s: ti4_engine_core::ids::Seat| r.faction_text(s).to_owned();
        let mut seats = serde_json::Map::new();
        let mut commodities = serde_json::Map::new();
        for opt in rendered.options.iter().filter(|o| !o.is_decline()) {
            let Some(found) = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok()).map(ti4_engine_core::ids::Seat).find(|s| name_of(*s) == opt.id) else {
                continue;
            };
            seats.insert(opt.id.clone(), serde_json::Value::from(r.seat_text(found)));
            let (have, max) = core.players().by_seat(found).map_or((0, 0), |p| (p.econ.commodities, p.faction.data().commodities));
            commodities.insert(opt.id.clone(), serde_json::json!({ "have": have, "max": max }));
        }
        rendered.details.insert("seats".to_owned(), serde_json::Value::Object(seats));
        rendered.details.insert("commodities".to_owned(), serde_json::Value::Object(commodities));
    }

    const TRADE: Footprint =
        Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Pending], &[Slice::Players]);

    /// The core half of `strategy_cards::trade_primary`. `None` where the position is outside what the core
    /// holds: a question inside a battle or an invasion, diplomacy on (a favour keeps a promise), a commodity
    /// change that causes events (Cabal, Siphon, Trade Agreement), two seats of one faction (the options are
    /// named by faction), a seat the tables do not hold.
    pub fn trade_primary(gs: &mut GameState, player: &PlayerId, script: &Recording) -> Option<CoreWindow<bool>> {
        if outside(gs, script) || replenish_causes_events(gs) {
            return None;
        }
        let (mut cx, r) = named_tables(gs)?;
        let seat = r.seat(player.as_str())?;
        let before = gs.clone();
        let kernel = Kernel::plain();
        let mut driver = Driver::new(TradePrimary::new(seat), Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        let driven = crate::core_sync::with_core(gs, &mut cx, Sources::default(), TRADE, |core, _| {
            crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &|_, core, rendered| complete_trade(&r, core, rendered))
        })
        .ok()?
        .ok()?;
        // The announcement of the three trade goods is staged on the string state (not a codec slice).
        crate::supply::note_trade_goods_gained(gs, player, TRADE_PRIMARY_GOODS, "trade_primary");
        Some(CoreWindow { run: Run::new(true).with_events_vec(Vec::new()), questions: driven.rendered })
    }
}

#[cfg(not(feature = "typed-core"))]
mod stub {
    use ti4_model::id::PlayerId;
    use ti4_model::state::GameState;

    use crate::dual::CoreWindow;
    use crate::dual_window::Recording;

    pub fn trade_primary(_: &mut GameState, _: &PlayerId, _: &Recording) -> Option<CoreWindow<bool>> {
        None
    }
}

#[cfg(all(test, feature = "typed-core"))]
mod tests {
    use ti4_content::ContentStore;
    use ti4_model::content_types::POK;
    use ti4_model::id::PlayerId;

    use crate::choice::{Scripted, Table};
    use crate::dual::{report, take_report, with_mask, Area, AreaMask, Backend};

    fn counts(label: &str) -> (u64, u64) {
        crate::dual::thread_window_counts(label)
    }

    fn run(answers: &[&str], stock: &[i32], factions: &[(&str, &str)]) -> (u64, u64) {
        let _guard = crate::dual::tests::serial();
        let _ = take_report();
        crate::dual::reset_thread_windows();
        with_mask(AreaMask::STRING.with(Area::Strategy, Backend::Dual), || {
            let mut state = crate::fixtures::seated_game(factions, POK);
            for (seat, n) in factions.iter().zip(stock) {
                state.player_mut(&PlayerId::new(seat.0)).unwrap().commodities = *n;
            }
            let mut table = Table::with_default(Box::new(Scripted::new(answers.iter().map(|a| (*a).to_owned()))));
            crate::strategy_cards::trade_primary(&mut state, ContentStore::embedded(), POK, None, &mut table, &PlayerId::new("a"))
                .unwrap();
            assert!(state.player(&PlayerId::new("a")).unwrap().trade_goods >= 3);
        });
        let r = report();
        assert_eq!(r.total_differences(), 0, "{:?}", r.differences);
        let c = counts("strategy_cards::trade_primary");
        let _ = take_report();
        c
    }

    /// Two seats take the offer in turn (the last option every time would be "done": the picks are by name).
    #[test]
    fn the_trade_primary_runs_in_lockstep_with_the_core() {
        let f = [("a", "sol"), ("b", "hacan"), ("c", "arborec")];
        assert_eq!(run(&["arborec", "hacan"], &[0, 0, 0], &f).0, 1);
        assert_eq!(run(&["hacan", "done"], &[0, 0, 0], &f).0, 1);
        assert_eq!(run(&["done"], &[0, 1, 0], &f).0, 1);
        // Nobody needs a replenish: no question at all.
        assert_eq!(run(&[], &[0, 99, 99], &f).0, 1);
    }

    /// A Cabal, a Trade Agreement or two seats of one faction make the core half decline (the string stands);
    /// probed directly, since the shared report is also fed by parallel tests.
    #[test]
    fn the_core_half_declines_where_the_position_holds_more() {
        let script = crate::dual_window::Recording::default();
        let a = PlayerId::new("a");
        let probe = |f: &[(&str, &str)]| {
            let mut state = crate::fixtures::seated_game(f, POK);
            super::trade_primary(&mut state, &a, &script).is_some()
        };
        assert!(probe(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")]));
        assert!(!probe(&[("a", "sol"), ("b", "cabal"), ("c", "arborec")]));
        assert!(!probe(&[("a", "sol"), ("b", "sol")]));
        let mut state = crate::fixtures::seated_game(&[("a", "sol"), ("b", "hacan")], POK);
        state.diplomacy.enabled = true;
        assert!(super::trade_primary(&mut state, &a, &script).is_none());
    }
}
