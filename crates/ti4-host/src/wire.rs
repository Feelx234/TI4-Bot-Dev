//! Big integers on the JSON wire.
//!
//! A JavaScript `Number` holds integers exactly only up to 2^53. A seed is a `u64` and a deck
//! segment's end is `usize::MAX`, so a file that passes through a browser (`response.json()`,
//! `JSON.parse`, a paste) comes back rounded. The exports ([`crate::export`]) therefore write such
//! integers as decimal strings, and everything that reads them accepts both a number (every file
//! made before the strings) and a string. A rounded float is read too, so an old file still loads:
//! a value at or above 2^64 reads as `u64::MAX`, which is what `usize::MAX` became.

use serde::Serializer;
use serde::de::{self, Deserializer, Visitor};
use serde_json::Value;

/// The largest integer that a JavaScript `Number` holds exactly.
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// An integer as it can be read: `value` as a number or a string. `None` for anything else, and
/// for a fraction or a negative number.
#[must_use]
pub fn read_u128(value: &Value) -> Option<u128> {
    match value {
        Value::Number(number) => {
            if let Some(whole) = number.as_u64() {
                Some(u128::from(whole))
            } else {
                number.as_f64().and_then(whole_float)
            }
        }
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// [`read_u128`] limited to a `u64` (a larger value saturates, as a rounded 2^64 should).
#[must_use]
pub fn read_u64(value: &Value) -> Option<u64> {
    read_u128(value).map(|whole| u64::try_from(whole).unwrap_or(u64::MAX))
}

fn whole_float(float: f64) -> Option<u128> {
    // A browser rounds 2^64 - 1 to the float 2^64; `as` saturates.
    (float.is_finite() && float >= 0.0 && float.fract() == 0.0).then_some(float as u128)
}

/// `serde(with = ...)` for a `u64` written as a string and read as a string or a number.
pub mod u64_text {
    use super::{Deserializer, LenientU64, Serializer};

    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        deserializer.deserialize_any(LenientU64)
    }
}

/// [`u64_text`] for an optional field.
pub mod opt_u64_text {
    use super::{Deserializer, LenientU64, Serializer};
    use serde::Deserialize;

    #[allow(clippy::ref_option)]
    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        Ok(Option::<Wrapped>::deserialize(deserializer)?.map(|wrapped| wrapped.0))
    }

    struct Wrapped(u64);

    impl<'de> Deserialize<'de> for Wrapped {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserializer.deserialize_any(LenientU64).map(Self)
        }
    }
}

/// Reads only: a `u64` from a number or a string. For a field that is still written as a number
/// (the offline save's seed stays within 2^53; see `web/src/offline/local/launcher.ts`).
pub fn lenient_u64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    deserializer.deserialize_any(LenientU64)
}

/// The visitor behind the helpers above.
pub struct LenientU64;

impl Visitor<'_> for LenientU64 {
    type Value = u64;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an unsigned integer, as a number or a decimal string")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
        Ok(value)
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<u64, E> {
        u64::try_from(value).map_err(|_| E::custom("a negative number"))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<u64, E> {
        whole_float(value)
            .map(|whole| u64::try_from(whole).unwrap_or(u64::MAX))
            .ok_or_else(|| E::custom("not a whole non-negative number"))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<u64, E> {
        text.trim()
            .parse()
            .map_err(|_| E::custom(format!("{text:?} is not an unsigned integer")))
    }
}

/// A `u64` from a number or a string, for use as `Value` plumbing.
///
/// # Errors
/// A message when `value` is neither.
pub fn u64_of(value: &Value, what: &str) -> Result<u64, String> {
    read_u64(value).ok_or_else(|| format!("{what} is not an unsigned integer: {value}"))
}

/// Turns the number or string at `key` of the object `value` into a number (what the typed
/// readers of the stored forms expect), if it is there.
pub fn number_at(value: &mut Value, key: &str) {
    if let Some(slot) = value.get_mut(key)
        && let Some(whole) = read_u128(slot)
    {
        *slot = u64::try_from(whole).map_or_else(|_| Value::from(u64::MAX), Value::from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Probe {
        #[serde(with = "u64_text")]
        seed: u64,
        #[serde(default, with = "opt_u64_text")]
        other: Option<u64>,
    }

    #[test]
    fn a_big_seed_is_a_string_and_survives_a_number_hostile_trip() {
        let probe = Probe {
            seed: 18_432_220_952_314_264_515,
            other: Some(u64::MAX),
        };
        let text = serde_json::to_string(&probe).unwrap();
        assert_eq!(
            text,
            r#"{"seed":"18432220952314264515","other":"18446744073709551615"}"#
        );
        assert_eq!(serde_json::from_str::<Probe>(&text).unwrap(), probe);
    }

    #[test]
    fn numbers_strings_and_rounded_floats_are_all_read() {
        let read = |text: &str| serde_json::from_str::<Probe>(text).unwrap();
        assert_eq!(read(r#"{"seed": 42}"#).seed, 42);
        assert_eq!(read(r#"{"seed": "42"}"#).seed, 42);
        // What a browser makes of u64::MAX: the float 2^64.
        assert_eq!(read(r#"{"seed": 18446744073709552000}"#).seed, u64::MAX);
        assert_eq!(read(r#"{"seed": 1.8446744073709552e19}"#).seed, u64::MAX);
        assert_eq!(read(r#"{"seed": 1, "other": null}"#).other, None);
        assert!(serde_json::from_str::<Probe>(r#"{"seed": -1}"#).is_err());
        assert!(serde_json::from_str::<Probe>(r#"{"seed": "x"}"#).is_err());
    }
}
