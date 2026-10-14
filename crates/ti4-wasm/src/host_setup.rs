//! How a hosted game is set up: the table the person asks for, the game it makes, and what the
//! setup form of `web/` may offer (card sets, factions, maps, presets).
//!
//! A [`Setup`] is a request: it becomes a [`TableSetup`] and the faction the person chose, and
//! the game is made by `ti4_host::setup` (`draw`, `create`), as for a lobby game of the server
//! and the table of web2, so a seed gives the same board and the same opening state in all.

use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_content::galaxy::Galaxy;
use ti4_engine::game::Game;
use ti4_engine::seating;
use ti4_host::maps::catalog::{self, MapChoice};
use ti4_host::setup::{self, SeatOrder, SpeakerOrder, TableSetup, card_set};
use ti4_model::content_types::POK;
use ti4_model::id::PlayerId;

use crate::SEATS;

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
    /// A prepared board of `ti4_host::view::scenario` (its seed and players win).
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
}

/// The request of `ti4_host_start`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    pub setup: Setup,
    #[serde(default)]
    pub resume: Option<Resume>,
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
    pub seed: u64,
    pub players: usize,
    pub human: usize,
    /// The seats the rule bot plays (bit 0 is seat `a`).
    pub rule_seats: u32,
    /// The setup with the choices that were left open filled in.
    pub setup: Setup,
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
            setup.seed = scenario.seed;
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
            human,
            rule_seats,
            setup,
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
