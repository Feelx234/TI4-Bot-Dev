//! How a hosted game is set up: the game a page asks for, the game it makes, and what the setup
//! form of `web/` may offer (card sets, factions, maps, presets).
//!
//! A page asks for one of three games ([`Start`]):
//!
//! - [`Setup`], the form of `web/`: it becomes a [`TableSetup`] and the faction the person chose.
//! - [`TableRequest`], the table of web2's single player: what the host chose, and its players.
//! - [`Local`], a game of a fixed setup (web2's `?local=` and `?scenario=`, for tests and tools).
//!
//! The first two are made by `ti4_host::setup` (`draw`, `create`), as for a lobby game of the
//! server, so a seed gives the same board and the same opening state in all.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::game::Game;
use ti4_engine::seating;
use ti4_host::maps::catalog::{self, MapChoice};
use ti4_host::setup::{self, SeatOrder, SpeakerOrder, TableSeats, TableSetup, bot_kind, card_set};
use ti4_model::content_types::POK;
use ti4_model::id::PlayerId;
use ti4_model::state::ReactionMode;

use crate::SEATS;
use crate::host_session::SeatSwitch;

fn local_game_id() -> String {
    "local".to_owned()
}

/// Who plays the seats of the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Bots {
    /// The rule bot ([`setup::bot_kind::RULE`]). `"rule"` is its name in the form of `web/`.
    #[default]
    #[serde(rename = "smart", alias = "rule")]
    Rule,
    /// [`setup::bot_kind::RANDOM`].
    #[serde(rename = "random")]
    Random,
}

/// The table of a game. Everything the game's opening state is made from; a save carries it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    #[serde(default = "local_game_id")]
    pub game_id: String,
    pub seed: u64,
    pub players: usize,
    /// 0-based place of the person. Left out: drawn from the seed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_seat: Option<usize>,
    /// A faction id of `seating::LOBBY_FACTIONS`. Left out: drawn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_faction: Option<String>,
    /// One of `card_set::OFFERED`. Left out: `card_set::DEFAULT`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_set: Option<String>,
    /// A template alias, `"random"` for the seeded random board. Left out: the template that is
    /// recommended for the number of players.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    /// A start preset of `ti4_host::preset` (a test table). Not offered by the form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// `"seat"` (the first seat opens) or `"random"`. Left out: random.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerOrder>,
    /// A prepared game of `ti4_host::view::scenario` (its players win, and its seed unless it
    /// plays on any seed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    /// Who plays the seats of the others. Left out: the rule bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bots: Option<Bots>,
}

/// What a save brings to a new run of the same game.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Resume {
    /// The ids of the person's answers.
    #[serde(default)]
    pub answers: Vec<String>,
    /// The answers after them that a redo can bring back.
    #[serde(default)]
    pub redo: Vec<String>,
    /// The cards the person is never offered (applied when the replay ends).
    #[serde(default)]
    pub never: Vec<String>,
    /// The time of day of each event, so a game that is played again keeps the times it had.
    #[serde(default)]
    pub event_times: Vec<String>,
    /// The version each event was made at.
    #[serde(default)]
    pub event_versions: Vec<Option<u64>>,
    /// The version of the game when it was saved: the next one follows it.
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub generation: u64,
    /// How many decisions the saved game had, and their hash: checked when the replay ends.
    #[serde(default)]
    pub decisions: Option<usize>,
    #[serde(default)]
    pub decisions_hash: Option<String>,
    /// Every decision of the saved game, of every seat, in order. With it the game is given
    /// each of them again, and no seat decides again (`answers` is then not read); without it
    /// the person's `answers` are given, and the other seats decide again from the seed.
    #[serde(default)]
    pub log: Vec<String>,
    /// The changes of the seats that the page plays, in order.
    #[serde(default)]
    pub switches: Vec<SeatSwitch>,
    /// The steps that the undo before this run took back (`Restart::undone`).
    #[serde(default)]
    pub undone: Vec<ti4_host::planning::steps::PlanStep>,
    /// What each seat has staged and not sent (`ti4_host::staged`).
    #[serde(default)]
    pub staged: std::collections::BTreeMap<ti4_model::id::PlayerId, ti4_host::staged::SeatStaged>,
}

/// The request of `ti4_host_start`: one of `setup`, `table` and `local`, and what a save
/// brings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    #[serde(default)]
    pub setup: Option<Setup>,
    #[serde(default)]
    pub table: Option<TableRequest>,
    #[serde(default)]
    pub local: Option<Local>,
    #[serde(default)]
    pub resume: Option<Resume>,
    /// The mode of each reaction card of the seats that the page plays, from the first
    /// decision on (`resume.never` of a save of `web/` is added to it).
    #[serde(default)]
    pub modes: BTreeMap<String, ReactionMode>,
    /// Whether the game waits for the page before a decision of a seat that it does not play:
    /// 0 (no), 1 (before each) or 2 (before each that takes the wait of a table). See the local
    /// command `stepping`.
    #[serde(default)]
    pub stepping: u32,
    /// The page takes part in the staged work of its seats: it is sent `seat_staged` and may
    /// send `put_staged`, as a connection of the server that subscribed with `staged`.
    #[serde(default)]
    pub staged: bool,
}

impl Start {
    /// The game this request asks for.
    ///
    /// # Errors
    /// A message when the request names no game or two, or the game is not one the host offers.
    pub fn build(&self) -> Result<Built, String> {
        match (&self.setup, &self.table, &self.local) {
            (Some(setup), None, None) => setup.build(),
            (None, Some(table), None) => table.build(),
            (None, None, Some(local)) => local.build(),
            _ => Err("start: give one of setup, table and local".to_owned()),
        }
    }
}

/// Who plays a seat of a table ([`TableRequest`]), by the names of a lobby.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SeatKind {
    /// The page answers the seat.
    #[default]
    #[serde(rename = "human")]
    Human,
    /// The rule bot (`bot_kind::RULE`).
    #[serde(rename = "smart")]
    Rule,
    /// A seat that decides at random (`bot_kind::RANDOM`).
    #[serde(rename = "random")]
    Random,
}

impl SeatKind {
    /// The kind as a fact of the update names it (`ti4_host::setup::SeatFact::kind`).
    const fn as_str(self) -> &'static str {
        match self {
            Self::Human => setup::HUMAN_KIND,
            Self::Rule => bot_kind::RULE,
            Self::Random => bot_kind::RANDOM,
        }
    }
}

/// One player of a table: who plays the seat, and the faction that was chosen for it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatRequest {
    #[serde(default)]
    pub kind: SeatKind,
    #[serde(default)]
    pub faction: Option<String>,
    /// The name of the player at the table. With a name for every player, the updates of the
    /// game say who sits in each seat.
    #[serde(default)]
    pub name: Option<String>,
}

/// The table of web2's single player: what the host chose (`ti4_host::setup::TableSetup`), and
/// its players in the order in which they came. The player who came as number `i` has the id
/// of [`SEATS`]`[i]`, wherever the draw seats it: the masks of this host name a player by that
/// letter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableRequest {
    pub setup: TableSetup,
    /// Missing entries are seats of the page with no chosen faction.
    #[serde(default)]
    pub seats: Vec<SeatRequest>,
}

impl TableRequest {
    /// The table with its values checked, and a player for every seat.
    ///
    /// # Errors
    /// A message when the table is not one the host offers.
    pub fn checked(&self) -> Result<Self, String> {
        let mut table = self.clone();
        table.setup.check()?;
        let count = table.setup.player_count;
        if count > SEATS.len() || table.seats.len() > count {
            return Err(format!(
                "a table of {count} seats has {} players",
                table.seats.len()
            ));
        }
        table.seats.resize_with(count, SeatRequest::default);
        // The values of a kind are the names of the setup options.
        debug_assert_eq!((bot_kind::RULE, bot_kind::RANDOM), ("smart", "random"));
        Ok(table)
    }

    /// The ids of the players in the order in which they came.
    #[must_use]
    pub fn joined(&self) -> Vec<PlayerId> {
        SEATS[..self.setup.player_count]
            .iter()
            .map(|name| PlayerId::new(*name))
            .collect()
    }

    /// What the start draws: who sits where, and the faction of every seat.
    ///
    /// # Errors
    /// A message when a chosen faction is not offered.
    pub fn drawn(&self) -> Result<setup::Drawn, String> {
        let chosen: Vec<Option<String>> =
            self.seats.iter().map(|seat| seat.faction.clone()).collect();
        setup::draw(&self.setup, &chosen)
    }

    /// Who sits in each seat, by the id of its player. `None` when a player has no name.
    #[must_use]
    pub fn seats(&self) -> Option<TableSeats> {
        self.joined()
            .into_iter()
            .zip(&self.seats)
            .map(|(player, seat)| {
                Some((
                    player,
                    setup::SeatFact {
                        nickname: seat.name.clone()?,
                        kind: seat.kind.as_str().to_owned(),
                    },
                ))
            })
            .collect()
    }

    /// The players of `kind` as a mask (bit 0 is the player who came first).
    #[must_use]
    pub fn mask(&self, kind: SeatKind) -> u32 {
        self.seats
            .iter()
            .enumerate()
            .filter(|(_, seat)| seat.kind == kind)
            .fold(0, |mask, (place, _)| mask | (1 << place))
    }

    /// The game that the table starts, as the server starts it for the table of a lobby: the
    /// page answers its players, the rule bot and the random seats decide as on the server, so
    /// a table of a seed with the same kinds is the same game on both hosts.
    ///
    /// # Errors
    /// A message when the table is not one the host offers.
    pub fn build(&self) -> Result<Built, String> {
        let table = self.checked()?;
        let content = ContentStore::embedded();
        let drawn = table.drawn()?;
        let players = drawn.seated(&table.joined());
        let (state, galaxy) = setup::create(content, &table.setup, &players, &drawn)?;
        let opening = table
            .setup
            .scenario()?
            .and_then(|scenario| ti4_host::view::scenario::opening(scenario, &state, &galaxy));
        let seed = table.setup.game_seed();
        let game = Game::with_seeded_random(state, content, seed)
            .with_sources(POK)
            .with_galaxy(galaxy.clone());
        Ok(Built {
            game,
            galaxy: Some(galaxy),
            seed,
            players: table.setup.player_count,
            humans: table.mask(SeatKind::Human),
            rule_seats: table.mask(SeatKind::Rule),
            table: true,
            seats: table.seats(),
            game_id: local_game_id(),
            request: serde_json::json!({ "table": table }),
            opening,
        })
    }
}

/// A game of a fixed setup: a seeded game on the recommended map for its count, or a scenario
/// of `ti4_host::view::scenario` (its seed and its count win). Seat `i` has the id
/// [`SEATS`]`[i]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Local {
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "eight")]
    pub players: usize,
    /// The seats that the page plays (bit 0 is seat `a`).
    pub humans: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
    /// The seats that the rule bot plays where the page does not. It decides from the seed, the
    /// seat and the number of the decision, as on the server; the other seats decide at random.
    #[serde(default)]
    pub rule_seats: u32,
}

const fn eight() -> usize {
    8
}

impl Local {
    /// The game of this setup.
    ///
    /// # Errors
    /// A message when the scenario is not known, or the count is not one of a game.
    pub fn build(&self) -> Result<Built, String> {
        let scenario = match &self.scenario {
            Some(id) => Some(ti4_host::view::scenario::find(id).ok_or_else(|| {
                let known: Vec<&str> = ti4_host::view::scenario::SCENARIOS
                    .iter()
                    .map(|scenario| scenario.id)
                    .collect();
                format!("no scenario '{id}': the scenarios are {}", known.join(", "))
            })?),
            None => None,
        };
        let (seed, players) = scenario.map_or((self.seed, self.players), |scenario| {
            (scenario.seed, scenario.players)
        });
        let game = crate::new_game(
            ContentStore::embedded(),
            seed,
            players,
            self.scenario.as_deref(),
        )
        .map_err(|error| error.to_string())?;
        let opening = scenario.zip(game.galaxy()).and_then(|(scenario, galaxy)| {
            ti4_host::view::scenario::opening(scenario, &game.state, galaxy)
        });
        let all = (1_u32 << players) - 1;
        Ok(Built {
            galaxy: game.galaxy().cloned(),
            game,
            seed,
            players,
            humans: self.humans & all,
            rule_seats: self.rule_seats & all,
            table: false,
            seats: None,
            game_id: local_game_id(),
            request: serde_json::json!({ "local": self }),
            opening,
        })
    }
}

fn mix(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A game ready to be played.
pub struct Built {
    pub game: Game<'static>,
    pub galaxy: Option<Galaxy>,
    /// The seed the seats decide from.
    pub seed: u64,
    pub players: usize,
    /// The seats that the page plays (bit 0 is seat `a`).
    pub humans: u32,
    /// The seats the rule bot plays where the page does not.
    pub rule_seats: u32,
    /// The game is of a table: a random seat decides from the seed, as on the server.
    pub table: bool,
    /// Who sits in each seat, when the game is of a table whose players have names.
    pub seats: Option<TableSeats>,
    pub game_id: String,
    /// The request with the choices that were left open filled in: a save replays with it.
    pub request: serde_json::Value,
    /// The opening of the scenario that the game starts from (`ti4_host::view::scenario`).
    pub opening: Option<ti4_host::view::scenario::Opening>,
}

impl Setup {
    /// The table of `ti4_host::setup` that this request asks for. The seats keep the order of the
    /// join: the person sits at the place that they asked for.
    #[must_use]
    pub fn table(&self) -> TableSetup {
        let map = match self.map.as_deref() {
            None => MapChoice::from_stored(catalog::recommended_for(self.players).as_deref()),
            Some("random") => MapChoice::Random,
            Some(alias) => MapChoice::from_stored(Some(alias)),
        };
        TableSetup {
            player_count: self.players,
            map,
            strategy_cards: self
                .card_set
                .clone()
                .unwrap_or_else(|| card_set::DEFAULT.to_owned()),
            seat_order: SeatOrder::Join,
            speaker_order: self.speaker.unwrap_or_default(),
            start_from: self.scenario.clone(),
            preset: self.preset.clone(),
            seed: self.seed,
        }
    }

    /// Checks the table and fills in what was left open.
    ///
    /// # Errors
    /// A message for the person when the table is not one the host offers.
    pub fn resolved(&self) -> Result<Self, String> {
        let mut setup = self.clone();
        if let Some(id) = &setup.scenario {
            let scenario =
                ti4_host::view::scenario::find(id).ok_or_else(|| format!("no scenario '{id}'"))?;
            setup.seed = scenario.game_seed(setup.seed);
            setup.players = scenario.players;
        }
        setup.table().check()?;
        let human = match setup.human_seat {
            Some(seat) if seat < setup.players => seat,
            Some(seat) => {
                return Err(format!(
                    "seat {seat} is not at a table of {}",
                    setup.players
                ));
            }
            None => usize::try_from(mix(setup.seed) % setup.players as u64)
                .expect("a seat fits a usize"),
        };
        setup.human_seat = Some(human);
        if let Some(faction) = &setup.human_faction
            && !seating::LOBBY_FACTIONS.contains(&faction.as_str())
        {
            return Err(format!("faction '{faction}' is not played here"));
        }
        Ok(setup)
    }

    /// The game this table plays.
    ///
    /// # Errors
    /// A message when the table is not offered, or the map does not build.
    pub fn build(&self) -> Result<Built, String> {
        let setup = self.resolved()?;
        let content = ContentStore::embedded();
        let players = setup.players;
        let human = setup.human_seat.expect("resolved sets the seat");
        let table = setup.table();
        let chosen: Vec<Option<String>> = (0..players)
            .map(|seat| {
                (seat == human)
                    .then(|| setup.human_faction.clone())
                    .flatten()
            })
            .collect();
        let joined: Vec<PlayerId> = SEATS[..players].iter().map(|s| PlayerId::new(*s)).collect();
        let drawn = setup::draw(&table, &chosen)?;
        let (state, galaxy) = setup::create(content, &table, &drawn.seated(&joined), &drawn)?;
        let opening = table
            .scenario()?
            .and_then(|scenario| ti4_host::view::scenario::opening(scenario, &state, &galaxy));
        let rule_seats = if setup.bots == Some(Bots::Random) {
            0
        } else {
            ((1_u32 << players) - 1) & !(1 << human)
        };
        let game = Game::with_seeded_random(state, content, table.game_seed())
            .with_sources(POK)
            .with_galaxy(galaxy.clone());
        Ok(Built {
            game,
            galaxy: Some(galaxy),
            seed: setup.seed,
            players,
            humans: 1 << human,
            rule_seats,
            table: false,
            seats: None,
            game_id: setup.game_id.clone(),
            request: serde_json::to_value(&setup).map_err(|error| error.to_string())?,
            opening,
        })
    }
}

/// What the setup form may offer: the options of `ti4_host::setup`, with the maps of every
/// count, the presets and what identifies the engine.
#[must_use]
pub fn describe() -> serde_json::Value {
    let mut options = serde_json::to_value(setup::setup_options("The page", 0)).unwrap_or_default();
    let maps: serde_json::Map<String, serde_json::Value> = setup::PLAYER_COUNTS
        .map(|count| {
            (
                count.to_string(),
                serde_json::to_value(catalog::catalog(Some(count)).unwrap_or_default())
                    .unwrap_or_default(),
            )
        })
        .collect();
    if let Some(object) = options.as_object_mut() {
        object.insert("maps".to_owned(), maps.into());
        object.insert("presets".to_owned(), ti4_host::preset::KNOWN.into());
        object.insert(
            "content_digest".to_owned(),
            ti4_content::provenance::embedded_digest().corpus.into(),
        );
        object.insert("engine_build".to_owned(), env!("CARGO_PKG_VERSION").into());
    }
    options
}

/// A pictured map for the setup form: the tiles of the board a table of `player_count` gets for
/// `choice`, with a place for each seat (`/api/maps/{alias}/preview` of the server).
///
/// # Errors
/// A message when the choice does not build for that many players.
pub fn map_preview(
    choice: &MapChoice,
    player_count: usize,
    variant: u32,
) -> Result<serde_json::Value, String> {
    let seed = catalog::variant_seed_salted(0x5eed_0ff1_1e00, choice, player_count, variant);
    catalog::preview(choice, player_count, seed)
        .map(|preview| serde_json::to_value(preview).unwrap_or_default())
}
