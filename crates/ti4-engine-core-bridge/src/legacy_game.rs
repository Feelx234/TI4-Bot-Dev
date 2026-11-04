//! The legacy `Game` of a spec, for the oracle and the shadow only (never production): the position the NATIVE setup builds
//! (`GameSpec::build`, parity-proved by `tests/setup_parity.rs`) played by the frozen string engine.

use ti4_engine_legacy::game::Game as LegacyGame;
use ti4_selfplay::spec::GameSpec;

/// The game of `spec` on the legacy engine: the native position, seeded the same, with the same map and scope.
///
/// # Errors
/// A spec that does not build.
pub fn legacy_game(spec: &GameSpec) -> Result<LegacyGame<'static>, String> {
    let game = spec.build()?;
    let (state, sources) = (game.state.clone(), game.sources());
    let galaxy = game.galaxy().cloned();
    let mut legacy = LegacyGame::with_seeded_random(state, ti4_content::ContentStore::embedded(), spec.seed).with_sources(sources);
    if let Some(galaxy) = galaxy {
        legacy = legacy.with_galaxy(galaxy);
    }
    Ok(legacy)
}
