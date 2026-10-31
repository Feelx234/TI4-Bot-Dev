//! The play facts of the live loop against the string engine's (package KF2, `ti4_engine::live::plays`): for every position variant the
//! facts an action card played inside a window reads (`PlayFacts`: the unit a seat places for infantry and cruiser, the Mahact
//! pools, Reveal Prototype's set and payability, the blanked laws, the scope, the card tables) are computed twice, by
//! `reactions_core::play_facts` from the string `GameState` and by `live::plays::play_facts` from the core position and the live
//! kernel, and compared field by field. The variants are those of `check_leaders` plus the ones the facts care about: unit upgrades,
//! a Reveal Prototype in hand with ships in the active system, a Mahact fleet pool.
//!
//! Departures that are DECISIONS, counted as `gaps` (never mismatches): the string declines `reveal` (None) for a table with a seat
//! whose research it does not port; the core answers (its hooks are the live kernel's).

use ti4_content::ContentStore;
use ti4_engine::core_sync::{CoreView, Ctx, Sources};
use ti4_engine::generated::IdStr;
use ti4_engine::live::plays::{self, Texts};
use ti4_engine::render::Render;
use ti4_engine_core::generated::{ActionCard, Requirement, Technology, Unit as CoreUnit};
use ti4_engine_core::kernel::ability::PlayFacts;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::{ActionCardId, PlayerId, TechnologyId, UnitTypeId};
use ti4_model::state::GameState;
use ti4_model::units::Unit;

use crate::workload::Position;

#[derive(Debug, Default, Clone)]
pub struct PlayReport {
    pub positions: usize,
    pub variants: usize,
    pub seats: usize,
    pub answers: usize,
    pub unmapped: usize,
    pub mismatches: Vec<String>,
    /// Where the string declines and the core answers (counted by field).
    pub gaps: std::collections::BTreeMap<String, usize>,
    /// Seats whose Reveal Prototype set was non-empty (a check that compares only empty sets proves nothing).
    pub nonempty_reveal: usize,
    /// Seats where Reveal Prototype was payable.
    pub payable: usize,
    /// Seats that hold a Mahact token of another seat.
    pub mahact_held: usize,
    /// Seats whose infantry or cruiser form differs from the plain one (an upgrade or a faction unit).
    pub special_forms: usize,
}

impl PlayReport {
    pub fn add(&mut self, o: &Self) {
        self.positions += o.positions;
        self.variants += o.variants;
        self.seats += o.seats;
        self.answers += o.answers;
        self.unmapped += o.unmapped;
        self.mismatches.extend(o.mismatches.iter().cloned());
        for (k, n) in &o.gaps {
            *self.gaps.entry(k.clone()).or_default() += n;
        }
        self.nonempty_reveal += o.nonempty_reveal;
        self.payable += o.payable;
        self.mahact_held += o.mahact_held;
        self.special_forms += o.special_forms;
    }

    fn same<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: T, core: T) {
        self.answers += 1;
        if string != core && self.mismatches.len() < 60 {
            self.mismatches.push(format!("{what}: string {string:?} vs core {core:?}"));
        }
    }
}

/// The technology that makes `unit` available (a unit upgrade).
fn tech_of(unit: CoreUnit) -> Technology {
    match unit.data().required_technology {
        Requirement::Known(t) => t,
        other => panic!("{unit:?} is no upgrade: {other:?}"),
    }
}

/// The variants of a position that matter to the play facts, on top of `check_leaders::variants`.
fn variants(gs: &GameState) -> Vec<(String, GameState)> {
    let mut out: Vec<(String, GameState)> = crate::check_leaders::variants(gs).into_iter().map(|(n, g)| (n.to_owned(), g)).collect();
    let ids: Vec<PlayerId> = gs.players.iter().map(|p| p.id.clone()).collect();
    // Unit upgrades: every seat owns a different slice of the unit upgrades, one of them each for infantry and cruiser.
    let upgrades: Vec<Technology> = Technology::ALL.iter().copied().filter(|t| ti4_engine_core::query::tech::is_unit_upgrade(*t)).collect();
    let mut g = gs.clone();
    for (i, id) in ids.iter().enumerate() {
        if let Some(seat) = g.player_mut(id) {
            for t in upgrades.iter().skip(i % 3).step_by(2 + i % 2) {
                seat.technologies.insert(TechnologyId::new(t.as_str()));
            }
            seat.technologies.insert(TechnologyId::new(tech_of(CoreUnit::CruiserII).as_str()));
        }
    }
    out.push(("unit upgrades".to_owned(), g));
    // Reveal Prototype: the first seat holds it, has the resources, and ships of several lines stand in the active system.
    let mut g = gs.clone();
    if let (Some(first), Some(home)) = (ids.first(), gs.players.first().and_then(|p| p.home_system.clone())) {
        // The prerequisites of the upgrades: sixteen plain technologies of the four colours.
        let plain: Vec<Technology> = Technology::ALL
            .iter()
            .copied()
            .filter(|t| ti4_engine_core::query::tech::colour_type(*t).is_some() && !ti4_engine_core::query::tech::is_unit_upgrade(*t) && ti4_engine_core::query::tech::faction_of(*t).is_none())
            .take(16)
            .collect();
        if let Some(seat) = g.player_mut(first) {
            seat.action_cards.push(ActionCardId::new(ActionCard::RevealPrototype.as_str()));
            seat.trade_goods = 12;
            for t in plain {
                seat.technologies.insert(TechnologyId::new(t.as_str()));
            }
        }
        let sys = g.board.entry(home.clone()).or_default();
        for kind in ["cruiser", "dreadnought", "carrier", "fighter", "destroyer"] {
            sys.units.push(Unit::new(UnitTypeId::new(kind), first.clone()));
        }
        if let Some(second) = ids.get(1) {
            sys.units.push(Unit::new(UnitTypeId::new("destroyer"), second.clone()));
            sys.units.push(Unit::new(UnitTypeId::new(CoreUnit::WarSun.as_str()), second.clone()));
        }
        g.active_system = Some(home);
    }
    out.push(("reveal prototype".to_owned(), g.clone()));
    // The same with the seat's upgrades for those lines already owned (the set must still list them: the string lists every upgrade of
    // the line, owned or not).
    if let Some(seat) = ids.first().and_then(|id| g.player_mut(id)) {
        seat.technologies.insert(TechnologyId::new(tech_of(CoreUnit::CruiserII).as_str()));
        seat.technologies.insert(TechnologyId::new(tech_of(CoreUnit::DreadnoughtII).as_str()));
        seat.trade_goods = 0;
    }
    out.push(("reveal prototype, poor".to_owned(), g));
    // A Mahact fleet pool (only where a Mahact seat is at the table): every other seat's token sits in it.
    let mahact: Vec<&PlayerId> = gs.players.iter().filter(|p| p.faction.as_str() == "mahact").map(|p| &p.id).collect();
    if let Some(m) = mahact.first() {
        let mut g = gs.clone();
        let pool: Vec<String> = ids.iter().filter(|id| id != m).flat_map(|id| [id.as_str().to_owned(), id.as_str().to_owned()]).collect();
        g.faction_marks.insert(format!("mahact:fleet:{m}"), pool.join(","));
        out.push(("mahact pool".to_owned(), g));
    }
    out
}

fn reads() -> SliceSet {
    let all: Vec<Slice> = Slice::ALL.iter().copied().filter(|s| *s != Slice::Rng).collect();
    SliceSet::of(&all)
}

/// Compare the play facts on one position and its variants.
pub fn check_position(p: &Position, report: &mut PlayReport) {
    let _content = ContentStore::embedded();
    report.positions += 1;
    let kernel = ti4_engine::live::kernel::kernel_for(p.sources);
    for (name, g) in variants(&p.state) {
        report.variants += 1;
        let label = format!("{} [{name}]", p.label);
        let Ok(mut cx) = Ctx::of(&g) else {
            report.unmapped += 1;
            continue;
        };
        let view = match CoreView::of(&g, &mut cx, Sources { galaxy: Some(&p.galaxy), rng: None }, reads()) {
            Ok(v) => v,
            Err(e) => {
                report.unmapped += 1;
                report.mismatches.push(format!("{label}: CoreView refused the position: {e}"));
                continue;
            }
        };
        let r = Render::new(cx.owners.ids().to_vec());
        let Some(string) = ti4_engine::reactions_core::string_play_facts(&r, &g, &p.sources) else {
            report.unmapped += 1;
            report.mismatches.push(format!("{label}: the string side gave no play facts"));
            continue;
        };
        let texts = Texts::new(&r, p.sources);
        let Some(core) = plays::play_facts(&view, kernel, &texts) else {
            report.unmapped += 1;
            report.mismatches.push(format!("{label}: the core side gave no play facts"));
            continue;
        };
        compare(&label, &string, &core, report);
    }
}

fn compare(label: &str, string: &PlayFacts, core: &PlayFacts, report: &mut PlayReport) {
    report.same(&format!("{label} scope"), string.scope, core.scope);
    report.same(&format!("{label} blanked"), string.blanked, core.blanked);
    report.same(&format!("{label} action_card"), string.action_card, core.action_card);
    report.same(&format!("{label} supply_base"), string.supply_base, core.supply_base);
    report.same(&format!("{label} name_group"), &string.name_group, &core.name_group);
    report.same(&format!("{label} has_effect"), &string.has_effect, &core.has_effect);
    report.same(&format!("{label} skimmer"), string.skimmer, core.skimmer);
    report.same(&format!("{label} rules_version"), string.rules_version, core.rules_version);
    report.same(&format!("{label} roster"), string.roster.map(std::ptr::from_ref), core.roster.map(std::ptr::from_ref));
    // The table-wide facts (`SharedPlay`). A replenishment that causes events the core does not hold, and the Fracture, are decisions
    // (the core holds Cabal's note and the Trade Agreement since W10; the Fracture is Thunder's Edge), counted as gaps.
    let (a, b) = (&string.shared, &core.shared);
    report.same(&format!("{label} neutral"), a.neutral, b.neutral);
    report.same(&format!("{label} own_breakthrough"), a.own_breakthrough, b.own_breakthrough);
    report.same(&format!("{label} courageous"), (a.courageous_subtype, a.courageous_cause), (b.courageous_subtype, b.courageous_cause));
    report.same(&format!("{label} hits"), a.hits, b.hits);
    for (what, string_side, core_side) in [("replenish_events", a.replenish_events, b.replenish_events), ("fracture_in_play", a.fracture_in_play, b.fracture_in_play)] {
        match (string_side, core_side) {
            (true, false) => *report.gaps.entry(format!("{what} (the core holds it, or it is out of scope)")).or_default() += 1,
            (x, y) => report.same(&format!("{label} {what}"), x, y),
        }
    }
    report.same(&format!("{label} seats"), string.seats.len(), core.seats.len());
    for (s, c) in string.seats.iter().zip(&core.seats) {
        report.seats += 1;
        let at = |what: &str| format!("{label} seat {} {what}", s.who.0);
        report.same(&at("who"), s.who, c.who);
        report.same(&at("mahact_held"), s.mahact_held, c.mahact_held);
        report.same(&at("infantry"), s.infantry, c.infantry);
        report.same(&at("cruiser"), s.cruiser, c.cruiser);
        report.same(&at("fighter_on_card"), s.fighter_on_card, c.fighter_on_card);
        report.same(&at("more"), format!("{:?}", s.more), format!("{:?}", c.more));
        match (&s.reveal, &c.reveal) {
            (None, Some(_)) => *report.gaps.entry("reveal (the string declines at this table)".to_owned()).or_default() += 1,
            (a, b) => report.same(&at("reveal"), a, b),
        }
        // The payability is the string's only where it researches at all.
        if s.reveal.is_some() {
            report.same(&at("reveal_payable"), s.reveal_payable, c.reveal_payable);
        }
        report.nonempty_reveal += usize::from(c.reveal.as_ref().is_some_and(|set| set.iter().next().is_some()));
        report.payable += usize::from(c.reveal_payable);
        report.mahact_held += usize::from(c.mahact_held > 0);
        report.special_forms += usize::from(c.infantry.0 != Some(CoreUnit::InfantryI) || c.cruiser.0 != Some(CoreUnit::CruiserI));
    }
}

