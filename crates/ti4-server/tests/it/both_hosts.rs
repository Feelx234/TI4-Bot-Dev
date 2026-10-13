//! One table, two hosts: a table of a seed, with the same seats played by the same kinds, is the
//! same game in the wasm host (single player of web2) and on the server.
//!
//! Both start the table with `ti4_view::setup`, and a seat that no person plays decides from the
//! seed, its id and the number of the decision (`ti4_rule_bot::choose`). This test holds the two
//! deciders together: the wasm host counts the decisions in its own way.
//!
//! One test: the wasm host keeps its game in statics.

use std::time::{Duration, Instant};

use ti4_content::ContentStore;
use ti4_engine::choice::DecisionRecord;
use ti4_model::id::PlayerId;
use ti4_server::session::{GameSession, SeatController, SessionConfig};
use ti4_view::setup::{self, TableSetup};

/// The players of a table in the order in which they came: the kind of the seat, and the
/// faction that was chosen for it.
type Seats = [(&'static str, Option<&'static str>)];

/// The request of the setup exports of the wasm host.
fn request(setup: &TableSetup, seats: &Seats) -> String {
    serde_json::json!({
        "setup": setup,
        "seats": seats
            .iter()
            .map(|(kind, faction)| serde_json::json!({ "kind": kind, "faction": faction }))
            .collect::<Vec<_>>(),
    })
    .to_string()
}

/// The first round of the table in the wasm host.
fn in_the_page(setup: &TableSetup, seats: &Seats) -> Vec<DecisionRecord> {
    let status = ti4_wasm::with_table_native(ti4_wasm::ti4_play_setup, &request(setup, seats));
    let report = ti4_wasm::response(status).expect("the table plays in the wasm host");
    assert!(report.contains("\"stopped\":null"), "{report}");
    ti4_wasm::decisions_native()
}

/// The table on the server, until `decisions` decisions are made or the game is over. The ids
/// of the players are those of the wasm host: the player who came as number `i` is the letter
/// `i`.
fn on_the_server(setup: &TableSetup, seats: &Seats, decisions: usize) -> Vec<DecisionRecord> {
    let content = ContentStore::embedded();
    let joined: Vec<PlayerId> = ["a", "b", "c", "d", "e", "f", "g", "h"][..seats.len()]
        .iter()
        .map(|name| PlayerId::new(*name))
        .collect();
    let chosen: Vec<Option<String>> = seats
        .iter()
        .map(|(_, faction)| faction.map(str::to_owned))
        .collect();
    let drawn = setup::draw(setup, &chosen).expect("the table is drawn");
    let players = drawn.seated(&joined);
    let (state, galaxy) = setup::create(content, setup, &players, &drawn).expect("the game");
    let tiles = ti4_server::map::build_board_tiles(content, &galaxy);
    let mut config = SessionConfig::new("both_hosts", state)
        .with_seed(setup.game_seed())
        .with_player_ids(players)
        .with_galaxy(galaxy, tiles)
        .with_random_bot_delay_ms(0);
    for (player, (kind, _)) in joined.into_iter().zip(seats) {
        let controller = match *kind {
            setup::bot_kind::RULE => SeatController::BotSmart,
            setup::bot_kind::RANDOM => SeatController::BotRandom,
            other => panic!("this test has no person: {other}"),
        };
        config = config.with_seat(player, controller);
    }
    let session = GameSession::start(config);
    let mut last = (0, Instant::now());
    loop {
        let made = session.decision_log().len();
        if made != last.0 {
            last = (made, Instant::now());
        }
        assert!(session.error().is_none(), "{:?}", session.error());
        if made >= decisions || session.is_finished() {
            break;
        }
        assert!(
            last.1.elapsed() < Duration::from_secs(60),
            "the server stalled after {made} decisions"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    session.stop();
    session.decision_log()
}

#[test]
fn slow_a_table_of_a_seed_is_the_same_game_in_the_page_and_on_the_server() {
    ti4_wasm::set_round_limit_native(Some(1));
    let mut pok = TableSetup::new(5, 23);
    pok.strategy_cards = setup::card_set::POK.to_owned();
    let tables: [(TableSetup, &Seats); 2] = [
        (
            TableSetup::new(4, 7),
            &[
                ("smart", None),
                ("smart", None),
                ("smart", None),
                ("smart", None),
            ],
        ),
        // Random seats too, a chosen faction, and another set of cards.
        (
            pok,
            &[
                ("smart", Some("hacan")),
                ("random", None),
                ("smart", None),
                ("random", Some("ghost")),
                ("smart", None),
            ],
        ),
    ];
    for (setup, seats) in tables {
        let page = in_the_page(&setup, seats);
        assert!(
            page.len() > 100,
            "a round of {} seats has more than {} decisions",
            seats.len(),
            page.len()
        );
        let server = on_the_server(&setup, seats, page.len());
        assert!(server.len() >= page.len(), "the server stopped early");
        for (number, (here, there)) in page.iter().zip(&server).enumerate() {
            assert_eq!(
                here,
                there,
                "decision {number} of {} seats on seed {}: the page (left) and the server",
                seats.len(),
                setup.seed
            );
        }
    }
    ti4_wasm::set_round_limit_native(None);
}
