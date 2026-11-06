//! `ti4-mcts-features` on fixed positions: rule-bot games of fixed seeds, stopped at a fixed decision.

use ti4_engine_core::choice::Choice;
use ti4_engine_core::generated::{ActionCard, SecretObjective};
use ti4_engine_core::seen::Seen;
use ti4_engine_core::state::CoreState;
use ti4_mcts_features::encode::{self, Encoded, FA, FG, FO, FP, FS, MAX_OBJ, MAX_SYS, SEATS};
use ti4_mcts_features::kind::{self, TreeKind};
use ti4_mcts_features::shard::{Meta, Record, ShardWriter};
use ti4_mcts_features::{filter, npy};
use ti4_mcts_lab::{rule_bot, start, Asked, Decide, Table};

/// Plays the rule bot and keeps the position and the question of the first ask that `want` accepts (then stops the game).
struct Catch<F: Fn(&Asked<'_>) -> bool> {
    want: F,
    got: Option<(CoreState, Choice)>,
}

impl<F: Fn(&Asked<'_>) -> bool> Decide for Catch<F> {
    fn decide(&mut self, t: &Table, a: &Asked<'_>) -> Result<usize, String> {
        if self.got.is_none() && (self.want)(a) {
            self.got = Some((a.at.clone(), a.choice.clone()));
            return Err("caught".to_owned());
        }
        Ok(rule_bot(t, a).0)
    }
}

fn position(seed: u64, want: impl Fn(&Asked<'_>) -> bool) -> (Table, CoreState, Choice) {
    let (t, mut pos) = start(&ti4_mcts_lab::spec(seed)).expect("the table builds");
    let mut c = Catch { want, got: None };
    let _ = ti4_mcts_lab::play(&t, &mut pos, &mut c, 6000, 12, |_, _| {});
    let (core, choice) = c.got.expect("the decision was reached");
    (t, core, choice)
}

fn first_of(seed: u64, k: TreeKind, from_round: u32) -> (Table, CoreState, Choice) {
    position(seed, move |a| kind::tree_kind_of(a.choice) == Some(k) && a.at.meta().round >= from_round && a.choice.opts.len() >= 2)
}

fn enc(t: &Table, core: &CoreState, choice: &Choice) -> Encoded {
    let seen = Seen::new(core, t.kernel, choice.actor);
    encode::encode(&seen, choice)
}

#[test]
fn the_widths_are_the_constants_and_the_names_match() {
    let (t, core, choice) = first_of(3, TreeKind::Activate, 1);
    let e = enc(&t, &core, &choice);
    assert_eq!(e.sys.len(), MAX_SYS * FS);
    assert_eq!(e.seat.len(), SEATS * FP);
    assert_eq!(e.obj.len(), MAX_OBJ * FO);
    assert_eq!(e.glob.len(), FG);
    assert_eq!(e.opt.len(), choice.opts.len() * FA);
    assert_eq!(e.adj.len(), MAX_SYS * encode::ADJ_BYTES);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let names = encode::column_names(&seen, &choice);
    assert_eq!([names[0].len(), names[1].len(), names[2].len(), names[3].len(), names[4].len()], [FS, FP, FO, FG, FA]);
    for block in &names {
        let set: std::collections::BTreeSet<&String> = block.iter().collect();
        assert_eq!(set.len(), block.len(), "column names are unique within a block");
    }
    assert!(e.sys.iter().chain(&e.seat).chain(&e.obj).chain(&e.glob).chain(&e.opt).all(|v| v.is_finite() && (-1.0..=64.0).contains(v)));
}

#[test]
fn the_map_has_37_systems_with_symmetric_adjacency_and_four_homes() {
    let (t, core, choice) = first_of(5, TreeKind::Draft, 1);
    let e = enc(&t, &core, &choice);
    let present = (0..MAX_SYS).filter(|i| e.sys[i * FS] == 1.0).count();
    assert_eq!(present, 37, "Learn to Play 4p");
    assert!(e.sys_id[..37].iter().all(|x| *x >= 0) && e.sys_id[37..].iter().all(|x| *x == -1));
    let bit = |i: usize, j: usize| e.adj[i * encode::ADJ_BYTES + j / 8] & (1 << (j % 8)) != 0;
    for i in 0..37 {
        assert!(!bit(i, i));
        for j in 0..37 {
            assert_eq!(bit(i, j), bit(j, i), "adjacency {i} {j}");
        }
        assert!((0..37).any(|j| bit(i, j)), "slot {i} has a neighbour");
    }
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let f = encode::Frame::new(&seen, choice.actor);
    let homes: Vec<usize> = f.homes.iter().map(|h| h.expect("every seat has a home on the map")).collect();
    assert_eq!(homes.iter().collect::<std::collections::BTreeSet<_>>().len(), 4);
    for (r, h) in homes.iter().enumerate() {
        assert_eq!(f.dist[*h][*h], 0);
        // The home flags of the system token.
        let names = encode::column_names(&seen, &choice);
        let col = names[0].iter().position(|n| *n == format!("home_of_p{r}")).expect("a home column");
        assert_eq!(e.sys[h * FS + col], 1.0);
    }
    // The actor's home holds its starting fleet: some space units of seat block 0.
    let col = names_of(&t, &core, &choice, 0, "p0_space_Carrier");
    assert!(e.sys[homes[0] * FS + col] > 0.0, "a carrier at home at the start");
}

fn names_of(t: &Table, core: &CoreState, choice: &Choice, block: usize, name: &str) -> usize {
    let seen = Seen::new(core, t.kernel, choice.actor);
    encode::column_names(&seen, choice)[block].iter().position(|n| n == name).unwrap_or_else(|| panic!("no column {name}"))
}

#[test]
fn the_seats_are_actor_relative() {
    let (t, core, choice) = first_of(7, TreeKind::Menu, 1);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let rel = encode::relative_seats(&seen, choice.actor);
    assert_eq!(rel[0], choice.actor);
    let e_actor = enc(&t, &core, &choice);
    assert_eq!(e_actor.seat_fac[0], seen.seat(choice.actor).map(|v| v.faction as i16).unwrap_or(-1));
    // The same position seen by the next seat: its seat blocks are a rotation of the actor's (the seat sheet does not depend on the viewer).
    let other = rel[1];
    let mut as_other = choice.clone();
    as_other.actor = other;
    let seen_other = Seen::new(&core, t.kernel, other);
    let e_other = encode::encode(&seen_other, &as_other);
    for r in 0..SEATS {
        let r_other = (r + SEATS - 1) % SEATS;
        assert_eq!(&e_actor.seat[r * FP..(r + 1) * FP], &e_other.seat[r_other * FP..(r_other + 1) * FP], "seat block {r}");
        assert_eq!(e_actor.seat_fac[r], e_other.seat_fac[r_other]);
    }
    // The unit counts of the system tokens rotate too.
    for slot in 0..37 {
        for r in 0..SEATS {
            let r_other = (r + SEATS - 1) % SEATS;
            let a = &e_actor.sys[slot * FS + 1 + r * 17..slot * FS + 1 + (r + 1) * 17];
            let b = &e_other.sys[slot * FS + 1 + r_other * 17..slot * FS + 1 + (r_other + 1) * 17];
            assert_eq!(a, b, "slot {slot} seat block {r}");
        }
    }
}

#[test]
fn hidden_information_does_not_reach_the_features() {
    let (t, core, choice) = first_of(11, TreeKind::Activate, 2);
    let before = enc(&t, &core, &choice);
    let mut changed = core.clone();
    // Every other seat's action cards and secrets in hand replaced by others (the counts stay: they are public), every deck reversed.
    let n = changed.players().len();
    for i in 0..n {
        let p = changed.players_mut().edit(i);
        if p.seat == choice.actor {
            continue;
        }
        let (hand, secrets) = (p.cards.hand.len(), p.cards.secrets.len());
        p.cards.hand.clear();
        for k in 0..hand {
            p.cards.hand.push(ActionCard::ALL[(k * 7 + 3) % ActionCard::COUNT]);
        }
        p.cards.secrets.clear();
        for k in 0..secrets {
            p.cards.secrets.push(SecretObjective::ALL[(k * 5 + 17 + i) % SecretObjective::COUNT]);
        }
    }
    let decks = changed.decks_mut();
    let mut objective = decks.objective.cards().to_vec();
    objective.reverse();
    decks.objective.set(objective);
    let mut agenda = decks.agenda.cards().to_vec();
    agenda.reverse();
    decks.agenda.set(agenda);
    let mut action = decks.action.cards().to_vec();
    action.reverse();
    decks.action.set(action);
    let mut secret = decks.secret.cards().to_vec();
    secret.reverse();
    decks.secret.set(secret);
    assert_ne!(changed, core, "the change is real");
    assert_eq!(enc(&t, &changed, &choice), before, "the features read nothing hidden");
    // The control: the actor's own secrets are read.
    let mut mine = core.clone();
    let i = (0..n).find(|i| mine.players().iter().nth(*i).is_some_and(|p| p.seat == choice.actor)).expect("the actor plays");
    let p = mine.players_mut().edit(i);
    p.cards.secrets.clear();
    p.cards.secrets.push(SecretObjective::ALL[1]);
    p.cards.secrets.push(SecretObjective::ALL[2]);
    assert_ne!(enc(&t, &mine, &choice), before, "the actor's own hand is a feature");
}

#[test]
fn the_activation_filter_on_a_fixed_position() {
    let (t, core, choice) = first_of(13, TreeKind::Activate, 1);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let kept = filter::activate(&seen, &choice).expect("an activation");
    assert_eq!(kept.keep.len(), choice.opts.len());
    assert!(!kept.passthrough);
    assert!(kept.kept() >= 1 && kept.kept() < choice.opts.len(), "{} of {}", kept.kept(), choice.opts.len());
    for (i, o) in choice.opts.iter().enumerate() {
        let system = filter::system_of(o).expect("a system option");
        assert_eq!(kept.keep[i], kept.why[i] != 0);
        assert_eq!(kept.why[i], filter::reasons(&seen, choice.actor, system));
        // A system with one of my docks is always a production system.
        let mine_dock = seen.system(system).planets().as_slice().iter().any(|p| seen.system(system).planet(*p).units.iter().any(|u| u.owner() == choice.actor && u.kind().data().production.is_some()));
        if mine_dock {
            assert!(kept.why[i] & filter::PRODUCE != 0);
        }
    }
    // The encoding's mask is the filter.
    let e = encode::encode(&seen, &choice);
    assert_eq!(e.mask, kept.keep);
    // Deterministic.
    assert_eq!(encode::encode(&seen, &choice), e);
    assert_eq!(filter::activate(&Seen::new(&core, t.kernel, choice.actor), &choice), Some(kept));
}

#[test]
fn options_point_at_their_systems_and_objectives() {
    let (t, core, choice) = first_of(17, TreeKind::Move, 1);
    let e = enc(&t, &core, &choice);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let f = encode::Frame::new(&seen, choice.actor);
    for (i, o) in choice.opts.iter().enumerate() {
        if let ti4_engine_core::choice::Opt::Tactical(ti4_engine_core::choice::tactical::Opt::Move { origin, kind, .. }) = o {
            assert_eq!(e.opt_ptr[i * 3], f.slot(*origin).map_or(-1, |x| x as i16));
            assert_eq!(e.opt_ptr[i * 3 + 2], kind.index() as i16);
        }
    }
    let (t, core, choice) = first_of(19, TreeKind::Score, 1);
    let e = enc(&t, &core, &choice);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let objs = encode::objective_slots(&seen);
    for (i, o) in choice.opts.iter().enumerate() {
        if let ti4_engine_core::choice::Opt::Objectives(ti4_engine_core::choice::objectives::Opt::Score(r)) = o {
            let slot = e.opt_ptr[i * 3 + 1];
            assert!(slot >= 0, "a scored objective has a slot");
            assert_eq!(objs[slot as usize], *r);
        }
    }
}

#[test]
fn a_shard_round_trips_its_shapes_and_rotates_the_outcome() {
    let (t, core, choice) = first_of(23, TreeKind::Draft, 1);
    let seen = Seen::new(&core, t.kernel, choice.actor);
    let e = encode::encode(&seen, &choice);
    let rel = encode::relative_seats(&seen, choice.actor);
    let k = e.k();
    let mut w = ShardWriter::new();
    w.names = Some(encode::column_names(&seen, &choice));
    let mut rel4 = [0u8; SEATS];
    for (i, s) in rel.iter().take(SEATS).enumerate() {
        rel4[i] = s.0;
    }
    let mut pi = vec![0.0; k];
    pi[0] = 1.0;
    for _ in 0..3 {
        w.push(Record { enc: e.clone(), pi: pi.clone(), meta: Meta { actor: choice.actor.0, rel: rel4, ..Meta::default() } });
    }
    // Seat 2 wins with 10 VP.
    let mut z = vec![0.0; SEATS];
    z[2] = 1.0;
    let vp = vec![3.0, 4.0, 10.0, 5.0];
    w.end_game(&z, &vp);
    let dir = std::env::temp_dir().join(format!("mcts-shard-test-{}", std::process::id()));
    let n = w.write(&dir).expect("written");
    assert_eq!(n, 3);
    let read = |name: &str| std::fs::read(dir.join(name)).expect("file");
    let header_len = |bytes: &[u8]| 10 + usize::from(u16::from_le_bytes([bytes[8], bytes[9]]));
    let z_file = read("z.npy");
    let h = header_len(&z_file);
    let zs: Vec<f32> = z_file[h..].chunks(2).map(|c| npy::f16_value(u16::from_le_bytes([c[0], c[1]]))).collect();
    assert_eq!(zs.len(), 3 * SEATS);
    let winner_block = rel.iter().position(|s| s.0 == 2).expect("seat 2 plays");
    for rec in 0..3 {
        for r in 0..SEATS {
            assert_eq!(zs[rec * SEATS + r], if r == winner_block { 1.0 } else { 0.0 });
        }
    }
    let off = read("opt_off.npy");
    let h = header_len(&off);
    let offs: Vec<u32> = off[h..].chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    assert_eq!(offs, vec![0, k as u32, 2 * k as u32, 3 * k as u32]);
    let sys = read("sys.npy");
    assert_eq!(sys.len() - header_len(&sys), 3 * MAX_SYS * FS * 2);
    let schema = String::from_utf8(read("schema.json")).expect("utf8");
    assert!(schema.contains("\"feature_version\": 1") && schema.contains("\"p0_space_Carrier\""));
    let _ = std::fs::remove_dir_all(&dir);
}
