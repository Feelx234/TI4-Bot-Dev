//! The versioned frame fingerprint: one SHA-256 over what a rebuilt frame must reproduce.
//!
//! A rebuild is only proven, not claimed, if every frame it produces is *identical* to the frame it
//! is reproducing. Field-by-field comparison of a `ReviewFrame` is not practical — it embeds a whole
//! `GameState` — so this module reduces the frame to a canonical, versioned digest.
//!
//! # What is bound
//!
//! Engine step, decision count, action count, round, phase, active player, whether the step resolved
//! a choice, whether an action completed, terminal and error state, the new plain-text events, the
//! structured events with their payloads, and the canonical serialization of the whole
//! `GameState`. The state is hashed in full rather than field-by-field on purpose: the plan asks for
//! "canonical state ... pending timing", and the in-flight timing windows, reroll sets and transaction
//! bookkeeping all live inside `GameState`. Naming the sub-fields here would silently under-bind the
//! moment the engine grows one.
//!
//! # What is not
//!
//! Filesystem paths, branch names and ids, UI selection, wall-clock timestamps, and the session's
//! own manifest — none of those may change whether a frame is the same frame. The frame's own `index`
//! is excluded too: it is position in a session, which the caller already knows, and binding it
//! would let two different numbering schemes look like different games.
//!
//! The version string is part of the hashed value, so a fingerprint can never be mistaken for one
//! computed under a different binding.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ti4_review::ReviewFrame;

/// Binding version, embedded in every digest.
pub const FRAME_FINGERPRINT_VERSION: &str = "r02-frame-v1";

/// A frame's identity: `sha256:<hex>`, versioned.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FrameFingerprint(String);

impl FrameFingerprint {
    /// Digest a real reviewer frame.
    #[must_use]
    pub fn of(frame: &ReviewFrame) -> Self {
        Self::from_value(&bound(frame))
    }

    /// Digest an already-projected value. Public so a caller can fingerprint the same projection it
    /// built itself — and so the binding's sensitivity can be tested without a running game.
    ///
    /// # Panics
    /// Never in practice: a `serde_json::Value` always serializes, and the `expect` records that
    /// invariant rather than papering over a failure mode.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let bytes = serde_json::to_vec(value).expect("a serde_json value always serializes");
        let mut hasher = Sha256::new();
        hasher.update(FRAME_FINGERPRINT_VERSION.as_bytes());
        hasher.update([0u8]);
        hasher.update(bytes);
        Self(format!("sha256:{}", hex(&hasher.finalize())))
    }

    #[must_use]
    pub fn as_hex(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn matches(&self, frame: &ReviewFrame) -> bool {
        self == &Self::of(frame)
    }
}

/// The projection the digest is taken over.
///
/// Kept as a value rather than a struct so the field set is visible in one place and the ordering is
/// `serde_json`'s canonical one.
#[must_use]
pub fn bound(frame: &ReviewFrame) -> Value {
    serde_json::json!({
        "engine_step": frame.engine_step,
        "decision_count": frame.decision_count,
        "action_count": frame.action_count,
        "round": frame.round,
        "phase": frame.phase,
        "active": frame.active,
        "resolved_choice": frame.resolved_choice,
        "action_completed": frame.action_completed,
        "finished": frame.finished,
        "error": frame.error,
        "new_events": frame.new_events,
        "structured_events": frame.structured_events,
        "state": frame.state,
    })
}

/// The state alone, so a mismatch can say "the state differs" without dumping it.
#[must_use]
pub fn state_fingerprint(frame: &ReviewFrame) -> FrameFingerprint {
    FrameFingerprint::from_value(&serde_json::json!({ "state": frame.state }))
}

/// Name the first thing two frames disagree about, for a divergence diagnostic.
///
/// Returns `None` when the fingerprints agree, and a coarse but useful label plus the two values
/// otherwise. `state` is reported by digest rather than by content: the point is to tell a human
/// *where* to look, not to print a whole game state.
#[must_use]
pub fn first_difference(left: &ReviewFrame, right: &ReviewFrame) -> Option<String> {
    if FrameFingerprint::of(left) == FrameFingerprint::of(right) {
        return None;
    }
    let steps = [
        (
            "engine_step",
            left.engine_step == right.engine_step,
            format!("{}, {}", left.engine_step, right.engine_step),
        ),
        (
            "round",
            left.round == right.round,
            format!("{}, {}", left.round, right.round),
        ),
        (
            "phase",
            left.phase == right.phase,
            format!("{:?}, {:?}", left.phase, right.phase),
        ),
        (
            "active",
            left.active == right.active,
            format!("{:?}, {:?}", left.active, right.active),
        ),
        (
            "decision_count",
            left.decision_count == right.decision_count,
            format!("{}, {}", left.decision_count, right.decision_count),
        ),
        (
            "action_count",
            left.action_count == right.action_count,
            format!("{}, {}", left.action_count, right.action_count),
        ),
        (
            "resolved_choice",
            left.resolved_choice == right.resolved_choice,
            format!("{}, {}", left.resolved_choice, right.resolved_choice),
        ),
        (
            "finished",
            left.finished == right.finished,
            format!("{}, {}", left.finished, right.finished),
        ),
        (
            "error",
            left.error == right.error,
            format!("{:?}, {:?}", left.error, right.error),
        ),
        (
            "new_events",
            left.new_events == right.new_events,
            format!(
                "{} event(s) vs {}: {:?} vs {:?}",
                left.new_events.len(),
                right.new_events.len(),
                head(&left.new_events),
                head(&right.new_events),
            ),
        ),
        (
            "structured_events",
            left.structured_events == right.structured_events,
            format!(
                "{} vs {} event(s)",
                left.structured_events.len(),
                right.structured_events.len()
            ),
        ),
    ]
    .into_iter()
    .find(|(_, equal, _)| !*equal)
    .map(|(field, _, detail)| format!("{field}: {detail}"));
    if steps.is_some() {
        return steps;
    }
    Some(format!(
        "state: {} vs {}",
        state_fingerprint(left).as_hex(),
        state_fingerprint(right).as_hex()
    ))
}

fn head(events: &[String]) -> Vec<&str> {
    events.iter().map(String::as_str).take(3).collect()
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).expect("nibble"));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).expect("nibble"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bound_of(pairs: &[(&str, i64)]) -> Value {
        let mut map = serde_json::Map::new();
        for (key, value) in pairs {
            map.insert((*key).to_owned(), serde_json::json!(*value));
        }
        Value::Object(map)
    }

    #[test]
    fn equal_projections_digest_alike_and_key_order_does_not_matter() {
        let left = FrameFingerprint::from_value(&bound_of(&[("a", 1), ("b", 2)]));
        let right = FrameFingerprint::from_value(&bound_of(&[("b", 2), ("a", 1)]));
        assert_eq!(left, right, "canonical JSON ordering is what is hashed");
        assert!(left.as_hex().starts_with("sha256:"));
        assert_eq!(left.as_hex().len(), 7 + 64);
    }

    #[test]
    fn any_bound_difference_changes_the_digest() {
        let base = FrameFingerprint::from_value(&bound_of(&[("a", 1), ("b", 2), ("c", 3)]));
        for pairs in [
            vec![("a", 2), ("b", 2), ("c", 3)],
            vec![("a", 1), ("b", 3), ("c", 3)],
            vec![("a", 1), ("b", 2), ("c", 4)],
            vec![("a", 1), ("b", 2)],
        ] {
            let other = FrameFingerprint::from_value(&bound_of(&pairs));
            assert_ne!(base, other, "{pairs:?} must not collide with the base");
        }
    }

    #[test]
    fn an_added_field_is_a_different_binding_not_a_reused_digest() {
        // A rebuild that dropped a bound field would otherwise look identical to one that kept it.
        let without = FrameFingerprint::from_value(&bound_of(&[("a", 1)]));
        let with = FrameFingerprint::from_value(&bound_of(&[("a", 1), ("engine_step", 0)]));
        assert_ne!(without, with);
    }

    #[test]
    fn the_version_is_part_of_the_digest() {
        // Recomputing by hand shows the version is folded in before the payload, so a digest made
        // under another binding cannot equal one made under this one.
        let value = bound_of(&[("a", 1)]);
        let bytes = serde_json::to_vec(&value).expect("serializes");
        let mut hasher = Sha256::new();
        hasher.update(FRAME_FINGERPRINT_VERSION.as_bytes());
        hasher.update([0u8]);
        hasher.update(&bytes);
        let expected = format!("sha256:{}", hex(&hasher.finalize()));
        assert_eq!(FrameFingerprint::from_value(&value).as_hex(), expected);

        let mut hasher = Sha256::new();
        hasher.update(b"r02-frame-v0");
        hasher.update([0u8]);
        hasher.update(&bytes);
        assert_ne!(format!("sha256:{}", hex(&hasher.finalize())), expected);
    }
}
