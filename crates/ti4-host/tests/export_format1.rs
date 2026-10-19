//! A replay file of format 1 (the export before the setup was added; a trimmed copy of a real one,
//! with the seed and the open deck segment rounded the way a browser copy rounds them) still loads,
//! and says why it cannot be replayed on its own.

use ti4_host::export::ReplayExport;
use ti4_model::deck_reserve::OPEN;

#[test]
fn a_format_1_file_still_loads() {
    let text = include_str!("fixtures/replay_format1_sample.json");
    let export = ReplayExport::parse(text).expect("the old file loads");
    assert_eq!(export.format_version, 1);
    assert_eq!(export.version, 1);
    assert_eq!(export.player_ids.len(), 6);
    assert_eq!(export.map_template.as_deref(), Some("6pScpt2021Prelim"));
    // The seed was rounded by the copy; it reads as the number that is written.
    assert_eq!(export.seed, Some(18_432_220_952_314_264_000));
    assert_eq!(export.history.decisions.len(), 6);
    assert_eq!(export.history.decisions[0].chosen, "ps");
    assert_eq!(export.history.rng_marks.deck_plan[0].until, 335);
    assert_eq!(export.history.rng_marks.deck_plan[1].until, OPEN);
    assert_eq!(export.history.rng_marks[&331]["dice"], 11);
    assert!(export.setup.is_none() && export.build.is_none() && export.check.is_none());
    let error = export.replay().expect_err("no setup, no replay");
    assert!(error.contains("no setup"), "{error}");
}

#[test]
fn a_format_1_file_written_again_becomes_a_readable_format_2_envelope_with_string_integers() {
    let text = include_str!("fixtures/replay_format1_sample.json");
    let export = ReplayExport::parse(text).unwrap();
    let again = serde_json::to_string(&export).unwrap();
    assert!(
        again.contains(r#""seed":"18432220952314264000""#),
        "{again}"
    );
    assert!(
        again.contains(r#""until":"18446744073709551615""#),
        "{again}"
    );
    let back = ReplayExport::parse(&again).unwrap();
    assert_eq!(back.seed, export.seed);
    assert_eq!(back.history.rng_marks, export.history.rng_marks);
}
