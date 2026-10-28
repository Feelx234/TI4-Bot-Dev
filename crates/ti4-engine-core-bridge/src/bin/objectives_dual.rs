//! The dual run of the objectives area (package A4, docs/ENGINE_CORE_STATE.md 7): every
//! `(position, seat, objective)` of the golden positions, plain and with imagined variants, asked of
//! the string engine and the typed core with every answer compared.
//!
//! ```text
//! objectives_dual CONFIG GAMES STRIDE [FUZZ]     golden positions (every STRIDE-th step of GAMES games)
//! objectives_dual templates [FUZZ]               the opening of every map template
//! ```
//! `FUZZ` (default 2) is the number of randomly edited copies of each position (planets taken,
//! units, technologies, attachments, coexistence, goods and tokens), so the corners the golden
//! games rarely reach (stations, attachments, coexisting, rich seats) are asked too.
//!
//! Per position, seat and variant: the public objectives (all 40, revealed or not) through
//! `counting_progress`, `remaining_position_progress` and `bought_progress_at`; the secrets
//! through `secrets::counting_progress` / `remaining_position_progress`; and the aggregate
//! `Observed::revealed_objective_progress_imagining` / `held_secret_progress_imagining` with every
//! objective revealed or held. Exit status 1 on any difference.

use ti4_content::ContentStore;
use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, Observed, SeatObservation, ask_private};
use ti4_engine::dual::{self, Area, AreaMask, Backend};
use ti4_engine::generated::IdStr;
use ti4_engine::objectives::Imagined;
use ti4_engine::objectives_core::{ObjView, scoped};
use ti4_engine_core::generated::{Attachment, Planet, PublicObjective, SecretObjective, System, Technology};
use ti4_engine_core_bridge::workload::{Position, golden_positions, template_openings};
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, SystemId, TechnologyId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// A small deterministic generator (xorshift).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const SHIPS: [&str; 6] = ["carrier", "cruiser", "destroyer", "dreadnought", "fighter", "warsun"];
const GROUND: [&str; 4] = ["infantry", "mech", "pds", "spacedock"];

/// Edit a copy of a position at random, through the public state.
fn fuzz(state: &mut ti4_model::state::GameState, galaxy: &ti4_content::galaxy::Galaxy, rng: &mut Rng) {
    let players: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
    let on_map: Vec<String> = galaxy.system_ids().into_iter().map(str::to_owned).collect();
    for player in &players {
        for _ in 0..rng.below(6) {
            // Take a planet printed on a tile of the map.
            let planet = Planet::ALL[rng.below(Planet::COUNT)];
            let Some(sys) = planet.data().system else { continue };
            let sid = SystemId::new(sys.as_str());
            if !on_map.iter().any(|s| s == sid.as_str()) {
                continue;
            }
            state.system_mut(&sid).set_control(PlanetId::new(planet.as_str()), player.clone());
        }
        for _ in 0..rng.below(5) {
            if on_map.is_empty() {
                break;
            }
            let sid = SystemId::new(on_map[rng.below(on_map.len())].as_str());
            let ship = SHIPS[rng.below(SHIPS.len())];
            state.system_mut(&sid).units.push(Unit::new(UnitTypeId::new(ship), player.clone()));
        }
        for _ in 0..rng.below(5) {
            let planet = Planet::ALL[rng.below(Planet::COUNT)];
            let Some(sys) = planet.data().system else { continue };
            let sid = SystemId::new(sys.as_str());
            if !on_map.iter().any(|s| s == sid.as_str()) {
                continue;
            }
            let kind = GROUND[rng.below(GROUND.len())];
            let unit = Unit::new(UnitTypeId::new(kind), player.clone());
            state.system_mut(&sid).planet_units.entry(PlanetId::new(planet.as_str())).or_default().push(unit);
        }
        let techs = rng.below(7);
        for _ in 0..techs {
            let t = Technology::ALL[rng.below(Technology::COUNT)];
            if let Some(p) = state.player_mut(player) {
                p.technologies.insert(TechnologyId::new(t.as_str()));
            }
        }
        if let Some(p) = state.player_mut(player) {
            p.trade_goods = i32::try_from(rng.below(14)).unwrap_or(0);
            p.commodities = i32::try_from(rng.below(4)).unwrap_or(0);
            p.tactic_tokens = i32::try_from(rng.below(7)).unwrap_or(0);
            p.strategic_tokens = i32::try_from(rng.below(5)).unwrap_or(0);
        }
    }
    for _ in 0..rng.below(8) {
        let planet = Planet::ALL[rng.below(Planet::COUNT)];
        let a = Attachment::ALL[rng.below(Attachment::COUNT)];
        state.planet_attachments.entry(PlanetId::new(planet.as_str())).or_default().push(a.as_str().to_owned());
    }
    for _ in 0..rng.below(5) {
        let planet = Planet::ALL[rng.below(Planet::COUNT)];
        state.exhausted_planets.insert(PlanetId::new(planet.as_str()));
    }
    // Coexistence: a seat shares a planet someone else holds.
    for _ in 0..rng.below(3) {
        let planet = Planet::ALL[rng.below(Planet::COUNT)];
        let Some(sys) = planet.data().system else { continue };
        let sid = SystemId::new(sys.as_str());
        if !on_map.iter().any(|s| s == sid.as_str()) || players.len() < 2 {
            continue;
        }
        let who = players[rng.below(players.len())].clone();
        let pid = PlanetId::new(planet.as_str());
        let st = state.system_mut(&sid);
        if st.planet_control.get(&pid).is_some_and(|c| *c != who) {
            st.coexisting.entry(pid).or_default().insert(who);
        }
    }
    // Secrets in hand.
    for player in &players {
        let n = rng.below(4);
        if let Some(p) = state.player_mut(player) {
            p.secret_objectives = (0..n)
                .map(|_| SecretObjectiveId::new(SecretObjective::ALL[rng.below(SecretObjective::COUNT)].as_str()))
                .collect();
        }
    }
}

/// The imagined variants of a position, for one seat: none, then random mixes.
struct Variant {
    planets: Vec<PlanetId>,
    systems: Vec<String>,
    technologies: Vec<String>,
    structures: Vec<(String, String)>,
}

fn variants(rng: &mut Rng, count: usize) -> Vec<Variant> {
    let mut out = vec![Variant { planets: vec![], systems: vec![], technologies: vec![], structures: vec![] }];
    for i in 0..count {
        let mut v = Variant { planets: vec![], systems: vec![], technologies: vec![], structures: vec![] };
        let kind = i % 5;
        for _ in 0..=rng.below(3) {
            if kind == 0 || kind == 4 {
                v.planets.push(PlanetId::new(Planet::ALL[rng.below(Planet::COUNT)].as_str()));
            }
            if kind == 1 || kind == 4 {
                v.systems.push(System::ALL[rng.below(System::COUNT)].as_str().to_owned());
            }
            if kind == 2 || kind == 4 {
                v.technologies.push(Technology::ALL[rng.below(Technology::COUNT)].as_str().to_owned());
            }
            if kind == 3 || kind == 4 {
                let p = Planet::ALL[rng.below(Planet::COUNT)];
                if let Some(s) = p.data().system {
                    v.structures.push((s.as_str().to_owned(), p.as_str().to_owned()));
                }
            }
        }
        out.push(v);
    }
    out
}

struct Run {
    asked: u64,
    differences: u64,
    printed: usize,
}

impl Run {
    fn check(&mut self, context: impl FnOnce() -> String) {
        self.asked += 1;
        let report = dual::report();
        let now = report.total_differences();
        if now > self.differences {
            if self.printed < 40 {
                println!("DIFFERENCE at {}", context());
                for d in report.differences.iter().skip(usize::try_from(self.differences).unwrap_or(0)).take(3) {
                    println!("  {d}");
                }
                self.printed += 1;
            }
            self.differences = now;
        }
    }
}

fn run_position(p: &Position, label: &str, rng: &mut Rng, nvariants: usize, run: &mut Run) {
    let content = ContentStore::embedded();
    let all_public: Vec<ObjectiveId> = PublicObjective::ALL.iter().map(|o| ObjectiveId::new(o.as_str())).collect();
    let all_secret: Vec<SecretObjectiveId> =
        SecretObjective::ALL.iter().map(|o| SecretObjectiveId::new(o.as_str())).collect();
    let view = ObjView::build(&p.state, content, p.sources, Some(&p.galaxy));
    let Some(view) = view else {
        println!("no view for {label}");
        return;
    };
    let players: Vec<PlayerId> = p.state.players.iter().map(|x| x.id.clone()).collect();
    for player in &players {
        for (vi, v) in variants(rng, nvariants).iter().enumerate() {
            let imagined = Imagined {
                planets: &v.planets,
                systems: &v.systems,
                technologies: &v.technologies,
                structures: &v.structures,
            };
            for with_map in [true, false] {
                scoped(Some(&view), || {
                    let mut position = ti4_engine::objectives::Position::imagining_all(
                        &p.state, content, p.sources, player, &imagined,
                    );
                    if with_map {
                        position = position.with_galaxy(&p.galaxy);
                    }
                    for alias in &all_public {
                        let _ = ti4_engine::objectives::counting_progress(alias, &position);
                        let _ = ti4_engine::objectives::remaining_position_progress(alias, &position);
                        let _ = ti4_engine::objectives::bought_progress_at(&position, alias);
                        run.check(|| format!("{label} {player} variant {vi} map {with_map} {alias}"));
                    }
                    secrets_round(p, &position, &all_secret, &imagined, player, with_map, label, vi, run);
                });
            }
            // The aggregate, with every objective revealed and every secret held.
            let mut state = p.state.clone();
            state.revealed_objectives = all_public.clone();
            if let Some(seat) = state.player_mut(player) {
                seat.secret_objectives = all_secret.clone();
            }
            let seen = Observed::new(&state, content, p.sources, Some(&p.galaxy));
            let _ = seen.revealed_objective_progress_imagining(player, &imagined);
            run.check(|| format!("{label} {player} variant {vi} revealed_objective_progress_imagining"));
            let _ = seen.revealed_objective_progress_imagining(player, &Imagined::NONE);
            run.check(|| format!("{label} {player} revealed_objective_progress"));
            aggregate_secrets(p, &state, player, &imagined, label, vi, run);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn secrets_round(
    p: &Position,
    position: &ti4_engine::objectives::Position<'_>,
    all_secret: &[SecretObjectiveId],
    imagined: &Imagined<'_>,
    player: &PlayerId,
    with_map: bool,
    label: &str,
    vi: usize,
    run: &mut Run,
) {
    let _ = position;
    let content = ContentStore::embedded();
    let sp = ti4_engine::secrets::Position {
        state: &p.state,
        content,
        sources: p.sources,
        player,
        galaxy: with_map.then_some(&p.galaxy),
        imagined: *imagined,
    };
    for alias in all_secret {
        let _ = ti4_engine::secrets::counting_progress(alias, &sp);
        let _ = ti4_engine::secrets::remaining_position_progress(alias, &sp);
        run.check(|| format!("{label} {player} variant {vi} map {with_map} secret {alias}"));
    }
}

/// Asks the held-secret questions through a private binding, as a bot does.
struct Probe<'i, 'j> {
    imagined: &'i Imagined<'j>,
}

impl Decider for Probe<'_, '_> {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        Ok(choice.options[0].clone())
    }
    fn choose_seeing(&mut self, choice: &Choice, seen: &SeatObservation<'_>) -> Result<ChoiceOption, IllegalChoice> {
        let _ = seen.held_secret_progress();
        let _ = seen.held_secret_progress_imagining(self.imagined);
        Ok(choice.options[0].clone())
    }
}

fn aggregate_secrets(p: &Position, state: &GameState, player: &PlayerId, imagined: &Imagined<'_>, label: &str, vi: usize, run: &mut Run) {
    let choice = Choice::new(player.clone(), "probe", vec![ChoiceOption::labelled("x", "x", "x")]);
    let mut probe = Probe { imagined };
    let _ = ask_private(&choice, state, ContentStore::embedded(), p.sources, Some(&p.galaxy), &mut probe);
    run.check(|| format!("{label} {player} variant {vi} held_secret_progress_imagining"));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (positions, fuzz_copies, nvariants) = match args.first().map(String::as_str) {
        Some("templates") => {
            let (positions, _) = template_openings();
            (positions, args.get(1).and_then(|f| f.parse().ok()).unwrap_or(2usize), 9)
        }
        Some(config) => {
            let games: usize = args.get(1).and_then(|g| g.parse().ok()).unwrap_or(2);
            let stride: usize = args.get(2).and_then(|g| g.parse().ok()).unwrap_or(40);
            let (positions, _) = golden_positions(config, games, stride).expect("games");
            (positions, args.get(3).and_then(|f| f.parse().ok()).unwrap_or(2usize), 9)
        }
        None => {
            eprintln!("usage: objectives_dual CONFIG GAMES STRIDE [FUZZ] | templates [FUZZ]");
            std::process::exit(2);
        }
    };
    let _ = dual::take_report();
    dual::set_fail_fast(false);
    dual::set_default_mask(AreaMask::STRING.with(Area::Objectives, Backend::Dual));
    let mut run = Run { asked: 0, differences: 0, printed: 0 };
    for (i, p) in positions.iter().enumerate() {
        run_position(p, &format!("#{i} {}", p.label), &mut rng, nvariants, &mut run);
        for f in 0..fuzz_copies {
            let mut copy = p.clone();
            fuzz(&mut copy.state, &copy.galaxy, &mut rng);
            run_position(&copy, &format!("#{i}f{f} {}", p.label), &mut rng, nvariants, &mut run);
        }
    }
    let report = dual::take_report();
    println!("positions {} | questions {} | fuzz copies {} per position", positions.len(), run.asked, fuzz_copies);
    print!("{report}");
    std::process::exit(i32::from(report.total_differences() > 0));
}
