//! Lane B (8d item 1, docs/D28B2_RESUME.md): the objectives family answers as the legacy originals do: the `Observed` accessors
//! (scoreable_public/_secret, revealed_objective_progress*, objectives_implicating_system, held_secret_progress*, revealed_cards,
//! agenda_deck_ends), the `Position` functions of `objectives` and `secrets`, on the positions of `surface_common` with and without the map,
//! for every seat, with several imagined options. The result types do not share a type with the legacy ones: compared through `Debug`.

mod surface_common;

use surface_common::{Pos, content, positions};
use ti4_engine as thin;
use ti4_engine_legacy as old;
use ti4_model::id::{ObjectiveId, PlanetId, PlayerId, SecretObjectiveId, SystemId};

fn players(state: &ti4_model::state::GameState) -> Vec<PlayerId> {
    state.players.iter().map(|p| p.id.clone()).collect()
}

/// The imagined options of one seat: nothing, two planets other seats hold, systems, technologies, a structure, everything.
struct Options {
    planets: Vec<PlanetId>,
    systems: Vec<String>,
    technologies: Vec<String>,
    structures: Vec<(String, String)>,
}

fn options(pos: &Pos, player: &PlayerId) -> Options {
    let planets: Vec<PlanetId> = pos
        .state
        .board
        .values()
        .flat_map(|s| s.planet_control.iter())
        .filter(|(_, owner)| *owner != player)
        .map(|(p, _)| p.clone())
        .take(3)
        .collect();
    let systems: Vec<String> = pos.state.board.keys().map(|s| s.as_str().to_owned()).take(4).collect();
    let technologies: Vec<String> = old::technology::active_aliases(content(), pos.sources).into_iter().take(6).map(|t| t.as_str().to_owned()).collect();
    let structures: Vec<(String, String)> = pos
        .state
        .board
        .iter()
        .flat_map(|(id, s)| s.planet_control.keys().map(move |p| (id.as_str().to_owned(), p.as_str().to_owned())))
        .take(2)
        .collect();
    Options { planets, systems, technologies, structures }
}

impl Options {
    fn sets(&self) -> Vec<(&'static str, [usize; 4])> {
        // (label, how many of each list): planets, systems, technologies, structures
        vec![
            ("none", [0, 0, 0, 0]),
            ("planets", [3, 0, 0, 0]),
            ("systems", [0, 4, 0, 0]),
            ("technologies", [0, 0, 6, 0]),
            ("structures", [0, 0, 0, 2]),
            ("all", [3, 4, 6, 2]),
        ]
    }
}

macro_rules! both_imagined {
    ($opts:expr, $n:expr, |$o:ident, $t:ident| $body:expr) => {{
        let n = $n;
        let ($o, $t) = (
            old::objectives::Imagined { planets: &$opts.planets[..n[0].min($opts.planets.len())], systems: &$opts.systems[..n[1].min($opts.systems.len())], technologies: &$opts.technologies[..n[2].min($opts.technologies.len())], structures: &$opts.structures[..n[3].min($opts.structures.len())] },
            thin::queries::objectives::Imagined { planets: &$opts.planets[..n[0].min($opts.planets.len())], systems: &$opts.systems[..n[1].min($opts.systems.len())], technologies: &$opts.technologies[..n[2].min($opts.technologies.len())], structures: &$opts.structures[..n[3].min($opts.structures.len())] },
        );
        $body
    }};
}

fn dbg<T: std::fmt::Debug>(t: &T) -> String {
    format!("{t:?}")
}

#[test]
fn the_observed_objective_accessors_answer_as_the_legacy_view_does() {
    let content = content();
    let (mut rows, mut maps) = (0_usize, 0_usize);
    for pos in positions() {
        for galaxy in [None, Some(&pos.galaxy)] {
            let (o, t) = (old::choice::Observed::new(&pos.state, content, pos.sources, galaxy), thin::observed::Observed::new(&pos.state, content, pos.sources, galaxy));
            let systems: Vec<SystemId> = pos.galaxy.system_ids().into_iter().map(SystemId::new).collect();
            for player in players(&pos.state) {
                let at = |what: &str| format!("{} (map {}): {player}: {what}", pos.label, galaxy.is_some());
                assert_eq!(o.scoreable_public(&player), t.scoreable_public(&player), "{}", at("scoreable_public"));
                assert_eq!(o.scoreable_secret(&player), t.scoreable_secret(&player), "{}", at("scoreable_secret"));
                assert_eq!(dbg(&o.revealed_objective_progress(&player)), dbg(&t.revealed_objective_progress(&player)), "{}", at("revealed_objective_progress"));
                let opts = options(&pos, &player);
                assert_eq!(dbg(&o.revealed_objective_progress_gaining(&player, &opts.planets)), dbg(&t.revealed_objective_progress_gaining(&player, &opts.planets)), "{}", at("gaining"));
                for (label, n) in opts.sets() {
                    both_imagined!(opts, n, |oi, ti| {
                        let (a, b) = (o.revealed_objective_progress_imagining(&player, &oi), t.revealed_objective_progress_imagining(&player, &ti));
                        assert_eq!(dbg(&a), dbg(&b), "{}", at(&format!("revealed_objective_progress_imagining {label}")));
                        assert_eq!(dbg(&o.revealed_objective_progress_text(&player, &oi)), dbg(&t.revealed_objective_progress_text(&player, &ti)), "{}", at(&format!("_text {label}")));
                        rows += a.len();
                    });
                }
                for system in &systems {
                    let (a, b) = (o.objectives_implicating_system(&player, system), t.objectives_implicating_system(&player, system));
                    assert_eq!(a, b, "{}", at(&format!("objectives_implicating_system {system}")));
                    maps += usize::from(galaxy.is_some() && !a.is_empty());
                }
            }
        }
    }
    assert!(rows > 100, "only {rows} progress rows compared");
    assert!(maps > 5, "only {maps} implicated systems compared");
}

/// A decider that reads the bound seat's held secrets (plain, imagined) and keeps them.
struct Reads {
    seen: Vec<(String, String)>,
    imagined: thin::queries::objectives::Imagined<'static>,
}
impl thin::wire::choice::Decider for Reads {
    fn choose(&mut self, _choice: &thin::wire::choice::Choice) -> Result<thin::wire::choice::ChoiceOption, thin::wire::choice::IllegalChoice> {
        Ok(thin::wire::choice::ChoiceOption::decline())
    }
    fn choose_seeing(&mut self, _choice: &thin::wire::choice::Choice, seen: &thin::observed::SeatObservation<'_>) -> Result<thin::wire::choice::ChoiceOption, thin::wire::choice::IllegalChoice> {
        self.seen.push(("held".to_owned(), dbg(&seen.held_secret_progress())));
        self.seen.push(("imagined".to_owned(), dbg(&seen.held_secret_progress_imagining(&self.imagined))));
        self.seen.push(("cards".to_owned(), dbg(&seen.revealed_cards())));
        self.seen.push(("ends".to_owned(), dbg(&seen.agenda_deck_ends())));
        Ok(thin::wire::choice::ChoiceOption::decline())
    }
}

#[test]
fn the_held_secret_progress_answers_as_the_legacy_reads_do() {
    let content = content();
    let mut rows = 0_usize;
    for pos in positions() {
        for galaxy in [None, Some(&pos.galaxy)] {
            for player in players(&pos.state) {
                let at = |what: &str| format!("{} (map {}): {player}: {what}", pos.label, galaxy.is_some());
                let opts = options(&pos, &player);
                // The offline free functions.
                let a = old::choice::held_secret_progress(&pos.state, content, pos.sources, galaxy, &player);
                let b = thin::observed::held_secret_progress(&pos.state, content, pos.sources, galaxy, &player);
                assert_eq!(dbg(&a), dbg(&b), "{}", at("held_secret_progress"));
                rows += a.len();
                for (label, n) in opts.sets() {
                    both_imagined!(opts, n, |oi, ti| {
                        let a = old::choice::held_secret_progress_imagining(&pos.state, content, pos.sources, galaxy, &player, &oi);
                        let b = thin::observed::held_secret_progress_imagining(&pos.state, content, pos.sources, galaxy, &player, &ti);
                        assert_eq!(dbg(&a), dbg(&b), "{}", at(&format!("held_secret_progress_imagining {label}")));
                    });
                }
                // The bound reads: against the legacy bound view.
                let choice = thin::wire::choice::Choice::new(player.clone(), "p", vec![thin::wire::choice::ChoiceOption::decline()]);
                let mut reads = Reads { seen: Vec::new(), imagined: thin::queries::objectives::Imagined::NONE };
                thin::observed::ask_private(&choice, &pos.state, content, pos.sources, galaxy, &mut reads).expect("an answer");
                let legacy_choice = old::choice::Choice::new(player.clone(), "p", vec![old::choice::ChoiceOption::decline()]);
                struct Old(Vec<(String, String)>);
                impl old::choice::Decider for Old {
                    fn choose(&mut self, _c: &old::choice::Choice) -> Result<old::choice::ChoiceOption, old::choice::IllegalChoice> {
                        Ok(old::choice::ChoiceOption::decline())
                    }
                    fn choose_seeing(&mut self, _c: &old::choice::Choice, seen: &old::choice::SeatObservation<'_>) -> Result<old::choice::ChoiceOption, old::choice::IllegalChoice> {
                        self.0.push(("held".to_owned(), dbg(&seen.held_secret_progress())));
                        self.0.push(("imagined".to_owned(), dbg(&seen.held_secret_progress_imagining(&old::objectives::Imagined::NONE))));
                        self.0.push(("cards".to_owned(), dbg(&seen.revealed_cards())));
                        self.0.push(("ends".to_owned(), dbg(&seen.agenda_deck_ends())));
                        Ok(old::choice::ChoiceOption::decline())
                    }
                }
                let mut legacy = Old(Vec::new());
                old::choice::ask_private(&legacy_choice, &pos.state, content, pos.sources, galaxy, &mut legacy).expect("an answer");
                assert_eq!(legacy.0, reads.seen, "{}", at("the bound reads"));
            }
        }
    }
    assert!(rows > 20, "only {rows} held secrets compared");
}

#[test]
fn the_position_functions_answer_as_the_legacy_ones_do() {
    use thin::queries::objectives as t;
    let content = content();
    let aliases: Vec<String> = content.records(ti4_model::content_types::ContentType::PublicObjectives).iter().filter_map(|r| r.text("alias").map(ToOwned::to_owned)).collect();
    assert!(aliases.len() > 20);
    let mut answered = 0_usize;
    for pos in positions() {
        for with_map in [false, true] {
            for player in players(&pos.state) {
                let at = |what: &str| format!("{} (map {with_map}): {player}: {what}", pos.label);
                let opts = options(&pos, &player);
                for (label, n) in opts.sets() {
                    both_imagined!(opts, n, |oi, ti| {
                        let mut op = old::objectives::Position::imagining_all(&pos.state, content, pos.sources, &player, &oi);
                        let mut tp = t::Position::imagining_all(&pos.state, content, pos.sources, &player, &ti);
                        if with_map {
                            op = op.with_galaxy(&pos.galaxy);
                            tp = tp.with_galaxy(&pos.galaxy);
                        }
                        for alias in &aliases {
                            let id = ObjectiveId::new(alias.as_str());
                            let a = old::objectives::counting_progress(&id, &op);
                            assert_eq!(dbg(&a), dbg(&t::counting_progress(&id, &tp)), "{}", at(&format!("counting_progress {alias} {label}")));
                            let b = old::objectives::remaining_position_progress(&id, &op);
                            assert_eq!(dbg(&b), dbg(&t::remaining_position_progress(&id, &tp)), "{}", at(&format!("remaining_position_progress {alias} {label}")));
                            let c = old::objectives::bought_progress_at(&op, &id);
                            assert_eq!(dbg(&c), dbg(&t::bought_progress_at(&tp, &id)), "{}", at(&format!("bought_progress_at {alias} {label}")));
                            answered += usize::from(a.is_some() || b.is_some() || c.is_some());
                        }
                    });
                }
                let op = if with_map { old::objectives::Position::new(&pos.state, content, pos.sources, &player).with_galaxy(&pos.galaxy) } else { old::objectives::Position::new(&pos.state, content, pos.sources, &player) };
                let tp = if with_map { t::Position::new(&pos.state, content, pos.sources, &player).with_galaxy(&pos.galaxy) } else { t::Position::new(&pos.state, content, pos.sources, &player) };
                assert_eq!(dbg(&old::objectives::home_planets_not_controlled(&op)), dbg(&t::home_planets_not_controlled(&tp)), "{}", at("home_planets_not_controlled"));
                assert_eq!(dbg(&old::objectives::neighbour_planet_counts(&op)), dbg(&t::neighbour_planet_counts(&tp)), "{}", at("neighbour_planet_counts"));
            }
        }
    }
    assert!(answered > 500, "only {answered} answered rows compared");
    for alias in &aliases {
        let id = ObjectiveId::new(alias.as_str());
        assert_eq!(dbg(&old::objectives::cost_of(&id)), dbg(&t::cost_of(&id)), "cost_of {alias}");
    }
    assert_eq!(dbg(&old::objectives::cost_of(&ObjectiveId::new("no_such_objective"))), dbg(&t::cost_of(&ObjectiveId::new("no_such_objective"))));
}

#[test]
fn the_family_tokens_are_the_legacy_ones() {
    use thin::queries::objectives as t;
    // Every token the legacy engine can print comes out of the counting families of the corpus; the cost tokens are four plus the two spends.
    let content = content();
    let pos = &positions()[3];
    let mut tokens = std::collections::BTreeSet::new();
    for player in players(&pos.state) {
        let id = player.clone();
        let op = old::objectives::Position::new(&pos.state, content, pos.sources, &id).with_galaxy(&pos.galaxy);
        for record in content.records(ti4_model::content_types::ContentType::PublicObjectives) {
            let Some(alias) = record.text("alias") else { continue };
            let oid = ObjectiveId::new(alias);
            if let Some(p) = old::objectives::counting_progress(&oid, &op).or_else(|| old::objectives::remaining_position_progress(&oid, &op)) {
                tokens.insert(old::objectives::family_token(&p.family));
            }
        }
    }
    assert!(tokens.len() > 15, "{tokens:?}");
    let tp = t::Position::new(&pos.state, content, pos.sources, &pos.state.players[0].id).with_galaxy(&pos.galaxy);
    let mut native = std::collections::BTreeSet::new();
    for player in players(&pos.state) {
        let tp = t::Position::new(&pos.state, content, pos.sources, &player).with_galaxy(&pos.galaxy);
        for record in content.records(ti4_model::content_types::ContentType::PublicObjectives) {
            let Some(alias) = record.text("alias") else { continue };
            let oid = ObjectiveId::new(alias);
            if let Some(p) = t::counting_progress(&oid, &tp).or_else(|| t::remaining_position_progress(&oid, &tp)) {
                native.insert(t::family_token(&p.family));
            }
        }
    }
    let _ = tp;
    assert_eq!(tokens, native);
    for kind in [old::production::Spend::Resources, old::production::Spend::Influence] {
        let n = if kind == old::production::Spend::Resources { thin::queries::production::Spend::Resources } else { thin::queries::production::Spend::Influence };
        assert_eq!(old::objectives::family_token(&old::objectives::CountFamily::CombinedValue { kind }), t::family_token(&t::CountFamily::CombinedValue { kind: n }));
        assert_eq!(old::objectives::cost_family_token(&old::objectives::CostFamily::Spend(kind)), t::cost_family_token(&t::CostFamily::Spend(n)));
    }
    assert_eq!(old::objectives::cost_family_token(&old::objectives::CostFamily::TradeGoods), t::cost_family_token(&t::CostFamily::TradeGoods));
    assert_eq!(old::objectives::cost_family_token(&old::objectives::CostFamily::Tokens), t::cost_family_token(&t::CostFamily::Tokens));
    assert_eq!(old::objectives::cost_family_token(&old::objectives::CostFamily::AllThree), t::cost_family_token(&t::CostFamily::AllThree));
}

#[test]
fn the_secret_position_functions_answer_as_the_legacy_ones_do() {
    let content = content();
    let aliases: Vec<String> = content.records(ti4_model::content_types::ContentType::SecretObjectives).iter().filter_map(|r| r.text("alias").map(ToOwned::to_owned)).collect();
    assert!(aliases.len() > 20);
    let mut answered = 0_usize;
    for pos in positions() {
        for galaxy in [None, Some(&pos.galaxy)] {
            for player in players(&pos.state) {
                let at = |what: &str| format!("{} (map {}): {player}: {what}", pos.label, galaxy.is_some());
                let opts = options(&pos, &player);
                for (label, n) in opts.sets() {
                    both_imagined!(opts, n, |oi, ti| {
                        let op = old::secrets::Position { state: &pos.state, content, sources: pos.sources, player: &player, galaxy, imagined: oi };
                        let tp = thin::queries::secrets::Position { state: &pos.state, content, sources: pos.sources, player: &player, galaxy, imagined: ti };
                        for alias in &aliases {
                            let id = SecretObjectiveId::new(alias.as_str());
                            let a = old::secrets::progress(&id, &op);
                            assert_eq!(dbg(&a), dbg(&thin::queries::secrets::progress(&id, &tp)), "{}", at(&format!("secrets::progress {alias} {label}")));
                            answered += usize::from(a.is_some());
                        }
                        let (a, b) = (old::secrets::ship_systems_beside_anomaly(&op), thin::queries::secrets::ship_systems_beside_anomaly(&tp));
                        assert_eq!(a, b, "{}", at(&format!("ship_systems_beside_anomaly {label}")));
                    });
                }
            }
        }
    }
    assert!(answered > 300, "only {answered} answered rows compared");
}

#[test]
fn the_ship_systems_beside_an_anomaly_are_named() {
    // The random maps of the corpus hold no anomaly: a hub whose centre is one, with ships in two of its ring systems (and a third beside none).
    let content = content();
    let all = ti4_content::galaxy::all_systems(content, ti4_model::content_types::POK);
    let anomaly = all.iter().find(|(_, s)| s.is_anomaly()).map(|(id, _)| (*id).to_owned()).expect("an anomaly in the corpus");
    let hub = old::fixtures::hub_with_centre(&anomaly);
    let mut state = old::fixtures::game(&["a", "b"]);
    let seat = PlayerId::new("a");
    for system in &hub.outer[..2] {
        old::fixtures::put(&mut state, &SystemId::new(system.clone()), "cruiser", &seat, 1);
    }
    let galaxy = Some(&hub.galaxy);
    let op = old::secrets::Position { state: &state, content, sources: ti4_model::content_types::POK, player: &seat, galaxy, imagined: old::objectives::Imagined::NONE };
    let tp = thin::queries::secrets::Position { state: &state, content, sources: ti4_model::content_types::POK, player: &seat, galaxy, imagined: thin::queries::objectives::Imagined::NONE };
    let named = thin::queries::secrets::ship_systems_beside_anomaly(&tp);
    assert_eq!(old::secrets::ship_systems_beside_anomaly(&op), named);
    assert_eq!(named.expect("a map").len(), 2, "two systems beside the anomaly");
    let no_map = thin::queries::secrets::Position { galaxy: None, ..tp };
    assert_eq!(thin::queries::secrets::ship_systems_beside_anomaly(&no_map), None);
}
