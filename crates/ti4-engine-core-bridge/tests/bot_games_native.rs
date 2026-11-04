//! Lane `core-botstops-1` (docs/BOTSTOPS_RESUME.md): the shipped rule bot, READING THE VIEW, plays whole games on the core through the `Game` wrapper
//! without a step error. Before the lane the core stopped in 8 of the 24 games `perf_ab` plays (3 to 6 players, seeds 1 to 6); the random-pick bot of the
//! native playthrough never reached the positions. The games below are the shortest of the 8; the root causes also have unit tests of their own:
//!
//! * 5p seed 4: the first Mentak win with Salvage Operations (`aftermath: MissingFact`: a native table lent no buildable list),
//! * 4p seed 5: a Muaat Nova Seed over another player's carried ground forces (`UnportedTactical(NovaSeedCargo)`), then the tile's purged planets
//!   (`Board: planet entry in an untouched system`), then Salvage Operations,
//! * 5p seed 5: the same planet entries after Nova Seed.

use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};
use ti4_engine::native::Game;
use ti4_engine::wire::choice::Table;
use ti4_engine_core_bridge::native_play::lobby_spec;
use ti4_selfplay::spec::SEATS;

/// `perf_ab`'s smart bot: `choose_explained` with the view, seeded by the game seed, the seat's place and the decision index.
struct Smart {
    seed: u64,
    index: usize,
}

impl Smart {
    fn decide(&mut self, choice: &Choice, seen: Option<&SeatObservation<'_>>) -> ChoiceOption {
        let place = SEATS.iter().position(|x| *x == choice.player.as_str()).unwrap_or(0);
        let view = seen.filter(|_| !choice.options.is_empty());
        let (picked, _, _) = ti4_rule_bot::choose_explained(choice, view, self.seed, place, self.index);
        self.index += 1;
        choice.options[picked.min(choice.options.len() - 1)].clone()
    }
}

impl Decider for Smart {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        Ok(self.decide(choice, None))
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        Ok(self.decide(choice, Some(seen)))
    }
    fn wants_view(&self) -> bool {
        true
    }
}

fn play_to_the_end(players: usize, seed: u64) {
    let spec = lobby_spec("sol", players, seed);
    let legacy = spec.build().expect("a lobby game builds");
    let galaxy = legacy.galaxy().cloned().expect("a lobby game has a map");
    let table = Table::with_default(Box::new(Smart { seed, index: 0 }));
    let mut game = Game::try_with_table(legacy.state.clone(), ContentStore::embedded(), table).expect("playable").with_galaxy(galaxy);
    let mut steps = 0;
    while !game.state.finished {
        let result = game.step();
        assert_eq!(result.error, None, "{players}p seed {seed}: step {steps}");
        steps += 1;
        assert!(steps < 20_000, "{players}p seed {seed}: the game ends");
    }
}

#[test]
fn the_smart_bot_plays_five_players_seed_four_to_the_end_on_the_core() {
    play_to_the_end(5, 4);
}

#[test]
fn the_smart_bot_plays_four_players_seed_five_to_the_end_on_the_core() {
    play_to_the_end(4, 5);
}

#[test]
fn the_smart_bot_plays_five_players_seed_five_to_the_end_on_the_core() {
    play_to_the_end(5, 5);
}
