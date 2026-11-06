//! Property tests of the request/answer model (docs/FORK_RNG_THEORY.md 4.7 U): positions, determinism, splitting,
//! independence of seeds. No reference implementation needed: these are statements about `ti4-chance` alone.

use ti4_chance::{Domain, Faces, Kind, Request, Streams, answer, answer_mut};

fn dice(n: u8, sides: u8) -> Request {
    Request::new(Domain::Dice, Kind::Dice { n, sides, hits_on: None })
}

#[test]
fn an_answer_moves_its_stream_exactly_as_far_as_its_dice() {
    // A die is not a fixed number of words (the sampler rejects and redraws), so "as far as its dice" means: the
    // position after an answer of n dice is the position after n single dice, no other stream moves, and n = 0 moves
    // nothing.
    for seed in 0..100u64 {
        let mut a = Streams::new(seed);
        let mut b = Streams::new(seed);
        a.die(Domain::Neuraloop, 7);
        b.die(Domain::Neuraloop, 7);
        for n in 0..=12u8 {
            for sides in [1u8, 2, 3, 6, 10, 20, 255] {
                let before = a.position(Domain::Dice);
                let faces = answer_mut(&dice(n, sides), &mut a);
                assert_eq!(faces.len(), usize::from(n));
                for _ in 0..n {
                    b.die(Domain::Dice, u32::from(sides));
                }
                assert_eq!(a.position(Domain::Dice), b.position(Domain::Dice));
                if n == 0 {
                    assert_eq!(a.position(Domain::Dice), before);
                } else {
                    assert!(a.position(Domain::Dice) >= before + u128::from(n), "at least one word per die");
                }
                assert_eq!(a.position(Domain::Neuraloop), b.position(Domain::Neuraloop));
            }
        }
        assert_eq!(a, b);
    }
}

#[test]
fn answer_is_deterministic_and_pure() {
    for seed in 0..100u64 {
        let mut s = Streams::new(seed);
        s.die(Domain::Dice, 10);
        let before = s.clone();
        let req = dice(9, 10);
        let (f1, s1) = answer(&req, &s);
        let (f2, s2) = answer(&req, &s);
        assert_eq!((f1, &s1), (f2, &s2));
        assert_eq!(s, before, "the pure form leaves its input alone");
        assert!(s.shares_streams_with(&before));
        assert_eq!(answer_mut(&req, &mut s), f1);
        assert_eq!(s, s1);
        assert!(req.accepts(&f1));
    }
}

#[test]
fn a_split_group_equals_drawing_one_by_one() {
    for seed in 0..60u64 {
        for n in 0..=64usize {
            let mut grouped = Streams::new(seed);
            let mut single = Streams::new(seed);
            let mut got = Vec::new();
            for req in Request::dice_groups(Domain::Dice, n, 10, Some(7)) {
                assert!(req.len() <= ti4_chance::MAX_FACES);
                got.extend_from_slice(answer_mut(&req, &mut grouped).as_slice());
            }
            let want: Vec<u8> = (0..n).map(|_| u8::try_from(single.die(Domain::Dice, 10)).unwrap()).collect();
            assert_eq!(got, want, "seed {seed} n {n}");
            assert_eq!(grouped, single);
        }
    }
}

#[test]
fn another_seed_gives_independent_streams() {
    // What a search would need instead of reseeding (decision D2: a search never touches the streams, it answers with its
    // own randomness): another game's streams do not reveal this game's future.
    let draw = |s: &mut Streams, d: Domain| (0..64).map(|_| s.die(d, 1 << 30)).collect::<Vec<_>>();
    for seed in 0..50u64 {
        let mut real = Streams::new(seed);
        real.die(Domain::Dice, 10);
        let mut other = Streams::new(seed ^ 0xfeed);
        let mut future = real.clone();
        assert_ne!(draw(&mut other, Domain::Dice), draw(&mut future, Domain::Dice));
    }
    // Domains of one seed are independent streams too.
    let mut s = Streams::new(9);
    let lit = Domain::LITERALS;
    let firsts: Vec<_> = lit.iter().map(|d| draw(&mut s, *d)).collect();
    for i in 0..lit.len() {
        for j in i + 1..lit.len() {
            assert_ne!(firsts[i], firsts[j], "{:?} {:?}", lit[i], lit[j]);
        }
    }
}

#[test]
fn faces_round_trip() {
    let f = Faces::from_slice(&[3, 10, 1]).unwrap();
    assert_eq!(f.as_slice(), [3, 10, 1]);
    assert_eq!(f.hits(3), 2);
    assert_eq!(Faces::from_slice(&[]).unwrap(), Faces::default());
}
