//! The prep layer: *prepared* steps for every planner.
//!
//! A plain [`Step`] is one activation judged on the position as it is. A prepared step is an
//! activation preceded by a prep that changes what the activation can do, judged on the
//! position *after* the prep:
//!
//! | kind | knob | prep | what it adds to the act step |
//! |---|---|---|---|
//! | [`PrepKind::Build`] | [`tuning::bw_prep_build`] | a production: a home dock activation, Construction's production (Thunder's Edge), Warfare's secondary | ground forces to land, a carrier to bring them, cruisers to win a fight, a ship to send |
//! | [`PrepKind::Warfare`] | [`tuning::bw_prep_warfare`] | an activation next to the target, paid for by Warfare's extra activation when tokens are short | the ships (and cargo) that could reach the neighbour but not the target |
//! | [`PrepKind::TechRange`] | [`tuning::bw_prep_tech_range`] | a movement technology | ships one hex short |
//! | [`PrepKind::TechStrength`] | [`tuning::bw_prep_tech_strength`] | a combat or upgrade technology | the fleet's strength, judged with `fleet_strength` |
//!
//! [`prepared`] appends the prepared steps to [`super::PlanCtx::steps`], so
//! [`super::PlanCtx::route`] and every planner built on it see them without change. A prepared
//! step carries its prep ([`PrepInfo`]: tokens, strategic tokens, resources, the card to pick or
//! play, units to build, systems to activate first) and its probability is prep feasibility x act
//! success x 0.9 for the extra step. A route holds at most one prepared step, and all steps and
//! preps together stay within the seat's tactic tokens and three activations.

use std::collections::BTreeMap;

use ti4_engine::choice::Observed;
use ti4_engine::fleet_strength::{Descriptors, descriptors, matchup};
use ti4_model::id::{PlanetId, PlayerId, SystemId};

use super::{PlanCtx, Step};
use crate::tuning;

/// Which prep a prepared step uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepKind {
    Build,
    Warfare,
    TechRange,
    TechStrength,
}

/// What a prep costs and asks for.
#[derive(Debug, Clone, PartialEq)]
pub struct PrepInfo {
    pub kind: PrepKind,
    /// "build + act", "warfare move + act", ... (the start of a plan's reason).
    pub label: String,
    /// Tactic tokens the prep itself uses (a gather activation or a dock activation).
    pub tokens: i32,
    /// Strategy tokens it uses (a secondary).
    pub strategic: i32,
    pub resources: i64,
    /// Systems activated first.
    pub systems: Vec<SystemId>,
    /// The subset of `systems` where ships and cargo gather (the movement hook moves everything).
    pub gather: Vec<SystemId>,
    /// Unit types to produce.
    pub builds: Vec<String>,
    /// Strategy card to draft.
    pub cards: Vec<String>,
    /// Strategy card to play now.
    pub play: Vec<String>,
    /// Probability that the prep itself works.
    pub odds: f64,
}

/// The fight and landing in one system as the seat can bring them.
#[derive(Debug, Clone)]
pub struct SysCase {
    pub system: SystemId,
    /// Planets worth taking: (planet, worth, ground forces needed), best per troop first.
    pub targets: Vec<(PlanetId, f64, usize)>,
    /// Ground forces that reach it now (plus those there).
    pub ground: usize,
    /// Whether a ship can reach it now.
    pub ships: bool,
    /// An own ship is there already.
    pub present: bool,
    pub hostile: bool,
    /// The fleet that could fight there now, as unit counts.
    pub own: Vec<(String, usize)>,
    pub enemy: Descriptors,
    pub margin: f64,
    pub faction: String,
}

/// What an activation does: planets taken, a ship arriving, and the odds.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub planets: Vec<PlanetId>,
    pub ship: bool,
    pub success: f64,
}

impl SysCase {
    #[must_use]
    pub fn new(seen: &Observed<'_>, me: &PlayerId, system: &SystemId) -> Self {
        let (ships, ground) = crate::objectives::reach(seen, me, system);
        let movable = seen.movable_into(me, system);
        let board = seen.system(system);
        let here = board
            .units
            .iter()
            .filter(|u| &u.owner == me && crate::is_ship(seen, u));
        let present = here.clone().next().is_some();
        let (own, _) = crate::tally(here.chain(movable.iter().map(|m| &m.unit)));
        Self {
            targets: crate::targets(seen, me, system),
            hostile: crate::has_hostile_ships(seen, me, system),
            enemy: crate::hostile_space(seen, me, system),
            margin: crate::fight_margin(seen, system),
            faction: crate::faction(seen, me),
            system: system.clone(),
            ground,
            ships,
            present,
            own,
        }
    }

    /// The strength matchup of the fleet `own` against the defence (infinite without one).
    #[must_use]
    pub fn matchup_of(&self, seen: &Observed<'_>, own: &[(String, usize)]) -> f64 {
        if !self.hostile {
            return f64::INFINITY;
        }
        let mine = descriptors(seen.content(), seen.sources(), &self.faction, own, &[]);
        matchup(&mine, &self.enemy)
    }

    /// The planets `ground` forces take, greedily in target order.
    #[must_use]
    pub fn taken(&self, ground: usize) -> Vec<PlanetId> {
        let mut left = ground;
        let mut out = Vec::new();
        for (planet, _, need) in &self.targets {
            if *need <= left {
                left -= need;
                out.push(planet.clone());
            }
        }
        out
    }

    /// What the activation does with `ground` forces and fleet `own`, if it does anything. With
    /// `reachable` false nothing arrives.
    #[must_use]
    pub fn outcome(
        &self,
        seen: &Observed<'_>,
        ground: usize,
        own: &[(String, usize)],
        reachable: bool,
    ) -> Option<Outcome> {
        if !reachable || own.is_empty() {
            return None;
        }
        let m = self.matchup_of(seen, own);
        if m < self.margin {
            return None;
        }
        let planets = self.taken(ground);
        let ship = !self.present;
        if planets.is_empty() && !ship {
            return None;
        }
        let fight = if self.hostile {
            1.0 / (1.0 + (-2.0 * m).exp())
        } else {
            1.0
        };
        Some(Outcome {
            success: fight * if planets.is_empty() { 1.0 } else { 0.9 },
            planets,
            ship,
        })
    }

    /// The activation as it stands.
    #[must_use]
    pub fn plain(&self, seen: &Observed<'_>) -> Option<Outcome> {
        self.outcome(seen, self.ground, &self.own, self.ships)
    }
}

fn with_added(fleet: &[(String, usize)], id: &str, n: usize) -> Vec<(String, usize)> {
    let mut out = fleet.to_vec();
    match out.iter_mut().find(|(k, _)| k == id) {
        Some((_, c)) => *c += n,
        None => out.push((id.to_owned(), n)),
    }
    out
}

/// Production slots of the home system (docks' production values, as the engine counts them).
fn home_slots(seen: &Observed<'_>, me: &PlayerId, home: &SystemId) -> i64 {
    seen.system(home)
        .planet_units
        .iter()
        .flat_map(|(planet, units)| {
            let res = ti4_content::galaxy::planet(seen.content(), planet.as_str(), seen.sources())
                .map_or(0, |p| p.resources());
            units
                .iter()
                .filter(|u| &u.owner == me)
                .filter_map(move |u| crate::unit_type(seen, u.type_id.as_str()))
                .map(move |t| t.production(res))
        })
        .sum::<i64>()
        .max(0)
}

/// How the seat gets a production before the act step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Activate the home dock (one more command token).
    Dock,
    /// The seat's own Construction card, whose primary offers a production (Thunder's Edge).
    Construction,
    /// The seat's own Warfare primary (Thunder's Edge): a free tactical action into the home
    /// dock, which produces without a token.
    WarfarePrimary,
    /// The printed Warfare primary frees a token: a home dock activation with it.
    DockWithWarfare,
    /// Another seat's Warfare secondary, which produces at home for a strategy token.
    WarfareSecondary,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Dock => "home dock activation",
            Self::Construction => "Construction production",
            Self::WarfarePrimary => "Warfare primary, free activation of the dock",
            Self::DockWithWarfare => "Warfare primary token, home dock activation",
            Self::WarfareSecondary => "Warfare secondary production",
        }
    }
}

/// The ways to get a production, as (source, tactic tokens, strategy tokens, odds).
fn sources(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx) -> Vec<(Source, i32, i32, f64)> {
    let mut out = Vec::new();
    if let Some(home) = crate::home_system(seen, me)
        && ctx.tactic_tokens >= 2
        && !seen.systems_with_token(me).contains(&home)
        && crate::has_own_production(seen, me, &home)
    {
        out.push((Source::Dock, 1, 0, 0.95));
    }
    if crate::ready_card(seen, me, "Construction").is_some_and(|c| c.starts_with("te")) {
        out.push((Source::Construction, 0, 0, 0.95));
    }
    let access = warfare(seen, me);
    if let Some(card) = &access.own
        && let Some(home) = crate::home_system(seen, me)
        && crate::has_own_production(seen, me, &home)
    {
        if card.starts_with("te") {
            out.push((Source::WarfarePrimary, 0, 0, 0.95));
        } else if access.extra_activation
            && ctx.tactic_tokens == 1
            && !seen.systems_with_token(me).contains(&home)
        {
            out.push((Source::DockWithWarfare, 1, 0, 0.95));
        }
    }
    if ctx.strategic_tokens >= 1 && crate::secondary_to_come(seen, me, "Warfare") {
        out.push((Source::WarfareSecondary, 0, 1, 0.6));
    }
    out
}

/// Build prep for `case`: what a production at home adds, if it makes the activation do more.
fn build_prep(
    seen: &Observed<'_>,
    me: &PlayerId,
    ctx: &PlanCtx,
    case: &SysCase,
    plain: Option<&Outcome>,
) -> Option<(Outcome, PrepInfo)> {
    let (source, tokens, strategic, odds) = sources(seen, me, ctx)
        .into_iter()
        .max_by(|a, b| a.3.total_cmp(&b.3).then(b.1.cmp(&a.1)))?;
    let home = crate::home_system(seen, me)?;
    let slots = home_slots(seen, me, &home);
    if slots <= 0 {
        return None;
    }
    let carrier = crate::unit_type(seen, "carrier")?;
    let cruiser = crate::unit_type(seen, "cruiser")?;
    let infantry = crate::unit_type(seen, "infantry")?;
    let carrier_range = crate::carrier_reach(seen, me);
    let cruiser_range = i32::try_from(cruiser.move_value()).unwrap_or(1);
    let have_taken = plain.map_or(0, |o| o.planets.len());
    let mut units: Vec<(String, usize)> = Vec::new();
    let mut slots_left = slots;
    let mut budget = ctx.resources as f64;
    let mut fleet = case.own.clone();
    let mut ground = case.ground;
    if case.targets.is_empty() {
        // A ship goal: a new cruiser that can get there.
        if case.present
            || plain.is_some_and(|o| o.ship)
            || cruiser.cost() > budget
            || (home != case.system && !seen.can_reach(me, &home, &case.system, cruiser_range))
        {
            return None;
        }
        units.push((cruiser.id().to_owned(), 1));
        fleet = with_added(&fleet, cruiser.id(), 1);
        budget -= cruiser.cost();
        slots_left -= 1;
    } else {
        if home != case.system && !seen.can_reach(me, &home, &case.system, carrier_range) {
            return None;
        }
        // Ground forces needed for one planet more.
        let extra = (1..=usize::try_from(slots_left).unwrap_or(0))
            .find(|e| case.taken(case.ground + e).len() > have_taken)?;
        let extra_i = i64::try_from(extra).unwrap_or(0);
        let movable = seen.movable_into(me, &case.system);
        let cap_home: i64 = movable
            .iter()
            .filter(|m| m.origin == home && m.capacity > 0 && crate::is_ship(seen, &m.unit))
            .map(|m| m.capacity)
            .sum();
        let waiting = i64::try_from(
            seen.loadable(me, &home)
                .iter()
                .filter(|c| crate::is_ground(seen, &c.unit))
                .count(),
        )
        .unwrap_or(0);
        let spare = (cap_home - waiting).max(0);
        let cap = carrier.capacity().max(1);
        let carriers = ((extra_i - spare).max(0) + cap - 1) / cap;
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let cost = extra_i as f64 * infantry.cost() + carriers as f64 * carrier.cost();
        if extra_i + carriers > slots_left || cost > budget {
            return None;
        }
        slots_left -= extra_i + carriers;
        budget -= cost;
        units.push((infantry.id().to_owned(), extra));
        ground += extra;
        if carriers > 0 {
            let n = usize::try_from(carriers).unwrap_or(0);
            units.push((carrier.id().to_owned(), n));
            fleet = with_added(&fleet, carrier.id(), n);
        }
    }
    // Cruisers until the fight is won.
    let mut added = 0usize;
    while case.hostile
        && case.matchup_of(seen, &fleet) < case.margin
        && slots_left > 0
        && budget >= cruiser.cost()
        && added < 4
    {
        fleet = with_added(&fleet, cruiser.id(), 1);
        slots_left -= 1;
        budget -= cruiser.cost();
        added += 1;
    }
    if added > 0 {
        units.push((cruiser.id().to_owned(), added));
    }
    let out = case.outcome(seen, ground, &fleet, true)?;
    if out.planets.len() <= have_taken && !(out.ship && !plain.is_some_and(|o| o.ship)) {
        return None;
    }
    #[expect(clippy::cast_possible_truncation, reason = "a handful of resources")]
    let resources = (ctx.resources as f64 - budget).ceil() as i64;
    let mut systems = Vec::new();
    if matches!(
        source,
        Source::Dock | Source::DockWithWarfare | Source::WarfarePrimary
    ) {
        systems.push(home);
    }
    let card = match source {
        Source::Construction => Some("Construction"),
        Source::WarfarePrimary | Source::DockWithWarfare => Some("Warfare"),
        _ => None,
    };
    Some((
        out,
        PrepInfo {
            kind: PrepKind::Build,
            label: "build + act".to_owned(),
            tokens,
            strategic,
            resources,
            gather: Vec::new(),
            systems,
            builds: units.iter().map(|(u, _)| u.clone()).collect(),
            cards: card.map(str::to_owned).into_iter().collect(),
            play: card.map(str::to_owned).into_iter().collect(),
            odds,
        },
    ))
    .map(|(o, mut p)| {
        p.label = format!("build + act ({})", source.label());
        (o, p)
    })
}

/// What the seat's Warfare card gives this turn (shared by the build and move preps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarfareAccess {
    /// The seat's own ready Warfare card.
    pub own: Option<String>,
    /// Playing it gives one more activation: Thunder's Edge's free tactical action, or the
    /// printed card's recalled token (needs a token on the board).
    pub extra_activation: bool,
    /// Another seat's Warfare is still to come: its secondary produces at home.
    pub secondary: bool,
}

/// The seat's Warfare options.
#[must_use]
pub fn warfare(seen: &Observed<'_>, me: &PlayerId) -> WarfareAccess {
    let own = crate::ready_card(seen, me, "Warfare");
    let extra_activation = own
        .as_ref()
        .is_some_and(|c| c.starts_with("te") || !seen.systems_with_token(me).is_empty());
    WarfareAccess {
        secondary: crate::secondary_to_come(seen, me, "Warfare"),
        extra_activation,
        own,
    }
}

/// Ground and fleet the seat has at `case.system` after gathering at `gather` (an activation next
/// to it): the ships that could reach `gather` join those that reach the system.
fn gathered(
    seen: &Observed<'_>,
    me: &PlayerId,
    case: &SysCase,
    gather: &SystemId,
) -> (usize, Vec<(String, usize)>) {
    let mut set: BTreeMap<(SystemId, usize), ti4_engine::tactical::Movable> = BTreeMap::new();
    for m in seen
        .movable_into(me, gather)
        .into_iter()
        .chain(seen.movable_into(me, &case.system))
    {
        set.entry((m.origin.clone(), m.index)).or_insert(m);
    }
    let carriers: Vec<_> = set
        .values()
        .filter(|m| m.capacity > 0 && crate::is_ship(seen, &m.unit))
        .collect();
    let capacity: i64 = carriers.iter().map(|m| m.capacity).sum();
    let origins: std::collections::BTreeSet<&SystemId> =
        carriers.iter().map(|m| &m.origin).collect();
    let loadable: usize = origins
        .into_iter()
        .map(|o| {
            seen.loadable(me, o)
                .iter()
                .filter(|c| crate::is_ground(seen, &c.unit))
                .count()
        })
        .sum();
    let board = seen.system(&case.system);
    let there = board
        .units
        .iter()
        .filter(|u| &u.owner == me && crate::is_ground(seen, u))
        .count();
    let ground = loadable.min(usize::try_from(capacity).unwrap_or(0)) + there;
    let present = board
        .units
        .iter()
        .filter(|u| &u.owner == me && crate::is_ship(seen, u));
    let (own, _) = crate::tally(present.chain(set.values().map(|m| &m.unit)));
    (ground, own)
}

/// Warfare prep: gather beside the target (Warfare's extra activation if tokens are short).
fn warfare_prep(
    seen: &Observed<'_>,
    me: &PlayerId,
    ctx: &PlanCtx,
    case: &SysCase,
    plain: Option<&Outcome>,
) -> Option<(Outcome, PrepInfo)> {
    let access = warfare(seen, me);
    let use_warfare = ctx.tactic_tokens < 2;
    if ctx.tactic_tokens < 1 || (use_warfare && !access.extra_activation) {
        return None;
    }
    let galaxy = seen.galaxy()?;
    let tokened = seen.systems_with_token(me);
    let have_taken = plain.map_or(0, |o| o.planets.len());
    let (gather, out) = galaxy
        .adjacent(case.system.as_str())
        .into_iter()
        .map(SystemId::new)
        .filter(|x| !tokened.contains(x) && !crate::has_hostile_ships(seen, me, x))
        .filter_map(|x| {
            let (ground, own) = gathered(seen, me, case, &x);
            case.outcome(seen, ground, &own, true).map(|o| (x, o))
        })
        .filter(|(_, o)| o.planets.len() > have_taken || (o.ship && !plain.is_some_and(|p| p.ship)))
        .max_by(|a, b| {
            a.1.planets
                .len()
                .cmp(&b.1.planets.len())
                .then(a.1.success.total_cmp(&b.1.success))
                .then_with(|| b.0.cmp(&a.0))
        })?;
    Some((
        out,
        PrepInfo {
            kind: PrepKind::Warfare,
            label: if use_warfare {
                "warfare move + act (Warfare gives the activation)".to_owned()
            } else {
                "warfare move + act (tokens enough, no Warfare)".to_owned()
            },
            tokens: i32::from(!use_warfare),
            strategic: 0,
            resources: 0,
            systems: vec![gather.clone()],
            gather: vec![gather],
            builds: Vec::new(),
            cards: if use_warfare {
                vec!["Warfare".to_owned()]
            } else {
                Vec::new()
            },
            play: if use_warfare {
                vec!["Warfare".to_owned()]
            } else {
                Vec::new()
            },
            odds: if use_warfare { 0.95 * 0.85 } else { 0.85 },
        },
    ))
}

/// The prepared steps for every system the seat could activate (own-token systems left out), for
/// the kinds switched on. A prepared step is kept only where it does more than the plain step.
#[must_use]
pub fn prepared(seen: &Observed<'_>, me: &PlayerId, ctx: &PlanCtx) -> Vec<Step> {
    let any = tuning::bw_prep_build() || tuning::bw_prep_warfare();
    let Some(galaxy) = seen.galaxy().filter(|_| any) else {
        return Vec::new();
    };
    let tokened = seen.systems_with_token(me);
    let mut out = Vec::new();
    for id in galaxy.system_ids() {
        let system = SystemId::new(id);
        if tokened.contains(&system)
            || ctx.mecatol.held && ctx.mecatol.system.as_ref() == Some(&system)
        {
            continue;
        }
        let case = SysCase::new(seen, me, &system);
        let plain = case.plain(seen);
        let complete = plain.as_ref().is_some_and(|o| {
            o.success >= tuning::bw_likely()
                && if case.targets.is_empty() {
                    o.ship
                } else {
                    o.planets.len() == case.targets.len()
                }
        });
        if complete {
            continue;
        }
        let mut options: Vec<(Outcome, PrepInfo)> = Vec::new();
        if tuning::bw_prep_build() {
            options.extend(build_prep(seen, me, ctx, &case, plain.as_ref()));
        }
        if tuning::bw_prep_warfare() {
            options.extend(warfare_prep(seen, me, ctx, &case, plain.as_ref()));
        }
        let best = options.into_iter().max_by(|a, b| {
            let key = |o: &(Outcome, PrepInfo)| o.0.success * o.1.odds;
            key(a).total_cmp(&key(b))
        });
        if let Some((o, prep)) = best {
            out.push(Step {
                system,
                planets: o.planets,
                ship: o.ship,
                spare_ship: true,
                success: prep.odds * o.success * 0.9,
                prep: Some(prep),
            });
        }
    }
    out
}
