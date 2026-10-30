//! Generators (docs/todo/D4-core-tests.md 5.1). The scenario generator is a pure function of `(seats, seed)`: proptest draws the pair, the
//! scenario is rebuilt from it, so a failing case is reproduced by two numbers. No floats, no hash iteration.

use proptest::prelude::*;
use ti4_engine_core::generated::{Faction, Planet, System, Unit};

use crate::scenario::{LOBBY, Scenario, plain_systems};

/// A tiny splitmix64: the generator's only source of randomness.
#[derive(Debug, Clone)]
pub struct Mix(pub u64);

impl Mix {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number below `n` (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next() % den < num
    }
}

/// `n` distinct lobby factions, chosen by `mix`.
#[must_use]
pub fn pick_factions(mix: &mut Mix, n: usize) -> Vec<Faction> {
    let mut pool: Vec<Faction> = LOBBY.to_vec();
    let mut out = Vec::new();
    for _ in 0..n {
        out.push(pool.remove(mix.below(pool.len())));
    }
    out
}

/// A random but legal position: `seats` distinct real factions, a map of 7 plain systems, a few ships per system and ground forces on
/// planets (never above the plastic limits), controlled planets, goods, commodities and tokens.
#[must_use]
pub fn random_scenario(seats: usize, seed: u64) -> Scenario {
    let mut mix = Mix(seed);
    let factions = pick_factions(&mut mix, seats);
    let systems = plain_systems();
    let map: Vec<System> = (0..7).map(|i| systems[(mix.below(systems.len() / 7) * 7 + i) % systems.len()]).fold(Vec::new(), |mut v, s| {
        if !v.contains(&s) {
            v.push(s);
        }
        v
    });
    let mut sc = Scenario::new(&factions).map_from(&map);
    let ships = [Unit::CarrierI, Unit::CruiserI, Unit::DestroyerI, Unit::DreadnoughtI, Unit::FighterI];
    let mut used = std::collections::BTreeMap::<(u8, &'static str), usize>::new();
    for seat in 0..seats as u8 {
        for _ in 0..mix.below(5) {
            let system = map[mix.below(map.len())];
            let unit = ships[mix.below(ships.len())];
            let key = (seat, unit_key(unit));
            let cap = unit_cap(unit);
            if used.get(&key).copied().unwrap_or(0) < cap {
                *used.entry(key).or_default() += 1;
                sc = sc.unit(system, None, unit, seat, 1);
            }
        }
        for _ in 0..mix.below(4) {
            let system = map[mix.below(map.len())];
            let planets = system.data().planets;
            let planet: Planet = planets[mix.below(planets.len())];
            let key = (seat, "inf");
            if used.get(&key).copied().unwrap_or(0) < 12 {
                *used.entry(key).or_default() += 1;
                sc = sc.unit(system, Some(planet), Unit::InfantryI, seat, 1);
            }
            if mix.chance(1, 2) {
                sc = sc.control(planet, seat);
            }
        }
        sc = sc.goods(seat, mix.below(6) as i16).commodities(seat, mix.below(3) as i16).tokens(seat, mix.below(4) as i16, mix.below(4) as i16, mix.below(3) as i16);
    }
    sc
}

fn unit_key(u: Unit) -> &'static str {
    match u {
        Unit::CarrierI => "car",
        Unit::CruiserI => "cru",
        Unit::DestroyerI => "des",
        Unit::DreadnoughtI => "dre",
        _ => "fig",
    }
}

fn unit_cap(u: Unit) -> usize {
    match u {
        Unit::CarrierI => 4,
        Unit::CruiserI | Unit::DestroyerI => 8,
        Unit::DreadnoughtI => 5,
        _ => 12,
    }
}

/// Seats 2 to 8 and a seed.
pub fn arb_scenario() -> impl Strategy<Value = Scenario> {
    (2usize..=8, any::<u64>()).prop_map(|(n, seed)| random_scenario(n, seed))
}

/// A pick script: each byte answers one question modulo the options offered (so every script is legal by construction).
pub fn arb_play() -> impl Strategy<Value = Vec<u8>> {
    proptest::collection::vec(any::<u8>(), 0..24)
}

/// The proptest configuration of the kit: `PROPTEST_CASES` (default 64) cases, regression files committed under `tests/proptest-regressions/`.
#[must_use]
pub fn config() -> ProptestConfig {
    let cases = std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(64);
    ProptestConfig {
        cases,
        failure_persistence: Some(Box::new(proptest::test_runner::FileFailurePersistence::WithSource("proptest-regressions"))),
        ..ProptestConfig::default()
    }
}
