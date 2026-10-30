//! The free technology windows of a turn (package W12; string `technology::start_turn` and `technology::end_turn`) as
//! [`Window`]s: Psychoarchaeology (start of a turn), Predictive Intelligence and Bio-Stims (end of a turn).
//!
//! Each is the string code split at its questions. Psychoarchaeology asks again after each use until no specialty planet is
//! ready or the owner declines; Predictive Intelligence moves one token per answer (at most as many as the owner holds) and
//! exhausts the card when anything moved; Bio-Stims readies one planet or technology and exhausts itself.
//! What stays with the string (the adapter bails): Transit Diodes and Chaos Mapping at the start of a turn.

use crate::choice::agenda::Shown;
use crate::choice::strategy::Pool;
use crate::choice::technology::{Ask as TAsk, Opt as TOpt, Subtype as TSub};
use crate::choice::generic::Content;
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{Planet, System, Technology, Unit};
use crate::ids::Seat;
use crate::kernel::{fx, fx_strategy};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next};
use crate::query::board;
use crate::state::{Code, CoreState, EventKind, EventRec, Key, Payload, Val};

fn ctx(tech: Technology, subtype: TSub) -> Ctx {
    Ctx {
        origin: Origin::Content(Content::Technology(tech)),
        subtype: Subtype::Technology(subtype),
        optional: false,
        target: None,
        outstanding: SmallList::new(),
        space_battle: false,
        invasion_seq: None,
    }
}

fn owns(s: &CoreState, who: Seat, tech: Technology) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(tech))
}

fn ready_card(s: &CoreState, who: Seat, tech: Technology) -> bool {
    s.players().by_seat(who).is_some_and(|p| p.tech.owned.contains(tech) && !p.tech.exhausted.contains(tech))
}

/// `planets::tech_specialties_now` is not empty: a printed specialty or an attached one.
fn specialty(s: &CoreState, planet: Planet) -> bool {
    !planet.data().specialties.is_empty() || board::attachments(s, planet).as_slice().iter().any(|a| !a.data().specialties.is_empty())
}

/// The planets `who` controls, with their system, in the order of the string's `controlled_planets` (system id, then planet id).
fn controlled(s: &CoreState, who: Seat) -> impl Iterator<Item = (System, Planet)> {
    board::controlled_planets(s, who).as_slice().to_vec().into_iter()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Start,
    Asking,
    Done,
}

// -- Psychoarchaeology ------------------------------------------------------------------------------

/// "Psychoarchaeology: exhaust a specialty for 1 trade good": each ready specialty planet may be converted once; declining
/// preserves it for a later use. `source` is the interned `"psychoarchaeology"`, the source of the staged trade good.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Psychoarchaeology {
    pub who: Seat,
    source: Code,
    stage: Stage,
}

impl Psychoarchaeology {
    #[must_use]
    pub const fn new(who: Seat, source: Code) -> Self {
        Self { who, source, stage: Stage::Start }
    }

    /// Whether the card applies at all (owned: it is never exhausted).
    #[must_use]
    pub fn applies(s: &CoreState, who: Seat) -> bool {
        owns(s, who, Technology::Psychoarchaeology)
    }

    fn candidates(&self, s: &CoreState) -> Vec<(System, Planet)> {
        controlled(s, self.who).filter(|(_, p)| !board::exhausted(s, *p) && specialty(s, *p)).collect()
    }
}

impl Window for Psychoarchaeology {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if Self::applies(sx.state, self.who) && !self.candidates(sx.state).is_empty() { Stage::Asking } else { Stage::Done };
        }
        if self.stage == Stage::Asking { Next::Continue(Ask::Technology(TAsk::Psychoarchaeology)) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::Psychoarchaeology),
            ctx: ctx(Technology::Psychoarchaeology, TSub::PsychoarchaeologyExhaustSpecialty),
            opts: self
                .candidates(s)
                .into_iter()
                .map(|(system, planet)| Opt::Technology(TOpt::Planet(planet, Shown { votes: 0, system: Some(system) })))
                .chain(std::iter::once(Opt::Decline))
                .collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => self.stage = Stage::Done,
            Pick::Opt(Opt::Technology(TOpt::Planet(planet, _))) if self.candidates(sx.state).iter().any(|(_, p)| *p == planet) => {
                fx::exhaust_planet(sx.state, planet);
                fx::gain_trade_goods_staged(sx.state, self.who, 1, self.source);
                // Asked again while a ready specialty is left.
                self.stage = if self.candidates(sx.state).is_empty() { Stage::Done } else { Stage::Asking };
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Production Biomes ----------------------------------------------------------------------------

/// Production Biomes (a component action): "Exhaust this card and spend 1 token from your strategy pool to gain 4 trade goods
/// and choose 1 other player; that player gains 2 trade goods." Nothing is asked (the action does not resolve, 22.3) with no
/// other player or an empty strategy pool; the one question names the other player, and the answer pays everything: the token
/// (announced), the card exhausted, 4 trade goods to the owner and 2 to the player named (each announced). `source` is the
/// interned `"production_biomes"`, the reason of the announced token and the source of the trade goods. [`Self::done`] is
/// whether the action resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionBiomes {
    pub who: Seat,
    source: Code,
    stage: Stage,
    pub done: bool,
}

impl ProductionBiomes {
    #[must_use]
    pub const fn new(who: Seat, source: Code) -> Self {
        Self { who, source, stage: Stage::Start, done: false }
    }

    fn others(&self, s: &CoreState) -> SmallList<Seat, 8> {
        s.players().iter().map(|p| p.seat).filter(|seat| *seat != self.who).collect()
    }

    fn can_pay(&self, s: &CoreState) -> bool {
        s.players().by_seat(self.who).is_some_and(|p| p.econ.strategic_tokens > 0)
    }
}

impl Window for ProductionBiomes {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if self.others(sx.state).as_slice().is_empty() || !self.can_pay(sx.state) { Stage::Done } else { Stage::Asking };
        }
        if self.stage == Stage::Asking { Next::Continue(Ask::Technology(TAsk::ProductionBiomes)) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::ProductionBiomes),
            ctx: ctx(Technology::ProductionBiomes, TSub::ProductionBiomesChoosePlayer),
            opts: self.others(s).as_slice().iter().map(|seat| Opt::Technology(TOpt::Player(*seat))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let Pick::Opt(Opt::Technology(TOpt::Player(chosen))) = pick else { return Err(Illegal) };
        if self.stage != Stage::Asking || !self.others(sx.state).as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        if !fx::spend_strategic_token(sx.state, self.who) {
            return Err(Illegal);
        }
        // `supply::note_strategy_token_spent(.., "production_biomes")`.
        fx::stage(
            sx.state,
            EventRec {
                id: 0,
                kind: EventKind::StrategyTokenSpent,
                payload: Payload::new().with(Key::Player, Val::Seat(self.who)).with(Key::Reason, Val::Code(self.source)),
                cancelled: false,
            },
        );
        fx_strategy::exhaust_technology(sx.state, self.who, Technology::ProductionBiomes);
        fx::gain_trade_goods_staged(sx.state, self.who, 4, self.source);
        fx::gain_trade_goods_staged(sx.state, chosen, 2, self.source);
        self.done = true;
        self.stage = Stage::Done;
        Ok(())
    }
}

// -- Vortex ---------------------------------------------------------------------------------------

/// Vortex (Cabal technology, a component action): "ACTION: Exhaust this card to capture 1 non-structure unit type of another player
/// from their reinforcements, if that player has a unit in a system adjacent to one of your space docks." The unit types offered are
/// the non-structure types of the other seats (not the neutral owner) with a unit in a system next to a dock of the seat's, that still
/// have a model in the box, ordered by seat and unit type; nothing is asked with one, nothing happens without a map or a type. The
/// answer captures (the seat's sheet holds the model) and exhausts the card. [`Self::done`] is whether the action resolved. The
/// neighbours are the map's plain adjacency, the seat's own card is the caller's check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vortex {
    pub who: Seat,
    pub has_map: bool,
    neutral: Option<Seat>,
    scope: crate::tables::Scope,
    stage: Stage,
    pub done: bool,
}

impl Vortex {
    #[must_use]
    pub const fn new(who: Seat, has_map: bool, neutral: Option<Seat>, scope: crate::tables::Scope) -> Self {
        Self { who, has_map, neutral, scope, stage: Stage::Start, done: false }
    }

    const ASK: Ask = Ask::Technology(TAsk::Vortex);

    fn targets(&self, s: &CoreState) -> Vec<(Seat, Unit)> {
        use crate::generated::UnitBaseType;
        if !self.has_map {
            return Vec::new();
        }
        let is_dock = |p: &crate::state::Piece| p.owner() == self.who && self.scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Spacedock);
        let mut docks: Vec<System> = s.board().systems().filter(|(_, c)| c.units.as_slice().iter().any(is_dock)).map(|(sys, _)| sys).collect();
        docks.extend(
            s.board().planets().filter(|(_, c)| c.units.as_slice().iter().any(is_dock)).filter_map(|(planet, c)| c.placed_in.or(planet.data().system)),
        );
        let mut found: Vec<(Seat, Unit)> = Vec::new();
        for dock in docks {
            let Some(near) = board::neighbours(s, dock) else { continue };
            for index in near.iter() {
                let Some(next) = System::ALL.get(index).copied() else { continue };
                let pieces = s.board().sys(next).units.as_slice().iter().chain(board::planets_of(s, next).flat_map(|p| s.board().planet(p).units.as_slice().iter()));
                for piece in pieces {
                    let (owner, kind) = (piece.owner(), piece.kind());
                    if owner == self.who || Some(owner) == self.neutral {
                        continue;
                    }
                    if self.scope.unit(kind).is_none_or(|d| d.structure) || crate::query::fleet::remaining_all(s, self.scope, owner, kind) == 0 {
                        continue;
                    }
                    if !found.contains(&(owner, kind)) {
                        found.push((owner, kind));
                    }
                }
            }
        }
        found.sort_unstable();
        found
    }

    fn capture(&mut self, s: &mut CoreState, owner: Seat, unit: Unit) {
        if let Some(p) = s.players_mut().by_seat_mut(self.who) {
            p.misc.captured_units.push((owner, unit));
        }
        fx_strategy::exhaust_technology(s, self.who, Technology::Vortex);
        self.done = true;
        self.stage = Stage::Done;
    }
}

impl Window for Vortex {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            match self.targets(sx.state).as_slice() {
                [] => self.stage = Stage::Done,
                [(owner, unit)] => {
                    let (owner, unit) = (*owner, *unit);
                    self.capture(sx.state, owner, unit);
                }
                _ => self.stage = Stage::Asking,
            }
        }
        if self.stage == Stage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Self::ASK,
            ctx: Ctx {
                origin: Origin::FactionAbility(crate::choice::generic::Content::Technology(Technology::Vortex)),
                subtype: Subtype::Technology(TSub::VortexTarget),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: self.targets(s).into_iter().map(|(owner, unit)| Opt::Technology(TOpt::Capture(owner, unit))).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match pick {
            Pick::Opt(Opt::Technology(TOpt::Capture(owner, unit))) if self.stage == Stage::Asking && self.targets(sx.state).contains(&(owner, unit)) => {
                self.capture(sx.state, owner, unit);
                Ok(())
            }
            _ => Err(Illegal),
        }
    }
}

// -- Wormhole Generator -----------------------------------------------------------------------------

/// Wormhole Generator (Ghost technology, a component action): "ACTION: Exhaust this card to place or move a Creuss wormhole token into
/// either a system that contains a planet you control or a non-home system that does not contain another player's ships." The
/// destinations (each token into each such system it is not already in, tokens in the order of their names and systems in map order) are
/// the position's: `destinations`, the adapter's read of the map (the placed systems in map order, the printed home systems, the
/// units of other seats). The card must be ready, the question may be declined; the answer sets the token (a token already on the map
/// moves) and exhausts the card. [`Self::done`] is whether the action resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WormholeGenerator {
    pub who: Seat,
    destinations: Vec<(crate::generated::Wormhole, System)>,
    stage: Stage,
    pub done: bool,
}

impl WormholeGenerator {
    #[must_use]
    pub fn new(who: Seat, destinations: Vec<(crate::generated::Wormhole, System)>) -> Self {
        Self { who, destinations, stage: Stage::Start, done: false }
    }

    const ASK: Ask = Ask::Technology(TAsk::WormholeGenerator);
}

impl Window for WormholeGenerator {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if ready_card(sx.state, self.who, Technology::WormholeGenerator) && !self.destinations.is_empty() { Stage::Asking } else { Stage::Done };
        }
        if self.stage == Stage::Asking { Next::Continue(Self::ASK) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| {
            let mut opts: Opts = self.destinations.iter().map(|(kind, system)| Opt::Technology(TOpt::Token(*kind, *system))).collect();
            opts.push(Opt::Decline);
            Choice {
                actor: self.who,
                ask: Self::ASK,
                ctx: Ctx {
                    origin: Origin::FactionAbility(crate::choice::generic::Content::Technology(Technology::WormholeGenerator)),
                    subtype: Subtype::Technology(TSub::CreussToken),
                    optional: false,
                    target: None,
                    outstanding: SmallList::new(),
                    space_battle: false,
                    invasion_seq: None,
                },
                opts,
            }
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => {}
            Pick::Opt(Opt::Technology(TOpt::Token(kind, system))) if self.destinations.contains(&(kind, system)) => {
                crate::kernel::fx_explore::place_wormhole_token(sx.state, kind, system);
                fx_strategy::exhaust_technology(sx.state, self.who, Technology::WormholeGenerator);
                self.done = true;
            }
            _ => return Err(Illegal),
        }
        self.stage = Stage::Done;
        Ok(())
    }
}

// -- the end of a turn: Predictive Intelligence, then Bio-Stims -----------------------------------------

/// The suspension points of [`TurnEnd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EStage {
    Start,
    /// Waiting on a token to move (Predictive Intelligence).
    Restack,
    /// Waiting on what to ready (Bio-Stims).
    Ready,
    Done,
}

/// The free windows at the end of an action-phase turn: Predictive Intelligence (move tokens between pools, once the card is
/// ready; exhausted when anything moved), then Bio-Stims (ready a specialty planet or a technology; exhausts itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnEnd {
    pub who: Seat,
    stage: EStage,
    /// Tokens the owner may still move (the total held when the window began, less those moved).
    left: i32,
    changed: bool,
}

const POOLS: [Pool; 3] = [Pool::Tactic, Pool::Fleet, Pool::Strategic];

fn held(s: &CoreState, who: Seat, pool: Pool) -> i16 {
    s.players().by_seat(who).map_or(0, |p| match pool {
        Pool::Tactic => p.econ.tactic_tokens,
        Pool::Fleet => p.econ.fleet_tokens,
        Pool::Strategic => p.econ.strategic_tokens,
    })
}

fn pool_mut(s: &mut CoreState, who: Seat, pool: Pool) -> Option<&mut i16> {
    s.players_mut().by_seat_mut(who).map(|p| match pool {
        Pool::Tactic => &mut p.econ.tactic_tokens,
        Pool::Fleet => &mut p.econ.fleet_tokens,
        Pool::Strategic => &mut p.econ.strategic_tokens,
    })
}

impl TurnEnd {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: EStage::Start, left: 0, changed: false }
    }

    /// Whether either card applies (a ready Predictive Intelligence or Bio-Stims).
    #[must_use]
    pub fn applies(s: &CoreState, who: Seat) -> bool {
        ready_card(s, who, Technology::PredictiveIntelligence) || ready_card(s, who, Technology::BioStims)
    }

    fn moves(&self, s: &CoreState) -> Vec<(Pool, Pool)> {
        let mut out = Vec::new();
        for from in POOLS {
            if held(s, self.who, from) <= 0 {
                continue;
            }
            for to in POOLS {
                if from != to {
                    out.push((from, to));
                }
            }
        }
        out
    }

    /// The Bio-Stims options: exhausted specialty planets of the owner, then exhausted technologies other than Bio-Stims.
    fn readyable(&self, s: &CoreState) -> Vec<TOpt> {
        let planets = controlled(s, self.who)
            .filter(|(_, p)| board::exhausted(s, *p) && specialty(s, *p))
            .map(|(system, p)| TOpt::Planet(p, Shown { votes: 0, system: Some(system) }));
        let techs = s
            .players()
            .by_seat(self.who)
            .into_iter()
            .flat_map(|p| p.tech.exhausted.iter())
            .filter(|t| *t != Technology::BioStims)
            .map(TOpt::ReadyTechnology);
        planets.chain(techs).collect()
    }

    /// Move on from the Predictive Intelligence part (settled: the card is exhausted when anything moved) to Bio-Stims.
    fn to_bio_stims(&mut self, s: &mut CoreState) {
        if self.changed {
            fx_exhaust(s, self.who, Technology::PredictiveIntelligence);
            self.changed = false;
        }
        self.stage = if ready_card(s, self.who, Technology::BioStims) && !self.readyable(s).is_empty() { EStage::Ready } else { EStage::Done };
    }
}

fn fx_exhaust(s: &mut CoreState, who: Seat, tech: Technology) {
    crate::kernel::fx_strategy::exhaust_technology(s, who, tech);
}

impl Window for TurnEnd {
    type Stage = EStage;

    fn stage(&self) -> EStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == EStage::Start {
            if ready_card(sx.state, self.who, Technology::PredictiveIntelligence) {
                self.left = POOLS.iter().map(|p| i32::from(held(sx.state, self.who, *p))).sum();
                if self.left > 0 && !self.moves(sx.state).is_empty() {
                    self.stage = EStage::Restack;
                } else {
                    self.to_bio_stims(sx.state);
                }
            } else {
                self.to_bio_stims(sx.state);
            }
        }
        match self.stage {
            EStage::Restack => Next::Continue(Ask::Technology(TAsk::Restack)),
            EStage::Ready => Next::Continue(Ask::Technology(TAsk::BioStims)),
            EStage::Start | EStage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        match self.stage {
            EStage::Restack => Some(Choice {
                actor: self.who,
                ask: Ask::Technology(TAsk::Restack),
                ctx: ctx(Technology::PredictiveIntelligence, TSub::PredictiveIntelligenceRedistribute),
                opts: self.moves(s).into_iter().map(|(from, to)| Opt::Technology(TOpt::Move { from, to })).chain(std::iter::once(Opt::Done)).collect::<Opts>(),
            }),
            EStage::Ready => Some(Choice {
                actor: self.who,
                ask: Ask::Technology(TAsk::BioStims),
                ctx: ctx(Technology::BioStims, TSub::BioStimsReady),
                opts: self.readyable(s).into_iter().map(Opt::Technology).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
            }),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match (self.stage, pick) {
            (EStage::Restack, Pick::Opt(Opt::Done)) => self.to_bio_stims(sx.state),
            (EStage::Restack, Pick::Opt(Opt::Technology(TOpt::Move { from, to }))) if self.moves(sx.state).contains(&(from, to)) => {
                if let Some(n) = pool_mut(sx.state, self.who, from) {
                    *n -= 1;
                }
                if let Some(n) = pool_mut(sx.state, self.who, to) {
                    *n += 1;
                }
                self.changed = true;
                self.left -= 1;
                if self.left <= 0 || self.moves(sx.state).is_empty() {
                    self.to_bio_stims(sx.state);
                }
            }
            (EStage::Ready, Pick::Opt(Opt::Decline)) => self.stage = EStage::Done,
            (EStage::Ready, Pick::Opt(Opt::Technology(done))) if self.readyable(sx.state).contains(&done) => {
                match done {
                    TOpt::Planet(planet, _) => {
                        fx::ready_planet(sx.state, planet);
                    }
                    TOpt::ReadyTechnology(tech) => {
                        if let Some(p) = sx.state.players_mut().by_seat_mut(self.who) {
                            p.tech.exhausted.remove(tech);
                        }
                    }
                    TOpt::Move { .. } | TOpt::Exhaust | TOpt::Transit { .. } | TOpt::Player(_) | TOpt::Capture(..) | TOpt::Token(..) => return Err(Illegal),
                }
                fx_exhaust(sx.state, self.who, Technology::BioStims);
                self.stage = EStage::Done;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

// -- Sarween Tools and AI Development Algorithm: "when 1 or more of your units use PRODUCTION" ----------------------------

/// The suspension points of [`ProductionUsed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PStage {
    Start,
    Asking,
    Done,
}

/// `technology::production_used`: Sarween Tools flat 1 while owned; AI Development Algorithm asks to exhaust for a discount of
/// the unit upgrades owned (a "may": it forecloses its other ability). The result lands in `transient.production.discount_remaining`,
/// written, not added: a value left from an earlier use must not leak into this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductionUsed {
    pub who: Seat,
    stage: PStage,
    discount: i32,
    upgrades: u8,
}

impl ProductionUsed {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: PStage::Start, discount: 0, upgrades: 0 }
    }

    fn settle(&mut self, s: &mut CoreState) {
        s.transient_mut().production.discount_remaining = self.discount;
        self.stage = PStage::Done;
    }
}

impl Window for ProductionUsed {
    type Stage = PStage;

    fn stage(&self) -> PStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == PStage::Start {
            self.discount = i32::from(owns(sx.state, self.who, Technology::SarweenTools));
            let upgrades = sx
                .state
                .players()
                .by_seat(self.who)
                .map_or(0, |p| p.tech.owned.iter().filter(|t| crate::query::tech::is_unit_upgrade(*t)).count());
            if ready_card(sx.state, self.who, Technology::AIDevelopmentAlgorithm) && upgrades > 0 {
                self.upgrades = u8::try_from(upgrades).unwrap_or(u8::MAX);
                self.stage = PStage::Asking;
            } else {
                self.settle(sx.state);
            }
        }
        if self.stage == PStage::Asking { Next::Continue(Ask::Technology(TAsk::Aida { upgrades: self.upgrades })) } else { Next::Done }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == PStage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::Aida { upgrades: self.upgrades }),
            ctx: ctx(Technology::AIDevelopmentAlgorithm, TSub::AidaExhaustForProductionDiscount),
            opts: [Opt::Technology(TOpt::Exhaust), Opt::Decline].into_iter().collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != PStage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Technology(TOpt::Exhaust)) => {
                fx_exhaust(sx.state, self.who, Technology::AIDevelopmentAlgorithm);
                self.discount += i32::from(self.upgrades);
            }
            Pick::Opt(Opt::Decline) => {}
            _ => return Err(Illegal),
        }
        self.settle(sx.state);
        Ok(())
    }
}

// -- Transit Diodes and the start of a turn ----------------------------------------------------------------------------------

/// "Transit Diodes: redeploy ground forces": up to four ground forces of the seat, one per answer, each to a planet it controls (a
/// planet that received one this window cannot send one out). The card is exhausted when anything moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitDiodes {
    pub who: Seat,
    stage: Stage,
    moved: u8,
    arrivals: SmallList<(System, Planet), 4>,
}

const TRANSIT_MOVES: u8 = 4;

impl TransitDiodes {
    #[must_use]
    pub const fn new(who: Seat) -> Self {
        Self { who, stage: Stage::Start, moved: 0, arrivals: SmallList::new() }
    }

    /// Whether the card applies (owned and ready).
    #[must_use]
    pub fn applies(s: &CoreState, who: Seat) -> bool {
        ready_card(s, who, Technology::TransitDiodes)
    }

    /// `technology::transit_options`: every ground force that may move and every planet it may move to, in the string's order.
    fn options(&self, s: &CoreState) -> Vec<TOpt> {
        use crate::query::fleet::is_ground_force;
        let destinations: Vec<(System, Planet)> = controlled(s, self.who).collect();
        let mut seen: Vec<(System, Option<Planet>, Unit, Planet)> = Vec::new();
        let mut out = Vec::new();
        for (from_system, cell) in s.board().systems() {
            let mut sources: Vec<(Option<Planet>, Unit)> = cell.units.as_slice().iter().filter(|u| u.owner() == self.who).map(|u| (None, u.kind())).collect();
            for planet in board::planets_of(s, from_system) {
                sources.extend(s.board().planet(planet).units.as_slice().iter().filter(|u| u.owner() == self.who).map(|u| (Some(planet), u.kind())));
            }
            for (from, unit) in sources {
                if !is_ground_force(unit) || from.is_some_and(|p| self.arrivals.as_slice().contains(&(from_system, p))) {
                    continue;
                }
                for (to_system, to) in &destinations {
                    if from_system == *to_system && from == Some(*to) {
                        continue;
                    }
                    if seen.contains(&(from_system, from, unit, *to)) {
                        continue;
                    }
                    seen.push((from_system, from, unit, *to));
                    out.push(TOpt::Transit { from_system, from, unit, to_system: *to_system, to: *to });
                }
            }
        }
        out
    }
}

impl Window for TransitDiodes {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.stage = if Self::applies(sx.state, self.who) { Stage::Asking } else { Stage::Done };
        }
        if self.stage == Stage::Asking {
            if self.moved >= TRANSIT_MOVES || self.options(sx.state).is_empty() {
                self.finish(sx.state);
            } else {
                return Next::Continue(Ask::Technology(TAsk::TransitDiodes { left: TRANSIT_MOVES - self.moved }));
            }
        }
        Next::Done
    }

    fn ask_of(&self, s: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.who,
            ask: Ask::Technology(TAsk::TransitDiodes { left: TRANSIT_MOVES - self.moved }),
            ctx: ctx(Technology::TransitDiodes, TSub::TransitDiodesRedeploy),
            opts: self.options(s).into_iter().map(Opt::Technology).chain(std::iter::once(Opt::Decline)).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        match pick {
            Pick::Opt(Opt::Decline) => self.finish(sx.state),
            Pick::Opt(Opt::Technology(done @ TOpt::Transit { from_system, from, unit, to_system, to })) if self.options(sx.state).contains(&done) => {
                let who = self.who;
                let pool = match from {
                    Some(p) => sx.state.board_mut().planet_mut(p).units.as_mut_slice(),
                    None => sx.state.board_mut().sys_mut(from_system).units.as_mut_slice(),
                };
                // `take_transit_unit`: the first ground force of that type the seat has there.
                let Some(at) = pool.iter().position(|u| u.owner() == who && u.kind() == unit && crate::query::fleet::is_ground_force(u.kind())) else {
                    return Err(Illegal);
                };
                let piece = match from {
                    Some(p) => sx.state.board_mut().planet_mut(p).units.remove(at),
                    None => sx.state.board_mut().sys_mut(from_system).units.remove(at),
                };
                let cell = sx.state.board_mut().planet_mut(to);
                cell.units.push(piece);
                cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
                sx.state.board_mut().sys_mut(to_system).flags.set(crate::state::SysFlags::TOUCHED, true);
                self.arrivals.push((to_system, to));
                self.moved += 1;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

impl TransitDiodes {
    fn finish(&mut self, s: &mut CoreState) {
        if self.moved > 0 {
            fx_exhaust(s, self.who, Technology::TransitDiodes);
        }
        self.stage = Stage::Done;
    }
}

/// The free windows at the start of an action-phase turn: Psychoarchaeology, then Transit Diodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnStart {
    psycho: Psychoarchaeology,
    transit: TransitDiodes,
    on_transit: bool,
}

impl TurnStart {
    #[must_use]
    pub const fn new(who: Seat, source: Code) -> Self {
        Self { psycho: Psychoarchaeology::new(who, source), transit: TransitDiodes::new(who), on_transit: false }
    }
}

impl Window for TurnStart {
    type Stage = (bool, Stage);

    fn stage(&self) -> Self::Stage {
        if self.on_transit { (true, self.transit.stage()) } else { (false, self.psycho.stage()) }
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if !self.on_transit {
            match self.psycho.step(sx) {
                Next::Done => self.on_transit = true,
                waiting => return waiting,
            }
        }
        self.transit.step(sx)
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.on_transit { self.transit.ask_of(s, k) } else { self.psycho.ask_of(s, k) }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.on_transit { self.transit.answer(sx, pick) } else { self.psycho.answer(sx, pick) }
    }
}

#[cfg(test)]
mod vortex_tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{Faction, UnitBaseType};
    use crate::kernel::Resolver;
    use crate::kernel::stage::AbilityTable;
    use crate::state::{Piece, Player, Players};
    use crate::tables::Scope;

    const SCOPE: Scope = Scope(u32::MAX);

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..3u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::Vortex);
        s
    }

    fn plain(base: UnitBaseType) -> Unit {
        Unit::ALL.iter().copied().find(|u| u.data().base_type == base && u.data().faction.is_none()).expect("a plain unit")
    }

    /// Four systems with a planet each, in system order.
    fn systems() -> [(System, Planet); 4] {
        let mut found: Vec<(System, Planet)> = Vec::new();
        for planet in Planet::ALL {
            if let Some(system) = planet.data().system
                && !found.iter().any(|(s, _)| *s == system)
            {
                found.push((system, planet));
            }
        }
        found.sort_unstable();
        [found[0], found[1], found[2], found[3]]
    }

    /// d0 - d1 - d2 - d3 in a row; the seat's dock on a planet of d1, so d0 and d2 are next to it and d3 is not.
    fn scene() -> CoreState {
        let mut s = table();
        let sys = systems();
        let mut galaxy = crate::galaxy::CoreGalaxy::new(&[]);
        for (i, (system, _)) in sys.iter().enumerate() {
            galaxy.place(crate::ids::system_at(*system as usize), crate::hex::Hex::new(i as i32, 0)).expect("on the grid");
        }
        s.map_mut().set_galaxy(galaxy);
        s.board_mut().planet_mut(sys[1].1).units.push(Piece::new(plain(UnitBaseType::Spacedock), Seat(0)));
        s
    }

    fn driver(has_map: bool, neutral: Option<Seat>) -> Driver<Vortex> {
        Driver::new(Vortex::new(Seat(0), has_map, neutral, SCOPE), Resolver::new(&[Seat(0), Seat(1), Seat(2)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    #[test]
    fn vortex_offers_the_non_structure_types_next_to_a_dock_in_seat_and_unit_order_and_captures_the_one_chosen() {
        let (k, mut s) = (Kernel::plain(), scene());
        let sys = systems();
        let (cruiser, dread, pds) = (plain(UnitBaseType::Cruiser), plain(UnitBaseType::Dreadnought), plain(UnitBaseType::Pds));
        s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(dread, Seat(2)));
        s.board_mut().sys_mut(sys[2].0).units.push(Piece::new(cruiser, Seat(1)));
        s.board_mut().planet_mut(sys[2].1).units.push(Piece::new(pds, Seat(1)));
        s.board_mut().sys_mut(sys[3].0).units.push(Piece::new(cruiser, Seat(2)));
        s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(cruiser, Seat(0)));
        let mut d = driver(true, None);
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.opts.as_slice(), &[Opt::Technology(TOpt::Capture(Seat(1), cruiser)), Opt::Technology(TOpt::Capture(Seat(2), dread))], "no structure, no far system, nothing of its own");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Technology(TOpt::Capture(Seat(2), cruiser)))), Err(DriverError::Illegal));
        assert!(s.players().by_seat(Seat(0)).unwrap().misc.captured_units.is_empty(), "nothing before a legal answer");
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Technology(TOpt::Capture(Seat(2), dread)))).unwrap(), Status::Done);
        assert!(d.window.done);
        let p = s.players().by_seat(Seat(0)).unwrap();
        assert_eq!(p.misc.captured_units.as_slice(), &[(Seat(2), dread)]);
        assert!(p.tech.exhausted.contains(Technology::Vortex));
    }

    #[test]
    fn vortex_takes_a_lone_target_without_a_question_and_does_nothing_without_a_map_a_target_or_a_box() {
        let (k, sys) = (Kernel::plain(), systems());
        let cruiser = plain(UnitBaseType::Cruiser);
        let mut s = scene();
        // On a planet of the neighbour, not in its space area: it counts as well.
        s.board_mut().planet_mut(sys[0].1).units.push(Piece::new(cruiser, Seat(1)));
        let mut d = driver(true, None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done);
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().misc.captured_units.as_slice(), &[(Seat(1), cruiser)]);
        // The string arm had no map; nobody next to the dock.
        let mut s = scene();
        s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(cruiser, Seat(1)));
        let mut d = driver(false, None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        let mut s = scene();
        let mut d = driver(true, None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        // The neutral owner is not a target.
        let mut s = scene();
        s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(cruiser, Seat(2)));
        let mut d = driver(true, Some(Seat(2)));
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        // Every cruiser of the other seat stands on the board: none is left in its box.
        let mut s = scene();
        for _ in 0..8 {
            s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(cruiser, Seat(1)));
        }
        let mut d = driver(true, None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done, "no model left to capture");
        // A dock in space counts as well.
        let mut s = table();
        let mut galaxy = crate::galaxy::CoreGalaxy::new(&[]);
        for (i, (system, _)) in sys.iter().take(2).enumerate() {
            galaxy.place(crate::ids::system_at(*system as usize), crate::hex::Hex::new(i as i32, 0)).expect("on the grid");
        }
        s.map_mut().set_galaxy(galaxy);
        s.board_mut().sys_mut(sys[0].0).units.push(Piece::new(plain(UnitBaseType::Spacedock), Seat(0)));
        s.board_mut().sys_mut(sys[1].0).units.push(Piece::new(cruiser, Seat(1)));
        let mut d = driver(true, None);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(d.window.done);
    }
}

#[cfg(test)]
mod wormhole_tests {
    use super::*;
    use crate::driver::{Driver, DriverError, Status};
    use crate::generated::{Faction, Wormhole};
    use crate::kernel::Resolver;
    use crate::kernel::stage::AbilityTable;
    use crate::state::{Player, Players};

    fn table() -> CoreState {
        let mut s = CoreState::all_loaded();
        let list = (0..2u8).map(|i| Player::new(Seat(i), Faction::from_index(usize::from(i)).unwrap())).collect();
        s.players = Players::from_vec(list);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(Technology::WormholeGenerator);
        s
    }

    fn dests() -> Vec<(Wormhole, System)> {
        let (a, b) = (System::from_index(5).unwrap(), System::from_index(9).unwrap());
        vec![(Wormhole::Alpha, a), (Wormhole::Alpha, b), (Wormhole::Beta, a), (Wormhole::Beta, b)]
    }

    fn driver(dests: Vec<(Wormhole, System)>) -> Driver<WormholeGenerator> {
        Driver::new(WormholeGenerator::new(Seat(0), dests), Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1)
    }

    fn token(kind: Wormhole, system: System) -> Pick {
        Pick::Opt(Opt::Technology(TOpt::Token(kind, system)))
    }

    #[test]
    fn the_wormhole_generator_offers_the_destinations_and_a_decline_sets_the_token_and_exhausts_the_card() {
        let (k, mut s) = (Kernel::plain(), table());
        let mut d = driver(dests());
        let Status::Asking(q) = d.run(&mut s, &k).unwrap() else { panic!("asks") };
        assert_eq!(q.ask, Ask::Technology(TAsk::WormholeGenerator));
        assert_eq!(q.ctx.subtype, Subtype::Technology(TSub::CreussToken));
        assert_eq!(q.opts.len(), 5, "four destinations and the decline");
        assert_eq!(q.opts.as_slice().last(), Some(&Opt::Decline));
        // A pair that is not a destination is refused and nothing is written.
        assert_eq!(d.resume(&mut s, &k, token(Wormhole::Gamma, System::from_index(5).unwrap())), Err(DriverError::Illegal));
        assert!(s.explore().wormhole_tokens.get(&Wormhole::Alpha).is_none());
        assert_eq!(d.resume(&mut s, &k, token(Wormhole::Alpha, System::from_index(9).unwrap())).unwrap(), Status::Done);
        assert!(d.window.done);
        assert_eq!(s.explore().wormhole_tokens.get(&Wormhole::Alpha), Some(&System::from_index(9).unwrap()));
        assert!(s.players().by_seat(Seat(0)).unwrap().tech.exhausted.contains(Technology::WormholeGenerator));
        // The token moves: the same kind into the other system replaces the first.
        let mut s2 = s.clone();
        s2.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.remove(Technology::WormholeGenerator);
        let mut d = driver(dests());
        d.run(&mut s2, &k).unwrap();
        d.resume(&mut s2, &k, token(Wormhole::Alpha, System::from_index(5).unwrap())).unwrap();
        assert_eq!(s2.explore().wormhole_tokens.get(&Wormhole::Alpha), Some(&System::from_index(5).unwrap()));
    }

    #[test]
    fn the_wormhole_generator_does_nothing_declined_without_a_destination_or_with_a_spent_or_missing_card() {
        let k = Kernel::plain();
        let mut s = table();
        let before = s.clone();
        let mut d = driver(dests());
        d.run(&mut s, &k).unwrap();
        assert_eq!(d.resume(&mut s, &k, Pick::Opt(Opt::Decline)).unwrap(), Status::Done);
        assert!(!d.window.done);
        assert_eq!(s, before, "a decline writes nothing");
        let mut s = table();
        let mut d = driver(Vec::new());
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert!(!d.window.done);
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.exhausted.insert(Technology::WormholeGenerator);
        let mut d = driver(dests());
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        let mut s = table();
        s.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.remove(Technology::WormholeGenerator);
        let mut d = driver(dests());
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
    }
}
