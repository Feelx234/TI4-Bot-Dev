//! Every integration test of the server, as one binary: 28 separate ones linked the engine 28
//! times and filled the target directory. Run one file with `--test it <module>::`.

mod bot_lobby_service;
mod crash_recovery;
mod dev_scenario_recovery;
mod dev_scenarios;
mod draft_last_card;
mod draft_scripts;
mod fixtures_verification;
mod history;
mod leadership_batch;
mod lobby_lifecycle;
mod map_selection;
mod planning;
mod planning_runner;
mod planning_transport;
mod player_lobby_admission;
mod projection_redaction;
mod protocol_roundtrip;
mod random_seats;
mod reaction_modes;
mod replay_export;
mod secondary_planning_runner;
mod session_deterministic_replay;
mod session_disconnect;
mod session_e2e_game;
mod session_planning;
mod session_recovery_replay;
mod session_rejections;
mod session_secondary_planning;
mod size_bounds;
mod undo;
mod ws_lifecycle;
