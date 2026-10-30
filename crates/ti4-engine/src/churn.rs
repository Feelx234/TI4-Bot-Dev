//! Churn modes (docs/todo/D2-prereqs.md D2-4a): rebuild a part of the game from its portable image
//! before every `Game::step`, to prove that the image carries everything. Switched by the native
//! environment variable `TI4_CHURN` (a comma list, read once): `resolver` rebuilds the timing resolver
//! (`resolver_sync`); `state` sends the game state and the dice streams through the core before every step
//! (`state := materialize(load(state))`, D2-5 item 8: the flip, simulated). Never on in the browser build, and
//! never on by default.

/// Whether the timing resolver is rebuilt from its image before every step.
#[must_use]
pub fn resolver() -> bool {
    mode("resolver")
}

/// Whether the game state goes through the core before every step.
#[must_use]
pub fn state() -> bool {
    mode("state")
}

#[cfg(not(target_arch = "wasm32"))]
fn mode(name: &str) -> bool {
    use std::sync::OnceLock;
    static MODES: OnceLock<Vec<String>> = OnceLock::new();
    MODES
        .get_or_init(|| {
            std::env::var("TI4_CHURN")
                .map(|v| v.split(',').map(|m| m.trim().to_owned()).filter(|m| !m.is_empty()).collect())
                .unwrap_or_default()
        })
        .iter()
        .any(|m| m == name)
}

#[cfg(target_arch = "wasm32")]
fn mode(_: &str) -> bool {
    false
}
