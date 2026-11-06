//! Proof tests U (docs/FORK_RNG_THEORY.md 4.7): `ti4-chance` reproduces today's draws bit for bit.
//!
//! The reference is the old implementation, not recorded vectors: the core's `state::Rng` seeded by the setup's
//! `rng::Literals` (the same `SHA-256(seed_le || text)` as the thin crate's `GameRng`, which the core's own tests and
//! the dual runs pin against it). Since lane C2 the core's streams are this crate's, so the old code is kept as a frozen
//! copy in `tests/reference` (see there).

use ti4_chance::{Domain, Kind, ModForm, Request, Streams, answer, answer_mut, derive_seed};
mod reference;

use reference::{Literals, Rng as OldRng, RngDomain, domain_text, seeded_speaker_index};

/// The new domain of an old one; exhaustive, so a variant added to either side fails to compile here.
fn new_of(d: RngDomain) -> Domain {
    match d {
        RngDomain::Dice => Domain::Dice,
        RngDomain::Objectives => Domain::Objectives,
        RngDomain::Agendas => Domain::Agendas,
        RngDomain::ActionCards => Domain::ActionCards,
        RngDomain::Secrets => Domain::Secrets,
        RngDomain::Relics => Domain::Relics,
        RngDomain::Galaxy => Domain::Galaxy,
        RngDomain::Exploration => Domain::Exploration,
        RngDomain::Map => Domain::Map,
        RngDomain::Speaker => Domain::Speaker,
        RngDomain::Fracture => Domain::Fracture,
        RngDomain::Neuraloop => Domain::Neuraloop,
        RngDomain::YssarilHero => Domain::YssarilHero,
        RngDomain::Direct => Domain::Direct,
        RngDomain::DeckA => Domain::DeckA,
        RngDomain::DeckB => Domain::DeckB,
        RngDomain::RetryProbe => Domain::RetryProbe,
        RngDomain::SeatingFactionAssignment => Domain::SeatingFactionAssignment,
        RngDomain::SeatingLobbyDraw => Domain::SeatingLobbyDraw,
        RngDomain::WideRoster => Domain::WideRoster,
        RngDomain::KeleresVariant => Domain::KeleresVariant,
        RngDomain::YinAscendant => Domain::YinAscendant,
    }
}

const OLD: [RngDomain; 22] = [
    RngDomain::Dice,
    RngDomain::Objectives,
    RngDomain::Agendas,
    RngDomain::ActionCards,
    RngDomain::Secrets,
    RngDomain::Relics,
    RngDomain::Galaxy,
    RngDomain::Exploration,
    RngDomain::Map,
    RngDomain::Speaker,
    RngDomain::Fracture,
    RngDomain::Neuraloop,
    RngDomain::YssarilHero,
    RngDomain::Direct,
    RngDomain::DeckA,
    RngDomain::DeckB,
    RngDomain::RetryProbe,
    RngDomain::SeatingFactionAssignment,
    RngDomain::SeatingLobbyDraw,
    RngDomain::WideRoster,
    RngDomain::KeleresVariant,
    RngDomain::YinAscendant,
];

/// Deterministic test parameters (splitmix64), so no extra dependency is needed.
struct Mix(u64);
impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn positions_of(old: &OldRng) -> Vec<(Domain, u128)> {
    old.positions().map(|(d, p)| (new_of(d), p)).collect()
}

#[test]
fn domains_texts_seeds_and_order_are_todays() {
    for (i, old) in OLD.into_iter().enumerate() {
        let new = new_of(old);
        assert_eq!(new, Domain::LITERALS[i], "the literal list is in domain order");
        assert_eq!(new.text(), domain_text(old), "{old:?}");
        assert_eq!(Domain::of_text(new.text()), Some(new));
        for seed in [0, 1, 7, u64::MAX, 0x5eed_1234_abcd] {
            let text = new.text();
            assert_eq!(derive_seed(seed, text), reference::derive_seed(seed, text));
        }
    }
    let all: Vec<RngDomain> = OLD.to_vec();
    for a in &all {
        for b in &all {
            assert_eq!(a.cmp(b), new_of(*a).cmp(&new_of(*b)), "{a:?} vs {b:?}: positions are reported in domain order");
        }
    }
}

#[test]
fn dice_of_every_domain_match_the_core_rng() {
    // Every literal domain, 200 seeds, a mixed sequence of dice (all sides 1..=255 and some wide u32 dice as today's
    // `die(u32)` allows), interleaved across domains so the stream table and its order are exercised too.
    for seed in 0..200u64 {
        let mut old = OldRng::new(seed);
        let mut new = Streams::new(seed);
        let mut mix = Mix(seed);
        for _ in 0..400 {
            let d = OLD[usize::try_from(mix.below(22)).unwrap()];
            let sides = match mix.below(8) {
                0 => u32::try_from(mix.below(1 << 20)).unwrap() + 1,
                1 => u32::MAX,
                _ => u32::try_from(mix.below(255)).unwrap() + 1,
            };
            assert_eq!(new.die(new_of(d), sides), old.die(d, &Literals, sides), "seed {seed} {d:?} d{sides}");
        }
        assert_eq!(new.positions().collect::<Vec<_>>(), positions_of(&old), "seed {seed}");
    }
}

#[test]
fn answers_of_every_kind_match_one_core_die_per_face() {
    for seed in 0..150u64 {
        let mut old = OldRng::new(seed);
        let mut new = Streams::new(seed);
        let mut mix = Mix(seed ^ 0xabc);
        for _ in 0..300 {
            let d = OLD[usize::try_from(mix.below(22)).unwrap()];
            let sides = u8::try_from(mix.below(255) + 1).unwrap();
            let n = u8::try_from(mix.below(13)).unwrap();
            let kind = match mix.below(3) {
                0 => Kind::Dice { n, sides, hits_on: Some(sides) },
                1 => Kind::Index { n: sides },
                _ => Kind::DieMod { sides, n: n.max(1), form: ModForm::Face },
            };
            let req = Request::new(new_of(d), kind);
            let (pure, next) = answer(&req, &new);
            let faces = answer_mut(&req, &mut new);
            assert_eq!(pure, faces);
            assert_eq!(next, new);
            let expect: Vec<u8> = (0..req.len()).map(|_| u8::try_from(old.die(d, &Literals, u32::from(sides))).unwrap()).collect();
            assert_eq!(faces.as_slice(), expect, "seed {seed} {req:?}");
        }
        assert_eq!(new.positions().collect::<Vec<_>>(), positions_of(&old));
    }
}

#[test]
fn spy_and_sanctions_die_mod_is_todays_biased_pick() {
    // Spy: `(face - 1) % held` of `die(Dice, 10)`; Sanctions: `face % held`. Bit-identical, bias included (lane C9 fixes it).
    for seed in 0..300u64 {
        let mut old = OldRng::new(seed);
        let mut new = Streams::new(seed);
        for held in 1..=12u8 {
            for form in [ModForm::FaceMinusOne, ModForm::Face] {
                let face = old.die(RngDomain::Dice, &Literals, 10);
                let held_us = usize::from(held);
                let today = match form {
                    ModForm::FaceMinusOne => (usize::try_from(face).unwrap() - 1) % held_us,
                    ModForm::Face => usize::try_from(face).unwrap() % held_us,
                };
                let f = answer_mut(&Request::new(Domain::Dice, Kind::DieMod { sides: 10, n: held, form }), &mut new);
                assert_eq!(u32::from(f.as_slice()[0]), face, "the raw face is what the ledger records");
                assert_eq!(usize::from(form.index(f.as_slice()[0], held)), today);
            }
        }
    }
}

#[test]
fn setup_deck_shuffles_match() {
    // The setup's sequence (ti4-engine-core-setup deck.rs): objectives stage I then II on one stream, exploration per
    // trait, relics, agendas, action cards, secrets, one generator. A shuffle depends only on the length, so deck-sized
    // ranges stand in for the card ids; lengths around the real decks and the edge cases 0 and 1.
    let order = [
        RngDomain::Objectives,
        RngDomain::Objectives,
        RngDomain::Exploration,
        RngDomain::Exploration,
        RngDomain::Exploration,
        RngDomain::Exploration,
        RngDomain::Relics,
        RngDomain::Agendas,
        RngDomain::ActionCards,
        RngDomain::Secrets,
    ];
    for seed in 0..200u64 {
        let mut old = OldRng::new(seed);
        let mut new = Streams::new(seed);
        let mut mix = Mix(seed ^ 0xdec);
        for d in order {
            let len = match mix.below(10) {
                0 => 0,
                1 => 1,
                _ => usize::try_from(mix.below(160)).unwrap(),
            };
            let mut a: Vec<u32> = (0..u32::try_from(len).unwrap()).collect();
            let mut b = a.clone();
            old.shuffle(d, &Literals, &mut a);
            new.shuffle(new_of(d), &mut b);
            assert_eq!(a, b, "seed {seed} {d:?} len {len}");
        }
        assert_eq!(new.positions().collect::<Vec<_>>(), positions_of(&old));
    }
}

#[test]
fn seeded_speaker_index_matches() {
    for seed in 0..500u64 {
        for seats in 1..=8usize {
            let mut s = Streams::new(seed);
            assert_eq!(Some(s.index(Domain::Speaker, seats)), seeded_speaker_index(seats, seed));
        }
    }
}

#[test]
fn set_positions_matches_the_core() {
    for seed in 0..100u64 {
        let mut mix = Mix(seed ^ 0x9051);
        let mut old = OldRng::new(seed);
        // A different history on the same seed: only the seed and the positions may matter after `set_positions`.
        let mut new = Streams::new(seed);
        for _ in 0..50 {
            let d = OLD[usize::try_from(mix.below(22)).unwrap()];
            old.die(d, &Literals, 10);
            new.die(new_of(d), 6);
        }
        let mut pos: Vec<(RngDomain, u128)> = old.positions().collect();
        pos.truncate(pos.len() / 2 + 1);
        for p in &mut pos {
            p.1 += u128::from(mix.below(1000));
        }
        old.set_positions(&pos, &Literals);
        new.set_positions(&pos.iter().map(|(d, p)| (new_of(*d), *p)).collect::<Vec<_>>());
        assert_eq!(new.positions().collect::<Vec<_>>(), positions_of(&old));
        for _ in 0..50 {
            let d = OLD[usize::try_from(mix.below(22)).unwrap()];
            assert_eq!(new.die(new_of(d), 10), old.die(d, &Literals, 10), "seed {seed}");
        }
    }
}

#[test]
fn clones_share_until_the_first_draw_like_the_core() {
    let mut a = Streams::new(3);
    a.die(Domain::Dice, 10);
    let b = a.clone();
    assert!(a.shares_streams_with(&b));
    a.die(Domain::Dice, 10);
    assert!(!a.shares_streams_with(&b));
    assert_ne!(a, b);
}
