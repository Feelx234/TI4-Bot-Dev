//! Lane D (KV): the held vote of the agenda phase (`driver::agenda::VotingWin`) and the close it composes. The core half of the lockstep
//! against the string (`ti4-engine`'s `agenda_live_tests`): the window is rebuilt from `(payload, position)` at every call, a clone of the
//! position in the middle of a vote continues identically, and the last answer closes the vote.

use std::rc::Rc;

use ti4_engine_core::choice::agenda::Opt as AOpt;
use ti4_engine_core::containers::{Cow, SmallList};
use ti4_engine_core::driver::agenda::VotingWin;
use ti4_engine_core::driver::live::{advance, ask, open, Env, Live, LiveError};
use ti4_engine_core::driver::NoSeeds;
use ti4_engine_core::kernel::stage::AbilityTable;
use ti4_engine_core::kernel::Registry;
use ti4_engine_core::rules::agenda::window::{Facts, Vote};
use ti4_engine_core::state::{Outcome, Window as OpenWindow, WindowKind};
use ti4_engine_core_tests::inv::ALL_SOURCES;
use ti4_engine_core_tests::prelude::*;
use ti4_engine_core_tests::scenario::{plain_systems, system_of};

const FACTIONS: [Faction; 3] = [Faction::TheArborec, Faction::TheEmbersOfMuaat, Faction::TheYssarilTribes];

fn table() -> CoreState {
    let find = |i: u8| {
        plain_systems()
            .iter()
            .flat_map(|s| s.data().planets.iter().copied())
            .find(|p| p.data().influence == i && p.data().source == Source::Base && !p.data().legendary)
            .unwrap_or_else(|| panic!("a plain planet with {i} influence"))
    };
    let [a, b, c] = [find(1), find(2), find(3)];
    let mut systems = vec![system_of(a)];
    for p in [b, c] {
        if !systems.contains(&system_of(p)) {
            systems.push(system_of(p));
        }
    }
    Scenario::new(&FACTIONS).map_from(&systems).control(a, 0).control(b, 1).control(c, 2).state()
}

fn env(k: &Kernel) -> Env<'_> {
    Env { kernel: k, registry: Cow::new(Registry::default()), table: AbilityTable::default(), seeds: Rc::new(NoSeeds) }
}

fn opened(s: &mut CoreState, k: &Kernel, agenda: Agenda, queue: &[Agenda]) {
    let mut vote = Vote::new(agenda, &[Outcome::For, Outcome::Against], &[Seat(0), Seat(1), Seat(2)], Facts::of_core(s, k));
    vote.settle(s, k);
    open(s, &VotingWin::new(vote, SmallList::from_slice(queue)));
}

fn back(o: Outcome) -> Pick {
    Pick::Opt(Opt::Agenda(AOpt::Back(o, Default::default())))
}

#[test]
fn the_held_vote_is_rebuilt_from_its_payload_and_the_position() {
    covers!("driver::agenda::VotingWin");
    let k = Kernel::plain();
    let mut s = table();
    opened(&mut s, &k, Agenda::ChecksAndBalances, &[]);
    let Some(OpenWindow::Voting(v)) = s.pending().window(WindowKind::Voting) else { panic!("the vote is on the stack") };
    let payload = (**v).clone();
    let restored = <VotingWin as ti4_engine_core::kernel::stage::Held>::from_pending(&s, &k).expect("the window rebuilds");
    assert_eq!(restored.vote.to_payload(), payload.vote, "restore after to_payload is the identity");
    let env = env(&k);
    let asked = ask::<VotingWin>(&s, &env).unwrap().expect("the first voter is asked");
    assert_eq!(asked.choice.actor, Seat(0));
    assert!(asked.nested.is_none() && s.pending().nested.is_none());
}

#[test]
fn a_clone_in_the_middle_of_a_vote_continues_to_the_same_position_and_the_last_answer_closes_the_vote() {
    covers!("driver::agenda::VotingWin");
    let k = Kernel::plain();
    let env = env(&k);
    let mut s = table();
    opened(&mut s, &k, Agenda::ChecksAndBalances, &[Agenda::RegulatedConscription]);
    // Seat 0 backs "for" and exhausts its planet; the position is cloned while seat 1 is asked.
    let (live, _) = advance::<VotingWin>(&mut s, &env, Some(back(Outcome::For))).unwrap();
    let Live::Asks(asked) = live else { panic!("the planets stage asks") };
    let planet = asked.choice.opts.as_slice().iter().find_map(|o| if let Opt::Agenda(AOpt::Exhaust { .. }) = o { Some(*o) } else { None }).expect("a planet to exhaust");
    advance::<VotingWin>(&mut s, &env, Some(Pick::Opt(planet))).unwrap();
    let mut other = s.clone();
    let answers = [Pick::Opt(Opt::Decline), Pick::Opt(Opt::Decline), Pick::Opt(Opt::Decline), Pick::Opt(Opt::Decline)];
    let run = |s: &mut CoreState| -> Result<Live, LiveError> {
        let mut last = None;
        for a in answers {
            if !ti4_engine_core::driver::live::is_open::<VotingWin>(s) {
                break;
            }
            last = Some(advance::<VotingWin>(s, &env, Some(a))?.0);
        }
        Ok(last.expect("the vote was answered"))
    };
    let (first, second) = (run(&mut s), run(&mut other));
    assert_eq!(first, second);
    assert_eq!(s, other, "a clone mid-window continues identically");
    // The vote closed: the law is in play, the scoring window of the agenda waits with the agenda behind it, and the vote left the stack.
    assert!(s.agenda().laws.get(&Agenda::ChecksAndBalances).is_some());
    assert!(s.pending().window(WindowKind::EventScoring).is_some());
    assert!(matches!(s.pending().window(WindowKind::AgendaQueueAfterEventScoring), Some(OpenWindow::AgendaQueueAfterEventScoring(q)) if q.as_slice() == [Agenda::RegulatedConscription]));
    assert!(s.pending().window(WindowKind::Voting).is_none());
    assert!(s.agenda().in_play.is_none() && s.agenda().choices.is_empty());
}

#[test]
fn an_illegal_answer_leaves_the_position_exactly_as_it_was() {
    covers!("driver::agenda::VotingWin");
    let k = Kernel::plain();
    let env = env(&k);
    let mut s = table();
    opened(&mut s, &k, Agenda::ChecksAndBalances, &[]);
    let held = s.clone();
    assert_eq!(advance::<VotingWin>(&mut s, &env, Some(Pick::Number(3))), Err(LiveError::Illegal));
    assert_eq!(s, held);
}

#[test]
fn a_silent_table_asks_the_speaker_and_the_answer_closes_the_vote() {
    covers!("driver::agenda::VotingWin");
    let k = Kernel::plain();
    let env = env(&k);
    let mut s = table();
    let mut vote = Vote::new(Agenda::ChecksAndBalances, &[Outcome::For, Outcome::Against], &[], Facts::of_core(&s, &k));
    vote.settle(&mut s, &k);
    open(&mut s, &VotingWin::new(vote, SmallList::new()));
    let (live, _) = advance::<VotingWin>(&mut s, &env, None).unwrap();
    let Live::Asks(asked) = live else { panic!("nobody voted: the speaker decides") };
    assert_eq!(asked.choice.actor, s.meta().speaker);
    let (live, _) = advance::<VotingWin>(&mut s, &env, Some(back(Outcome::Against))).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.pending().window(WindowKind::Voting).is_none());
    assert_eq!(s.agenda().log.last().map(|r| (r.outcome, r.speaker_decided)), Some((Some(Outcome::Against), true)));
}

// -- the job that opens the phase ------------------------------------------------------------------------------------------------------

use ti4_engine_core::driver::agenda::advance_agenda;
use ti4_engine_core::kernel::agenda_codes::AgendaGap;
use ti4_engine_core::kernel::ability::Unknown;
use ti4_engine_core::state::{DriverFlags, MetaFlags};

#[test]
fn the_agenda_job_is_a_counted_gap_outside_the_agenda_phase_and_before_the_custodians_are_gone() {
    covers!("driver::agenda::flow::run_open");
    let k = Kernel::plain();
    let env = env(&k);
    let gap = Err(LiveError::Unknown(Unknown::AgendaGap(AgendaGap::PhaseNotReady)));
    // The wrong phase, with the custodians gone.
    let mut s = table();
    s.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
    s.meta_mut().phase = Phase::Strategy;
    let held = s.clone();
    assert_eq!(advance_agenda(&mut s, &env, None), gap);
    assert_eq!(s, held, "a gap leaves the position as it was");
    // The agenda phase, the custodians still on Mecatol Rex.
    let mut s = table();
    s.meta_mut().phase = Phase::Agenda;
    assert_eq!(advance_agenda(&mut s, &env, None), gap);
    // Both in order, and nothing in the deck: the phase ends, the flag is set, the planets ready.
    let mut s = table();
    s.meta_mut().phase = Phase::Agenda;
    s.meta_mut().flags.set(MetaFlags::CUSTODIANS_REMOVED, true);
    s.decks_mut().agenda.set(Vec::new());
    let held = Planet::ALL.iter().copied().find(|p| s.board().planet(*p).control.is_some()).expect("a held planet");
    ti4_engine_core::kernel::fx::exhaust_planet(&mut s, held);
    let (live, _) = advance_agenda(&mut s, &env, None).unwrap();
    assert_eq!(live, Live::Done);
    assert!(s.pending().driver.has(DriverFlags::AGENDA_RESOLVED));
    assert!(s.board().planets().all(|(_, c)| !c.flags.has(ti4_engine_core::state::PlanetFlags::EXHAUSTED)), "8.4: the planets ready");
}

// -- the ground forces an agenda's effect destroyed ---------------------------------------------------------------------------------------

#[test]
fn the_staged_ground_forces_of_an_agenda_are_announced_with_their_cause_by_the_next_reconcile() {
    covers!("driver::agenda::ground");
    use ti4_engine_core::driver::turn::advance_reconcile;
    use ti4_engine_core::keys::DestroyReason;
    use ti4_engine_core::generated::Unit;
    use ti4_engine_core::state::{Code, Key, Piece, Val};
    let k = Kernel::plain();
    let mut env = env(&k);
    env.table.codes.agenda.ground_cause[Agenda::CoreMining as usize] = Code(7);
    let mut s = table();
    let planet = Planet::ALL.iter().copied().find(|p| s.board().planet(*p).control == Some(Seat(1))).expect("a planet of seat 1");
    let system = planet.data().system.expect("a placed planet");
    let piece = Piece::new(Unit::FactionlessMech, Seat(1)).with_sustained(true).with_galvanized(true);
    ti4_engine_core::kernel::fx_ground::stage_destroyed(&mut s, system, planet, piece, DestroyReason::Agenda(Agenda::CoreMining));
    let (_, out) = advance_reconcile(&mut s, &env, None).unwrap();
    let rec = out.emitted.iter().find(|e| e.kind == ti4_engine_core::state::EventKind::GroundForceDestroyed).expect("the destruction is announced");
    assert_eq!(rec.payload.get(Key::Cause), Some(Val::Code(Code(7))), "the cause is the agenda's text");
    assert_eq!(rec.payload.get(Key::Damaged), Some(Val::Flag(true)));
    assert_eq!(rec.payload.get(Key::Galvanized), Some(Val::Flag(true)));
    assert_eq!(rec.payload.get(Key::Player), Some(Val::Seat(Seat(1))));
    assert!(s.marks().tail.iter().all(|(k, _)| !matches!(k, ti4_engine_core::keys::MarkKey::PrivateStagedGround(_))), "the row is gone");
}

// -- Research Grant Reallocation -----------------------------------------------------------------------------------------------------------

#[test]
fn a_grant_offers_the_technologies_of_the_games_sources_and_pays_the_prerequisites_with_fleet_tokens() {
    covers!("driver::agenda::grant::Grant");
    use ti4_engine_core::choice::relic::Opt as ROpt;
    use ti4_engine_core::driver::agenda::grant::Grant;
    use ti4_engine_core::kernel::agenda_codes::AgendaCodes;
    use ti4_engine_core::tables::Scope;
    let base_only = Scope(ti4_engine_core::tables::source_bit(Source::Base));
    let mut rig = Rig::new(Scenario::lobby(2).tokens(0, 0, 4, 0), Grant::new(Seat(0), base_only, Some(Box::leak(Box::default())), AgendaCodes::default()));
    rig.run();
    let techs: Vec<Technology> = rig.opts().iter().filter_map(|o| if let Opt::Relic(ROpt::Tech(t)) = o { Some(*t) } else { None }).collect();
    assert!(!techs.is_empty() && techs.iter().all(|t| t.data().source == Source::Base), "only the sources in play: {techs:?}");
    let chosen = *techs.iter().find(|t| ti4_engine_core::query::tech::prerequisites(**t).iter().any(|n| *n > 0)).expect("a technology with a prerequisite");
    let cost: i16 = ti4_engine_core::query::tech::prerequisites(chosen).iter().map(|n| i16::try_from(*n).unwrap()).sum();
    rig.pick_opt(Opt::Relic(ROpt::Tech(chosen)));
    assert!(rig.is_done());
    let p = rig.state.players().by_seat(Seat(0)).unwrap();
    assert!(p.tech.owned.contains(chosen), "gained, not researched");
    assert_eq!(p.econ.fleet_tokens, (4 - cost).max(0), "one fleet token back to the reinforcements for each prerequisite");
}

#[test]
fn a_grant_without_the_unit_roster_gains_the_technology_but_declares_the_unit_upgrades_unsupported() {
    covers!("driver::agenda::grant::Grant");
    use ti4_engine_core::choice::relic::Opt as ROpt;
    use ti4_engine_core::driver::agenda::grant::Grant;
    use ti4_engine_core::kernel::agenda_codes::AgendaCodes;
    let mut rig = Rig::new(Scenario::lobby(2), Grant::new(Seat(0), ALL_SOURCES, None, AgendaCodes::default()));
    rig.run();
    let first = rig.opts().into_iter().find(|o| matches!(o, Opt::Relic(ROpt::Tech(_)))).expect("a technology to gain");
    rig.pick_opt(first);
    assert!(rig.is_done() && rig.driver.window.unsupported, "the caller declines the run: the upgrades of the units on the board need the roster");
}
