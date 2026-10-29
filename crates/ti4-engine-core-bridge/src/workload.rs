//! Positions to test and time on: the opening of every map template, and positions of golden bot
//! games played exactly as `selfplay run` plays them (`ti4-selfplay/examples/engine_profile.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation, SeededRandom};
use ti4_model::content_types::SourceSet;
use ti4_model::id::{PlayerId, SystemId};
use ti4_model::state::GameState;
use ti4_selfplay::spec::{GameSpec, Matrix, SEATS};

/// One board to ask questions about.
#[derive(Clone)]
pub struct Position {
    pub label: String,
    pub state: GameState,
    pub galaxy: Galaxy,
    pub sources: SourceSet,
}

/// One ship that could be asked to move: where, what, and its move value without boosts.
#[derive(Debug, Clone)]
pub struct ShipQuery {
    pub origin: SystemId,
    pub type_id: String,
    pub move_value: i32,
}

/// Every ship of `player` (and unit that moves as one), as `tactical::movable_into` lists them.
#[must_use]
pub fn ships(
    state: &GameState,
    content: &ContentStore,
    sources: SourceSet,
    player: &PlayerId,
) -> Vec<ShipQuery> {
    let types = ti4_content::units::catalogue(content, sources);
    let mut out = Vec::new();
    for origin in state.systems_with_units_of(player) {
        for (index, hull) in state.ships_of(player, origin).into_iter().enumerate() {
            let Some(kind) = types.get(hull.type_id.as_str()) else {
                continue;
            };
            if !kind.moves_as_ship() {
                continue;
            }
            let move_value = ti4_engine::tactical::effective_move_value_for_ship(
                state,
                kind,
                player,
                origin,
                Some(index),
                false,
                false,
            );
            out.push(ShipQuery {
                origin: origin.clone(),
                type_id: hull.type_id.to_string(),
                move_value,
            });
        }
    }
    out
}

struct Bot {
    seed: u64,
    decided: usize,
    random: SeededRandom,
}

impl Bot {
    fn decide(
        &mut self,
        choice: &Choice,
        seen: Option<&SeatObservation<'_>>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        let index = self.decided;
        let option = match seen {
            Some(seen) if !choice.options.is_empty() => {
                let seat = SEATS
                    .iter()
                    .position(|s| *s == choice.player.as_str())
                    .unwrap_or(0);
                // The thread plays the shipped tuning (`tuning::current()`, the thread default); a per-decision
                // `set(current())` would reset any bot_version a caller installed on this worker.
                let (picked, _, _) =
                    ti4_rule_bot::choose_explained(choice, Some(seen), self.seed, seat, index);
                choice.options[picked].clone()
            }
            _ => self.random.choose(choice)?,
        };
        self.decided += 1;
        Ok(option)
    }
}

struct Shared(Rc<RefCell<Bot>>);

impl Decider for Shared {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, None)
    }
    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        self.0.borrow_mut().decide(choice, Some(seen))
    }
}

/// The games of a selfplay config.
///
/// # Errors
/// An unreadable config.
pub fn specs(config: &str) -> Result<(Matrix, Vec<GameSpec>), String> {
    let text = std::fs::read_to_string(config).map_err(|e| format!("{config}: {e}"))?;
    let matrix: Matrix = serde_json::from_str(&text).map_err(|e| format!("{config}: {e}"))?;
    let (specs, _) = ti4_selfplay::driver::expand(&matrix)?;
    Ok((matrix, specs))
}

/// Play the first `games` games of a config with the rule bot and keep every `stride`-th position
/// (at a step boundary). Also returns each game's decision hash (the bench's).
///
/// # Errors
/// A config or a game that fails.
pub fn golden_positions(
    config: &str,
    games: usize,
    stride: usize,
) -> Result<(Vec<Position>, Vec<String>), String> {
    golden_games(config, games, stride, &|_| true, &mut |_, _| {})
}

/// How many games `golden_positions(config, games, ..)` plays: `games`, or fewer when the config has fewer.
///
/// # Errors
/// An unreadable config.
pub fn golden_game_count(config: &str, games: usize) -> Result<usize, String> {
    Ok(specs(config)?.1.len().min(games))
}

/// [`golden_positions`] restricted to the games (indices into the first `games` of the config) `include` accepts
/// (the shards of `dual_run`), calling `each(index, decision_hash)` after every game played. Every game is built
/// from its own spec and seed, so a game's play does not depend on which others ran in the process. The returned
/// hashes are those of the included games, in index order.
///
/// # Errors
/// A config or a game that fails.
pub fn golden_games(
    config: &str,
    games: usize,
    stride: usize,
    include: &dyn Fn(usize) -> bool,
    each: &mut dyn FnMut(usize, &str),
) -> Result<(Vec<Position>, Vec<String>), String> {
    let (matrix, specs) = specs(config)?;
    let mut positions = Vec::new();
    let mut hashes = Vec::new();
    for (index, spec) in specs.iter().take(games).enumerate() {
        if !include(index) {
            continue;
        }
        let mut game = spec.build()?;
        game.table
            .set_default(Box::new(Shared(Rc::new(RefCell::new(Bot {
                seed: spec.seed,
                decided: 0,
                random: SeededRandom::new(spec.seed),
            })))));
        let mut steps = 0usize;
        while !game.state.finished
            && game.state.round <= matrix.limits.max_rounds
            && steps < matrix.limits.max_steps
        {
            if steps % stride.max(1) == 0
                && let Some(galaxy) = game.galaxy()
            {
                positions.push(Position {
                    label: format!("{} seed {} step {steps}", spec.map, spec.seed),
                    state: game.state.clone(),
                    galaxy: galaxy.clone(),
                    sources: game.sources(),
                });
            }
            let result = game.step();
            steps += 1;
            if let Some(error) = result.error {
                return Err(format!("seed {}: {error}", spec.seed));
            }
        }
        let hash = ti4_wasm::host_session::decisions_hash(&game.table.log.records);
        each(index, &hash);
        hashes.push(hash);
    }
    Ok((positions, hashes))
}

/// The opening position of every map template the host knows, for each seat count it supports.
#[must_use]
pub fn template_openings() -> (Vec<Position>, Vec<String>) {
    let mut positions = Vec::new();
    let mut refused = Vec::new();
    let Ok(loader) = ti4_host::maps::template_loader::TemplateLoader::load() else {
        return (positions, vec!["template loader".to_owned()]);
    };
    // Every faction the server seats, rotated over templates and seats so every faction's
    // movement hooks sit on some board.
    let pool = ti4_engine::seating::LOBBY_FACTIONS;
    let mut turn = 0usize;
    for summary in loader.list_summaries() {
        let players = summary.player_count;
        for thunders_edge in [false, true] {
            let factions: Vec<String> = (0..players)
                .map(|s| pool[(turn + s) % pool.len()].to_owned())
                .collect();
            turn += players;
            let spec = GameSpec {
                setup: "templates".to_owned(),
                seed: 1,
                players,
                map: summary.alias.clone(),
                factions,
                speaker: "seat".to_owned(),
                card_set: None,
                rotation: None,
                starting_tech: true,
                thunders_edge,
                rules_version: None,
            };
            match spec.build() {
                Ok(game) => {
                    if let Some(galaxy) = game.galaxy() {
                        positions.push(Position {
                            label: format!("{} te={thunders_edge}", summary.alias),
                            state: game.state.clone(),
                            galaxy: galaxy.clone(),
                            sources: game.sources(),
                        });
                    }
                }
                Err(e) => refused.push(format!("{} ({players}p, te={thunders_edge}): {e}", summary.alias)),
            }
        }
    }
    (positions, refused)
}
