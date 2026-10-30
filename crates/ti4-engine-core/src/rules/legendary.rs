//! The legendary planet abilities (package W16; string `legendary.rs`) as one [`Window`]: the offer of the abilities whose card
//! is ready, one at a time, each exhausted before it resolves, and the question each asks.
//!
//! * At the end of the holder's turn ([`When::EndOfTurn`], `legendary::end_turn`): Hope's End (Imperial Arms Vault), Primor (The
//!   Atrament), Mallice (Exterrix Headquarters), Mirage (Mirage Flight Academy), Emelpar (The Acropolis), Mecatol Rex of Thunder's
//!   Edge (The Galactic Council) and Thunder's Edge itself (Jupiter Brain).
//! * As the holder passes ([`When::Pass`], `legendary::pass`): Faunus (Maxis Central Control), Industrex (Aurex Mechanica) and
//!   Garbozia (Dok 'N Pic's Salvage Yard).
//!
//! What stays the string's (the adapter bails; each has a test in `ti4-engine/src/legendary_core.rs`):
//! * Maxis Central Control, after the planet is named: the control gain (the `PLANET_CONTROL_GAINED` chain) and the exploration of
//!   the planet's trait. The candidates are an input of the window ([`Legendary::with_maxis`]): the core is lent no galaxy, and the
//!   string lists the systems of the map.
//! * The Imperial Arms Vault's draw when a faction hook changes it, and the draw's hand limit when the adapter gave no printed
//!   names (the child reports [`Report::Left`]).
//! * The Galactic Council with a hand of secrets over the limit (the string asks which to return), a Nomad's Artuno readied by The
//!   Acropolis, Jupiter Brain's breakthrough when the planet changes hands (`legendary::control_gained`).

use crate::choice::agenda::Shown;
use crate::choice::generic::Content;
use crate::choice::legendary::{Ask as LAsk, Opt as LOpt, Ready, Subtype as LSub, When};
use crate::choice::{Ask, Choice, Ctx, Opt, Opts, Origin, Pick, Subtype};
use crate::containers::SmallList;
use crate::generated::{ActionCard, Leader, Planet, Requirement, System, Unit, UnitBaseType};
use crate::ids::Seat;
use crate::kernel::children::{ErasedChild, Report, Spawn};
use crate::kernel::stage::{Illegal, Ran, Sx, Window};
use crate::kernel::{fx, fx_strategy, Kernel, Next};
use crate::keys::LedgerReason;
use crate::query::{board, explore};
use crate::state::{Code, CoreState, LeaderStatus, TransientFlags};
use crate::query::tech::UnitRoster;
use crate::tables::Scope;

/// Planets whose ability is used at the end of the holder's turn, in the order the string offers them.
pub const END_OF_TURN: [Planet; 8] =
    [Planet::HopesEnd, Planet::Primor, Planet::Mallice, Planet::LockedMallice, Planet::Mirage, Planet::Emelpar, Planet::MecatolRexTe, Planet::ThundersEdge];

/// Planets whose ability is used when the holder passes, in the order the string offers them.
pub const WHEN_YOU_PASS: [Planet; 3] = [Planet::Faunus, Planet::Industrex, Planet::Garbozia];

/// The suspension points of [`Legendary`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LStage {
    Start,
    /// Waiting on "use a legendary planet ability".
    Offer,
    /// An ability resolved (or had nothing to ask): the offer is made again.
    Resolved,
    Exterrix,
    ArmsVault,
    Place,
    Fighters,
    Acropolis,
    Council,
    Maxis,
    Salvage,
    Aurex,
    /// The Imperial Arms Vault's action card is drawn as a child window (the hand limit may ask).
    Drawing,
    Done,
}

/// One end-of-turn or when-you-pass window of the legendary planets of `who`.
#[derive(Debug, Clone)]
pub struct Legendary {
    pub who: Seat,
    when: When,
    /// The interned text `legendary`, the source of every question and of the trade goods Exterrix gains.
    src: Code,
    scope: Scope,
    roster: &'static UnitRoster,
    /// The unofficial action cards leave the deck before a draw (`deck::drop_unofficial_action_cards`).
    drop_unofficial: bool,
    /// `maxis_candidates`: `(system, planet)` in the order the string lists them.
    maxis: Vec<(System, Planet)>,
    stage: LStage,
    /// The ability being resolved.
    ability: Option<Planet>,
    draw: ErasedChild,
    /// How many abilities were used (declined offers do not count).
    pub used: u8,
    /// The window left a case to the string caller: the planet Maxis Central Control was told to take, or a draw the table could
    /// not judge. The adapter declines the run.
    pub unsupported: bool,
}

impl Legendary {
    #[must_use]
    pub const fn new(who: Seat, when: When, src: Code, scope: Scope, roster: &'static UnitRoster, drop_unofficial: bool) -> Self {
        Self { who, when, src, scope, roster, drop_unofficial, maxis: Vec::new(), stage: LStage::Start, ability: None, draw: ErasedChild::idle(), used: 0, unsupported: false }
    }

    /// The planets Maxis Central Control may take (the string's `maxis_candidates`).
    #[must_use]
    pub fn with_maxis(mut self, candidates: Vec<(System, Planet)>) -> Self {
        self.maxis = candidates;
        self
    }

    /// The abilities whose card is ready, in the order of the offer.
    fn offers(&self, core: &CoreState) -> SmallList<Planet, 8> {
        let table: &[Planet] = match self.when {
            When::EndOfTurn => &END_OF_TURN,
            When::Pass => &WHEN_YOU_PASS,
        };
        table.iter().copied().filter(|p| explore::legendary_available(core, self.who, *p)).collect()
    }

    fn ctx(&self, subtype: LSub) -> Ctx {
        Ctx {
            origin: Origin::Content(Content::Other(self.src)),
            subtype: Subtype::Legendary(subtype),
            optional: false,
            target: None,
            outstanding: SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        }
    }

    fn question(&self, ask: LAsk, subtype: LSub, opts: Opts) -> Choice {
        Choice { actor: self.who, ask: Ask::Legendary(ask), ctx: self.ctx(subtype), opts }
    }

    // -- the plans (what the string asks, and whether it asks) ------------------------------------------------

    /// `supply::allowed` for the seat's own form of `base` and `wanted` of it: the form (`None`: the seat has none), and how many
    /// the placement ban and the reinforcements let it put down.
    fn placeable(&self, core: &CoreState, k: &Kernel, base: UnitBaseType, wanted: usize) -> Option<(Unit, usize)> {
        let unit = fx_strategy::placed_unit(core, k, self.scope, self.roster, self.who, base)?;
        Some((unit, self.allowed_of(core, k, unit, wanted)))
    }

    /// `supply::allowed` for one unit type.
    fn allowed_of(&self, core: &CoreState, k: &Kernel, unit: Unit, wanted: usize) -> usize {
        if k.hooks.economy.effect_placement_forbidden(core, k, self.who, unit) {
            return 0;
        }
        usize::try_from(crate::query::fleet::remaining_all(core, self.scope, self.who, unit)).unwrap_or(0).min(wanted)
    }

    /// `(system, planet)` of every planet the seat controls: the options of "place on".
    fn own_planets(&self, core: &CoreState) -> crate::query::board::Controlled {
        board::controlled_planets(core, self.who)
    }

    /// The systems that hold a ship of the seat (the space area).
    fn ship_systems(&self, core: &CoreState) -> SmallList<System, 24> {
        let mut out = SmallList::new();
        for (system, cell) in core.board().systems() {
            if cell.units.as_slice().iter().any(|u| u.owner() == self.who && self.scope.unit(u.kind()).is_some_and(|d| d.ship)) {
                out.push(system);
            }
        }
        out
    }

    /// Every exhausted card The Acropolis could ready (`readyable`).
    fn readyable(&self, core: &CoreState, excluding: Planet) -> SmallList<(Ready, Shown), 16> {
        let mut out = SmallList::new();
        for (system, planet) in self.own_planets(core).as_slice() {
            if board::exhausted(core, *planet) {
                out.push((Ready::Planet(*planet), Shown { votes: 0, system: Some(*system) }));
            }
        }
        let Some(p) = core.players().by_seat(self.who) else { return out };
        let none = Shown::default();
        for tech in p.tech.exhausted.iter() {
            out.push((Ready::Technology(tech), none));
        }
        for relic in p.cards.relics_exhausted.iter() {
            out.push((Ready::Relic(relic), none));
        }
        for (planet, ()) in p.cards.legendary_exhausted.iter() {
            if *planet != excluding {
                out.push((Ready::Legendary(*planet), none));
            }
        }
        for (leader, status) in p.leaders.status.iter() {
            if *status == LeaderStatus::Exhausted {
                out.push((Ready::Leader(*leader), none));
            }
        }
        out
    }

    /// The ships Aurex Mechanica may place: for each unit upgrade the seat owns, the ship it unlocks.
    fn aurex_ships(&self, core: &CoreState) -> SmallList<Unit, 8> {
        let mut out = SmallList::new();
        let Some(p) = core.players().by_seat(self.who) else { return out };
        for tech in p.tech.owned.iter() {
            if !crate::query::tech::is_unit_upgrade(tech) {
                continue;
            }
            let ship = Unit::ALL.into_iter().find(|u| {
                self.scope.unit(*u).is_some_and(|d| d.ship && matches!(d.required_technology, Requirement::Known(t) if t == tech))
            });
            if let Some(ship) = ship {
                out.push(ship);
            }
        }
        out
    }

    /// The options of the Aurex Mechanica question: each ship the supply allows, in each system with a ship of the seat.
    fn aurex_options(&self, core: &CoreState, k: &Kernel) -> SmallList<(Unit, System), 24> {
        let systems = self.ship_systems(core);
        let mut out = SmallList::new();
        if systems.is_empty() {
            return out;
        }
        for ship in self.aurex_ships(core).as_slice() {
            if self.allowed_of(core, k, *ship, 1) == 0 {
                continue;
            }
            for system in systems.as_slice() {
                out.push((*ship, *system));
            }
        }
        out
    }

    // -- the flow ---------------------------------------------------------------------------------------------

    /// Make the offer (or end, when no card is ready).
    fn offer(&mut self, sx: &mut Sx<'_>) {
        self.ability = None;
        self.stage = if self.offers(sx.state).is_empty() { LStage::Done } else { LStage::Offer };
    }

    /// Resolve the ability of `planet`, its card already exhausted: the stage of its question, or [`LStage::Resolved`] when it asks
    /// nothing now.
    fn begin(&mut self, sx: &mut Sx<'_>, planet: Planet) {
        self.ability = Some(planet);
        self.used = self.used.saturating_add(1);
        let (core, k) = (&mut *sx.state, sx.kernel);
        self.stage = match planet {
            Planet::Mallice | Planet::LockedMallice => LStage::Exterrix,
            Planet::HopesEnd => LStage::ArmsVault,
            Planet::Primor => self.place_stage(core, k, UnitBaseType::Infantry, 2),
            Planet::Mirage => match self.placeable(core, k, UnitBaseType::Fighter, 2) {
                Some((_, placeable)) if placeable > 0 && !self.ship_systems(core).is_empty() => LStage::Fighters,
                _ => LStage::Resolved,
            },
            Planet::Emelpar => {
                if self.readyable(core, planet).is_empty() {
                    LStage::Resolved
                } else {
                    LStage::Acropolis
                }
            }
            Planet::MecatolRexTe => {
                if core.players().by_seat(self.who).is_some_and(|p| !p.cards.secrets.is_empty()) {
                    LStage::Council
                } else {
                    LStage::Resolved
                }
            }
            // "perform another action": the flag the turn reads to keep the turn with this seat.
            Planet::ThundersEdge => {
                core.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION);
                LStage::Resolved
            }
            Planet::Faunus => {
                if self.maxis.is_empty() {
                    LStage::Resolved
                } else {
                    LStage::Maxis
                }
            }
            Planet::Garbozia => {
                if core.cards().discarded_action.is_empty() {
                    LStage::Resolved
                } else {
                    LStage::Salvage
                }
            }
            Planet::Industrex => {
                if self.aurex_options(core, k).is_empty() {
                    LStage::Resolved
                } else {
                    LStage::Aurex
                }
            }
            _ => LStage::Resolved,
        };
    }

    /// `place_on_own_planet`: the question is asked when the seat has a form of `base`, may place at least one and controls a planet.
    fn place_stage(&self, core: &CoreState, k: &Kernel, base: UnitBaseType, count: usize) -> LStage {
        match self.placeable(core, k, base, count) {
            Some((_, placeable)) if placeable > 0 && !self.own_planets(core).is_empty() => LStage::Place,
            _ => LStage::Resolved,
        }
    }

    /// The base type and count `place_on_own_planet` is called with for the ability being resolved.
    fn place_plan(&self) -> (UnitBaseType, usize) {
        if self.ability == Some(Planet::HopesEnd) {
            (UnitBaseType::Mech, 1)
        } else {
            (UnitBaseType::Infantry, 2)
        }
    }
}

impl Window for Legendary {
    type Stage = LStage;

    fn stage(&self) -> LStage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if matches!(self.stage, LStage::Start | LStage::Resolved) {
            self.offer(sx);
        }
        if self.stage == LStage::Drawing {
            match self.draw.step(sx) {
                Some(Ran::Asks(a)) => return Next::Continue(a),
                Some(Ran::Pass(next)) => return next,
                Some(Ran::Done(report)) => {
                    self.unsupported |= report == Report::Left;
                    self.offer(sx);
                }
                None => self.offer(sx),
            }
        }
        if self.unsupported {
            self.stage = LStage::Done;
        }
        match self.stage {
            LStage::Start | LStage::Resolved | LStage::Drawing | LStage::Done => Next::Done,
            _ => match self.ask_of(sx.state, sx.kernel) {
                Some(choice) => Next::Continue(choice.ask),
                None => {
                    self.stage = LStage::Done;
                    Next::Done
                }
            },
        }
    }

    fn ask_of(&self, core: &CoreState, k: &Kernel) -> Option<Choice> {
        match self.stage {
            LStage::Drawing => self.draw.ask_of(core, k),
            LStage::Offer => {
                let offers = self.offers(core);
                let list = offers.as_slice().iter().map(|p| Opt::Legendary(LOpt::Use(*p))).chain(std::iter::once(Opt::Decline));
                let subtype = if self.when == When::EndOfTurn { LSub::EndOfTurn } else { LSub::Pass };
                Some(self.question(LAsk::Offer { when: self.when }, subtype, list.collect()))
            }
            LStage::Exterrix => {
                let commodities = core.players().by_seat(self.who).map_or(0, |p| p.econ.commodities);
                let opts = [Opt::Legendary(LOpt::Goods), Opt::Legendary(LOpt::Convert)].into_iter().collect();
                Some(self.question(LAsk::Exterrix { commodities }, LSub::Exterrix, opts))
            }
            LStage::ArmsVault => {
                let mut opts = Opts::default();
                if !k.hooks.economy.effect_placement_forbidden(core, k, self.who, Unit::FactionlessMech) {
                    opts.push(Opt::Legendary(LOpt::Mech));
                }
                opts.push(Opt::Legendary(LOpt::Card));
                Some(self.question(LAsk::ArmsVault, LSub::ArmsVault, opts))
            }
            LStage::Place => {
                let (base, count) = self.place_plan();
                let (unit, _) = self.placeable(core, k, base, count)?;
                let opts = self.own_planets(core).as_slice().iter().map(|(system, planet)| Opt::Legendary(LOpt::OnPlanet(*planet, Shown { votes: 0, system: Some(*system) }))).collect();
                Some(self.question(LAsk::PlaceOnPlanet { source: self.ability?, unit }, LSub::Place, opts))
            }
            LStage::Fighters => {
                let opts = self.ship_systems(core).as_slice().iter().map(|s| Opt::Legendary(LOpt::InSystem(*s))).collect();
                Some(self.question(LAsk::PlaceFighters, LSub::Place, opts))
            }
            LStage::Acropolis => {
                let opts = self.readyable(core, self.ability?).as_slice().iter().map(|(r, shown)| Opt::Legendary(LOpt::Ready(*r, *shown))).collect();
                Some(self.question(LAsk::Acropolis, LSub::Acropolis, opts))
            }
            LStage::Council => {
                let held = core.players().by_seat(self.who)?.cards.secrets.as_slice();
                let opts = held.iter().map(|s| Opt::Legendary(LOpt::DiscardSecret(*s))).collect();
                Some(self.question(LAsk::Council, LSub::GalacticCouncil, opts))
            }
            LStage::Maxis => {
                let opts = self.maxis.iter().map(|(s, p)| Opt::Legendary(LOpt::Take(*s, *p))).chain(std::iter::once(Opt::Decline)).collect();
                Some(self.question(LAsk::Maxis, LSub::Maxis, opts))
            }
            LStage::Salvage => {
                let opts = core.cards().discarded_action.iter().map(|c| Opt::Legendary(LOpt::Salvage(*c))).chain(std::iter::once(Opt::Decline)).collect();
                Some(self.question(LAsk::Salvage, LSub::Salvage, opts))
            }
            LStage::Aurex => {
                let opts = self.aurex_options(core, k).as_slice().iter().map(|(u, s)| Opt::Legendary(LOpt::Ship(*u, *s))).chain(std::iter::once(Opt::Decline)).collect();
                Some(self.question(LAsk::Aurex, LSub::Aurex, opts))
            }
            LStage::Start | LStage::Resolved | LStage::Done => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == LStage::Drawing {
            return self.draw.answer(sx, pick);
        }
        // An answer is an option of the question the window asks now.
        let offered = self.ask_of(sx.state, sx.kernel).ok_or(Illegal)?;
        let Pick::Opt(chosen) = pick else { return Err(Illegal) };
        if !offered.opts.as_slice().contains(&chosen) {
            return Err(Illegal);
        }
        let who = self.who;
        match (self.stage, chosen) {
            (LStage::Offer, Opt::Decline) => self.stage = LStage::Done,
            (LStage::Offer, Opt::Legendary(LOpt::Use(planet))) => {
                // Exhausted first: an ability that asks a follow-up must not be re-offered inside its own resolution.
                if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
                    p.cards.legendary_exhausted.insert(planet, ());
                }
                self.begin(sx, planet);
            }
            (LStage::Exterrix, Opt::Legendary(option)) => {
                let commodities = sx.state.players().by_seat(who).map_or(0, |p| i32::from(p.econ.commodities));
                let gained = if option == LOpt::Convert {
                    if let Some(p) = sx.state.players_mut().by_seat_mut(who) {
                        p.econ.commodities = 0;
                    }
                    fx::gain_trade_goods(sx.state, who, commodities)
                } else {
                    fx::gain_trade_goods(sx.state, who, 2)
                };
                if gained > 0 {
                    fx::stage(sx.state, fx::trade_goods_gained(who, gained, self.src));
                }
                self.stage = LStage::Resolved;
            }
            (LStage::ArmsVault, Opt::Legendary(LOpt::Mech)) => self.stage = self.place_stage(sx.state, sx.kernel, UnitBaseType::Mech, 1),
            (LStage::ArmsVault, Opt::Legendary(LOpt::Card)) => {
                match sx.table.links.start(&Spawn::DrawCards { who, count: 1, drop_unofficial: self.drop_unofficial }) {
                    Some(window) => {
                        self.draw.start(window);
                        self.stage = LStage::Drawing;
                    }
                    None => {
                        self.unsupported = true;
                        self.stage = LStage::Done;
                    }
                }
            }
            (LStage::Place, Opt::Legendary(LOpt::OnPlanet(planet, _))) => {
                let (base, count) = self.place_plan();
                let (unit, placeable) = self.placeable(sx.state, sx.kernel, base, count).ok_or(Illegal)?;
                let system = self.own_planets(sx.state).as_slice().iter().find(|(_, p)| *p == planet).map(|(s, _)| *s).ok_or(Illegal)?;
                for _ in 0..placeable {
                    fx::place_unit(sx.state, who, system, Some(planet), unit);
                }
                fx::stage_naaz_mech_placed(sx.state, who, system, unit);
                self.stage = LStage::Resolved;
            }
            (LStage::Fighters, Opt::Legendary(LOpt::InSystem(system))) => {
                let (unit, placeable) = self.placeable(sx.state, sx.kernel, UnitBaseType::Fighter, 2).ok_or(Illegal)?;
                for _ in 0..placeable {
                    fx::place_unit(sx.state, who, system, None, unit);
                }
                self.stage = LStage::Resolved;
            }
            (LStage::Acropolis, Opt::Legendary(LOpt::Ready(what, _))) => {
                ready(sx.state, who, what);
                self.stage = LStage::Resolved;
            }
            (LStage::Council, Opt::Legendary(LOpt::DiscardSecret(secret))) => {
                // Back to the deck before the draw, so a one-card deck still yields a card.
                fx::return_secret(sx.state, who, secret);
                let _ = fx::draw_secret(sx.state, who);
                self.stage = LStage::Resolved;
            }
            (LStage::Maxis, Opt::Decline) => self.stage = LStage::Resolved,
            (LStage::Maxis, Opt::Legendary(LOpt::Take(..))) => {
                // The control gain chain and the exploration after it are the string's.
                self.unsupported = true;
                self.stage = LStage::Done;
            }
            (LStage::Salvage, Opt::Decline) => self.stage = LStage::Resolved,
            (LStage::Salvage, Opt::Legendary(LOpt::Salvage(card))) => {
                salvage(sx.state, card);
                self.stage = LStage::Resolved;
            }
            (LStage::Aurex, Opt::Decline) => self.stage = LStage::Resolved,
            (LStage::Aurex, Opt::Legendary(LOpt::Ship(unit, system))) => {
                fx::place_unit(sx.state, who, system, None, unit);
                self.stage = LStage::Resolved;
            }
            _ => return Err(Illegal),
        }
        Ok(())
    }
}

/// `legendary::settle_control_points`: A Song Like Marrow attaches a victory point to Styx, "when you gain this card, gain 1 victory
/// point; when you lose this card, lose 1". The point follows control, reconciled from the board: the meta records who the point
/// was last settled with, and anything else is a difference to pay out (a gain stops at the victory target, a loss at zero).
pub fn settle_styx(core: &mut CoreState) {
    let holder = board::control(core, Planet::Styx);
    let previous = core.meta().styx_holder;
    if holder == previous {
        return;
    }
    if let Some(previous) = previous {
        fx::adjust_victory_points(core, previous, -1, LedgerReason::Styx);
    }
    if let Some(gained) = holder {
        fx::adjust_victory_points(core, gained, 1, LedgerReason::Styx);
    }
    core.meta_mut().styx_holder = holder;
}

/// `legendary::purge_salvaged`: playing a card out of Dok 'N Pic's Salvage Yard purges it off the yard (its first occurrence).
/// `false`, nothing changed, if the seat could not have played it (it does not hold Garbozia, or the yard lacks the card).
pub fn purge_salvaged(core: &mut CoreState, who: Seat, card: ActionCard) -> bool {
    if !explore::salvaged(core, who).contains(&card) {
        return false;
    }
    // Purged, not discarded: it must not come back round through the discard pile it came from.
    let yard = core.cards_mut().salvage_yard.edit();
    match yard.iter().position(|held| *held == card) {
        Some(at) => {
            yard.remove(at);
            true
        }
        None => false,
    }
}

/// The Acropolis readies `what`.
fn ready(core: &mut CoreState, who: Seat, what: Ready) {
    match what {
        Ready::Planet(planet) => {
            fx::ready_planet(core, planet);
        }
        Ready::Technology(tech) => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.tech.exhausted.remove(tech);
            }
        }
        Ready::Relic(relic) => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.cards.relics_exhausted.remove(relic);
            }
        }
        Ready::Legendary(planet) => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.cards.legendary_exhausted.remove(&planet);
            }
        }
        Ready::Leader(leader) => ready_leader(core, who, leader),
    }
}

/// `leaders::ready`: an exhausted leader is readied.
fn ready_leader(core: &mut CoreState, who: Seat, leader: Leader) {
    let Some(p) = core.players_mut().by_seat_mut(who) else { return };
    if p.leaders.status.get(&leader) == Some(&LeaderStatus::Exhausted) {
        p.leaders.status.insert(leader, LeaderStatus::Readied);
    }
}

/// Dok 'N Pic's Salvage Yard: the card leaves the discard pile (its first occurrence) for the yard.
fn salvage(core: &mut CoreState, card: ActionCard) {
    let pile = core.cards_mut().discarded_action.edit();
    let Some(at) = pile.iter().position(|held| *held == card) else { return };
    pile.remove(at);
    core.cards_mut().salvage_yard.edit().push(card);
}

#[cfg(test)]
mod styx_tests {
    use super::*;

    fn with_styx(holder: Option<Seat>) -> CoreState {
        use crate::generated::Faction;
        use crate::state::{Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::ALL[0]), Player::new(Seat(1), Faction::ALL[1])]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.victory_points = 2;
        s.board_mut().planet_mut(Planet::Styx).control = holder;
        s
    }

    #[test]
    fn the_point_follows_control_of_styx() {
        let mut s = with_styx(Some(Seat(0)));
        settle_styx(&mut s);
        let vp = |s: &CoreState, seat: u8| s.players().by_seat(Seat(seat)).map_or(-1, |p| p.econ.victory_points);
        let (a0, b0) = (vp(&s, 0), vp(&s, 1));
        assert_eq!(s.meta().styx_holder, Some(Seat(0)));
        // settled once: calling it again changes nothing
        settle_styx(&mut s);
        assert_eq!((vp(&s, 0), vp(&s, 1)), (a0, b0));
        // taken off the first seat: it loses the point and the second gains it
        s.board_mut().planet_mut(Planet::Styx).control = Some(Seat(1));
        settle_styx(&mut s);
        assert_eq!((vp(&s, 0), vp(&s, 1)), (a0 - 1, b0 + 1));
        assert_eq!(s.meta().styx_holder, Some(Seat(1)));
        assert_eq!(s.ledger().vp.len(), 3, "gain, loss, gain");
    }

    #[test]
    fn a_planet_nobody_holds_pays_nothing() {
        let mut s = with_styx(None);
        settle_styx(&mut s);
        assert!(s.ledger().vp.is_empty() && s.meta().styx_holder.is_none());
    }
}
