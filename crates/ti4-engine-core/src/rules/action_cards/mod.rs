//! Action cards (package B6; LRR 2, 22): the reads of `action_cards::{hand_limit, is_component_action,
//! is_playable}` that the core can answer from its own state. This is slice 1 of the package: reads only.
//! What needs another area's string rules (payment, research, the unit catalogue, neighbours) is not
//! asked here and the string function stands; each such place is a documented bail (see
//! [`has_legal_target`]). No `Env` trait: the faction answers are plain arguments.

pub mod announce;
pub mod effects;
pub mod reaction_slot;

use crate::generated::{ActionCard, System, Unit, UnitBaseType};
use crate::query::board;
use crate::state::Piece;
use crate::tables::Scope;
use crate::ids::Seat;
use crate::query::laws;
use crate::state::CoreState;

/// `action_cards::HAND_LIMIT`: the printed limit (LRR 2.4).
pub const HAND_LIMIT: usize = 7;

/// `action_cards::is_component_action`: the printed window is "Action".
#[must_use]
pub fn is_component_action(card: ActionCard) -> bool {
    card.data().component_action
}

/// `action_cards::hand_limit`: the law cap of Sanctions, then the faction override (`faction`, the
/// string module's hook, applied last as it overrides the law).
#[must_use]
pub fn hand_limit(core: &CoreState, blanked: bool, faction: impl FnOnce(usize) -> usize) -> usize {
    faction(laws::action_card_limit(core, blanked, HAND_LIMIT))
}

/// The seat-independent gates at the head of `action_cards::is_playable`: Political Censure's owner, the
/// faction bar and Political Secret's target cannot play action cards. `false` bars the play.
#[must_use]
pub fn may_play(core: &CoreState, blanked: bool, who: Seat, faction_forbids: bool, political_secret_bars: bool) -> bool {
    !(laws::censure_bars(core, blanked, who) || faction_forbids || political_secret_bars)
}

/// `action_cards::has_legal_target` for the cards whose target list is a plain read of the core:
/// `Some(answer)`, or `None` for the arms that need string-only rules (the bail: the string function
/// answers). A card without a requirement answers `Some(true)`.
///
/// Bails (`None`): Seize Artifact (neighbours).
#[must_use]
pub fn has_legal_target(core: &CoreState, blanked: bool, scope: Scope, who: Seat, card: ActionCard) -> Option<bool> {
    let rival = |keep: &dyn Fn(&crate::state::Player) -> bool| core.players().iter().any(|p| p.seat != who && keep(p));
    let base = |u: Unit| scope.unit(u).map(|d| d.base_type);
    // Every planet with the system the string board files it under.
    let planets = || {
        core.board().planets().filter_map(|(planet, cell)| cell.placed_in.or(planet.data().system).map(|s| (s, planet, cell)))
    };
    use ActionCard as C;
    match card {
        C::Insubordination => Some(rival(&|p| p.econ.tactic_tokens > 0)),
        C::Spy => Some(rival(&|p| !p.cards.hand.is_empty())),
        C::RepealLaw => Some(!laws::in_play(core, blanked).is_empty()),
        C::UnexpectedAction => Some(!crate::query::board::systems_with_token(core, who).is_empty()),
        C::Plague => Some(planets().any(|(_, _, cell)| cell.control.is_some_and(|owner| owner != who))),
        C::CrippleDefenses => {
            Some(planets().any(|(_, _, cell)| cell.units.as_slice().iter().any(|p| base(p.kind()) == Some(UnitBaseType::Pds))))
        }
        C::ReactorMeltdown => Some(planets().any(|(system, _, cell)| {
            cell.units.as_slice().iter().any(|p| base(p.kind()) == Some(UnitBaseType::Spacedock)) && !is_home_system(scope, system)
        })),
        C::LuckyShot => Some(System::ALL.iter().any(|&system| {
            board::controls_a_planet_in(core, system, who)
                && board::space_units(core, system).iter().any(|p| {
                    p.owner() != who
                        && matches!(base(p.kind()), Some(UnitBaseType::Dreadnought | UnitBaseType::Cruiser | UnitBaseType::Destroyer))
                })
        })),
        C::TacticalBombardment => Some(System::ALL.iter().any(|&system| {
            board::has_space_units_of(core, system, who)
                && board::planets_of(core, system).any(|planet| board::control(core, planet).is_some_and(|owner| owner != who))
        })),
        C::Scuttle => Some(System::ALL.iter().any(|&system| {
            board::space_units(core, system)
                .iter()
                .any(|p| p.owner() == who && scope.unit(p.kind()).is_none_or(|d| d.ship && d.base_type != UnitBaseType::Fighter))
        })),
        C::Uprising => Some(planets().any(|(system, _, cell)| cell.control.is_some_and(|owner| owner != who) && !is_home_system(scope, system))),
        C::RiseOfAMessiah | C::FrontlineDeployment => Some(!board::controlled_planets(core, who).is_empty()),
        C::WarEffort => Some(System::ALL.iter().any(|&system| {
            board::space_units(core, system).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ship))
        })),
        C::UnstablePlanet => Some(core.board().planets().any(|(planet, cell)| {
            let data = planet.data();
            cell.flags.has(crate::state::PlanetFlags::UNITS_ENTRY)
                && scope.0 & crate::tables::source_bit(data.source) != 0
                && data.traits.contains(&crate::generated::PlanetTrait::Hazardous)
        })),
        C::SeizeArtifact => None,
        _ => Some(true),
    }
}

/// `galaxy::is_home_system`: the printed system of a homeworld planet in scope.
pub(super) fn is_home_system(scope: Scope, system: System) -> bool {
    crate::generated::Planet::ALL.iter().any(|p| {
        let d = p.data();
        d.homeworld_of.is_some() && d.system == Some(system) && scope.0 & crate::tables::source_bit(d.source) != 0
    })
}

/// What `reaction_can_matter` reads of the window's event: who acted and in which system.
#[derive(Debug, Clone, Copy)]
pub struct Window {
    pub system: System,
    /// The event's `player` as a seat: `None` when the event names nobody (or nobody seated).
    pub actor: Option<Seat>,
}

fn pieces_in(core: &CoreState, system: System) -> impl Iterator<Item = Piece> + '_ {
    let space = board::space_units(core, system).iter().copied();
    let ground = board::planets_of(core, system).flat_map(move |p| board::planet_units(core, p).iter().copied());
    space.chain(ground)
}

/// `action_cards::reaction_can_matter` for a window that names a system, for the cards whose extra
/// requirement is a read of that system's pieces: `Some(answer)`, or `None` (bail: Harness Energy
/// needs the tile catalogue and the rift hook, Infiltrate needs the swap rules, and a card without a
/// requirement is `Some(true)`). The printed-cost check comes first at the call site.
#[must_use]
pub fn reaction_can_matter(core: &CoreState, scope: Scope, who: Seat, card: ActionCard, w: Window) -> Option<bool> {
    use ActionCard as C;
    let is_actor = w.actor == Some(who);
    let base = |u: Unit| scope.unit(u).map(|d| d.base_type);
    let ship = |u: Unit| scope.unit(u).is_some_and(|d| d.ship);
    let fighter = |u: Unit| base(u) == Some(UnitBaseType::Fighter);
    let space = board::space_units(core, w.system);
    let own = || space.iter().filter(|p| p.owner() == who);
    let own_all = || pieces_in(core, w.system).filter(|p| p.owner() == who);
    let has_own_ship = own().any(|p| ship(p.kind()));
    Some(match card {
        C::Bunker => w.actor.is_some() && !is_actor && board::controls_a_planet_in(core, w.system, who),
        C::Blitz => {
            is_actor
                && own().any(|p| {
                    let u = p.kind();
                    ship(u) && !fighter(u) && scope.unit(u).is_some_and(|d| d.bombardment.is_none())
                })
        }
        C::Disable => {
            is_actor
                && pieces_in(core, w.system).any(|p| p.owner() != who && base(p.kind()) == Some(UnitBaseType::Pds))
        }
        C::FighterPrototype => own_all().any(|p| fighter(p.kind())),
        C::Upgrade => own_all().any(|p| base(p.kind()) == Some(UnitBaseType::Cruiser)),
        C::Rally => space.iter().any(|p| p.owner() != who && ship(p.kind())),
        C::Rescue | C::Intercept => !is_actor && has_own_ship,
        C::EmergencyRepairs => pieces_in(core, w.system).any(|p| p.owner() == who && p.sustained()),
        C::HarnessEnergy | C::Infiltrate => return None,
        _ => true,
    })
}

/// `action_cards::cost_is_payable` (rules version 23) for the cards whose bill the core can judge. `goods`
/// is the seat's spendable trade goods, the Keleres agent's commodities included (the string payment
/// rules supply it until they land). `Some(true)` for a card without a printed cost; `None` (bail) for
/// the cards whose bill needs a payment or research rule not yet in the core: Focused Research,
/// Impersonation, Plagiarize, Mercenary Contract, Pirate Fleet and Reveal Prototype.
#[must_use]
pub fn cost_is_payable(core: &CoreState, who: Seat, card: ActionCard, goods: i64) -> Option<bool> {
    use ActionCard as C;
    match card {
        C::FocusedResearch
        | C::Impersonation
        | C::Plagiarize
        | C::MercenaryContract
        | C::PirateFleet
        | C::RevealPrototype => None,
        C::DivertFunding => Some(
            crate::query::tech::owned(core, who)
                .iter()
                .any(|t| !crate::query::tech::is_unit_upgrade(t) && t.data().faction.is_none()),
        ),
        C::Bribery => Some(goods >= 1),
        _ => Some(true),
    }
}

/// `ti4_model::state::RULES_ACTION_CARD_TARGETS`: from this version a component action whose card has no legal
/// target is not offered.
pub const RULES_ACTION_CARD_TARGETS: u32 = 10;
/// `ti4_model::state::RULES_ACTION_CARD_COSTS`: from this version a card that makes its player pay is not offered
/// to a seat that cannot.
pub const RULES_ACTION_CARD_COSTS: u32 = 23;

/// What `action_cards::is_playable` asks of the string side: the faction and law bars of the head gate and the
/// seat's spendable trade goods (read only for the card that pays in them).
pub struct Gates<'a> {
    pub faction_forbids: bool,
    pub political_secret_bars: bool,
    pub goods: &'a dyn Fn() -> i64,
}

/// `action_cards::is_playable`: the head gates, then (by rules version) the printed cost and the legal target.
/// `None` is the documented bail: Signal Jamming (the map's blockade rules) or an answer the core declines
/// (`cost_is_payable`, `has_legal_target`).
#[must_use]
pub fn is_playable(core: &CoreState, blanked: bool, scope: Scope, who: Seat, card: ActionCard, gates: &Gates<'_>) -> Option<bool> {
    if !may_play(core, blanked, who, gates.faction_forbids, gates.political_secret_bars) {
        return Some(false);
    }
    if card == ActionCard::SignalJamming {
        return None;
    }
    if core.meta().rules_at_least(RULES_ACTION_CARD_COSTS) {
        let goods = if card == ActionCard::Bribery { (gates.goods)() } else { 0 };
        if !cost_is_payable(core, who, card, goods)? {
            return Some(false);
        }
    }
    if core.meta().rules_at_least(RULES_ACTION_CARD_TARGETS) {
        return has_legal_target(core, blanked, scope, who, card);
    }
    Some(true)
}

/// The hand positions `action_cards::available_actions` offers as `play|<index>`: the cards with a component
/// action that are playable, in hand order. `None` when any card of the hand needs a string answer.
#[must_use]
pub fn playable_hand(core: &CoreState, blanked: bool, scope: Scope, who: Seat, gates: &Gates<'_>) -> Option<crate::containers::SmallList<u8, 16>> {
    let hand = &core.players().by_seat(who)?.cards.hand;
    let mut out = crate::containers::SmallList::new();
    for (index, card) in hand.as_slice().iter().enumerate() {
        if is_component_action(*card) && is_playable(core, blanked, scope, who, *card, gates)? {
            out.push(u8::try_from(index).ok()?);
        }
    }
    Some(out)
}

/// `action_cards::discard`: remove the card at `index` from the hand; `None` for a seat or position that is not
/// there. (The play itself, `perform`: announce, the reaction windows and the effect's resolution, runs through
/// the string `reactions::announce` until the abilities table is the core's.)
pub fn discard_from_hand(core: &mut CoreState, who: Seat, index: usize) -> Option<ActionCard> {
    let hand = &mut core.players_mut().by_seat_mut(who)?.cards.hand;
    (index < hand.len()).then(|| hand.remove(index))
}

/// The played effects that only set a stamp or a flag (the mutation half of the package, first batch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Simple {
    /// "+1 to each combat roll during this combat round": `combat_bonus_round`.
    MoraleBoost,
    /// "+1 to move value during this tactical action": `move_bonus_activation`.
    FlankSpeed,
    /// "other players cannot use SPACE CANNON against your ships": the activation joins the `SolarFlare` marks.
    SolarFlare,
    /// `STRATEGIC_CANCELLED`.
    Coup,
    /// `SKIP_NEXT_TURN`.
    Crisis,
    /// `ADDITIONAL_ACTION`.
    MasterPlan,
    /// The activation joins the `LostStar` marks.
    LostStar,
    /// "you vote last during this agenda": `hack_votes_last_agenda`.
    HackElection,
    /// `econ.stability`: keep the strategy cards, skip the next draft.
    PoliticalStability,
    /// `rout_round`.
    Rout,
    /// `waylay_barrage_round`.
    Waylay,
    /// The combat round joins the `FighterBonus` marks.
    FighterPrototype,
    /// The activation joins the `BunkerInvasion` marks.
    Bunker,
    /// The activation joins the `WarMachine` marks.
    WarMachine,
    /// The activation joins the `BlitzInvasion` marks.
    Blitz,
    /// `anomalies_ignored_activation`.
    NavSuite,
    /// `BLACK_MARKET`.
    BlackMarket,
    /// Rally: two fleet command tokens, as many as the reinforcements hold.
    Rally,
    /// Maneuvering Jets: cancel 1 hit this combat round.
    ManeuveringJets,
    /// Shields Holding: cancel 2 hits this combat round.
    ShieldsHolding,
    /// Reflective Shielding: stage 2 hits against the producer of the sustained hit.
    Reflective,
    /// Confounding Legal Text: the holder is the elected player.
    Confounding,
    /// Deadly Plot: discard the outcome, exhaust all the holder's planets.
    DeadlyPlot,
    /// Salvage: take the commodities of the combat's other sides.
    Salvage,
    /// Reverse Engineer: take the discarded component-action card.
    ReverseEngineer,
    /// Economic Initiative: ready each cultural planet the holder controls.
    EconomicInitiative,
    /// Distinguished Councilor: 5 extra votes this agenda.
    Councilor,
    /// Emergency Repairs: repair every unit of the holder in the active system.
    EmergencyRepairs,
    /// Disable: the activation joins the `DisableInvasion` marks when an opponent has a PDS in the active system.
    Disable,
}

/// A unit an effect places from the reinforcements: the form the seat places (its upgrades and faction overrides
/// are the string side's answer), `None` when it places nothing, and the ships of that type on the Crimson hero
/// card (counted against the supply).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub unit: Option<Unit>,
    pub on_card: i64,
}

impl Placed {
    pub const NONE: Self = Self { unit: None, on_card: 0 };
}

/// What a [`Simple`] effect reads from outside the core (faction and source answers).
#[derive(Debug, Clone, Copy)]
pub struct Env {
    /// `GameState::tokens_held_by_mahact` of the holder (a faction mark of the string state).
    pub mahact_held: i32,
    /// The sources in play.
    pub scope: Scope,
    /// A law blanks the law-reading answers (`laws::blanked`).
    pub blanked: bool,
    /// The interned `action_card`, the source of the trade goods a card gains.
    pub action_card: crate::state::Code,
    /// The interned `forward_supply_base`.
    pub supply_base: crate::state::Code,
    /// The order of the state's note ids (the string orders a hand by their text; the core has no id text).
    pub note_order: fn(crate::state::NoteKey, crate::state::NoteKey) -> core::cmp::Ordering,
    /// The infantry the seat places (Rise of a Messiah, Frontline Deployment).
    pub infantry: Placed,
    /// The cruiser the seat places (War Effort).
    pub cruiser: Placed,
    /// The fighters on the Crimson hero card (Fighter Conscription places the plain fighter).
    pub fighter: Placed,
    /// The facts the later cards of the package read (B6d): the neutral seat, the unit forms they place, the game's map.
    pub facts: Facts,
}

/// What the cards of package B6d read from outside the core (inputs of the card, like the adjacency of Argent's agent):
/// the neutral owner of the table, the forms the seat places, the systems of the game's map that carry a wormhole.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts {
    /// The seat of the neutral owner (`neutral_units::NEUTRAL`), present in the owner table when the card needs it.
    pub neutral: Option<Seat>,
    /// A readied Keleres agent could be offered before a payment of the holder (the goods window and the payment window ask
    /// the holders first: a window of its own the card does not carry).
    pub agent_offered: bool,
}

impl Simple {
    /// The effect a card plays, `None` for a card whose effect is not one of these (the bail: every other
    /// card keeps its string effect).
    #[must_use]
    pub const fn of(card: ActionCard) -> Option<Self> {
        use ActionCard as C;
        Some(match card {
            C::MoraleBoost1 | C::MoraleBoost2 | C::MoraleBoost3 | C::MoraleBoost4 => Self::MoraleBoost,
            C::FlankSpeed1 | C::FlankSpeed2 | C::FlankSpeed3 | C::FlankSpeed4 => Self::FlankSpeed,
            C::SolarFlare => Self::SolarFlare,
            C::CoupDEtat => Self::Coup,
            C::Crisis => Self::Crisis,
            C::MasterPlan => Self::MasterPlan,
            C::LostStarChart => Self::LostStar,
            C::HackElection => Self::HackElection,
            C::PoliticalStability => Self::PoliticalStability,
            C::Rout => Self::Rout,
            C::Waylay => Self::Waylay,
            C::FighterPrototype => Self::FighterPrototype,
            C::Bunker => Self::Bunker,
            C::WarMachine1 | C::WarMachine2 | C::WarMachine3 | C::WarMachine4 => Self::WarMachine,
            C::Blitz => Self::Blitz,
            C::NavSuite => Self::NavSuite,
            C::BlackMarketDealings => Self::BlackMarket,
            C::Rally => Self::Rally,
            C::ManeuveringJets1 | C::ManeuveringJets2 | C::ManeuveringJets3 | C::ManeuveringJets4 => Self::ManeuveringJets,
            C::ShieldsHolding1 | C::ShieldsHolding2 | C::ShieldsHolding3 | C::ShieldsHolding4 => Self::ShieldsHolding,
            C::ReflectiveShielding => Self::Reflective,
            C::ConfoundingLegalText => Self::Confounding,
            C::DeadlyPlot => Self::DeadlyPlot,
            C::Salvage => Self::Salvage,
            C::ReverseEngineer => Self::ReverseEngineer,
            C::EconomicInitiative => Self::EconomicInitiative,
            C::Disable => Self::Disable,
            C::DistinguishedCouncilor => Self::Councilor,
            C::EmergencyRepairs => Self::EmergencyRepairs,
            _ => return None,
        })
    }
}

/// Play a [`Simple`] effect for `who`.
pub fn play_simple(core: &mut CoreState, who: Seat, effect: Simple, env: Env) {
    use crate::containers::Opt32;
    use crate::state::{RoundKind, TransientFlags};
    let (round, activation) = (core.meta().combat_round_seq, core.meta().activation_seq);
    match effect {
        Simple::Coup => core.transient_mut().misc.flags.set(TransientFlags::STRATEGIC_CANCELLED),
        Simple::Crisis => core.transient_mut().misc.flags.set(TransientFlags::SKIP_NEXT_TURN),
        Simple::MasterPlan => core.transient_mut().misc.flags.set(TransientFlags::ADDITIONAL_ACTION),
        Simple::MoraleBoost => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.card_timers.combat_bonus_round = Opt32::some(round);
            }
        }
        Simple::FlankSpeed => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.card_timers.move_bonus_activation = Opt32::some(activation);
            }
        }
        Simple::LostStar => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.round_marks.push(RoundKind::LostStar, activation);
            }
        }
        Simple::HackElection => {
            let agenda = core.meta().agenda_seq;
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.card_timers.hack_votes_last_agenda = Opt32::some(agenda);
            }
        }
        Simple::PoliticalStability => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.econ.stability = true;
            }
        }
        Simple::Rout | Simple::Waylay => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                let slot = if effect == Simple::Rout { &mut p.card_timers.rout_round } else { &mut p.card_timers.waylay_barrage_round };
                *slot = Opt32::some(round);
            }
        }
        Simple::FighterPrototype | Simple::Bunker | Simple::WarMachine | Simple::Blitz => {
            let (kind, value) = match effect {
                Simple::FighterPrototype => (RoundKind::FighterBonus, round),
                Simple::Bunker => (RoundKind::BunkerInvasion, activation),
                Simple::WarMachine => (RoundKind::WarMachine, activation),
                _ => (RoundKind::BlitzInvasion, activation),
            };
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.round_marks.push(kind, value);
            }
        }
        Simple::NavSuite => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.card_timers.anomalies_ignored_activation = Opt32::some(activation);
            }
        }
        Simple::BlackMarket => core.transient_mut().misc.flags.set(TransientFlags::BLACK_MARKET),
        Simple::Rally => {
            let granted = crate::query::economy::tokens_in_reinforcements(core, who, env.mahact_held).min(2);
            if granted > 0
                && let Some(p) = core.players_mut().by_seat_mut(who)
            {
                p.econ.fleet_tokens += i16::try_from(granted).unwrap_or(0);
            }
        }
        Simple::ManeuveringJets | Simple::ShieldsHolding => {
            let hits = if effect == Simple::ManeuveringJets { 1 } else { 2 };
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                let running = match p.card_timers.cancel_hits_round {
                    Some((held, had)) if held == round => had,
                    _ => 0,
                };
                p.card_timers.cancel_hits_round = Some((round, running + hits));
            }
        }
        Simple::Reflective => {
            let combat = &mut core.transient_mut().combat;
            if let Some((system, victim, _unit, producer, _)) = combat.last_sustain
                && victim == who
                && producer != who
            {
                combat.pending_reflective_hits = Some((system, producer, 2));
            }
        }
        Simple::Confounding => core.agenda_mut().elected_override = Some(who),
        Simple::DeadlyPlot => {
            core.transient_mut().misc.flags.set(TransientFlags::AGENDA_DISCARDED);
            for (_, planet) in crate::query::board::controlled_planets(core, who).as_slice() {
                crate::kernel::fx::exhaust_planet(core, *planet);
            }
        }
        Simple::Salvage => {
            let Some((_, sides)) = core.transient().combat.last_combat_sides.clone() else { return };
            let mut goods = 0i16;
            for side in sides.as_slice() {
                if *side == who {
                    continue;
                }
                if let Some(p) = core.players_mut().by_seat_mut(*side) {
                    goods += p.econ.commodities;
                    p.econ.commodities = 0;
                }
            }
            if goods > 0
                && let Some(p) = core.players_mut().by_seat_mut(who)
            {
                p.econ.commodities += goods;
            }
        }
        Simple::ReverseEngineer => {
            let Some((by, card)) = core.transient().combat.last_action_discarded else { return };
            if by == who || !is_component_action(card) {
                return;
            }
            let Some(at) = core.cards().discarded_action.iter().position(|held| *held == card) else { return };
            core.cards_mut().discarded_action.edit().remove(at);
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.cards.hand.push(card);
            }
        }
        Simple::EconomicInitiative => {
            let owned = crate::query::board::controlled_planets(core, who);
            for (_, planet) in owned.as_slice() {
                let data = planet.data();
                let in_scope = env.scope.0 & crate::tables::source_bit(data.source) != 0;
                if in_scope && data.traits.contains(&crate::generated::PlanetTrait::Cultural) {
                    crate::kernel::fx::ready_planet(core, *planet);
                }
            }
        }
        Simple::Councilor => {
            let agenda = core.meta().agenda_seq;
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                let running = match p.card_timers.extra_votes_agenda {
                    Some(held) if held.agenda == agenda => held.votes,
                    _ => 0,
                };
                p.card_timers.extra_votes_agenda = Some(crate::state::ExtraVotes { agenda, votes: running + 5 });
            }
        }
        Simple::EmergencyRepairs => {
            let Some(system) = core.meta().active_system else { return };
            let planets: crate::containers::SmallList<crate::generated::Planet, 8> =
                crate::query::board::planets_of(core, system).collect();
            let repair = |list: &mut crate::state::UnitList| {
                for piece in list.as_mut_slice() {
                    if piece.owner() == who {
                        *piece = piece.with_sustained(false);
                    }
                }
            };
            repair(&mut core.board_mut().sys_mut(system).units);
            for planet in planets.as_slice() {
                repair(&mut core.board_mut().planet_mut(*planet).units);
            }
        }
        Simple::Disable => {
            let Some(system) = core.meta().active_system else { return };
            let opponent_pds = pieces_in(core, system).any(|p| {
                p.owner() != who && env.scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Pds)
            });
            if opponent_pds && let Some(p) = core.players_mut().by_seat_mut(who) {
                p.round_marks.push(RoundKind::DisableInvasion, activation);
            }
        }
        Simple::SolarFlare => {
            if let Some(p) = core.players_mut().by_seat_mut(who) {
                p.round_marks.push(RoundKind::SolarFlare, activation);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::Idx;

    #[test]
    fn component_action_is_the_printed_window() {
        assert!(is_component_action(ActionCard::Spy));
        assert!(!is_component_action(ActionCard::Blitz));
    }

    /// One test for every documented bail: the core declines, the string function answers.
    #[test]
    fn every_bail_declines_and_the_rest_answers() {
        let core = CoreState::all_loaded();
        let who = Seat::from_index(0).unwrap();
        let all = Scope(u32::MAX);
        for card in [
            ActionCard::SeizeArtifact,
        ] {
            assert_eq!(has_legal_target(&core, false, all, who, card), None, "{card:?}");
        }
        for card in [ActionCard::Uprising, ActionCard::RiseOfAMessiah, ActionCard::FrontlineDeployment, ActionCard::WarEffort, ActionCard::UnstablePlanet, ActionCard::ReactorMeltdown, ActionCard::CrippleDefenses, ActionCard::LuckyShot, ActionCard::Plague, ActionCard::TacticalBombardment, ActionCard::Scuttle] {
            assert_eq!(has_legal_target(&core, false, all, who, card), Some(false), "{card:?}: an empty board has no target");
        }
        for card in [
            ActionCard::FocusedResearch,
            ActionCard::Impersonation,
            ActionCard::Plagiarize,
            ActionCard::MercenaryContract,
            ActionCard::PirateFleet,
            ActionCard::RevealPrototype,
        ] {
            assert_eq!(cost_is_payable(&core, who, card, 9), None, "{card:?}");
        }
        assert_eq!(cost_is_payable(&core, who, ActionCard::Bribery, 0), Some(false));
        assert_eq!(cost_is_payable(&core, who, ActionCard::Bribery, 1), Some(true));
        assert_eq!(cost_is_payable(&core, who, ActionCard::Blitz, 0), Some(true));
        let scope = Scope(u32::MAX);
        let w = Window { system: System::ALL[0], actor: None };
        for card in [ActionCard::HarnessEnergy, ActionCard::Infiltrate] {
            assert_eq!(reaction_can_matter(&core, scope, who, card, w), None, "{card:?}");
        }
        assert_eq!(has_legal_target(&core, false, all, who, ActionCard::Blitz), Some(true));
        assert_eq!(has_legal_target(&core, false, all, who, ActionCard::Spy), Some(false), "no rival");
    }
}
