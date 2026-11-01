//! The dual run of the trade area (package B4, docs/ENGINE_CORE_STATE.md 7): every pair of seats of
//! the golden positions, plain and edited at random, asked of the string engine and the typed core
//! with every answer compared; settled deals and the per-turn records are compared on the whole
//! state.
//!
//! ```text
//! trade_dual CONFIG GAMES STRIDE [FUZZ]     golden positions (every STRIDE-th step of GAMES games)
//! trade_dual templates [FUZZ]               the opening of every map template
//! ```
//! `FUZZ` (default 3) is the number of randomly edited copies of each position (units and planets
//! moved, goods, fragments, cards, secrets and notes dealt, limits used, the agenda phase, Black
//! Market, I.I.H.Q., a Pride of Kenara), so the corners the golden games rarely reach are asked too.
//!
//! Per position, per ordered pair and random deal: `are_neighbours`, `neighbours`,
//! `neighbours_who_transacted`, `partners`, `may_transact`, `may_open_again`, `available_actions`,
//! `can_pay`, `why_illegal` (through the typed view), `presence`, and `TradeWindow::answer_with` (whole negotiations), `resolve` / `record_transaction`
//! / `note_negotiation` on copies under `Dual`. Exit status 1 on any difference.

use ti4_content::ContentStore;
use ti4_engine_legacy::dual::{self, Area, AreaMask, Backend};
use ti4_engine::generated::IdStr;
use ti4_engine_legacy::trade_core::TradeView;
use ti4_engine_legacy::transactions::{self, Offer, PlanetTerm, Terms};
use ti4_engine_core::generated::{ActionCard, Planet, SecretObjective};
use ti4_engine_core_bridge::workload::{Position, golden_positions, template_openings};
use ti4_model::id::{ActionCardId, PlanetId, PlayerId, SecretObjectiveId, SystemId, TechnologyId, UnitTypeId};
use ti4_model::state::{GameState, Phase, TransientFlags};
use ti4_model::units::Unit;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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

const SHIPS: [&str; 5] = ["carrier", "cruiser", "destroyer", "dreadnought", "fighter"];
const KINDS: [&str; 5] = ["CULTURAL", "HAZARDOUS", "INDUSTRIAL", "FRONTIER", "unknown"];

/// Edit a copy of a position at random, through the public state.
fn fuzz(state: &mut GameState, galaxy: &ti4_content::galaxy::Galaxy, rng: &mut Rng) {
    let players: Vec<PlayerId> = state.players.iter().map(|p| p.id.clone()).collect();
    let on_map: Vec<String> = galaxy.system_ids().into_iter().map(str::to_owned).collect();
    for player in &players {
        for _ in 0..rng.below(4) {
            let planet = Planet::ALL[rng.below(Planet::COUNT)];
            let Some(sys) = planet.data().system else { continue };
            let sid = SystemId::new(sys.as_str());
            if on_map.iter().any(|s| s == sid.as_str()) {
                state.system_mut(&sid).set_control(PlanetId::new(planet.as_str()), player.clone());
            }
        }
        for _ in 0..rng.below(4) {
            if on_map.is_empty() {
                break;
            }
            let sid = SystemId::new(on_map[rng.below(on_map.len())].as_str());
            let ship = SHIPS[rng.below(SHIPS.len())];
            state.system_mut(&sid).units.push(Unit::new(UnitTypeId::new(ship), player.clone()));
        }
        for _ in 0..rng.below(3) {
            let planet = Planet::ALL[rng.below(Planet::COUNT)];
            let Some(sys) = planet.data().system else { continue };
            let sid = SystemId::new(sys.as_str());
            if on_map.iter().any(|s| s == sid.as_str()) {
                let unit = Unit::new(UnitTypeId::new(if rng.below(4) == 0 { "hacan_mech" } else { "infantry" }), player.clone());
                state.system_mut(&sid).planet_units.entry(PlanetId::new(planet.as_str())).or_default().push(unit);
            }
        }
        if rng.below(8) == 0 {
            if let Some(p) = state.player_mut(player) {
                p.technologies.insert(TechnologyId::new("iihq"));
            }
        }
        let secrets: Vec<SecretObjectiveId> =
            (0..rng.below(3)).map(|_| SecretObjectiveId::new(SecretObjective::ALL[rng.below(SecretObjective::COUNT)].as_str())).collect();
        let cards: Vec<ActionCardId> = (0..rng.below(4)).map(|_| ActionCardId::new(ActionCard::ALL[rng.below(ActionCard::COUNT)].as_str())).collect();
        let frags: Vec<(String, i32)> =
            (0..rng.below(3)).map(|_| (KINDS[rng.below(4)].to_owned(), i32::try_from(rng.below(3)).unwrap_or(0))).collect();
        if let Some(p) = state.player_mut(player) {
            p.trade_goods = i32::try_from(rng.below(9)).unwrap_or(0);
            p.commodities = i32::try_from(rng.below(5)).unwrap_or(0);
            if rng.below(2) == 0 {
                p.secret_objectives = secrets;
            }
            if rng.below(2) == 0 {
                p.action_cards = cards;
            }
            for (k, n) in frags {
                p.relic_fragments.insert(k, n);
            }
        }
    }
    // Limits used and deals made.
    for _ in 0..rng.below(4) {
        if players.len() < 2 {
            break;
        }
        let a = players[rng.below(players.len())].clone();
        let b = players[rng.below(players.len())].clone();
        if a != b {
            state.record_transaction(&a, &b);
        }
    }
    for _ in 0..rng.below(8) {
        let a = players[rng.below(players.len())].clone();
        state.note_negotiation(&a);
    }
    for _ in 0..rng.below(4) {
        let a = players[rng.below(players.len())].clone();
        let b = players[rng.below(players.len())].clone();
        state.transactions_this_round.push((a, b));
    }
    if rng.below(5) == 0 {
        state.phase = Phase::Agenda;
    }
    if rng.below(5) == 0 {
        state.transient_flags.set(TransientFlags::BLACK_MARKET);
    }
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
    fn same<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: &T, typed: Option<T>) {
        self.asked += 1;
        if let Some(t) = typed
            && t != *string
        {
            self.differences += 1;
            if self.printed < 40 {
                println!("DIFFERENCE at {what}: string {string:?} typed {t:?}");
                self.printed += 1;
            }
        }
    }
}

fn random_terms(state: &GameState, content: &ContentStore, who: &PlayerId, rng: &mut Rng) -> Terms {
    let mut t = Terms::default();
    if rng.below(2) == 0 {
        t.trade_goods = i32::try_from(rng.below(6)).unwrap_or(0);
    }
    if rng.below(2) == 0 {
        t.commodities = i32::try_from(rng.below(4)).unwrap_or(0);
    }
    for _ in 0..rng.below(3) {
        t.fragments.push(KINDS[rng.below(KINDS.len())].to_owned());
    }
    if rng.below(5) == 0 {
        let notes = ti4_engine_legacy::promissory::available_notes(state, content, who);
        if !notes.is_empty() {
            t.promissory = Some(notes[rng.below(notes.len())].clone());
        } else if rng.below(3) == 0 {
            t.promissory = Some(ti4_engine_legacy::promissory::support(&ti4_engine_legacy::promissory::faction_name(state, who)));
        }
    }
    if rng.below(6) == 0 {
        if let Some(p) = state.player(who) {
            if !p.action_cards.is_empty() {
                t.action_card = Some(p.action_cards[rng.below(p.action_cards.len())].clone());
            } else if rng.below(2) == 0 {
                t.action_card = Some(ActionCardId::new(ActionCard::ALL[rng.below(ActionCard::COUNT)].as_str()));
            }
        }
    }
    if rng.below(6) == 0 {
        if let Some(p) = state.player(who) {
            if !p.secret_objectives.is_empty() {
                t.secret = Some(p.secret_objectives[rng.below(p.secret_objectives.len())].clone());
            } else if rng.below(2) == 0 {
                t.secret = Some(SecretObjectiveId::new(SecretObjective::ALL[rng.below(SecretObjective::COUNT)].as_str()));
            }
        }
    }
    if rng.below(6) == 0 {
        let planets = transactions::pride_of_kenara_planets(state, who);
        if let Some((planet, moves)) = planets.first() {
            t.planet = Some(PlanetTerm { planet: planet.clone(), moves_to: moves[rng.below(moves.len())].clone() });
        }
    }
    t
}

fn run_position(p: &Position, label: &str, rng: &mut Rng, deals: usize, run: &mut Run) {
    let content = ContentStore::embedded();
    let state = &p.state;
    let galaxy = &p.galaxy;
    let Some(view) = TradeView::build(state, content, galaxy) else {
        println!("no view for {label}");
        return;
    };
    let players: Vec<PlayerId> = state.players.iter().map(|x| x.id.clone()).collect();
    for a in &players {
        run.same(&format!("{label} presence {a}"), &transactions::presence(state, a), view.presence(a));
        let _ = transactions::neighbours(state, galaxy, a);
        let _ = transactions::neighbours_who_transacted(state, galaxy, a);
        let _ = transactions::partners(state, content, galaxy, a);
        let _ = transactions::available_actions(state, content, galaxy, a);
        run.check(|| format!("{label} {a} lists"));
        for b in &players {
            let _ = transactions::are_neighbours(state, galaxy, a, b);
            let _ = transactions::may_transact(state, content, galaxy, a, b);
            let _ = transactions::may_open_again(state, content, a, b);
            run.check(|| format!("{label} {a}/{b} pair"));
        }
    }
    for _ in 0..deals {
        if players.len() < 2 {
            break;
        }
        let a = players[rng.below(players.len())].clone();
        let b = players[rng.below(players.len())].clone();
        let offer = Offer {
            proposer: a.clone(),
            partner: b.clone(),
            given: random_terms(state, content, &a, rng),
            received: random_terms(state, content, &b, rng),
        };
        let _ = transactions::can_pay(state, content, &a, &offer.given);
        run.same(
            &format!("{label} can_pay {a}"),
            &transactions::can_pay(state, content, &a, &offer.given),
            view.can_pay(state, content, &a, &offer.given),
        );
        let _ = transactions::why_illegal(state, content, galaxy, &offer);
        run.check(|| format!("{label} why_illegal {a}/{b} {offer:?}"));
        // The whole negotiation, one answer at a time through `answer_with` (the window's lockstep dual: the
        // core window follows the string window question by question), with random picks, so refusals,
        // counters and the haggling bound are reached too.
        let mut open = state.clone();
        let mut window = transactions::TradeWindow::open_with_content(&mut open, content, &a, &b);
        for _ in 0..8 {
            let Some(q) = window.pending_choice(&open, content) else { break };
            let pick = q.options[rng.below(q.options.len())].clone();
            // The answer goes through a table, which is what records the question for the core window.
            let mut table = ti4_engine::choice::Table::with_default(Box::new(ti4_engine::choice::Scripted::new([pick.id.clone()])));
            let sources = p.sources;
            let _ = window.answer_with(&mut open, content, galaxy, &q, |c, s| {
                table.ask_seeing(c, &ti4_engine::choice::Observed::new(s, content, sources, Some(galaxy)))
            });
            if window.is_complete() {
                break;
            }
        }
        run.check(|| format!("{label} window {a}/{b}"));
        // Settle on a copy, under the dual run.
        let mut copy = state.clone();
        let _ = transactions::resolve(&mut copy, content, galaxy, &offer);
        transactions::record_transaction(&mut copy, &a, &b);
        transactions::note_negotiation(&mut copy, &a);
        run.check(|| format!("{label} resolve {a}/{b} {offer:?}"));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (positions, fuzz_copies) = match args.first().map(String::as_str) {
        Some("templates") => {
            let (positions, _) = template_openings();
            (positions, args.get(1).and_then(|f| f.parse().ok()).unwrap_or(3usize))
        }
        Some(config) => {
            let games: usize = args.get(1).and_then(|g| g.parse().ok()).unwrap_or(2);
            let stride: usize = args.get(2).and_then(|g| g.parse().ok()).unwrap_or(40);
            let (positions, _) = golden_positions(config, games, stride).expect("games");
            (positions, args.get(3).and_then(|f| f.parse().ok()).unwrap_or(3usize))
        }
        None => {
            eprintln!("usage: trade_dual CONFIG GAMES STRIDE [FUZZ] | templates [FUZZ]");
            std::process::exit(2);
        }
    };
    let _ = dual::take_report();
    dual::set_fail_fast(false);
    dual::set_default_mask(AreaMask::STRING.with(Area::TradeDiplomacy, Backend::Dual));
    let mut run = Run { asked: 0, differences: 0, printed: 0 };
    for (i, p) in positions.iter().enumerate() {
        run_position(p, &format!("#{i} {}", p.label), &mut rng, 6, &mut run);
        for f in 0..fuzz_copies {
            let mut copy = p.clone();
            fuzz(&mut copy.state, &copy.galaxy, &mut rng);
            run_position(&copy, &format!("#{i}f{f} {}", p.label), &mut rng, 12, &mut run);
        }
    }
    let report = dual::take_report();
    println!("positions {} | questions {} | fuzz copies {} per position | direct differences {}", positions.len(), run.asked, fuzz_copies, run.differences);
    println!(
        "typed settles {} | settles left to the string engine (note or planet card) {}",
        ti4_engine_legacy::trade_core::TYPED_WRITES.load(std::sync::atomic::Ordering::Relaxed),
        ti4_engine_legacy::trade_core::STRING_WRITES.load(std::sync::atomic::Ordering::Relaxed)
    );
    print!("{report}");
    std::process::exit(i32::from(report.total_differences() > 0 || run.differences > 0));
}
