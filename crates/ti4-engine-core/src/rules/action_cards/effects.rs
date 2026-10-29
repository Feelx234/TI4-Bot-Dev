//! The played effects of action cards that ask a question or follow a plan of their own, as one [`Window`]
//! (package B6; the string `action_cards::effect_for` functions). A card is a [`Fx`]; the window computes the
//! options of its question from the state, asks when there are two or more (the string `pick` takes the only
//! option without a question and does nothing for none), and applies the answer.
//!
//! Ported, by category:
//! * pick a seat: Insubordination, Assassinate Representative, Ancient Burial Sites, Forward Supply Base,
//!   Confusing Legal Text;
//! * predict an outcome: the Riders (Imperial, Construction, Diplomacy, Leadership, Politics, Technology, Trade,
//!   Warfare) and Sanction;
//! * pick a system or a planet: Unexpected Action, Mining Initiative, Uprising, Cripple Defenses, Reactor
//!   Meltdown;
//! * no question: Sabotage (the cancel is the reaction's), Puppets on a String, Extreme Duress, Counterstroke.
//!
//! Bails (the string effect runs alone, each with a test in `mod.rs`): every other card; Plague and Unstable
//! Planet (the destroyed ground forces are staged as a string marker), Repeal (the law leaving play is B9's),
//! Veto (the deck reserve), Spy (the seeded roller), Diplomatic Pressure (the promissory take).

use crate::choice::agenda::Frame;
use crate::choice::cards::{Ask, Opt, Subtype, What};
use crate::choice::generic::Rule;
use crate::choice::{Choice, Ctx, Opts, Origin, Pick};
use crate::containers::SmallList;
use crate::choice::explore::{Opt as EOpt, Step as EStep};
use crate::generated::{ActionCard, Agenda, Explore, ExploreResolution, ExploreType, Planet, PlanetTrait, StrategyCard, System, UnitBaseType, Wormhole};
use crate::ids::Seat;
use crate::keys::{Prediction, PredictionTag, VoteBar};
use crate::kernel::stage::{Child, Illegal, Ran, Sx, Window};
use crate::rules::explore;
use crate::rules::explore_windows::Reward;
use crate::rules::objective_windows::HandLimit;
use crate::rules::production::pay::PayBill;
use crate::kernel::{Kernel, Next, fx, fx_ground, fx_pay};
use crate::keys::{ActionCardCause, DestroyReason};
use crate::state::{RngDomain, SeedSource};
use crate::query::{board, economy, seats};
use crate::state::{CoreState, Piece, TransientFlags};

use super::{Env, is_home_system};

/// What a card does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fx {
    /// Sabotage: the effect itself is empty.
    Nothing,
    /// Insubordination: a rival with a tactic token loses one.
    Insubordination,
    /// Assassinate Representative: the chosen seat may not vote unless it predicted.
    Assassinate,
    /// Ancient Burial Sites: the chosen seat's cultural planets exhaust.
    BurialSites,
    /// Forward Supply Base: 3 trade goods, then 1 to another seat.
    SupplyBase,
    /// Confusing Legal Text: the chosen seat is the elected player instead.
    Confusing,
    /// A Rider or Sanction: predict an outcome under this tag (`None`: Imperial Rider's plain prediction).
    Rider(Option<PredictionTag>),
    /// Unexpected Action: lift a command token of the holder.
    Recall,
    /// Mining Initiative: trade goods for the resources of a planet of the holder.
    Mining,
    /// Uprising: exhaust a planet of a rival, gain its resources.
    Uprising,
    /// Cripple Defenses: each PDS on a planet is destroyed.
    Cripple,
    /// Reactor Meltdown: one space dock off a home system is destroyed.
    Meltdown,
    /// Puppets on a String.
    Puppets,
    /// Extreme Duress: the active seat is marked.
    Duress,
    /// Counterstroke: the active system's token goes back to the tactic pool.
    Counterstroke,
    /// Veto: the next agenda is revealed instead (no question).
    Veto,
    /// Industrial Initiative: 1 trade good for each industrial planet of the holder.
    Industrial,
    /// In The Silence Of Space: the holder's ships in a chosen system ignore blockades.
    Silence,
    /// Rise of a Messiah: 1 infantry on each planet the holder controls (no question).
    Messiah,
    /// Frontline Deployment: 3 infantry on a planet the holder controls.
    Frontline,
    /// War Effort: 1 cruiser in a system with the holder's ships.
    WarEffort,
    /// Insider Information: the holder's mark names the top 3 agendas of the deck (no question).
    Insider,
    /// Tactical Bombardment: the planets of rivals in a chosen system exhaust.
    Bombard,
    /// Diplomatic Pressure: a chosen seat gives the holder a promissory note of its choice.
    Pressure,
    /// Intercept: the other combatants of the active system may not retreat this combat round.
    Intercept,
    /// Rescue: a ship of the holder joins the ships in the active system, from a system without its token.
    Rescue,
    /// Fighter Conscription: a fighter in each system with a dock or a carrier and no foreign ship.
    Conscription,
    /// Manipulate Investments: five trade goods on strategy cards, on at least three different ones.
    Invest,
    /// Repeal Law: a law in play leaves play.
    Repeal,
    /// Spy: a seat with a hand hands the holder one random card (a die).
    Spy,
    /// Plague: one die per infantry on a rival planet; a six or better destroys one of them.
    Plague,
    /// Unstable Planet: a hazardous planet exhausts and loses up to three infantry.
    Unstable,
    /// Seize Artifact: a neighbour with fragments gives one (two questions).
    Seize,
    /// Reparations: the new controller of the holder's planet exhausts one of its planets, the holder readies one.
    Reparations,
    /// Public Disgrace: the picker of a strategy card chooses a different one.
    Disgrace,
    /// Exchange Program: another seat is asked; it names a planet, the other seat accepts, one infantry each stands there.
    Exchange,
    /// Summit: gain two command tokens, each into a pool of the holder's choice (the strategy area's pool question).
    Summit,
    /// Lucky Shot: a dreadnought, cruiser or destroyer of another seat, in a system the holder holds a planet in, is destroyed.
    Lucky,
    /// Scuttle: one or two of the holder's non-fighter ships return to the supply; their cost in trade goods.
    Scuttle,
    /// Archaeological Expedition: the top three cards of the deck of a planet's trait; fragments are kept.
    Dig,
    /// Pirate Contract: a neutral destroyer in a non-home system without a player's ship.
    PirateContract,
    /// Pirate Fleet: three resources, then a neutral carrier, cruiser, destroyer and two fighters in such a system.
    PirateFleet,
    /// Mercenary Contract: two trade goods, then two neutral infantry on a non-home planet without units.
    Mercenary,
    /// Ghost Ship: a destroyer into a non-home wormhole system without another player's ship.
    Ghost,
    /// Signal Jamming: a non-home system near the holder's ships, then the rival whose command token goes there.
    Jam,
    /// Upgrade: a cruiser of the holder in the active system becomes a dreadnought.
    Upgrade,
    /// Refit Troops: one or two of the holder's infantry are replaced by mechs.
    Refit,
    /// Decoy Operation: up to two of the holder's ground forces anywhere leave the board for a planet of the holder in the
    /// active system.
    Decoy,
    /// Ghost Squad: ground forces of the holder move between its planets in the active system, one move per question.
    Squad,
    /// Harness Energy: the holder's commodities are replenished.
    Harness,
    /// Brilliance: a seat without a breakthrough gains its own (the Fracture may enter play: the roll is the Fracture stream's).
    Brilliance,
    /// Impersonation: three influence, then a secret objective is drawn (and the hand limit enforced).
    Impersonation,
    /// Plagiarize: five influence, then a non-faction technology of a neighbour is taken from it.
    Plagiarize,
    /// Focused Research: four trade goods, one technology is researched.
    Focused,
    /// Divert Funding: a non-unit-upgrade, non-faction technology is returned, then another is researched.
    Divert,
    /// Reveal Prototype: a unit upgrade of the line of a unit in the combat is researched for four resources.
    Reveal,
    /// Lie in Wait: one action card is taken from each of two neighbours who traded this round.
    Lie,
    /// Bribery: any number of trade goods are spent, one extra vote each on this agenda.
    Bribery,
    /// Infiltrate: each PDS and space dock of another seat on the planet just taken is replaced by the holder's own.
    Infiltrate,
    /// Crash Landing: a ground force of the holder in the space area of the system its last ship was destroyed in lands on a
    /// planet there other than Mecatol Rex.
    Crash,
    /// Exploration Probe: a frontier token in or next to a system with a ship of the holder is explored (the frontier deck's top
    /// card is drawn and resolved without a planet).
    Probe,
}

impl Fx {
    /// Whether the effect rolls the game's dice (the window needs the random streams of the state).
    #[must_use]
    pub const fn rolls(self) -> bool {
        matches!(self, Self::Spy | Self::Plague | Self::Brilliance)
    }

    /// Whether the effect can be played from inside another area's window (a reaction slot, a card declared there): its facts
    /// (the forms the seat places, the research roster, the map, the traders) come from the adapter's play window, not from the
    /// facts the reactions table carries, so the cards of package B6d that read them are played by the string there (the census
    /// names them, as before).
    #[must_use]
    pub const fn nested(self) -> bool {
        !matches!(
            self,
            Self::PirateContract
                | Self::PirateFleet
                | Self::Mercenary
                | Self::Ghost
                | Self::Jam
                | Self::Upgrade
                | Self::Refit
                | Self::Decoy
                | Self::Squad
                | Self::Harness
                | Self::Brilliance
                | Self::Impersonation
                | Self::Plagiarize
                | Self::Focused
                | Self::Divert
                | Self::Reveal
                | Self::Lie
                | Self::Bribery
                | Self::Infiltrate
                | Self::Crash
                | Self::Probe
        )
    }

    /// Whether the effect places units of the neutral owner (the owner table must hold it).
    #[must_use]
    pub const fn needs_neutral(self) -> bool {
        matches!(self, Self::PirateContract | Self::PirateFleet | Self::Mercenary)
    }

    /// Whether the effect gains or researches a technology: the unit forms of the faction modules (the production kernel's
    /// hooks) decide what an upgrade does to the board.
    #[must_use]
    pub const fn technology(self) -> bool {
        matches!(self, Self::Plagiarize | Self::Focused | Self::Divert | Self::Reveal)
    }

    /// Whether the effect reads the game's map (the adapter lends the galaxy; without one it bails).
    #[must_use]
    pub const fn needs_map(self) -> bool {
        matches!(self, Self::Ghost | Self::Jam)
    }

    /// Whether the effect reads the game's map when there is one (the adapter lends the galaxy it has and says so in
    /// [`Facts::map`]; without one the effect does what the string does without a map). Exploration Probe's adjacency.
    #[must_use]
    pub const fn uses_map(self) -> bool {
        self.needs_map() || matches!(self, Self::Probe)
    }

    /// The bill the effect pays before anything else (the string `production::pay`): the currency and the amount.
    #[must_use]
    pub const fn bill(self) -> Option<(economy::Spend, i64)> {
        match self {
            Self::PirateFleet => Some((economy::Spend::Resources, 3)),
            Self::Impersonation => Some((economy::Spend::Influence, 3)),
            Self::Plagiarize => Some((economy::Spend::Influence, 5)),
            _ => None,
        }
    }

    /// The effect of a card, `None` for a card the window does not play (the bail).
    #[must_use]
    pub const fn of(card: ActionCard) -> Option<Self> {
        use ActionCard as C;
        Some(match card {
            C::Sabotage1 | C::Sabotage2 | C::Sabotage3 | C::Sabotage4 => Self::Nothing,
            C::Insubordination => Self::Insubordination,
            C::AssassinateRepresentative => Self::Assassinate,
            C::AncientBurialSites => Self::BurialSites,
            C::ForwardSupplyBase => Self::SupplyBase,
            C::ConfusingLegalText => Self::Confusing,
            C::ImperialRider => Self::Rider(None),
            C::ConstructionRider => Self::Rider(Some(PredictionTag::ConstRider)),
            C::DiplomacyRider => Self::Rider(Some(PredictionTag::DiploRider)),
            C::LeadershipRider => Self::Rider(Some(PredictionTag::LeadRider)),
            C::PoliticsRider => Self::Rider(Some(PredictionTag::PoliticRider)),
            C::TechnologyRider => Self::Rider(Some(PredictionTag::TechRider)),
            C::TradeRider => Self::Rider(Some(PredictionTag::TradeRider)),
            C::WarfareRider => Self::Rider(Some(PredictionTag::WarRider)),
            C::Sanction => Self::Rider(Some(PredictionTag::Sanction)),
            C::UnexpectedAction => Self::Recall,
            C::MiningInitiative => Self::Mining,
            C::Uprising => Self::Uprising,
            C::CrippleDefenses => Self::Cripple,
            C::ReactorMeltdown => Self::Meltdown,
            C::PuppetsOnAString => Self::Puppets,
            C::ExtremeDuress => Self::Duress,
            C::Counterstroke => Self::Counterstroke,
            C::Veto | C::Veto3 | C::Veto4 => Self::Veto,
            C::IndustrialInitiative => Self::Industrial,
            C::InTheSilenceOfSpace => Self::Silence,
            C::TacticalBombardment => Self::Bombard,
            C::InsiderInformation => Self::Insider,
            C::RiseOfAMessiah => Self::Messiah,
            C::FrontlineDeployment => Self::Frontline,
            C::WarEffort => Self::WarEffort,
            C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4 => Self::Pressure,
            C::Intercept => Self::Intercept,
            C::Rescue => Self::Rescue,
            C::FighterConscription => Self::Conscription,
            C::ManipulateInvestments => Self::Invest,
            C::RepealLaw => Self::Repeal,
            C::Spy => Self::Spy,
            C::Plague => Self::Plague,
            C::UnstablePlanet => Self::Unstable,
            C::SeizeArtifact => Self::Seize,
            C::LuckyShot => Self::Lucky,
            C::Summit => Self::Summit,
            C::ExchangeProgram => Self::Exchange,
            C::Scuttle => Self::Scuttle,
            C::Reparations => Self::Reparations,
            C::PublicDisgrace => Self::Disgrace,
            C::ArchaeologicalExpedition => Self::Dig,
            C::PirateContract1 | C::PirateContract2 | C::PirateContract3 | C::PirateContract4 => Self::PirateContract,
            C::PirateFleet => Self::PirateFleet,
            C::MercenaryContract => Self::Mercenary,
            C::GhostShip => Self::Ghost,
            C::SignalJamming => Self::Jam,
            C::Upgrade => Self::Upgrade,
            C::RefitTroops => Self::Refit,
            C::DecoyOperation => Self::Decoy,
            C::GhostSquad => Self::Squad,
            C::HarnessEnergy => Self::Harness,
            C::Brilliance => Self::Brilliance,
            C::Impersonation => Self::Impersonation,
            C::Plagiarize => Self::Plagiarize,
            C::FocusedResearch => Self::Focused,
            C::DivertFunding => Self::Divert,
            C::RevealPrototype => Self::Reveal,
            C::LieInWait => Self::Lie,
            C::Bribery => Self::Bribery,
            C::Infiltrate => Self::Infiltrate,
            C::CrashLanding => Self::Crash,
            C::ExplorationProbe => Self::Probe,
            _ => return None,
        })
    }
}

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Compute the options; apply, ask or finish.
    Start,
    /// Waiting on the holder's pick.
    Asking,
    /// Paying the bill of the card (the child window [`PayBill`]).
    Paying,
    /// The card chose its technology and owes the bill before the research lands (Reveal Prototype).
    Billing,
    /// Returning a secret objective over the hand limit (the child window [`HandLimit`]).
    Limiting,
    /// Exploration Probe: the token is picked; the frontier deck is explored next.
    Exploring,
    /// Exploration Probe: the command tokens of the card, each into a pool of the holder's choice.
    Pooling,
    /// Exploration Probe: the reward question of a Merchant Station (the child window [`Reward`]).
    Rewarding,
    Done,
}

/// What an Exploration Probe tells the adapter once its card has resolved: the card, how it resolved, what the seat held before
/// the draw and after the last effect (the words of the exploration log's row are `exploration::Holdings`' and the adapter's), and
/// the system a gamma token was placed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeRow {
    pub card: Explore,
    pub outcome: Explored,
    pub before: crate::query::explore::Holdings,
    pub after: crate::query::explore::Holdings,
    pub gamma: Option<System>,
}

/// How the card an Exploration Probe drew resolved (the outcome text and the other texts of the exploration log are the adapter's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Explored {
    /// A relic fragment of this deck was gained.
    Fragment(ExploreType),
    /// The card did what it says.
    Resolved,
    /// The card has nothing to act on without a planet (or a system the draw did not name): nothing happened.
    Unresolved,
}

/// One played card.
#[derive(Debug, Clone)]
pub struct Play {
    card: ActionCard,
    fx: Fx,
    who: Seat,
    env: Env,
    /// The agenda a prediction is about (the `Predict` question's frame).
    frame: Option<Frame>,
    /// Diplomatic Pressure: the seat that was chosen (asked for the note).
    victim: Option<Seat>,
    stage: Stage,
    /// The options of the pending question.
    opts: SmallList<Opt, 16>,
    /// Manipulate Investments: the trade goods placed so far and the cards they went on.
    placed: u8,
    used: SmallList<StrategyCard, 8>,
    /// The dice rolled so far, in order (Spy, Plague).
    faces: SmallList<u32, 16>,
    /// The position held something the window does not express (a galvanized ground force fell): not compared.
    unsupported: bool,
    /// The question of a card with several: 0 is the first (Seize Artifact: the neighbour, then the fragment;
    /// Reparations: the exhaust, then the ready).
    step: u8,
    /// Seize Artifact: the neighbour chosen. Public Disgrace: the picker and the card taken from them.
    taken: Option<(Seat, Option<StrategyCard>)>,
    /// Scuttle: the ship chosen first.
    first_ship: Option<(System, u8)>,
    /// Exchange Program: the planet offered.
    offer: Option<(System, Planet)>,
    /// The bill of the card while it is paid (a child window).
    pay: Child<PayBill>,
    /// Reveal Prototype: the technology chosen, researched once the bill is paid.
    chosen: Option<crate::generated::Technology>,
    /// Crash Landing: the type of ground force chosen.
    landing: Option<crate::generated::Unit>,
    /// Impersonation: the hand limit after the draw (a child window).
    hand: Child<HandLimit>,
    /// Signal Jamming: the system chosen.
    jam: Option<System>,
    /// Refit Troops: the first infantry chosen.
    troop: Option<(System, Planet, u8)>,
    /// Decoy Operation: the ground forces pulled so far, in the order they were picked.
    pulled: SmallList<(System, Planet, u8), 2>,
    /// Ghost Squad: the planets that have received forces (they do not ship out again).
    received: SmallList<Planet, 8>,
    /// The question the bill's child waits on.
    pay_ask: Option<crate::choice::Ask>,
    /// Exploration Probe: the system whose token was picked (explored once the pick is applied).
    probe: Option<System>,
    /// Exploration Probe: the card drawn and how it resolved.
    explored: Option<(Explore, Explored)>,
    /// Exploration Probe: the trade goods gained once the command tokens are placed.
    owed_goods: i32,
    /// Exploration Probe: command tokens still to place.
    pool_left: u8,
    /// Exploration Probe: the Merchant Station's reward question (a child window).
    reward: Child<Reward>,
    /// Exploration Probe: what the seat held before the draw, and once the card has resolved.
    held: Option<(crate::query::explore::Holdings, Option<crate::query::explore::Holdings>)>,
    /// Exploration Probe: the system a gamma token was placed in (not where it was already).
    gamma: Option<System>,
}

/// `RULES_PLAGIARIZE_OWNED` and `RULES_ACTION_CARD_COSTS` (`ti4_model::state`).
const RULES_PLAGIARIZE_OWNED: u32 = 28;
const RULES_ACTION_CARD_COSTS: u32 = 23;
/// `RULES_INFILTRATE_FIX` (`ti4_model::state`).
const RULES_INFILTRATE_FIX: u32 = 1;
/// `RULES_COMMAND_TOKEN_POOL`, `RULES_NO_EMPTY_CONVERT` and `RULES_OFFICIAL_ACTION_DECK` (`ti4_model::state`).
const RULES_COMMAND_TOKEN_POOL: u32 = 12;
const RULES_NO_EMPTY_CONVERT: u32 = 19;
const RULES_OFFICIAL_ACTION_DECK: u32 = 30;

/// The sides of the die the string `Dice` rolls.
const DIE_SIDES: u32 = 10;
/// Plague destroys an infantry on a six or better.
const PLAGUE_KILLS_ON: u32 = 6;

fn sorted(mut spots: SmallList<(System, Planet), 16>) -> SmallList<(System, Planet), 16> {
    spots.as_mut_slice().sort_unstable();
    spots
}

/// The `(system, planet)` pairs of the planets whose unit list satisfies `keep`, with the system the string
/// board files them under, in the string order.
fn planets_where(core: &CoreState, keep: impl Fn(System, Planet, &crate::state::PlanetCell) -> bool) -> SmallList<(System, Planet), 16> {
    let mut out = SmallList::new();
    for (planet, cell) in core.board().planets() {
        if let Some(system) = cell.placed_in.or(planet.data().system)
            && keep(system, planet, cell)
        {
            out.push((system, planet));
        }
    }
    sorted(out)
}

/// The sum of the fragment counts of a seat (the string sums the values of the map; a negative count is possible).
fn fragments_held(core: &CoreState, seat: Seat) -> i32 {
    core.players()
        .by_seat(seat)
        .map_or(0, |p| crate::generated::ExploreType::ALL.iter().map(|t| i32::from(p.cards.fragments.get(*t).unwrap_or(0))).sum())
}

fn has_base(env: Env, cell: &crate::state::PlanetCell, base: UnitBaseType) -> bool {
    cell.units.as_slice().iter().any(|p| env.scope.unit(p.kind()).map(|d| d.base_type) == Some(base))
}

impl Play {
    /// The play of `card` by `who`; `None` when the card is not ported or the position holds something the
    /// window cannot express (a prediction on no agenda).
    #[must_use]
    pub fn of(core: &CoreState, card: ActionCard, who: Seat, env: Env) -> Option<Self> {
        let fx = Fx::of(card)?;
        // Intercept's combatants include the units that fight from a planet (a Naaz Eidolon Maximum, the ground
        // forces a Nekro Alastor chose): the kernel's combat rules decide them, which this area may not import.
        if fx == Fx::Intercept
            && core.players().iter().any(|p| matches!(p.faction, crate::generated::Faction::TheNaazRokhaAlliance | crate::generated::Faction::TheNekroVirus))
        {
            return None;
        }
        // The deck reserve (a replay's forced draws) is the host's: the string draws through it.
        // A command token held in a Mahact fleet pool changes the reinforcements the pool question previews.
        if fx == Fx::Summit && env.mahact_held > 0 {
            return None;
        }
        if matches!(fx, Fx::Veto | Fx::Dig | Fx::Impersonation | Fx::Probe) && core.cards().reserve.is_some() {
            return None;
        }
        // A readied Keleres agent could be offered before a payment (a window of its own, the goods window).
        if (fx.bill().is_some() || matches!(fx, Fx::Mercenary | Fx::Bribery)) && env.facts.agent_offered {
            return None;
        }
        // The Siphon event, Cabal's note and the Trade Agreement hook follow a replenishment: the string's.
        if fx == Fx::Harness && env.facts.replenish_events {
            return None;
        }
        // Oceanbound hears coexistence begin: a Deepwrought holder's note is the string's (dropped by decision).
        if fx == Fx::Crash && env.facts.holder_deepwrought {
            return None;
        }
        // Games below rules version 1 played Infiltrate differently (kept for their logs).
        if fx == Fx::Infiltrate && env.facts.rules_version < RULES_INFILTRATE_FIX {
            return None;
        }
        // A mech the seat places with an announcement of its own (Naaz) is the string's.
        if fx == Fx::Refit && env.facts.mech.unit.is_none() {
            return None;
        }
        // The neutral owner has a seat only when the adapter put it in the table.
        if fx.needs_neutral() && env.facts.neutral.is_none() {
            return None;
        }
        let frame = match fx {
            Fx::Rider(_) => {
                let agenda = core.agenda().in_play;
                match (agenda, core.agenda().choices.len()) {
                    (Some(agenda), _) => Some(Frame { agenda, covert: core.agenda().covert, reveal: false }),
                    (None, 0 | 1) => Some(Frame { agenda: Agenda::ALL[0], covert: None, reveal: false }),
                    (None, _) => return None,
                }
            }
            _ => None,
        };
        Some(Self { card, fx, who, env, frame, victim: None, stage: Stage::Start, opts: SmallList::new(), placed: 0, used: SmallList::new(), faces: SmallList::new(), unsupported: false, step: 0, taken: None, first_ship: None, offer: None, pay: Child::idle(), hand: Child::idle(), chosen: None, landing: None, jam: None, troop: None, pulled: SmallList::new(), received: SmallList::new(), pay_ask: None, probe: None, explored: None, owed_goods: 0, pool_left: 0, reward: Child::idle(), held: None, gamma: None })
    }

    fn what(&self) -> Option<What> {
        Some(match self.fx {
            Fx::Insubordination | Fx::Assassinate | Fx::BurialSites | Fx::SupplyBase | Fx::Spy => What::Player,
            Fx::Repeal => What::Law,
            Fx::Seize if self.step == 0 => What::Player,
            Fx::Seize => What::Fragment,
            Fx::Reparations if self.step == 0 => What::Exhaust,
            Fx::Reparations => What::Ready,
            Fx::Dig => What::Dig,
            Fx::PirateContract | Fx::PirateFleet => What::Pirate,
            Fx::Probe => What::Probe,
            Fx::Mercenary => What::Landing,
            Fx::Ghost => What::Ghost,
            Fx::Upgrade => What::Cruiser,
            Fx::Plagiarize => What::Steal,
            Fx::Focused => What::Focus,
            Fx::Crash => What::Crash { system: self.jam?, second: self.step == 1 },
            Fx::Bribery => What::Spend,
            Fx::Lie => What::Lie { from: self.victim? },
            Fx::Divert if self.step == 0 => What::Return,
            Fx::Divert => What::Funding,
            Fx::Reveal => What::Prototype,
            Fx::Brilliance => What::Gain,
            Fx::Squad => What::Squad { system: self.jam? },
            Fx::Refit => What::Troops { second: self.troop.is_some() },
            Fx::Decoy if self.step == 0 => What::Pull { second: false },
            Fx::Decoy if self.step == 1 => What::Pull { second: true },
            Fx::Decoy => What::Land,
            Fx::Jam if self.step == 0 => What::Jam,
            Fx::Jam => What::JamWho { system: self.jam? },
            Fx::Exchange if self.step == 0 => What::Player,
            Fx::Exchange if self.step == 1 => What::Offer,
            Fx::Exchange => What::Accept,
            Fx::Lucky => What::Hull,
            Fx::Scuttle => What::Scuttle { second: self.first_ship.is_some() },
            Fx::Disgrace => {
                let (_, first) = self.taken?;
                What::Redraft { first: first?, by: self.who }
            }
            Fx::Confusing => What::Elect,
            Fx::Recall => What::Recall,
            Fx::Silence => What::Silence,
            Fx::Bombard => What::Bombard,
            Fx::Pressure if self.victim.is_some() => What::Note,
            Fx::Pressure => What::Player,
            Fx::Mining | Fx::Uprising | Fx::Cripple | Fx::Meltdown | Fx::Frontline | Fx::Plague | Fx::Unstable => What::Planet,
            Fx::WarEffort => What::Deploy,
            Fx::Rescue => What::Rescue,
            Fx::Invest => What::Invest { step: self.placed + 1, owed: Self::INVEST_DISTINCT.saturating_sub(self.used.len() as u8) },
            _ => return None,
        })
    }

    const INVEST_TOKENS: u8 = 5;
    const INVEST_DISTINCT: u8 = 3;

    /// The strategy cards a trade good may go on: the cards of the sources in play, or, when a card in play is
    /// not among them, the cards in play.
    fn invest_cards(&self, core: &CoreState) -> SmallList<StrategyCard, 12> {
        let mut in_play: SmallList<StrategyCard, 12> = core.strategy().unclaimed.as_slice().iter().copied().collect();
        for p in core.players().iter() {
            for c in p.strat.as_slice() {
                in_play.push(*c);
            }
        }
        let scope = self.env.scope.0;
        let mut cards: SmallList<StrategyCard, 12> =
            StrategyCard::ALL.iter().copied().filter(|c| scope & crate::tables::source_bit(c.data().source) != 0).collect();
        if in_play.as_slice().iter().any(|c| !cards.as_slice().contains(c)) {
            cards = StrategyCard::ALL.iter().copied().filter(|c| in_play.as_slice().contains(c)).collect();
        }
        cards
    }

    fn seat_opts(seats: impl Iterator<Item = Seat>) -> SmallList<Opt, 16> {
        seats.map(Opt::Seat).collect()
    }

    /// The options of the card's question, in the string order.
    fn options(&self, core: &CoreState, k: &Kernel) -> SmallList<Opt, 16> {
        let who = self.who;
        let others = || core.players().iter().map(|p| p.seat).filter(move |s| *s != who);
        match self.fx {
            Fx::Insubordination => Self::seat_opts(
                core.players().iter().filter(|p| p.seat != who && p.econ.tactic_tokens > 0).map(|p| p.seat),
            ),
            Fx::Assassinate | Fx::BurialSites => Self::seat_opts(seats::seating_order(core).as_slice().iter().copied()),
            Fx::SupplyBase => Self::seat_opts(seats::seating_order(core).as_slice().iter().copied().filter(|s| *s != who)),
            Fx::Confusing => Self::seat_opts(others()),
            Fx::Pressure => match self.victim {
                None => Self::seat_opts(seats::seating_order(core).as_slice().iter().copied().filter(|s| *s != who)),
                Some(victim) => {
                    let mut held = crate::query::promissory::held_by(core, victim);
                    held.sort_by(|a, b| (self.env.note_order)(*a, *b));
                    held.into_iter().map(Opt::Note).collect()
                }
            },
            Fx::Rider(_) => core.agenda().choices.as_slice().iter().map(|o| Opt::Outcome(*o)).collect(),
            Fx::Recall => {
                let mut held: SmallList<System, 16> =
                    board::systems_with_token(core, who).iter().filter_map(|i| System::ALL.get(i).copied()).collect();
                held.as_mut_slice().sort_unstable();
                held.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::Bombard => {
                let mut held: SmallList<System, 16> = System::ALL
                    .iter()
                    .copied()
                    .filter(|s| {
                        board::has_space_units_of(core, *s, who)
                            && board::planets_of(core, *s).any(|p| board::control(core, p).is_some_and(|owner| owner != who))
                    })
                    .collect();
                held.as_mut_slice().sort_unstable();
                held.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::Silence => {
                let mut held: SmallList<System, 16> =
                    board::systems_with_units_of(core, who).iter().filter_map(|i| System::ALL.get(i).copied()).collect();
                held.as_mut_slice().sort_unstable();
                held.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::WarEffort => {
                let mut held: SmallList<System, 16> =
                    board::systems_with_units_of(core, who).iter().filter_map(|i| System::ALL.get(i).copied()).collect();
                held.as_mut_slice().sort_unstable();
                held.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::Invest => {
                let cards = self.invest_cards(core);
                if cards.len() < usize::from(Self::INVEST_DISTINCT) || self.placed >= Self::INVEST_TOKENS {
                    return SmallList::new();
                }
                let remaining = Self::INVEST_TOKENS - self.placed;
                let owed = Self::INVEST_DISTINCT.saturating_sub(self.used.len() as u8);
                cards.as_slice().iter().filter(|c| remaining > owed || !self.used.as_slice().contains(c)).map(|c| Opt::Strat(*c)).collect()
            }
            Fx::Rescue => {
                // "After a player moves ships into a system that contains your ships": somebody else activated, and
                // the holder has a ship there. The origins are systems without the holder's token that hold a ship of it.
                let ship = |s: System| board::space_units(core, s).iter().any(|p| p.owner() == who && self.env.scope.unit(p.kind()).is_some_and(|d| d.ship));
                let Some(destination) = core.meta().active_system else { return SmallList::new() };
                if !ship(destination) || core.meta().active == Some(who) {
                    return SmallList::new();
                }
                let mut held: SmallList<System, 16> =
                    System::ALL.iter().copied().filter(|s| *s != destination && !board::has_token(core, *s, who) && ship(*s)).collect();
                held.as_mut_slice().sort_unstable();
                held.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::Exchange => match (self.step, self.taken) {
                (0, _) => Self::seat_opts(seats::seating_order(core).as_slice().iter().copied().filter(|s| *s != who)),
                (1, Some((other, _))) => {
                    let scope = self.env.scope;
                    let mut offers = SmallList::new();
                    for (system, planet) in board::controlled_planets(core, other).as_slice() {
                        // Ground forces: infantry and mechs (and the Titans' PDS, which is both).
                        let own_ground = core.board().planet(*planet).units.as_slice().iter().any(|p| {
                            p.owner() == other
                                && scope.unit(p.kind()).is_some_and(|d| {
                                    matches!(d.base_type, UnitBaseType::Infantry | UnitBaseType::Mech) || (d.base_type == UnitBaseType::Pds && d.ground_force)
                                })
                        });
                        if own_ground {
                            offers.push(Opt::Spot(*system, *planet));
                        }
                    }
                    offers
                }
                (_, Some(_)) => [Opt::Answer(true), Opt::Answer(false)].into_iter().collect(),
                _ => SmallList::new(),
            },
            Fx::Lucky => {
                let mut seen: SmallList<(System, Seat, crate::generated::Unit, bool), 16> = SmallList::new();
                let mut out = SmallList::new();
                for system in System::ALL {
                    if !board::controls_a_planet_in(core, system, who) {
                        continue;
                    }
                    for (index, p) in board::space_units(core, system).iter().enumerate() {
                        if p.owner() == who {
                            continue;
                        }
                        let Some(d) = self.env.scope.unit(p.kind()) else { continue };
                        if !matches!(d.base_type, UnitBaseType::Dreadnought | UnitBaseType::Cruiser | UnitBaseType::Destroyer) {
                            continue;
                        }
                        // One option per distinguishable kill: two identical hulls in one system are the same shot.
                        let shape = (system, p.owner(), p.kind(), p.sustained());
                        if seen.as_slice().contains(&shape) {
                            continue;
                        }
                        seen.push(shape);
                        out.push(Opt::Ship { system, index: u8::try_from(index).unwrap_or(u8::MAX), owner: p.owner(), kind: p.kind() });
                    }
                }
                out
            }
            Fx::Scuttle => {
                let mut out = SmallList::new();
                for system in System::ALL {
                    for (index, p) in board::space_units(core, system).iter().enumerate() {
                        if p.owner() != who || self.env.scope.unit(p.kind()).is_some_and(|d| !d.ship || d.base_type == UnitBaseType::Fighter) {
                            continue;
                        }
                        let at = u8::try_from(index).unwrap_or(u8::MAX);
                        if self.first_ship == Some((system, at)) {
                            continue;
                        }
                        out.push(Opt::Ship { system, index: at, owner: who, kind: p.kind() });
                    }
                }
                if self.first_ship.is_some() {
                    out.push(Opt::Stop);
                }
                out
            }
            Fx::Seize if self.step == 0 => {
                let mut near: SmallList<Seat, 2> = SmallList::new();
                let order = seats::seating_order(core);
                if let Some(at) = order.as_slice().iter().position(|s| *s == who) {
                    let n = order.len();
                    let before = order.as_slice()[(at + n - 1) % n];
                    let after = order.as_slice()[(at + 1) % n];
                    near.push(before);
                    if before != after {
                        near.push(after);
                    }
                }
                Self::seat_opts(near.as_slice().iter().copied().filter(|s| fragments_held(core, *s) > 0))
            }
            Fx::Seize => match self.taken {
                Some((victim, _)) => crate::generated::ExploreType::ALL
                    .iter()
                    .filter(|t| core.players().by_seat(victim).and_then(|p| p.cards.fragments.get(**t)).unwrap_or(0) > 0)
                    .map(|t| Opt::Fragment(*t))
                    .collect(),
                None => SmallList::new(),
            },
            Fx::Reparations => {
                let Some((_, _, gainer, previous)) = core.transient().combat.last_control_gained else { return SmallList::new() };
                // "A planet you control" is about the planet's former holder, not its new one.
                if previous != Some(who) || gainer == who {
                    return SmallList::new();
                }
                let mut spots = SmallList::<(System, Planet), 16>::new();
                let owner = if self.step == 0 { gainer } else { who };
                for (system, planet) in board::controlled_planets(core, owner).as_slice() {
                    // Readying only reaches planets that are exhausted: readying a ready one is not a thing the board records.
                    if self.step == 0 || board::exhausted(core, *planet) {
                        spots.push((*system, *planet));
                    }
                }
                spots.as_mut_slice().sort_by_key(|(_, p)| *p);
                spots.as_slice().iter().map(|(s, p)| Opt::Spot(*s, *p)).collect()
            }
            Fx::Jam if self.step == 0 => {
                // Non-home systems of the map that hold one of the holder's ships or touch such a system.
                let Some(galaxy) = core.galaxy() else { return SmallList::new() };
                let scope = self.env.scope;
                let on_map = |s: System| galaxy.coord_of(crate::ids::system_at(System::ALL.iter().position(|x| *x == s).unwrap_or(0))).is_some();
                let mut reach: SmallList<System, 64> = SmallList::new();
                let mut add = |s: System| {
                    if !reach.as_slice().contains(&s) {
                        reach.push(s);
                    }
                };
                for (i, system) in System::ALL.iter().copied().enumerate() {
                    if on_map(system) && board::space_units(core, system).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ship)) {
                        add(system);
                        for j in galaxy.adjacent(crate::ids::system_at(i)).iter() {
                            if let Some(near) = System::ALL.get(j) {
                                add(*near);
                            }
                        }
                    }
                }
                reach.as_mut_slice().sort_unstable();
                reach.as_slice().iter().copied().filter(|s| !is_home_system(scope, *s)).map(Opt::System).collect()
            }
            Fx::Bribery => {
                // "Any number" includes zero, but with no trade good there is nothing to ask.
                let held = core.players().by_seat(who).map_or(0, |p| p.econ.trade_goods.max(0));
                if held == 0 {
                    return SmallList::new();
                }
                (0..=held).filter_map(|n| u8::try_from(n).ok()).map(Opt::Count).collect()
            }
            Fx::Lie => match self.victim.and_then(|v| core.players().by_seat(v)) {
                Some(p) => p.cards.hand.iter().map(|c| Opt::Card(*c)).collect(),
                None => SmallList::new(),
            },
            Fx::Crash if self.step == 9 => SmallList::new(),
            Fx::Crash if self.step == 0 => self.crash_kinds(core).as_slice().iter().map(|u| Opt::Kind(*u)).collect(),
            Fx::Crash => match self.jam {
                Some(system) => self.crash_planets(core, system).as_slice().iter().map(|p| Opt::Spot(system, *p)).collect(),
                None => SmallList::new(),
            },
            Fx::Focused | Fx::Reveal | Fx::Divert if self.step == 9 => SmallList::new(),
            Fx::Focused => crate::rules::production::strategy_cards::Research::researchable_for(core, k, self.env.scope, who).as_slice().iter().map(|t| Opt::Tech(*t)).collect(),
            Fx::Reveal => crate::rules::production::strategy_cards::Research::researchable_for(core, k, self.env.scope, who)
                .as_slice()
                .iter()
                .filter(|t| self.env.facts.reveal.contains(**t))
                .map(|t| Opt::Tech(*t))
                .collect(),
            Fx::Divert if self.step == 0 => {
                let Some(p) = core.players().by_seat(who) else { return SmallList::new() };
                let mut mine: SmallList<crate::generated::Technology, 40> = p.tech.owned.iter().collect();
                mine.as_mut_slice().sort_by(|a, b| (self.env.facts.tech_order)(*a, *b));
                mine.as_slice()
                    .iter()
                    .filter(|t| !crate::query::tech::is_unit_upgrade(**t) && crate::query::tech::faction_of(**t).is_none())
                    .map(|t| Opt::Tech(*t))
                    .collect()
            }
            Fx::Divert => crate::rules::production::strategy_cards::Research::researchable_for(core, k, self.env.scope, who).as_slice().iter().map(|t| Opt::Tech(*t)).collect(),
            Fx::Plagiarize => {
                let mut near: SmallList<Seat, 2> = SmallList::new();
                let order = seats::seating_order(core);
                if let Some(at) = order.as_slice().iter().position(|s| *s == who) {
                    let n = order.len();
                    let (before, after) = (order.as_slice()[(at + n - 1) % n], order.as_slice()[(at + 1) % n]);
                    near.push(before);
                    if before != after {
                        near.push(after);
                    }
                }
                let mine = core.players().by_seat(who).map(|p| p.tech.owned);
                let mut out = SmallList::new();
                for neighbour in near.as_slice() {
                    let Some(p) = core.players().by_seat(*neighbour) else { continue };
                    let mut techs: SmallList<crate::generated::Technology, 40> = p.tech.owned.iter().collect();
                    techs.as_mut_slice().sort_by(|a, b| (self.env.facts.tech_order)(*a, *b));
                    for tech in techs.as_slice() {
                        let non_faction = crate::query::tech::faction_of(*tech).is_none();
                        let already_mine = self.env.facts.rules_version >= RULES_PLAGIARIZE_OWNED && mine.is_some_and(|m| m.contains(*tech));
                        if non_faction && !already_mine {
                            out.push(Opt::Steal { from: *neighbour, tech: *tech });
                        }
                    }
                }
                out
            }
            Fx::Brilliance => seats::seating_order(core)
                .as_slice()
                .iter()
                .copied()
                .filter(|s| core.players().by_seat(*s).is_some_and(|p| p.cards.breakthrough.is_none()))
                .map(Opt::Seat)
                .collect(),
            Fx::Squad => {
                let Some(system) = self.jam else { return SmallList::new() };
                let scope = self.env.scope;
                let mut own: SmallList<Planet, 8> = board::planets_of(core, system).filter(|p| board::control(core, *p) == Some(who)).collect();
                own.as_mut_slice().sort_unstable();
                let mut out = SmallList::new();
                for from in own.as_slice() {
                    if self.received.as_slice().contains(from) {
                        continue;
                    }
                    let ground: SmallList<crate::generated::Unit, 12> = board::planet_units(core, *from)
                        .iter()
                        .filter(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ground_force))
                        .map(|p| p.kind())
                        .collect();
                    let mut kinds: SmallList<crate::generated::Unit, 12> = SmallList::new();
                    for k in ground.as_slice() {
                        if !kinds.as_slice().contains(k) {
                            kinds.push(*k);
                        }
                    }
                    kinds.as_mut_slice().sort_by(|a, b| (self.env.facts.unit_order)(*a, *b));
                    for to in own.as_slice() {
                        if to == from {
                            continue;
                        }
                        for kind in kinds.as_slice() {
                            let count = u8::try_from(ground.as_slice().iter().filter(|k| *k == kind).count()).unwrap_or(u8::MAX);
                            out.push(Opt::Move { from: *from, to: *to, kind: *kind, count });
                        }
                    }
                }
                out
            }
            Fx::Decoy if self.step == 2 => self.decoy_destinations(core),
            Fx::Decoy if self.step == 3 => SmallList::new(),
            Fx::Decoy => {
                let mut out: SmallList<Opt, 16> = self.ground_forces(core).as_slice().iter().copied().map(|(system, planet, index, kind)| Opt::Troop { system, planet, index, kind }).collect();
                if let Some(first) = self.pulled.as_slice().first() {
                    out.retain(|o| !matches!(o, Opt::Troop { system, planet, index, .. } if (*system, *planet, *index) == *first));
                    out.push(Opt::Stop);
                }
                out
            }
            Fx::Refit => {
                let facts = self.env.facts.mech;
                let left = facts.unit.map_or(0, |u| crate::query::fleet::remaining(core, self.env.scope, who, u, facts.on_card));
                if left <= 0 {
                    return SmallList::new();
                }
                let mut out: SmallList<Opt, 16> = SmallList::new();
                for (system, planet) in planets_where(core, |_, _, cell| !cell.units.as_slice().is_empty()).as_slice() {
                    for (i, p) in core.board().planet(*planet).units.as_slice().iter().enumerate() {
                        if p.owner() == who && self.env.scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Infantry) {
                            let index = u8::try_from(i).unwrap_or(u8::MAX);
                            if self.troop != Some((*system, *planet, index)) {
                                out.push(Opt::Troop { system: *system, planet: *planet, index, kind: p.kind() });
                            }
                        }
                    }
                }
                if self.troop.is_some() {
                    out.push(Opt::Stop);
                }
                out
            }
            Fx::Upgrade => {
                let Some(system) = core.meta().active_system else { return SmallList::new() };
                let facts = self.env.facts.dreadnought;
                let left = facts.unit.map_or(0, |u| crate::query::fleet::remaining(core, self.env.scope, who, u, facts.on_card));
                if left <= 0 {
                    return SmallList::new();
                }
                board::space_units(core, system)
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.owner() == who && self.env.scope.unit(p.kind()).is_some_and(|d| d.base_type == UnitBaseType::Cruiser))
                    .map(|(i, p)| Opt::Ship { system, index: u8::try_from(i).unwrap_or(u8::MAX), owner: who, kind: p.kind() })
                    .collect()
            }
            Fx::Jam => core.players().iter().filter(|p| p.seat != who).map(|p| Opt::Seat(p.seat)).collect(),
            Fx::Ghost => {
                // The systems on the map (the hex order of the galaxy), off-map ones left out.
                let Some(galaxy) = core.galaxy() else { return SmallList::new() };
                let scope = self.env.scope;
                let mut on_map: SmallList<(crate::hex::Hex, System), 64> = SmallList::new();
                for (i, system) in System::ALL.iter().copied().enumerate() {
                    if let Some(hex) = galaxy.coord_of(crate::ids::system_at(i)) {
                        on_map.push((hex, system));
                    }
                }
                on_map.as_mut_slice().sort_unstable();
                on_map
                    .as_slice()
                    .iter()
                    .map(|(_, s)| *s)
                    .filter(|s| crate::tables::printed_wormholes(*s) != 0)
                    .filter(|s| !is_home_system(scope, *s))
                    .filter(|s| !board::space_units(core, *s).iter().any(|p| p.owner() != who && scope.unit(p.kind()).is_some_and(|d| d.ship)))
                    .map(Opt::System)
                    .collect()
            }
            Fx::Mercenary => {
                // Two trade goods are held (the goods are spent after the infantry land, so the check comes first).
                if core.players().by_seat(who).is_none_or(|p| p.econ.trade_goods < 2) {
                    return SmallList::new();
                }
                let scope = self.env.scope;
                planets_where(core, |system, planet, cell| {
                    cell.flags.has(crate::state::PlanetFlags::UNITS_ENTRY)
                        && cell.units.as_slice().is_empty()
                        && !is_home_system(scope, system)
                        && scope.0 & crate::tables::source_bit(planet.data().source) != 0
                })
                .as_slice()
                .iter()
                .map(|(s, p)| Opt::Spot(*s, *p))
                .collect()
            }
            Fx::PirateContract | Fx::PirateFleet => {
                // A system with a board entry and no unit of a seated player in its space area, off the homeworlds.
                let mut open: SmallList<System, 16> = System::ALL
                    .iter()
                    .copied()
                    .filter(|s| {
                        core.board().sys(*s).flags.has(crate::state::SysFlags::TOUCHED)
                            && !board::space_units(core, *s).iter().any(|p| core.players().by_seat(p.owner()).is_some())
                            && !is_home_system(self.env.scope, *s)
                    })
                    .collect();
                open.as_mut_slice().sort_unstable();
                open.as_slice().iter().map(|s| Opt::System(*s)).collect()
            }
            Fx::Dig => {
                let mut spots = SmallList::<(System, Planet), 16>::new();
                for (system, planet) in board::controlled_planets(core, who).as_slice() {
                    if self.dig_deck(core, *planet).is_some() {
                        spots.push((*system, *planet));
                    }
                }
                spots.as_slice().iter().map(|(s, p)| Opt::Spot(*s, *p)).collect()
            }
            Fx::Probe => {
                // The frontier tokens of the systems that hold a ship of the holder or touch one (the map's adjacency when the
                // adapter lent it; without one only the systems with a ship), in the order of the system ids.
                let scope = self.env.scope;
                let galaxy = if self.env.facts.map { core.galaxy() } else { None };
                let mut reach: crate::containers::Set<System, 4> = crate::containers::Set::new();
                for (i, system) in System::ALL.iter().copied().enumerate() {
                    if board::space_units(core, system).iter().any(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ship)) {
                        reach.insert(system);
                        if let Some(galaxy) = galaxy {
                            for j in galaxy.adjacent(crate::ids::system_at(i)).iter() {
                                if let Some(near) = System::ALL.get(j) {
                                    reach.insert(*near);
                                }
                            }
                        }
                    }
                }
                core.explore().frontier.iter().filter(|system| reach.contains(*system)).map(Opt::System).collect()
            }
            Fx::Disgrace => {
                let Some((_, Some(first))) = self.taken else { return SmallList::new() };
                core.strategy().unclaimed.as_slice().iter().filter(|c| **c != first).map(|c| Opt::Strat(*c)).collect()
            }
            Fx::Repeal => crate::query::laws::in_play(core, self.env.blanked).as_slice().iter().map(|l| Opt::Law(*l)).collect(),
            Fx::Spy => Self::seat_opts(core.players().iter().filter(|p| p.seat != who && !p.cards.hand.is_empty()).map(|p| p.seat)),
            Fx::Plague => planets_where(core, |_, _, cell| cell.control.is_some_and(|owner| owner != who))
                .as_slice()
                .iter()
                .map(|(s, p)| Opt::Spot(*s, *p))
                .collect(),
            Fx::Unstable => planets_where(core, |_, planet, cell| {
                let data = planet.data();
                cell.flags.has(crate::state::PlanetFlags::UNITS_ENTRY)
                    && self.env.scope.0 & crate::tables::source_bit(data.source) != 0
                    && data.traits.contains(&PlanetTrait::Hazardous)
            })
            .as_slice()
            .iter()
            .map(|(s, p)| Opt::Spot(*s, *p))
            .collect(),
            Fx::Mining | Fx::Frontline => board::controlled_planets(core, who).as_slice().iter().map(|(s, p)| Opt::Spot(*s, *p)).collect(),
            Fx::Uprising => planets_where(core, |system, _, cell| {
                cell.control.is_some_and(|owner| owner != who) && !is_home_system(self.env.scope, system)
            })
            .as_slice()
            .iter()
            .map(|(s, p)| Opt::Spot(*s, *p))
            .collect(),
            Fx::Cripple => planets_where(core, |_, _, cell| has_base(self.env, cell, UnitBaseType::Pds))
                .as_slice()
                .iter()
                .map(|(s, p)| Opt::Spot(*s, *p))
                .collect(),
            Fx::Meltdown => planets_where(core, |system, _, cell| {
                has_base(self.env, cell, UnitBaseType::Spacedock) && !is_home_system(self.env.scope, system)
            })
            .as_slice()
            .iter()
            .map(|(s, p)| Opt::Spot(*s, *p))
            .collect(),
            _ => SmallList::new(),
        }
    }

    /// `planets::traits_now` first trait as an exploration deck: the printed traits (in scope) in printed order, then the
    /// traits the planet's attachments add.
    fn dig_deck(&self, core: &CoreState, planet: Planet) -> Option<crate::generated::ExploreType> {
        let mut found = crate::query::explore::traits_of(self.env.scope, planet);
        for a in core.board().planet(planet).attach.as_slice() {
            for t in a.data().traits {
                if let Some(deck) = crate::query::explore::deck_of(*t)
                    && !found.as_slice().contains(&deck)
                {
                    found.push(deck);
                }
            }
        }
        found.as_slice().first().copied()
    }

    /// `action_cards::place_units`: up to `count` of the unit, as many as the supply still holds.
    fn place(&self, core: &mut CoreState, placed: super::Placed, system: System, planet: Option<Planet>, count: usize) {
        let Some(unit) = placed.unit else { return };
        let left = crate::query::fleet::remaining(core, self.env.scope, self.who, unit, placed.on_card);
        for _ in 0..usize::try_from(left).unwrap_or(0).min(count) {
            fx::place_unit(core, self.who, system, planet, unit);
        }
    }

    fn goods(&self, core: &mut CoreState, who: Seat, amount: i64) {
        let amount = i32::try_from(amount).unwrap_or(0);
        fx::gain_trade_goods_staged(core, who, amount, self.env.action_card);
    }

    /// The ships leave their systems (highest place first within a system) and the holder gains their cost.
    fn scuttle(&self, core: &mut CoreState, picked: &[(System, u8)]) {
        let mut goods = 0i32;
        let mut order: SmallList<(System, u8), 2> = SmallList::from_slice(picked);
        // Per system, from the highest place down (the string groups by system; systems in text order).
        order.as_mut_slice().sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        for (system, index) in order.as_slice() {
            let units = &mut core.board_mut().sys_mut(*system).units;
            if usize::from(*index) < units.len() {
                let gone = units.remove(usize::from(*index));
                // The cost in halves, rounded half up.
                goods += self.env.scope.unit(gone.kind()).and_then(|d| d.cost_halves).map_or(0, |h| (i32::from(h) + 1) / 2);
            }
        }
        fx::gain_trade_goods_staged(core, self.who, goods, self.env.action_card);
    }

    /// Crash Landing: the types of ground force the holder has in the space area of the system, in the order of the id texts.
    fn crash_kinds(&self, core: &CoreState) -> SmallList<crate::generated::Unit, 8> {
        let Some(system) = self.jam.or_else(|| core.transient().combat.last_ship_destroyed.map(|(s, _, _)| s)) else { return SmallList::new() };
        let mut kinds: SmallList<crate::generated::Unit, 8> = SmallList::new();
        for p in board::space_units(core, system) {
            if p.owner() == self.who && self.env.scope.unit(p.kind()).is_some_and(|d| d.ground_force) && !kinds.as_slice().contains(&p.kind()) {
                kinds.push(p.kind());
            }
        }
        kinds.as_mut_slice().sort_by(|a, b| (self.env.facts.unit_order)(*a, *b));
        kinds
    }

    /// Crash Landing: the planets of the system, other than Mecatol Rex.
    fn crash_planets(&self, core: &CoreState, system: System) -> SmallList<Planet, 8> {
        board::planets_in(core, self.env.scope, system)
            .as_slice()
            .iter()
            .copied()
            .filter(|p| !matches!(p, Planet::MecatolRex | Planet::MecatolRexTe))
            .collect()
    }

    /// `galaxy::is_space_station`: a station of a source in play (a planet outside the sources is not looked up at all).
    fn is_station(&self, planet: Planet) -> bool {
        let data = planet.data();
        self.env.scope.0 & crate::tables::source_bit(data.source) != 0 && data.traits.contains(&PlanetTrait::Spacestation)
    }

    /// Decoy Operation: the planets the forces can land on. A space station takes no ground forces; the planets are the
    /// holder's in the active system.
    fn decoy_destinations(&self, core: &CoreState) -> SmallList<Opt, 16> {
        let Some(active) = core.meta().active_system else { return SmallList::new() };
        board::controlled_planets(core, self.who)
            .as_slice()
            .iter()
            .filter(|(system, planet)| *system == active && !self.is_station(*planet))
            .map(|(system, planet)| Opt::Spot(*system, *planet))
            .collect()
    }

    /// `technology::research` of the chosen technology (the string ignores its answer).
    fn research(&self, core: &mut CoreState, k: &Kernel, t: crate::generated::Technology) {
        if let Some(roster) = self.env.facts.roster {
            crate::rules::production::strategy_cards::Research::research_chosen(core, k, self.env.scope, *roster, self.who, t);
        }
    }

    /// The holder's ground forces on the board, system by system, planet by planet, in the order of the planets' lists.
    fn ground_forces(&self, core: &CoreState) -> SmallList<(System, Planet, u8, crate::generated::Unit), 24> {
        let mut out = SmallList::new();
        for (system, planet) in planets_where(core, |_, _, cell| !cell.units.as_slice().is_empty()).as_slice() {
            for (i, p) in core.board().planet(*planet).units.as_slice().iter().enumerate() {
                if p.owner() == self.who && self.env.scope.unit(p.kind()).is_some_and(|d| d.ground_force) {
                    out.push((*system, *planet, u8::try_from(i).unwrap_or(u8::MAX), p.kind()));
                }
            }
        }
        out
    }

    /// Decoy Operation: the pulled forces leave their planets (the last place takes the freed one: `swap_remove`, highest place
    /// first within a system) and stand on the planet, in the order they were picked.
    fn land(&self, core: &mut CoreState, system: System, planet: Planet) {
        let moved: SmallList<Piece, 2> =
            self.pulled.as_slice().iter().filter_map(|(_, p, i)| core.board().planet(*p).units.as_slice().get(usize::from(*i)).copied()).collect();
        let mut order: SmallList<(System, Planet, u8), 2> = self.pulled.clone();
        order.as_mut_slice().sort_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)));
        for (_, from, index) in order.as_slice() {
            let units = &mut core.board_mut().planet_mut(*from).units;
            let at = usize::from(*index);
            if at < units.len() {
                let last = units.len() - 1;
                units.as_mut_slice().swap(at, last);
                units.pop();
            }
        }
        let cell = core.board_mut().planet_mut(planet);
        for piece in moved.as_slice() {
            cell.units.push(*piece);
        }
        cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
        core.board_mut().sys_mut(system).flags.set(crate::state::SysFlags::TOUCHED, true);
    }

    /// Refit Troops: the picked infantry become mechs, system by system (the string groups by system id), the highest place of
    /// a list first so the removals do not shift each other.
    fn refit(&self, core: &mut CoreState, picked: &[(System, Planet, u8)]) {
        let mut order: SmallList<(System, Planet, u8), 2> = SmallList::from_slice(picked);
        order.as_mut_slice().sort_by(|a, b| a.0.cmp(&b.0).then(b.2.cmp(&a.2)));
        let Some(mech) = self.env.facts.mech.unit else { return };
        for (system, planet, index) in order.as_slice() {
            let units = &mut core.board_mut().planet_mut(*planet).units;
            if usize::from(*index) < units.len() {
                units.remove(usize::from(*index));
            }
            fx::place_unit(core, self.who, *system, Some(*planet), mech);
        }
    }

    /// The failed half of Exchange Program: both seats discard one fleet token (an empty pool has nothing to discard).
    fn refuse_exchange(core: &mut CoreState, who: Seat, other: Seat) {
        for seat in [who, other] {
            if let Some(p) = core.players_mut().by_seat_mut(seat)
                && p.econ.fleet_tokens > 0
            {
                p.econ.fleet_tokens -= 1;
            }
        }
    }

    /// Put the first choice back with its picker and off the mat.
    fn restore_first_choice(core: &mut CoreState, picker: Seat, first: StrategyCard) {
        if let Some(at) = core.strategy().unclaimed.as_slice().iter().position(|c| *c == first) {
            core.strategy_mut().unclaimed.remove(at);
        }
        fx::deal_strategy_card(core, picker, first);
    }

    /// The effect that needs no pick (and the part of a pick effect that precedes the question).
    fn before(&mut self, core: &mut CoreState) {
        match self.fx {
            Fx::SupplyBase => {
                fx::gain_trade_goods_staged(core, self.who, 3, self.env.supply_base);
            }
            Fx::Focused => {
                // Four trade goods are held, else the card cannot resolve.
                if core.players().by_seat(self.who).is_none_or(|p| p.econ.trade_goods < 4) {
                    self.step = 9;
                }
            }
            Fx::Crash => {
                // The system of the handoff and the holder's own last ship; something to land and somewhere to land.
                let Some((system, victim, _)) = core.transient().combat.last_ship_destroyed else {
                    self.step = 9;
                    return;
                };
                self.jam = Some(system);
                if victim != self.who || self.crash_kinds(core).is_empty() || self.crash_planets(core, system).is_empty() {
                    self.step = 9;
                }
            }
            Fx::Infiltrate => {
                let Some((_, planet, _, _)) = core.transient().combat.last_control_gained else { return };
                let scope = self.env.scope;
                let who = self.who;
                let mut used = [0_i64; 2];
                let mut swaps: SmallList<(Piece, crate::generated::Unit), 8> = SmallList::new();
                for piece in core.board().planet(planet).units.as_slice() {
                    if piece.owner() == who {
                        continue;
                    }
                    let (slot, plain, own) = match scope.unit(piece.kind()).map(|d| d.base_type) {
                        Some(UnitBaseType::Pds) => (0, crate::generated::Unit::PDSI, self.env.facts.pds),
                        Some(UnitBaseType::Spacedock) => (1, crate::generated::Unit::SpaceDockI, self.env.facts.spacedock),
                        _ => continue,
                    };
                    // "From your reinforcements": the box still holds that kind, counting the swaps already planned.
                    let left = crate::query::fleet::remaining(core, scope, who, plain, 0);
                    if left <= 0 || left - used[slot] <= 0 {
                        continue;
                    }
                    used[slot] += 1;
                    swaps.push((*piece, own.unit.unwrap_or(plain)));
                }
                for (old, new) in swaps.as_slice() {
                    let list = &mut core.board_mut().planet_mut(planet).units;
                    if let Some(at) = list.as_slice().iter().position(|p| p == old) {
                        list.as_mut_slice()[at] = Piece::new(*new, who);
                    }
                }
            }
            Fx::Squad => self.jam = core.meta().active_system,
            Fx::Harness => {
                fx::replenish_commodities(core, self.who);
            }
            Fx::Decoy => {
                // The destination first, so a card with nowhere to land removes nothing; up to two forces are taken unasked.
                let found = self.ground_forces(core);
                if self.decoy_destinations(core).is_empty() || found.is_empty() {
                    self.step = 3;
                } else if found.len() <= 2 {
                    self.pulled = found.as_slice().iter().map(|(s, p, i, _)| (*s, *p, *i)).collect();
                    self.step = 2;
                }
            }
            Fx::Puppets => {
                if core.players().by_seat(self.who).is_some_and(|p| p.econ.passed) {
                    core.transient_mut().misc.flags.set(TransientFlags::PUPPET_ACTION);
                }
            }
            Fx::Duress => {
                if let Some(target) = core.meta().active
                    && target != self.who
                    && let Some(p) = core.players_mut().by_seat_mut(target)
                {
                    p.econ.duress_by = Some(self.who);
                }
            }
            Fx::Insider => {
                use crate::keys::{Atom, MarkKey, MarkVal, Word};
                let key = MarkKey::PrivateSeat(self.who, Word::InsiderPeek);
                let top: SmallList<Agenda, 3> = core.decks().agenda.cards().iter().copied().take(3).collect();
                let marks = &mut core.marks_mut().tail.edit();
                if top.is_empty() {
                    marks.remove(&key);
                } else {
                    marks.insert(key, MarkVal::List(top.as_slice().iter().map(|a| MarkVal::Atom(Atom::Agenda(*a))).collect()));
                }
            }
            Fx::Messiah => {
                for (system, planet) in board::controlled_planets(core, self.who).as_slice() {
                    self.place(core, self.env.infantry, *system, Some(*planet), 1);
                }
            }
            Fx::Veto => {
                if let Some(top) = core.decks_mut().agenda.draw() {
                    core.agenda_mut().veto_replacement = Some(top);
                }
            }
            Fx::Industrial => {
                let count = board::controlled_planets(core, self.who)
                    .as_slice()
                    .iter()
                    .filter(|(_, planet)| {
                        let data = planet.data();
                        self.env.scope.0 & crate::tables::source_bit(data.source) != 0 && data.traits.contains(&PlanetTrait::Industrial)
                    })
                    .count();
                self.goods(core, self.who, i64::try_from(count).unwrap_or(0));
            }
            Fx::Reparations => {
                if let Some((_, _, gainer, previous)) = core.transient().combat.last_control_gained
                    && previous == Some(self.who)
                    && gainer != self.who
                {
                    self.taken = Some((gainer, None));
                }
            }
            Fx::Disgrace => {
                let Some((picker, first)) = core.transient().misc.last_strategy_choice else { return };
                // The row's guard says "another player"; the card cannot act on itself.
                if picker == self.who {
                    return;
                }
                let Some(held) = core.players().by_seat(picker) else { return };
                let Some(at) = held.strat.as_slice().iter().position(|c| *c == first) else { return };
                if let Some(p) = core.players_mut().by_seat_mut(picker) {
                    p.strat.remove(at);
                }
                core.strategy_mut().unclaimed.push(first);
                if core.strategy().unclaimed.as_slice().iter().all(|c| *c == first) {
                    // "If able": nothing else is on the mat, so the first choice stands.
                    Self::restore_first_choice(core, picker, first);
                    return;
                }
                self.taken = Some((picker, Some(first)));
            }
            Fx::Intercept => {
                // The combatants are the seats with ships in the active system (the seated ones, in seating order).
                let Some(system) = core.meta().active_system else { return };
                let ships = |seat: Seat| board::space_units(core, system).iter().any(|p| p.owner() == seat && self.env.scope.unit(p.kind()).is_some_and(|d| d.ship));
                let combatants: SmallList<Seat, 8> = seats::seating_order(core).as_slice().iter().copied().filter(|s| ships(*s)).collect();
                if !combatants.as_slice().contains(&self.who) {
                    return;
                }
                let round = core.meta().combat_round_seq;
                for seat in combatants.as_slice().iter().filter(|s| **s != self.who) {
                    if let Some(p) = core.players_mut().by_seat_mut(*seat) {
                        p.card_timers.retreat_barred_round = crate::containers::Opt32::some(round);
                    }
                }
            }
            Fx::Conscription => {
                let who = self.who;
                let scope = self.env.scope;
                let unit_of = |p: &Piece| scope.unit(p.kind());
                let mut eligible: SmallList<System, 16> = SmallList::new();
                for system in System::ALL {
                    let docked = board::planets_of(core, system)
                        .any(|pl| board::planet_units(core, pl).iter().any(|p| p.owner() == who && unit_of(p).is_some_and(|d| d.base_type == UnitBaseType::Spacedock)));
                    let space = board::space_units(core, system);
                    let has_capacity = space.iter().any(|p| p.owner() == who && unit_of(p).is_some_and(|d| d.capacity.is_some_and(|c| c > 0)));
                    if !docked && !has_capacity {
                        continue;
                    }
                    if space.iter().any(|p| p.owner() != who && unit_of(p).is_some_and(|d| d.ship)) {
                        continue;
                    }
                    eligible.push(system);
                }
                eligible.as_mut_slice().sort_unstable();
                for system in eligible.as_slice() {
                    let left = crate::query::fleet::remaining(core, scope, who, crate::generated::Unit::FighterI, self.env.fighter.on_card);
                    if left <= 0 {
                        break;
                    }
                    fx::place_unit(core, who, *system, None, crate::generated::Unit::FighterI);
                }
            }
            Fx::Counterstroke => {
                let Some(system) = core.meta().active_system else { return };
                if !board::has_token(core, system, self.who) {
                    return;
                }
                core.board_mut().sys_mut(system).cmd.remove(self.who);
                let granted = economy::tokens_in_reinforcements(core, self.who, self.env.mahact_held).min(1);
                if granted > 0
                    && let Some(p) = core.players_mut().by_seat_mut(self.who)
                {
                    p.econ.tactic_tokens += 1;
                }
            }
            _ => {}
        }
    }

    /// Apply the chosen option.
    /// Apply the chosen option; `true` when another question of the same card follows.
    fn apply(&mut self, core: &mut CoreState, k: &Kernel, seeds: &dyn SeedSource, opt: Opt) -> bool {
        let who = self.who;
        match (self.fx, opt) {
            (Fx::Insubordination, Opt::Seat(target)) => {
                if let Some(p) = core.players_mut().by_seat_mut(target) {
                    p.econ.tactic_tokens -= 1;
                }
            }
            (Fx::Assassinate, Opt::Seat(victim)) => {
                if !core.agenda().predictions.contains_key(&victim) {
                    core.agenda_mut().predictions.insert(victim, Prediction::Barred(VoteBar::Assassin));
                }
            }
            (Fx::BurialSites, Opt::Seat(target)) => {
                for (_, planet) in board::controlled_planets(core, target).as_slice() {
                    let data = planet.data();
                    let in_scope = self.env.scope.0 & crate::tables::source_bit(data.source) != 0;
                    if in_scope && data.traits.contains(&PlanetTrait::Cultural) {
                        fx::exhaust_planet(core, *planet);
                    }
                }
            }
            (Fx::SupplyBase, Opt::Seat(other)) => {
                fx::gain_trade_goods_staged(core, other, 1, self.env.supply_base);
            }
            (Fx::Confusing, Opt::Seat(elected)) => core.agenda_mut().elected_override = Some(elected),
            (Fx::Rider(tag), Opt::Outcome(outcome)) => {
                let prediction = tag.map_or(Prediction::Plain(outcome), |t| Prediction::Tagged(outcome, t));
                core.agenda_mut().predictions.insert(who, prediction);
            }
            (Fx::Recall, Opt::System(system)) => {
                core.board_mut().sys_mut(system).cmd.remove(who);
            }
            (Fx::Invest, Opt::Strat(card)) => {
                let goods = &mut core.strategy_mut().goods;
                let held = goods.get(&card).copied().unwrap_or(0);
                goods.insert(card, held + 1);
                if !self.used.as_slice().contains(&card) {
                    self.used.push(card);
                }
                self.placed += 1;
                return self.placed < Self::INVEST_TOKENS;
            }
            (Fx::Rescue, Opt::System(origin)) => {
                let Some(destination) = core.meta().active_system else { return false };
                let scope = self.env.scope;
                let units = &mut core.board_mut().sys_mut(origin).units;
                if let Some(at) = units.as_slice().iter().position(|p| p.owner() == who && scope.unit(p.kind()).is_some_and(|d| d.ship)) {
                    let ship = units.remove(at);
                    core.board_mut().sys_mut(destination).units.push(ship);
                }
            }
            (Fx::Frontline, Opt::Spot(system, planet)) => self.place(core, self.env.infantry, system, Some(planet), 3),
            (Fx::WarEffort, Opt::System(system)) => self.place(core, self.env.cruiser, system, None, 1),
            (Fx::Bombard, Opt::System(system)) => {
                let hit: SmallList<Planet, 8> =
                    board::planets_of(core, system).filter(|p| board::control(core, *p).is_some_and(|owner| owner != who)).collect();
                for planet in hit.as_slice() {
                    fx::exhaust_planet(core, *planet);
                }
            }
            (Fx::Silence, Opt::System(system)) => {
                let activation = core.meta().activation_seq;
                if let Some(p) = core.players_mut().by_seat_mut(who) {
                    p.card_timers.silence_activation = crate::containers::Opt32::some(activation);
                    p.card_timers.silence_system = Some(system);
                }
            }
            (Fx::Mining, Opt::Spot(_, planet)) => {
                let worth = economy::value_now(core, self.env.scope, self.env.blanked, planet, economy::Spend::Resources, 0);
                self.goods(core, who, worth);
            }
            (Fx::Uprising, Opt::Spot(_, planet)) => {
                let worth = economy::value_now(core, self.env.scope, self.env.blanked, planet, economy::Spend::Resources, 0);
                fx::exhaust_planet(core, planet);
                self.goods(core, who, worth);
            }
            (Fx::Pressure, Opt::Seat(victim)) => {
                self.victim = Some(victim);
                return true;
            }
            (Fx::Pressure, Opt::Note(key)) => {
                core.cards_mut().promissory.edit().insert(key, who);
                let faceup = crate::query::promissory::is_play_area(key.note, key.faction)
                    && !crate::query::promissory::is_action_placed(core, key.note)
                    && key.note != crate::generated::PromissoryNote::Antivirus;
                if faceup {
                    core.cards_mut().promissory_faceup.edit().insert(key, ());
                }
            }
            (Fx::Exchange, Opt::Seat(other)) => {
                self.taken = Some((other, None));
                self.step = 1;
                return true;
            }
            (Fx::Exchange, Opt::Spot(system, planet)) => {
                self.offer = Some((system, planet));
                self.step = 2;
                return true;
            }
            (Fx::Exchange, Opt::Answer(accepted)) => {
                let (Some((other, _)), Some((system, planet))) = (self.taken, self.offer) else { return false };
                if accepted {
                    fx::place_unit(core, other, system, Some(planet), crate::generated::Unit::InfantryI);
                    fx::place_unit(core, who, system, Some(planet), crate::generated::Unit::InfantryI);
                } else {
                    Self::refuse_exchange(core, who, other);
                }
            }
            (Fx::Lucky, Opt::Ship { system, index, .. }) => {
                let units = &mut core.board_mut().sys_mut(system).units;
                if usize::from(index) < units.len() {
                    units.remove(usize::from(index));
                }
            }
            (Fx::Scuttle, Opt::Ship { system, index, .. }) => {
                if self.first_ship.is_none() && self.opts.len() > 1 {
                    self.first_ship = Some((system, index));
                    return true;
                }
                let mut picked: SmallList<(System, u8), 2> = SmallList::new();
                if let Some(first) = self.first_ship {
                    picked.push(first);
                }
                picked.push((system, index));
                self.scuttle(core, picked.as_slice());
            }
            (Fx::Scuttle, Opt::Stop) => {
                if let Some(first) = self.first_ship {
                    self.scuttle(core, &[first]);
                }
            }
            (Fx::Seize, Opt::Seat(victim)) => {
                self.taken = Some((victim, None));
                self.step = 1;
                return true;
            }
            (Fx::Seize, Opt::Fragment(kind)) => {
                let Some((victim, _)) = self.taken else { return false };
                let count = core.players().by_seat(victim).and_then(|p| p.cards.fragments.get(kind)).unwrap_or(0);
                if count <= 0 {
                    return false;
                }
                if let Some(p) = core.players_mut().by_seat_mut(victim) {
                    if count - 1 == 0 {
                        p.cards.fragments.remove(kind);
                    } else {
                        p.cards.fragments.set(kind, count - 1);
                    }
                }
                if let Some(p) = core.players_mut().by_seat_mut(who) {
                    let mine = p.cards.fragments.get(kind).unwrap_or(0);
                    p.cards.fragments.set(kind, mine + 1);
                }
            }
            (Fx::Reparations, Opt::Spot(_, planet)) => {
                if self.step == 0 {
                    fx::exhaust_planet(core, planet);
                    self.step = 1;
                    return true;
                }
                fx::ready_planet(core, planet);
            }
            (Fx::Dig, Opt::Spot(_, planet)) => {
                let Some(deck) = self.dig_deck(core, planet) else { return false };
                for _ in 0..3 {
                    // The deck ran out before the third card.
                    let d = core.decks_mut();
                    // `rules::explore::draw` (the exploration area): the top card of one deck, none when the deck is absent or empty.
                    let drawn = if d.explore_present & (1 << deck as u8) == 0 { None } else { d.explore[deck as usize].draw() };
                    let Some(card) = drawn else { break };
                    if card.data().resolution == crate::generated::ExploreResolution::Fragment {
                        let kind = card.data().deck;
                        if let Some(p) = core.players_mut().by_seat_mut(who) {
                            let held = p.cards.fragments.get(kind).unwrap_or(0);
                            p.cards.fragments.set(kind, held + 1);
                        }
                    }
                }
            }
            (Fx::Disgrace, Opt::Strat(picked)) => {
                let Some((picker, _)) = self.taken else { return false };
                if let Some(at) = core.strategy().unclaimed.as_slice().iter().position(|c| *c == picked) {
                    core.strategy_mut().unclaimed.remove(at);
                    fx::deal_strategy_card(core, picker, picked);
                }
            }
            (Fx::Repeal, Opt::Law(law)) => {
                fx::repeal_law(core, law);
            }
            (Fx::Spy, Opt::Seat(victim)) => {
                let held = core.players().by_seat(victim).map_or(0, |p| p.cards.hand.len());
                if held == 0 {
                    return false;
                }
                // "Random" comes from the seeded dice too.
                let face = core.rng_mut().die(RngDomain::Dice, seeds, DIE_SIDES);
                self.faces.push(face);
                let index = (face as usize - 1) % held;
                if let Some(taken) = super::discard_from_hand(core, victim, index)
                    && let Some(p) = core.players_mut().by_seat_mut(who)
                {
                    p.cards.hand.push(taken);
                }
            }
            (Fx::Plague, Opt::Spot(system, planet)) => {
                let scope = self.env.scope;
                let infantry = core
                    .board()
                    .planet(planet)
                    .units
                    .as_slice()
                    .iter()
                    .filter(|p| scope.unit(p.kind()).map(|d| d.base_type) == Some(UnitBaseType::Infantry))
                    .count();
                if infantry == 0 {
                    return false;
                }
                let mut kills = 0;
                for _ in 0..infantry {
                    let face = core.rng_mut().die(RngDomain::Dice, seeds, DIE_SIDES);
                    self.faces.push(face);
                    kills += usize::from(face >= PLAGUE_KILLS_ON);
                }
                let reason = DestroyReason::ActionCard(ActionCardCause::Plague);
                if fx_ground::destroy_on_planet(core, scope, system, planet, UnitBaseType::Infantry, Some(kills), reason).is_err() {
                    self.unsupported = true;
                }
            }
            (Fx::Unstable, Opt::Spot(system, planet)) => {
                fx::exhaust_planet(core, planet);
                let reason = DestroyReason::ActionCard(ActionCardCause::UnstablePlanet);
                if fx_ground::destroy_on_planet(core, self.env.scope, system, planet, UnitBaseType::Infantry, Some(3), reason).is_err() {
                    self.unsupported = true;
                }
            }
            (Fx::PirateContract, Opt::System(system)) => {
                if let Some(neutral) = self.env.facts.neutral {
                    fx::place_unit(core, neutral, system, None, crate::generated::Unit::NeutralDestroyer);
                }
            }
            (Fx::Bribery, Opt::Count(n)) => {
                if n > 0 && fx_pay::pay_trade_goods(core, who, i32::from(n)) {
                    let agenda = core.meta().agenda_seq;
                    if let Some(p) = core.players_mut().by_seat_mut(who) {
                        let running = match p.card_timers.extra_votes_agenda {
                            Some(held) if held.agenda == agenda => held.votes,
                            _ => 0,
                        };
                        p.card_timers.extra_votes_agenda = Some(crate::state::ExtraVotes { agenda, votes: running + i64::from(n) });
                    }
                }
            }
            (Fx::Crash, Opt::Kind(kind)) => {
                self.landing = Some(kind);
                self.step = 1;
                return true;
            }
            (Fx::Crash, Opt::Spot(system, planet)) => {
                let Some(kind) = self.landing else { return false };
                let who = self.who;
                let units = &mut core.board_mut().sys_mut(system).units;
                let Some(at) = units.as_slice().iter().position(|p| p.owner() == who && p.kind() == kind) else { return false };
                let landed = units.remove(at);
                let cell = core.board_mut().planet_mut(planet);
                let others_there = cell.units.as_slice().iter().any(|p| p.owner() != who);
                cell.units.push(Piece::new(landed.kind(), who));
                cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
                if others_there && !cell.coexist.contains(who) {
                    cell.coexist.insert(who);
                    cell.flags.set(crate::state::PlanetFlags::COEXIST_ENTRY, true);
                }
            }
            (Fx::Lie, Opt::Card(card)) => {
                let Some(victim) = self.victim else { return false };
                let at = core.players().by_seat(victim).and_then(|p| p.cards.hand.iter().position(|c| *c == card));
                let Some(taken) = at.and_then(|i| super::discard_from_hand(core, victim, i)) else { return false };
                if let Some(p) = core.players_mut().by_seat_mut(who) {
                    p.cards.hand.push(taken);
                }
                self.step += 1;
                return true;
            }
            (Fx::Focused, Opt::Tech(t)) => {
                fx_pay::pay_trade_goods(core, who, 4);
                self.research(core, k, t);
            }
            (Fx::Divert, Opt::Tech(t)) if self.step == 0 => {
                if let Some(p) = core.players_mut().by_seat_mut(who) {
                    p.tech.owned.remove(t);
                }
                self.step = 1;
                return true;
            }
            (Fx::Divert, Opt::Tech(t)) => self.research(core, k, t),
            (Fx::Reveal, Opt::Tech(t)) => {
                // The four resources are paid first (a child window), then the research lands.
                self.chosen = Some(t);
            }
            (Fx::Plagiarize, Opt::Steal { from, tech }) => {
                fx::grant_technology(core, who, tech);
                if let Some(roster) = self.env.facts.roster {
                    crate::kernel::fx_strategy::apply_unit_upgrades(core, k, self.env.scope, roster, who);
                }
                if let Some(p) = core.players_mut().by_seat_mut(from) {
                    p.tech.owned.remove(tech);
                }
            }
            (Fx::Brilliance, Opt::Seat(target)) => {
                let Some(own) = self.env.facts.own_breakthrough.get(usize::from(target.0)).copied().flatten() else { return false };
                if let Some(p) = core.players_mut().by_seat_mut(target) {
                    p.cards.breakthrough = Some(own);
                }
                if !self.env.facts.fracture_in_play {
                    // The Fracture's roll (nothing for the Cabal's, which enters play without one); a hit puts it into play with
                    // questions of its own, which the window does not carry.
                    let enters = own.data().faction == crate::generated::FactionTag::Cabal || {
                        let face = core.rng_mut().die(RngDomain::Fracture, seeds, 10);
                        face == 1 || face == 10
                    };
                    if enters {
                        self.unsupported = true;
                    }
                }
            }
            (Fx::Squad, Opt::Move { from, to, kind, .. }) => {
                let moved: SmallList<Piece, 12> = board::planet_units(core, from).iter().filter(|p| p.owner() == who && p.kind() == kind).copied().collect();
                if moved.is_empty() {
                    return false;
                }
                let ground = self.env.scope.unit(kind).is_some_and(|d| d.ground_force);
                core.board_mut().planet_mut(from).units.retain(|p| !(ground && p.owner() == who && p.kind() == kind));
                let cell = core.board_mut().planet_mut(to);
                for piece in moved.as_slice() {
                    cell.units.push(*piece);
                }
                cell.flags.set(crate::state::PlanetFlags::UNITS_ENTRY, true);
                self.received.push(to);
                return true;
            }
            (Fx::Decoy, Opt::Troop { system, planet, index, .. }) => {
                self.pulled.push((system, planet, index));
                self.step = if self.step == 0 { 1 } else { 2 };
                return true;
            }
            (Fx::Decoy, Opt::Stop) => {
                self.step = 2;
                return true;
            }
            (Fx::Decoy, Opt::Spot(system, planet)) => self.land(core, system, planet),
            (Fx::Refit, Opt::Troop { system, planet, index, .. }) => {
                let Some(first) = self.troop else {
                    self.troop = Some((system, planet, index));
                    return true;
                };
                self.refit(core, &[first, (system, planet, index)]);
            }
            (Fx::Refit, Opt::Stop) => {
                if let Some(first) = self.troop {
                    self.refit(core, &[first]);
                }
            }
            (Fx::Upgrade, Opt::Ship { system, index, .. }) => {
                let units = &mut core.board_mut().sys_mut(system).units;
                if usize::from(index) < units.len() {
                    units.remove(usize::from(index));
                }
                if let Some(unit) = self.env.facts.dreadnought.unit {
                    fx::place_unit(core, who, system, None, unit);
                }
            }
            (Fx::Jam, Opt::System(system)) => {
                self.jam = Some(system);
                self.step = 1;
                return true;
            }
            (Fx::Jam, Opt::Seat(rival)) => {
                if let Some(system) = self.jam {
                    fx::place_command_token(core, rival, system);
                }
            }
            (Fx::Ghost, Opt::System(system)) => self.place(core, self.env.facts.destroyer, system, None, 1),
            // The token is taken and the card drawn once the pick is applied (`explore`, which has the kernel's services).
            (Fx::Probe, Opt::System(system)) => self.probe = Some(system),
            (Fx::Mercenary, Opt::Spot(system, planet)) => {
                if let Some(neutral) = self.env.facts.neutral {
                    for _ in 0..2 {
                        fx::place_unit(core, neutral, system, Some(planet), crate::generated::Unit::NeutralInfantry);
                    }
                }
                fx_pay::pay_trade_goods(core, who, 2);
            }
            (Fx::PirateFleet, Opt::System(system)) => {
                use crate::generated::Unit as U;
                if let Some(neutral) = self.env.facts.neutral {
                    for unit in [U::NeutralCarrier, U::NeutralCruiser, U::NeutralDestroyer, U::NeutralFighter, U::NeutralFighter] {
                        fx::place_unit(core, neutral, system, None, unit);
                    }
                }
            }
            (Fx::Cripple | Fx::Meltdown, Opt::Spot(_, planet)) => {
                let (base, limit) = if self.fx == Fx::Cripple { (UnitBaseType::Pds, usize::MAX) } else { (UnitBaseType::Spacedock, 1) };
                let scope = self.env.scope;
                let mut left = limit;
                core.board_mut().planet_mut(planet).units.retain(|p: &Piece| {
                    let hit = left > 0 && scope.unit(p.kind()).map(|d| d.base_type) == Some(base);
                    if hit {
                        left -= 1;
                    }
                    !hit
                });
            }
            _ => {}
        }
        false
    }

    /// Compute the options of the next question and apply the options that need no asking.
    fn advance(&mut self, core: &mut CoreState, k: &Kernel, seeds: &dyn SeedSource) {
        loop {
            if self.fx == Fx::Lie {
                // The next trader (of the first two) who holds a card.
                self.victim = None;
                while usize::from(self.step) < self.env.facts.traders.len() {
                    let held = self.env.facts.traders[usize::from(self.step)]
                        .filter(|v| core.players().by_seat(*v).is_some_and(|p| !p.cards.hand.is_empty()));
                    if let Some(v) = held {
                        self.victim = Some(v);
                        break;
                    }
                    self.step += 1;
                }
            }
            self.opts = self.options(core, k);
            match self.opts.as_slice() {
                [] if self.fx == Fx::Exchange && self.step == 1 => {
                    // Nothing to offer is a failed deal: both sides pay the fleet token.
                    if let Some((other, _)) = self.taken {
                        Self::refuse_exchange(core, self.who, other);
                    }
                    self.stage = Stage::Done;
                    return;
                }
                [] if self.fx == Fx::Reparations && self.step == 0 => {
                    // The new controller has no planet to exhaust: the holder readies one.
                    self.step = 1;
                }
                [] => {
                    self.stage = Stage::Done;
                    return;
                }
                [only] if self.fx != Fx::Squad => {
                    let only = *only;
                    if !self.apply(core, k, seeds, only) {
                        self.stage = self.settled();
                        return;
                    }
                }
                _ => {
                    self.stage = Stage::Asking;
                    return;
                }
            }
        }
    }

    fn actor(&self) -> Seat {
        match self.fx {
            Fx::Pressure => self.victim.unwrap_or(self.who),
            Fx::Disgrace => self.taken.map_or(self.who, |(picker, _)| picker),
            Fx::Reparations if self.step == 0 => self.taken.map_or(self.who, |(gainer, _)| gainer),
            Fx::Exchange if self.step == 2 => self.taken.map_or(self.who, |(other, _)| other),
            _ => self.who,
        }
    }

    /// Command tokens Summit gains.
    const SUMMIT_TOKENS: u8 = 2;

    /// The question as the driver names it (Summit asks the strategy area's pool question).
    pub(super) fn any_ask(&self) -> crate::choice::Ask {
        if let (Stage::Paying | Stage::Limiting | Stage::Rewarding, Some(ask)) = (self.stage, self.pay_ask) {
            return ask;
        }
        if self.stage == Stage::Pooling {
            return crate::choice::Ask::Strategy(crate::choice::strategy::Ask::GainPool { left: self.pool_left });
        }
        if self.fx == Fx::Summit {
            return crate::choice::Ask::Strategy(crate::choice::strategy::Ask::GainPool { left: Self::SUMMIT_TOKENS - self.step });
        }
        crate::choice::Ask::Cards(self.ask_value())
    }

    pub(super) fn ask_value(&self) -> Ask {
        match (self.what(), self.frame) {
            (Some(what), _) => Ask::Pick { card: self.card, what },
            (None, Some(frame)) => Ask::Predict { card: self.card, frame },
            (None, None) => Ask::Pick { card: self.card, what: What::Player },
        }
    }
}

impl Play {
    /// The part of the play before its first question (the effect that needs no pick, the bill, then the options of the
    /// first question); `asking` says whether it waits on a pick afterwards.
    pub fn begin(&mut self, sx: &mut Sx<'_>) {
        self.begin_play(sx);
        self.close_explored(sx.state);
    }

    fn begin_play(&mut self, sx: &mut Sx<'_>) {
        if self.stage != Stage::Start {
            return;
        }
        self.before(sx.state);
        // Rules version 23: with nothing to take the five influence are never spent (older games paid first).
        if self.fx == Fx::Plagiarize && self.env.facts.rules_version >= RULES_ACTION_CARD_COSTS && self.options(sx.state, sx.kernel).is_empty() {
            self.stage = Stage::Done;
            return;
        }
        if let Some((kind, cost)) = self.fx.bill() {
            self.pay.start(PayBill::new(self.who, kind, cost));
            self.stage = Stage::Paying;
            self.drive_bill(sx);
            return;
        }
        self.start_questions(sx.state, sx.kernel, sx.seeds);
        self.maybe_bill(sx);
        self.maybe_explore(sx);
    }

    /// Where a card stands once its last pick is applied: Reveal Prototype owes its bill, Exploration Probe explores, the rest is done.
    fn settled(&self) -> Stage {
        if self.fx == Fx::Reveal && self.chosen.is_some() {
            Stage::Billing
        } else if self.fx == Fx::Probe && self.probe.is_some() {
            Stage::Exploring
        } else {
            Stage::Done
        }
    }

    /// Reveal Prototype: the technology is chosen, the four resources are paid by a child window.
    fn maybe_bill(&mut self, sx: &mut Sx<'_>) {
        if self.stage == Stage::Billing {
            self.pay.start(PayBill::new(self.who, economy::Spend::Resources, 4));
            self.stage = Stage::Paying;
            self.drive_bill(sx);
        }
    }

    /// What follows a paid bill: the questions of the card, or Impersonation's draw and the hand limit.
    fn after_bill(&mut self, sx: &mut Sx<'_>) {
        if self.fx == Fx::Reveal {
            if let Some(t) = self.chosen.take() {
                self.research(sx.state, sx.kernel, t);
            }
            self.stage = Stage::Done;
            return;
        }
        if self.fx != Fx::Impersonation {
            self.start_questions(sx.state, sx.kernel, sx.seeds);
            return;
        }
        self.draw_secret(sx);
    }

    /// A secret objective is drawn (a deck that has run out draws nothing) and the hand limit enforced after a draw.
    fn draw_secret(&mut self, sx: &mut Sx<'_>) {
        if fx::draw_secret(sx.state, self.who).is_none() {
            self.stage = Stage::Done;
            return;
        }
        self.hand.start(HandLimit::new(self.who));
        self.stage = Stage::Limiting;
        self.drive_hand(sx);
    }

    /// Exploration Probe, `exploration_probe` and `explore_with(.., FRONTIER, None)`: the token is spent, the top card of the
    /// frontier deck is drawn and its log row opened (its texts are the adapter's), then the card resolves as far as its first
    /// question. Without a planet and without the system of the token, the card acts on the holder alone: a fragment, command
    /// tokens and trade goods, a relic, a secret objective, action cards, a Merchant Station's choice, the Enigmatic Device
    /// kept in the play area, a gamma token where the active system is; the ion storm, the Mirage and any card another deck
    /// holds do nothing or are not carried (`unsupported`).
    fn maybe_explore(&mut self, sx: &mut Sx<'_>) {
        if self.stage != Stage::Exploring {
            return;
        }
        self.stage = Stage::Done;
        let who = self.who;
        let Some(system) = self.probe.take() else { return };
        let core = &mut *sx.state;
        // The token goes first: a deck that has run out (or is not dealt) loses the token for nothing.
        if !explore::take_frontier(core, system) {
            return;
        }
        let Some(card) = explore::draw(core, ExploreType::Frontier) else { return };
        explore::open_record(core, who, ExploreType::Frontier, card, None, None, self.env.facts.explore_empty);
        self.held = Some((crate::query::explore::holdings(core, who, None), None));
        let data = card.data();
        let (outcome, goods) = match data.resolution {
            ExploreResolution::Fragment => {
                explore::gain_fragment(core, who, data.deck);
                (Explored::Fragment(data.deck), None)
            }
            // A card of another deck (an attachment, a planet's reward) is not in the frontier deck of any game.
            ExploreResolution::Attach => return self.decline(),
            ExploreResolution::Instant | ExploreResolution::Token => match card {
                Explore::MinorEntropicField => (Explored::Resolved, Some((1, 1))),
                Explore::EntropicField => (Explored::Resolved, Some((1, 2))),
                Explore::MajorEntropicField => (Explored::Resolved, Some((1, 3))),
                Explore::KeleresShip1 | Explore::KeleresShip2 => (Explored::Resolved, Some((2, 0))),
                Explore::DeadWorld => {
                    explore::relic_gain(core, who);
                    (Explored::Resolved, None)
                }
                Explore::DerelictVessel1 | Explore::DerelictVessel2 => {
                    self.explored = Some((card, Explored::Resolved));
                    return self.draw_secret(sx);
                }
                Explore::EnigmaticDevice1 | Explore::EnigmaticDevice2 => {
                    explore::play_area_card(core, who, card);
                    (Explored::Resolved, None)
                }
                Explore::GammaRelay | Explore::GammaWormhole => match core.meta().active_system {
                    Some(here) => {
                        if core.explore().wormhole_tokens.get(&Wormhole::Gamma) != Some(&here) {
                            self.gamma = Some(here);
                        }
                        explore::place_wormhole_token(core, Wormhole::Gamma, here);
                        (Explored::Resolved, None)
                    }
                    None => (Explored::Unresolved, None),
                },
                // "This system": the draw names no system and no planet, so these cards have nowhere to act.
                Explore::IonStorm | Explore::Mirage => (Explored::Unresolved, None),
                Explore::LostCrew1 | Explore::LostCrew2 => {
                    // A hand over its limit, a draw hook (Scheming) and the unofficial cards are the string's.
                    if !self.env.facts.action_draw_plain {
                        return self.decline();
                    }
                    fx::draw_action_cards(core, who, 2, self.env.facts.rules_version >= RULES_OFFICIAL_ACTION_DECK);
                    (Explored::Resolved, None)
                }
                Explore::MerchantStation1 | Explore::MerchantStation2 => {
                    self.explored = Some((card, Explored::Resolved));
                    return self.merchant_station(sx);
                }
                _ => return self.decline(),
            },
        };
        self.explored = Some((card, outcome));
        if let Some((tokens, goods)) = goods {
            self.owed_goods = goods;
            self.gain_tokens(sx, tokens);
        }
    }

    /// The run holds something the window does not carry: the adapter declines it.
    fn decline(&mut self) {
        self.unsupported = true;
        self.stage = Stage::Done;
    }

    /// `gain_command_tokens`: from rules version 12 each token goes into a pool of the holder's choice (the question of 52.4),
    /// before that all go into the strategy pool. Then the trade goods the card owes.
    fn gain_tokens(&mut self, sx: &mut Sx<'_>, count: u8) {
        // The reinforcements the pool question previews (and the cap) count the tokens a Mahact fleet holds.
        if count > 0 && self.env.mahact_held > 0 {
            return self.decline();
        }
        if count > 0 && self.env.facts.rules_version >= RULES_COMMAND_TOKEN_POOL {
            self.pool_left = count;
            self.stage = Stage::Pooling;
            return;
        }
        if count > 0 {
            fx::gain_token(sx.state, self.who, crate::choice::strategy::Pool::Strategic, i32::from(count));
        }
        self.finish_goods(sx);
    }

    /// The trade goods of an entropic field, gained after its command token (no announcement: the string explores with no timing).
    fn finish_goods(&mut self, sx: &mut Sx<'_>) {
        fx::gain_trade_goods(sx.state, self.who, self.owed_goods);
        self.owed_goods = 0;
        self.stage = Stage::Done;
    }

    /// Merchant Station: "replenish your commodities, or convert your commodities to trade goods", asked unless only one is offered
    /// (from rules version 19 a seat without commodities has nothing to convert).
    fn merchant_station(&mut self, sx: &mut Sx<'_>) {
        let has_commodities = sx.state.players().by_seat(self.who).is_some_and(|p| p.econ.commodities >= 1);
        let mut options = vec![EOpt::Replenish];
        if self.env.facts.rules_version < RULES_NO_EMPTY_CONVERT || has_commodities {
            options.push(EOpt::Convert);
        }
        self.reward.start(Reward::new(self.who, EStep::MerchantStation, &options, true));
        self.stage = Stage::Rewarding;
        self.drive_reward(sx);
    }

    /// Step the Merchant Station's question until it asks or ends; the answer is then carried out.
    fn drive_reward(&mut self, sx: &mut Sx<'_>) {
        self.pay_ask = None;
        loop {
            match self.reward.step(sx) {
                Some(Ran::Asks(ask)) => {
                    self.pay_ask = Some(ask);
                    return;
                }
                Some(Ran::Pass(_)) => {}
                Some(Ran::Done(window)) => {
                    self.stage = Stage::Done;
                    if window.chosen == Some(EOpt::Convert) {
                        let moved = explore::spend_commodities(sx.state, self.who, None);
                        fx::gain_trade_goods(sx.state, self.who, moved);
                    } else if self.env.facts.replenish_events {
                        // The Siphon event, Cabal's note and the Trade Agreement hook follow a replenishment: the string's.
                        self.decline();
                    } else if let Some(limit) = sx.state.players().by_seat(self.who).map(|p| i32::from(p.faction.data().commodities)) {
                        explore::gain_commodities(sx.state, self.who, limit);
                    }
                    return;
                }
                None => {
                    self.stage = Stage::Done;
                    return;
                }
            }
        }
    }

    /// Step the hand limit until it asks or ends.
    fn drive_hand(&mut self, sx: &mut Sx<'_>) {
        self.pay_ask = None;
        loop {
            match self.hand.step(sx) {
                Some(Ran::Asks(ask)) => {
                    self.pay_ask = Some(ask);
                    return;
                }
                Some(Ran::Pass(_)) => {}
                Some(Ran::Done(_)) | None => {
                    self.stage = Stage::Done;
                    return;
                }
            }
        }
    }

    /// The first question of the card (after the bill, when it has one).
    fn start_questions(&mut self, core: &mut CoreState, k: &Kernel, seeds: &dyn SeedSource) {
        if self.fx == Fx::Summit {
            self.stage = Stage::Asking;
            return;
        }
        if self.what().is_none() && !matches!(self.fx, Fx::Rider(_) | Fx::Lie) {
            self.stage = Stage::Done;
            return;
        }
        self.advance(core, k, seeds);
    }

    /// Step the bill until it asks or ends; a paid bill goes on to the questions, an unpaid one ends the card.
    fn drive_bill(&mut self, sx: &mut Sx<'_>) {
        self.pay_ask = None;
        loop {
            match self.pay.step(sx) {
                Some(Ran::Asks(ask)) => {
                    self.pay_ask = Some(ask);
                    return;
                }
                Some(Ran::Pass(_)) => {}
                Some(Ran::Done(bill)) => {
                    if bill.outcome == Some(true) {
                        self.after_bill(sx);
                    } else {
                        self.stage = Stage::Done;
                    }
                    return;
                }
                None => {
                    self.stage = Stage::Done;
                    return;
                }
            }
        }
    }

    /// The dice rolled, in order.
    #[must_use]
    pub fn faces(&self) -> &[u32] {
        self.faces.as_slice()
    }

    /// Exploration Probe: the card the frontier deck gave, how it resolved and what the seat held around it (`None`: no card was
    /// drawn, or the play has not finished).
    #[must_use]
    pub fn explored(&self) -> Option<ProbeRow> {
        let ((card, outcome), (before, Some(after))) = (self.explored?, self.held?) else { return None };
        Some(ProbeRow { card, outcome, before, after, gamma: self.gamma })
    }

    /// Exploration Probe: the seat's holdings once the card has resolved.
    fn close_explored(&mut self, core: &CoreState) {
        if self.stage == Stage::Done
            && let Some((_, after @ None)) = &mut self.held
        {
            *after = Some(crate::query::explore::holdings(core, self.who, None));
        }
    }

    /// Whether the position held something the window does not express (its result is not to be compared).
    #[must_use]
    pub const fn unsupported(&self) -> bool {
        self.unsupported
    }

    /// Whether the play waits on the holder's pick (or on a payment).
    #[must_use]
    pub fn asking(&self) -> bool {
        self.stage == Stage::Asking
            || self.stage == Stage::Pooling
            || (matches!(self.stage, Stage::Paying | Stage::Limiting | Stage::Rewarding) && self.pay_ask.is_some())
    }

    /// Apply a pick to the pending question (`Window::answer` without the services).
    ///
    /// # Errors
    /// The pick is not an option of the pending question.
    pub fn choose(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        let answered = self.choose_play(sx, pick);
        self.close_explored(sx.state);
        answered
    }

    fn choose_play(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage == Stage::Paying {
            self.pay.answer(sx, pick)?;
            self.drive_bill(sx);
            return Ok(());
        }
        if self.stage == Stage::Limiting {
            self.hand.answer(sx, pick)?;
            self.drive_hand(sx);
            return Ok(());
        }
        if self.stage == Stage::Rewarding {
            self.reward.answer(sx, pick)?;
            self.drive_reward(sx);
            return Ok(());
        }
        if self.stage == Stage::Pooling {
            let Pick::Opt(crate::choice::Opt::Strategy(crate::choice::strategy::Opt::Pool(pool))) = pick else { return Err(Illegal) };
            fx::gain_token(sx.state, self.who, pool, 1);
            self.pool_left = self.pool_left.saturating_sub(1);
            if self.pool_left == 0 {
                self.finish_goods(sx);
            }
            return Ok(());
        }
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let (seeds, k) = (sx.seeds, sx.kernel);
        let core = &mut *sx.state;
        if self.fx == Fx::Summit {
            let Pick::Opt(crate::choice::Opt::Strategy(crate::choice::strategy::Opt::Pool(pool))) = pick else { return Err(Illegal) };
            fx::gain_token(core, self.who, pool, 1);
            self.step += 1;
            if self.step >= Self::SUMMIT_TOKENS {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if self.fx == Fx::Squad && matches!(pick, Pick::Opt(crate::choice::Opt::Decline)) {
            self.stage = Stage::Done;
            return Ok(());
        }
        let Pick::Opt(crate::choice::Opt::Cards(opt)) = pick else { return Err(Illegal) };
        if !self.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        if self.apply(core, k, seeds, opt) {
            self.advance(core, k, seeds);
        } else {
            self.stage = self.settled();
        }
        self.maybe_bill(sx);
        self.maybe_explore(sx);
        Ok(())
    }
}

impl Window for Play {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        self.begin(sx);
        match self.stage {
            Stage::Asking => Next::Continue(self.any_ask()),
            Stage::Pooling => Next::Continue(self.any_ask()),
            Stage::Paying | Stage::Limiting | Stage::Rewarding => self.pay_ask.map_or(Next::Done, Next::Continue),
            Stage::Start | Stage::Billing | Stage::Exploring | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        if self.stage == Stage::Paying {
            return self.pay.ask_of(s, k);
        }
        if self.stage == Stage::Limiting {
            return self.hand.ask_of(s, k);
        }
        if self.stage == Stage::Rewarding {
            return self.reward.ask_of(s, k);
        }
        if self.stage == Stage::Pooling {
            return Some(crate::choice::strategy::gain_pool_choice(self.who, self.pool_left));
        }
        if self.fx == Fx::Summit {
            return (self.stage == Stage::Asking).then(|| crate::choice::strategy::gain_pool_choice(self.who, Self::SUMMIT_TOKENS - self.step));
        }
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.actor(),
            ask: crate::choice::Ask::Cards(self.ask_value()),
            ctx: Ctx {
                origin: if self.frame.is_some() { Origin::Rule(Rule::R8) } else { Origin::ActionCard(self.card) },
                subtype: crate::choice::Subtype::Cards(match self.what() {
                    Some(what) => Subtype::Pick { card: self.card, what },
                    None => Subtype::PredictOutcome,
                }),
                optional: false,
                // Exchange Program's answer is about the seat that asks.
                target: if matches!(self.fx, Fx::Squad | Fx::Crash) {
                    self.jam.map(crate::choice::Target::System)
                } else {
                    (self.fx == Fx::Exchange && self.step == 2).then_some(crate::choice::Target::Seat(self.who))
                },
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: {
                let mut opts = self.opts.as_slice().iter().map(|o| crate::choice::Opt::Cards(*o)).collect::<Opts>();
                if self.fx == Fx::Squad {
                    opts.push(crate::choice::Opt::Decline);
                }
                opts
            },
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        self.choose(sx, pick)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// One check for every documented bail of the play window: the card has no `Fx`, so the string effect runs.
    #[test]
    fn every_bail_has_no_window_and_the_ported_cards_have_one() {
        use ActionCard as C;
        for (card, why) in [
            (C::DirectHit1, "the exact sustain mark and the ship-lost observers"),
            (C::DirectHit4, "the exact sustain mark and the ship-lost observers"),
            (C::SkilledRetreat1, "destinations and previews"),
            (C::CourageousToTheEnd, "dice and a casualty choice"),
            (C::Strategize1, "performs another area's ability"),
        ] {
            assert_eq!(Fx::of(card), None, "{card:?}: {why}");
        }
        for card in [
            C::Sabotage3,
            C::Veto4,
            C::DiplomaticPressure2,
            C::Sanction,
            C::Uprising,
            C::Counterstroke,
            C::TacticalBombardment,
            C::RiseOfAMessiah,
            C::FrontlineDeployment,
            C::WarEffort,
            C::InsiderInformation,
            C::Intercept,
            C::Rescue,
            C::FighterConscription,
            C::ManipulateInvestments,
            C::RepealLaw,
            C::Spy,
            C::Plague,
            C::UnstablePlanet,
            C::SeizeArtifact,
            C::Reparations,
            C::PublicDisgrace,
            C::ArchaeologicalExpedition,
            C::Summit,
            C::LuckyShot,
            C::Scuttle,
            C::PirateContract1,
            C::PirateContract4,
            C::PirateFleet,
            C::MercenaryContract,
            C::GhostShip,
            C::SignalJamming,
            C::Upgrade,
            C::RefitTroops,
            C::DecoyOperation,
            C::GhostSquad,
            C::HarnessEnergy,
            C::Brilliance,
            C::Impersonation,
            C::Plagiarize,
            C::FocusedResearch,
            C::DivertFunding,
            C::RevealPrototype,
            C::LieInWait,
            C::Bribery,
            C::Infiltrate,
            C::CrashLanding,
            C::ExplorationProbe,
        ] {
            assert!(Fx::of(card).is_some(), "{card:?}");
        }
    }

    fn bare_env() -> Env {
        Env {
            mahact_held: 0,
            scope: crate::tables::Scope(u32::MAX),
            blanked: false,
            action_card: crate::state::Code(0),
            supply_base: crate::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            infantry: super::super::Placed::NONE,
            cruiser: super::super::Placed::NONE,
            fighter: super::super::Placed::NONE,
            facts: super::super::Facts::default(),
        }
    }

    /// Bail: Summit while the holder has a command token in a Mahact fleet pool (the reinforcements the pool question
    /// previews differ from the core's count).
    #[test]
    fn bail_summit_declines_while_a_command_token_is_held_in_a_mahact_pool() {
        let core = CoreState::all_loaded();
        let who = Seat(0);
        assert!(Play::of(&core, ActionCard::Summit, who, bare_env()).is_some());
        assert!(Play::of(&core, ActionCard::Summit, who, Env { mahact_held: 1, ..bare_env() }).is_none());
    }

    /// Bail: a card that pays a bill (Pirate Fleet) while a readied Keleres agent could be offered before the payment.
    #[test]
    fn bail_a_paid_card_declines_while_the_keleres_agent_could_be_offered() {
        let core = CoreState::all_loaded();
        let facts = super::super::Facts { neutral: Some(Seat(1)), agent_offered: false, ..super::super::Facts::default() };
        let env = Env { facts, ..bare_env() };
        assert!(Play::of(&core, ActionCard::PirateFleet, Seat(0), env).is_some());
        let env = Env { facts: super::super::Facts { agent_offered: true, ..facts }, ..bare_env() };
        assert!(Play::of(&core, ActionCard::PirateFleet, Seat(0), env).is_none());
        // A card without a bill does not care.
        assert!(Play::of(&core, ActionCard::PirateContract1, Seat(0), env).is_some());
    }

    /// Bail: Refit Troops for a seat whose mech is a faction unit announced when placed (no plain mech to place).
    #[test]
    fn bail_refit_troops_declines_without_a_plain_mech() {
        let core = CoreState::all_loaded();
        let with_mech = Env { facts: super::super::Facts { mech: super::super::Placed { unit: Some(crate::generated::Unit::FactionlessMech), on_card: 0 }, ..super::super::Facts::default() }, ..bare_env() };
        assert!(Play::of(&core, ActionCard::RefitTroops, Seat(0), with_mech).is_some());
        assert!(Play::of(&core, ActionCard::RefitTroops, Seat(0), bare_env()).is_none());
    }

    /// Bail: Brilliance gives the Cabal its breakthrough, which puts the Fracture into play without a roll: the Fracture's
    /// ingress questions are not carried, so the window marks the run unsupported (the adapter declines it).
    #[test]
    fn bail_brilliance_for_the_cabal_marks_the_run_unsupported() {
        use crate::driver::{Driver, Status};
        use crate::generated::{Breakthrough, Faction, FactionTag};
        use crate::kernel::stage::AbilityTable;
        use crate::kernel::Resolver;
        use crate::state::{Player, Players};
        let cabal_bt = Breakthrough::ALL.iter().copied().find(|b| b.data().faction == FactionTag::Cabal).expect("the Cabal breakthrough");
        let other_bt = Breakthrough::ALL.iter().copied().find(|b| b.data().faction != FactionTag::Cabal).expect("another");
        let run = |gain: Breakthrough| {
            let mut s = CoreState::all_loaded();
            s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
            s.seats = crate::state::Seats::new(&[Seat(0), Seat(1)], &[Seat(0), Seat(1)], 2);
            let mut facts = super::super::Facts::default();
            facts.own_breakthrough[1] = Some(gain);
            facts.own_breakthrough[0] = Some(gain);
            let env = Env { facts, ..bare_env() };
            let play = Play::of(&s, ActionCard::Brilliance, Seat(0), env).expect("a play");
            let k = Kernel::plain();
            let mut d = Driver::new(play, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
            let seeds: std::rc::Rc<dyn SeedSource> = std::rc::Rc::new(ZeroSeeds);
            d = d.with_seeds(seeds);
            let mut status = d.run(&mut s, &k).unwrap();
            while let Status::Asking(c) = status {
                let pick = c.pick(1).unwrap();
                status = d.resume(&mut s, &k, pick).unwrap();
            }
            (d.window.unsupported(), s.players().by_seat(Seat(1)).unwrap().cards.breakthrough)
        };
        let (unsupported, held) = run(cabal_bt);
        assert!(unsupported && held == Some(cabal_bt), "the Cabal's enters the Fracture without a roll");
        let _ = other_bt;
    }

    struct ZeroSeeds;

    impl SeedSource for ZeroSeeds {
        fn seed_for(&self, _: u64, _: RngDomain) -> [u8; 32] {
            [7; 32]
        }
    }

    /// Bail: Harness Energy while a replenishment causes events the core does not hold.
    #[test]
    fn bail_harness_energy_declines_while_a_replenishment_causes_events() {
        let core = CoreState::all_loaded();
        let env = Env { facts: super::super::Facts { replenish_events: true, ..super::super::Facts::default() }, ..bare_env() };
        assert!(Play::of(&core, ActionCard::HarnessEnergy, Seat(0), env).is_none());
        assert!(Play::of(&core, ActionCard::HarnessEnergy, Seat(0), bare_env()).is_some());
    }

    /// Bail: a card that places neutral units needs the neutral owner in the table.
    #[test]
    fn bail_a_neutral_card_declines_without_a_neutral_seat() {
        let core = CoreState::all_loaded();
        assert!(Play::of(&core, ActionCard::PirateContract2, Seat(0), bare_env()).is_none());
    }

    /// Bail: Archaeological Expedition while a replay reserve is installed (the string draws through it).
    #[test]
    fn bail_archaeological_expedition_declines_while_a_replay_reserve_is_installed() {
        let mut core = CoreState::all_loaded();
        let who = Seat(0);
        assert!(Play::of(&core, ActionCard::ArchaeologicalExpedition, who, bare_env()).is_some());
        core.cards_mut().reserve = Some(crate::containers::Cow::new(Default::default()));
        assert!(Play::of(&core, ActionCard::ArchaeologicalExpedition, who, bare_env()).is_none());
    }

    // -- Exploration Probe ---------------------------------------------------------------------------------------------

    /// Seat 0 (an Arborec) holds a cruiser in the first system, which holds a frontier token; the frontier deck is `deck`.
    fn probe_state(deck: &[crate::generated::Explore]) -> CoreState {
        use crate::generated::{ExploreType, Faction, Unit};
        use crate::state::{Player, Players};
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
        s.seats = crate::state::Seats::new(&[Seat(0), Seat(1)], &[Seat(0), Seat(1)], 2);
        let system = System::ALL[0];
        s.board_mut().sys_mut(system).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        s.explore_mut().frontier.insert(system);
        s.decks_mut().explore[ExploreType::Frontier as usize].set(deck.to_vec());
        s.decks_mut().explore_present |= 1 << ExploreType::Frontier as u8;
        s
    }

    /// Run a probe of seat 0 to its end, taking the first option of every question; the state after it, whether the run was declined,
    /// and the questions asked.
    fn run_probe(mut s: CoreState, env: Env) -> (CoreState, bool, usize) {
        use crate::driver::{Driver, Status};
        use crate::kernel::stage::AbilityTable;
        use crate::kernel::Resolver;
        let play = Play::of(&s, ActionCard::ExplorationProbe, Seat(0), env).expect("a play");
        let k = Kernel::plain();
        let mut d = Driver::new(play, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        d = d.with_seeds(std::rc::Rc::new(ZeroSeeds));
        let mut asked = 0;
        let mut status = d.run(&mut s, &k).unwrap();
        while let Status::Asking(c) = status {
            asked += 1;
            let pick = c.pick(0).unwrap();
            status = d.resume(&mut s, &k, pick).unwrap();
        }
        let unsupported = d.window.unsupported();
        (s, unsupported, asked)
    }

    fn probe_env() -> Env {
        Env { facts: super::super::Facts { action_draw_plain: true, ..super::super::Facts::default() }, ..bare_env() }
    }

    /// The probe asks which frontier token to explore: one option per token in (or, with the map, next to) a system with a ship of
    /// the holder, in the order of the system ids; a lone option is taken without a question.
    #[test]
    fn exploration_probe_asks_which_frontier_token_to_explore() {
        use crate::choice::{Ask as AnyAsk, Opt as AnyOpt};
        use crate::driver::{Driver, Status};
        use crate::generated::{Explore as E, Unit};
        use crate::kernel::stage::AbilityTable;
        use crate::kernel::Resolver;
        let mut s = probe_state(&[E::UnknownRelicFragment1]);
        let (first, second) = (System::ALL[0], System::ALL[1]);
        // A token with no ship of the holder's near it (no map) is not offered; a rival's ship does not count.
        s.board_mut().sys_mut(second).units.push(Piece::new(Unit::CruiserI, Seat(0)));
        s.explore_mut().frontier.insert(second);
        s.explore_mut().frontier.insert(System::ALL[2]);
        s.board_mut().sys_mut(System::ALL[2]).units.push(Piece::new(Unit::CruiserI, Seat(1)));
        let play = Play::of(&s, ActionCard::ExplorationProbe, Seat(0), probe_env()).expect("a play");
        let k = Kernel::plain();
        let mut d = Driver::new(play, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), AbilityTable::default(), 1);
        let Status::Asking(c) = d.run(&mut s, &k).unwrap() else { panic!("a question") };
        assert_eq!(c.ask, AnyAsk::Cards(Ask::Pick { card: ActionCard::ExplorationProbe, what: What::Probe }));
        let offered: Vec<_> = c.opts.as_slice().iter().map(|o| *o).collect();
        assert_eq!(offered, [AnyOpt::Cards(Opt::System(first)), AnyOpt::Cards(Opt::System(second))]);
        // One ship system with a token: no question.
        let mut lone = probe_state(&[E::UnknownRelicFragment1]);
        lone.explore_mut().frontier.insert(System::ALL[3]);
        let (after, unsupported, asked) = run_probe(lone, probe_env());
        assert!(!unsupported && asked == 0 && after.explore().frontier.len() == 1, "the token with no ship near it stays");
    }

    /// Bail: Exploration Probe while a replay reserve is installed (the string draws through it).
    #[test]
    fn bail_exploration_probe_declines_while_a_replay_reserve_is_installed() {
        let mut core = probe_state(&[crate::generated::Explore::UnknownRelicFragment1]);
        assert!(Play::of(&core, ActionCard::ExplorationProbe, Seat(0), probe_env()).is_some());
        core.cards_mut().reserve = Some(crate::containers::Cow::new(Default::default()));
        assert!(Play::of(&core, ActionCard::ExplorationProbe, Seat(0), probe_env()).is_none());
    }

    /// Bail: a card of another deck in the frontier deck (an attachment, a planet's reward) is not carried: the run is declined.
    #[test]
    fn bail_exploration_probe_declines_a_card_of_another_deck() {
        use crate::generated::Explore as E;
        for card in [E::DysonSphere, E::AbandonedWarehouses1, E::CulturalRelicFragment1, E::Freelancers1] {
            let (_, unsupported, _) = run_probe(probe_state(&[card]), probe_env());
            // A fragment of another deck is gained like the string does; every other card is declined.
            assert_eq!(unsupported, card != E::CulturalRelicFragment1, "{card:?}");
        }
    }

    /// Bail: Lost Crew while the draw is not plain (a full hand, a draw hook): declined; plain, two cards come.
    #[test]
    fn bail_exploration_probe_declines_a_lost_crew_whose_draw_is_not_plain() {
        use crate::generated::Explore as E;
        let mut s = probe_state(&[E::LostCrew1]);
        s.decks_mut().action.set(vec![ActionCard::Spy, ActionCard::Rout, ActionCard::Waylay]);
        let (after, unsupported, _) = run_probe(s.clone(), probe_env());
        assert!(!unsupported && after.players().by_seat(Seat(0)).unwrap().cards.hand.len() == 2, "two cards come");
        let (_, unsupported, _) = run_probe(s, Env { facts: super::super::Facts { action_draw_plain: false, ..super::super::Facts::default() }, ..bare_env() });
        assert!(unsupported, "not plain: the string's");
    }

    /// Bail: command tokens while a Mahact fleet holds tokens of the holder (the reinforcements differ): declined, at every rules version.
    #[test]
    fn bail_exploration_probe_declines_command_tokens_while_a_mahact_fleet_holds_the_holders_tokens() {
        use crate::generated::Explore as E;
        for version in [11, 12, u32::MAX] {
            let facts = super::super::Facts { rules_version: version, ..super::super::Facts::default() };
            let (_, unsupported, _) = run_probe(probe_state(&[E::EntropicField]), Env { mahact_held: 0, facts, ..bare_env() });
            assert!(!unsupported, "{version}");
            let (_, unsupported, _) = run_probe(probe_state(&[E::EntropicField]), Env { mahact_held: 1, facts, ..bare_env() });
            assert!(unsupported, "{version}");
            // A card without command tokens does not care.
            let (_, unsupported, _) = run_probe(probe_state(&[E::UnknownRelicFragment2]), Env { mahact_held: 1, facts, ..bare_env() });
            assert!(!unsupported, "{version}");
        }
    }

    /// Bail: a Merchant Station's replenish while a replenishment causes events the core does not hold: declined; the conversion is carried.
    #[test]
    fn bail_exploration_probe_declines_a_replenish_that_causes_events() {
        use crate::generated::Explore as E;
        let events = Env { facts: super::super::Facts { replenish_events: true, ..super::super::Facts::default() }, ..bare_env() };
        let mut s = probe_state(&[E::MerchantStation1]);
        s.players_mut().by_seat_mut(Seat(0)).unwrap().econ.commodities = 1;
        // The first option is the replenish.
        let (_, unsupported, asked) = run_probe(s.clone(), events);
        assert!(unsupported && asked == 1);
        let (after, unsupported, _) = run_probe(s, bare_env());
        assert!(!unsupported);
        assert_eq!(after.players().by_seat(Seat(0)).unwrap().econ.commodities, 3, "an Arborec holds three");
    }

    /// The ion storm, the Mirage and the Gamma Relay without an active system have nowhere to act: the card is drawn and logged, nothing else.
    #[test]
    fn exploration_probe_draws_a_card_that_has_nowhere_to_act() {
        use crate::generated::Explore as E;
        for card in [E::IonStorm, E::Mirage, E::GammaRelay] {
            let before = probe_state(&[card]);
            let (after, unsupported, asked) = run_probe(before.clone(), probe_env());
            assert!(!unsupported && asked == 0, "{card:?}");
            assert!(after.explore().frontier.is_empty(), "the token is spent");
            assert!(after.explore().wormhole_tokens.is_empty() && after.meta().ion_storm.is_none(), "{card:?}: nothing placed");
        }
    }

    /// The documented bails of the action-card area (docs/B6_CARDS.md): the cards whose string effect runs alone, each with the
    /// fact or rule of another area it needs. Every card of the game is either played by the core (a stamp, a window, or
    /// an effect a rules package registers: `REGISTERED`) or listed here, so a new card cannot be left out without notice.
    const BAILS: &[(ActionCard, &str)] = &{
        use ActionCard as C;
        [
            (C::CourageousToTheEnd, "dice, then the opponent's casualty choice with the card's holder as producer: the combat area's hit flow (core-b12 keeps the child casualty flows, ENGINE_REACTIONS 9.2)"),
            (C::ExperimentalBattlestation, "a space cannon roll whose hits go through the combat area's sustain and casualty questions (a child hit flow, core-b12's area) from a dock in or adjacent to the active system"),
            (C::FireTeam, "the staged ground roll's per-die rerolls are the combat area's (ported on core-b12 as a registered play; not merged here)"),
            (C::Overrule, "performs the primary ability of another strategy card: eight card kinds with their own windows and guards, to be run as child windows"),
            (C::ScrambleFrequency, "per-die reroll of a barrage, space cannon or bombardment roll of another seat: the combat area's dice plumbing (core-b12 documents it as a bail)"),
            (C::Strategize1, "performs the secondary ability of another strategy card: eight card kinds with their own windows and guards, to be run as child windows"),
            (C::Strategize2, "performs the secondary ability of another strategy card: eight card kinds with their own windows and guards, to be run as child windows"),
            (C::Strategize3, "performs the secondary ability of another strategy card: eight card kinds with their own windows and guards, to be run as child windows"),
            (C::Strategize4, "performs the secondary ability of another strategy card: eight card kinds with their own windows and guards, to be run as child windows"),
        ]
    };

    /// The effects the faction crate registers in the ability table (`ti4_engine_core_factions::cards`): they read the combat
    /// and invasion areas, which this area may not import.
    const REGISTERED: &[ActionCard] = &{
        use ActionCard as C;
        [
            C::Parley,
            C::DirectHit1,
            C::DirectHit2,
            C::DirectHit3,
            C::DirectHit4,
            C::SkilledRetreat1,
            C::SkilledRetreat2,
            C::SkilledRetreat3,
            C::SkilledRetreat4,
        ]
    };

    /// Every card is played by the core or a documented bail, never both (docs/B6_CARDS.md).
    #[test]
    fn every_action_card_is_ported_or_a_documented_bail() {
        let mut unlisted = Vec::new();
        for card in ActionCard::ALL {
            let ported = super::super::Simple::of(card).is_some() || Fx::of(card).is_some() || REGISTERED.contains(&card);
            let bail = BAILS.iter().any(|(c, _)| *c == card);
            if ported == bail {
                unlisted.push((card, ported));
            }
        }
        assert!(unlisted.is_empty(), "ported twice or neither ported nor a documented bail: {unlisted:?}");
        for (card, why) in BAILS {
            assert!(!why.is_empty(), "{card:?}");
            assert!(Fx::of(*card).is_none(), "{card:?}: a documented bail has no Fx");
        }
    }

    #[test]
    fn veto_declines_while_a_replay_reserve_is_installed() {
        let mut core = CoreState::all_loaded();
        let env = Env {
            mahact_held: 0,
            scope: crate::tables::Scope(u32::MAX),
            blanked: false,
            action_card: crate::state::Code(0),
            supply_base: crate::state::Code(0),
            note_order: |a, b| a.cmp(&b),
            infantry: super::super::Placed::NONE,
            cruiser: super::super::Placed::NONE,
            fighter: super::super::Placed::NONE,
            facts: super::super::Facts::default(),
        };
        let who = Seat(0);
        assert!(Play::of(&core, ActionCard::Veto, who, env).is_some());
        core.cards_mut().reserve = Some(crate::containers::Cow::new(Default::default()));
        assert!(Play::of(&core, ActionCard::Veto, who, env).is_none(), "the string draws through the reserve");
    }
}
