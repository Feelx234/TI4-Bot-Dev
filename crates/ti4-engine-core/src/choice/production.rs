//! Decision types of the `production` area: production, unit placement, cost payment.
//!
//! Owner: package B3 (docs/ENGINE_CORE_STATE.md 4.4). Ported so far: the payment question of a bill
//! (`Ask::Pay`). Everything an option's text and preview need is a number carried by the option, because
//! the renderer (`ti4-engine/src/render/production.rs`) sees no state. Never text; every type is `Copy`.

use crate::containers::SmallList;
use crate::generated::{Planet, System, Unit};
use crate::ids::Seat;
use crate::state::Code;

/// A currency of a bill or of a planet's face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Currency {
    Resources,
    Influence,
}

/// The payment question: the rest of a bill of `kind` with `owed` still to pay, when the player could spend `pool` of that currency and
/// holds `goods` trade goods (and `commodities` it may spend as trade goods, Keleres' agent).
///
/// ONE question for the rest of the bill (the multi-ask conversion, `docs/MULTIASK1_RESUME.md`): the options are the ATOMS one may
/// spend (each planet face, then trade goods, then commodities), and the answer is a [`PaySet`] of them, checked by [`set_worth`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PayAsk {
    pub kind: Currency,
    pub owed: i64,
    pub pool: i64,
    pub goods: i64,
    pub commodities: i64,
}

/// Leadership's purchase (52.3, `docs/MULTIASK1_RESUME.md`): ONE question for every command token bought at `each` influence apiece,
/// between `min` and `max` of them, and how to pay. The options are the payment atoms (as for [`PayAsk`], `pay.owed` is `each`); the
/// answer is a [`PaySet`] whose `buy` is the number bought ([`purchase_worth`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PurchaseAsk {
    pub pay: PayAsk,
    pub each: u8,
    pub min: u8,
    pub max: u8,
}

/// The answer to a payment question ([`PayAsk`]): the planet faces taken (bit `i` is the `i`-th [`Opt::Exhaust`] of the question's
/// options, in their order), the trade goods and the commodities spent, and (Leadership's purchase only) how many command tokens
/// it buys. A plain value: the question's option list says what each bit stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PaySet {
    pub faces: u32,
    pub goods: u8,
    pub commodities: u8,
    pub buy: u8,
}

impl PaySet {
    /// The set of the atoms `picks` (options of `opts`; a trade good or commodity once per one spent), `None` when one is not offered.
    #[must_use]
    pub fn of(opts: &[Opt], picks: &[Opt]) -> Option<Self> {
        let mut set = Self::default();
        for pick in picks {
            match pick {
                Opt::TradeGood { .. } => set.goods = set.goods.checked_add(1)?,
                Opt::Commodity { .. } => set.commodities = set.commodities.checked_add(1)?,
                Opt::Exhaust { .. } => {
                    let at = opts.iter().filter(|o| matches!(o, Opt::Exhaust { .. })).position(|o| o == pick)?;
                    if at >= 32 {
                        return None;
                    }
                    set.faces |= 1 << at;
                }
                _ => return None,
            }
        }
        Some(set)
    }

    /// The faces of `opts` the set takes, in option order.
    pub fn exhausted<'a>(self, opts: impl IntoIterator<Item = &'a Opt>) -> impl Iterator<Item = &'a Opt> {
        opts.into_iter().filter(|o| matches!(o, Opt::Exhaust { .. })).enumerate().filter(move |(i, _)| *i < 32 && self.faces & (1 << i) != 0).map(|(_, o)| o)
    }
}

/// The worth of the atoms of `set` against the options `opts` of a payment question, each atom with the worth the option shows:
/// `None` when the set names an atom the question does not offer (a bit past its faces, two faces of one planet, more trade goods or
/// commodities than held).
#[must_use]
pub fn set_atoms(opts: &[Opt], goods: i64, commodities: i64, set: PaySet) -> Option<SmallList<i64, 24>> {
    let faces = opts.iter().filter(|o| matches!(o, Opt::Exhaust { .. })).count();
    if faces < 32 && set.faces >> faces != 0 {
        return None;
    }
    let mut worths: SmallList<i64, 24> = SmallList::new();
    let mut planets: SmallList<Planet, 24> = SmallList::new();
    for o in set.exhausted(opts) {
        let Opt::Exhaust { planet, shown, .. } = o else { continue };
        if planets.as_slice().contains(planet) {
            return None;
        }
        planets.push(*planet);
        worths.push(i64::from(shown.worth));
    }
    let worth_of = |pick: fn(&Opt) -> bool| opts.iter().find(|o| pick(o)).map(|o| match o {
        Opt::TradeGood { shown } | Opt::Commodity { shown } => i64::from(shown.worth),
        _ => 0,
    });
    if set.goods > 0 {
        let w = worth_of(|o| matches!(o, Opt::TradeGood { .. }))?;
        if i64::from(set.goods) > goods {
            return None;
        }
        (0..set.goods).for_each(|_| worths.push(w));
    }
    if set.commodities > 0 {
        let w = worth_of(|o| matches!(o, Opt::Commodity { .. }))?;
        if i64::from(set.commodities) > commodities {
            return None;
        }
        (0..set.commodities).for_each(|_| worths.push(w));
    }
    Some(worths)
}

/// The canonical answer to a payment question: its atoms in option order (a planet once), each trade good, then each commodity, up to
/// the first that covers the bill. Always legal ([`set_worth`]): everything before the last atom was short of the bill, and the last
/// atom is no larger than the largest. `None` when the atoms cannot cover it.
#[must_use]
pub fn first_set(ask: &PayAsk, opts: &[Opt]) -> Option<PaySet> {
    set_from(ask, opts, 0)
}

/// [`first_set`] that takes the atom at `first` (an index into `opts`) before the others: "pay with this one, then whatever comes
/// first" (a trade good or commodity first: as many of those as needed first). Legal for the same reason. What `Choice::pick(index)`
/// answers a payment question with.
#[must_use]
pub fn set_from(ask: &PayAsk, opts: &[Opt], first: usize) -> Option<PaySet> {
    let lead = opts.get(first)?;
    let mut set = PaySet::default();
    let mut sum = 0i64;
    let mut planets: SmallList<Planet, 24> = SmallList::new();
    let face_of = |at: usize| u32::try_from(opts[..at].iter().filter(|o| matches!(o, Opt::Exhaust { .. })).count()).unwrap_or(u32::MAX);
    // The lead atom first.
    match lead {
        Opt::Exhaust { planet, shown, .. } if face_of(first) < 32 => {
            planets.push(*planet);
            set.faces |= 1 << face_of(first);
            sum += i64::from(shown.worth);
        }
        Opt::TradeGood { shown } if ask.goods > 0 => {
            set.goods = 1;
            sum += i64::from(shown.worth);
        }
        Opt::Commodity { shown } if ask.commodities > 0 => {
            set.commodities = 1;
            sum += i64::from(shown.worth);
        }
        _ => return None,
    }
    // A trade good or a commodity first means "pay in those": as many of them as the bill needs before anything else.
    match lead {
        Opt::TradeGood { shown } => {
            while sum < ask.owed && i64::from(set.goods) < ask.goods {
                set.goods += 1;
                sum += i64::from(shown.worth);
            }
        }
        Opt::Commodity { shown } => {
            while sum < ask.owed && i64::from(set.commodities) < ask.commodities {
                set.commodities += 1;
                sum += i64::from(shown.worth);
            }
        }
        _ => {}
    }
    let mut face = 0u32;
    for o in opts {
        if sum >= ask.owed {
            break;
        }
        match o {
            Opt::Exhaust { planet, shown, .. } => {
                if face < 32 && !planets.as_slice().contains(planet) {
                    planets.push(*planet);
                    set.faces |= 1 << face;
                    sum += i64::from(shown.worth);
                }
                face += 1;
            }
            Opt::TradeGood { shown } => {
                while sum < ask.owed && i64::from(set.goods) < ask.goods {
                    set.goods += 1;
                    sum += i64::from(shown.worth);
                }
            }
            Opt::Commodity { shown } => {
                while sum < ask.owed && i64::from(set.commodities) < ask.commodities {
                    set.commodities += 1;
                    sum += i64::from(shown.worth);
                }
            }
            _ => {}
        }
    }
    (sum >= ask.owed).then_some(set)
}

/// Up to `limit` legal answers of a payment question ([`set_worth`]), in a fixed order (faces by option order, then trade goods, then
/// commodities): what a test's policy or a sampling bot picks from. Bounded: the number of legal sets grows quickly with the planets.
#[must_use]
pub fn legal_sets(ask: &PayAsk, opts: &[Opt], limit: usize) -> Vec<PaySet> {
    let faces = opts.iter().filter(|o| matches!(o, Opt::Exhaust { .. })).count().min(20);
    let goods = if opts.iter().any(|o| matches!(o, Opt::TradeGood { .. })) { u8::try_from(ask.goods.clamp(0, 16)).unwrap_or(0) } else { 0 };
    let comms = if opts.iter().any(|o| matches!(o, Opt::Commodity { .. })) { u8::try_from(ask.commodities.clamp(0, 16)).unwrap_or(0) } else { 0 };
    let mut out = Vec::new();
    'all: for mask in 0..(1u32 << faces) {
        for g in 0..=goods {
            for c in 0..=comms {
                let set = PaySet { faces: mask, goods: g, commodities: c, buy: 0 };
                if set_worth(ask, opts, set).is_some() {
                    out.push(set);
                    if out.len() >= limit {
                        break 'all;
                    }
                }
            }
        }
    }
    out
}

/// The worth `set` pays when it answers Leadership's purchase question: `None` unless the old purchase loop could have ended with it
/// ("spend 3 influence for a command token?" yes or no, each purchase paid one atom per question until the running total covers it, an
/// overpayment carried to the next): `buy` tokens between `min` and `max`, nothing spent for none, else `sum >= each * buy` and
/// `sum - max < each * buy` (ordered with the largest atom last, every earlier atom was taken while a purchase was still open).
#[must_use]
pub fn purchase_worth(ask: &PurchaseAsk, opts: &[Opt], set: PaySet) -> Option<i64> {
    if set.buy < ask.min || set.buy > ask.max {
        return None;
    }
    let worths = set_atoms(opts, ask.pay.goods, ask.pay.commodities, set)?;
    let sum: i64 = worths.as_slice().iter().sum();
    let bill = i64::from(ask.each) * i64::from(set.buy);
    if set.buy == 0 {
        return (sum == 0).then_some(0);
    }
    let max = worths.as_slice().iter().copied().max()?;
    (sum >= bill && sum - max < bill).then_some(sum)
}

/// A canonical answer to Leadership's purchase question: `buy` tokens, paid by [`set_from`] over a bill of `each * buy` (the atom at
/// `first` first). `None` when that many cannot be paid.
#[must_use]
pub fn purchase_set(ask: &PurchaseAsk, opts: &[Opt], buy: u8, first: usize) -> Option<PaySet> {
    if buy == 0 {
        return (ask.min == 0).then_some(PaySet::default());
    }
    let bill = PayAsk { owed: i64::from(ask.each) * i64::from(buy), ..ask.pay };
    let set = PaySet { buy, ..set_from(&bill, opts, first)? };
    purchase_worth(ask, opts, set).map(|_| set)
}

/// The worth `set` pays when it answers the payment question `ask` with the options `opts`: `None` unless it is an answer the old
/// one-atom-per-question loop could have reached (`sum >= owed` and `sum - max < owed`: ordered with its largest atom last, every
/// earlier atom was taken while the bill was still open). Proved against the loop by brute force in `pay.rs`'s tests.
#[must_use]
pub fn set_worth(ask: &PayAsk, opts: &[Opt], set: PaySet) -> Option<i64> {
    if set.buy != 0 {
        return None;
    }
    let worths = set_atoms(opts, ask.goods, ask.commodities, set)?;
    let sum: i64 = worths.as_slice().iter().sum();
    let max = worths.as_slice().iter().copied().max()?;
    (sum >= ask.owed && sum - max < ask.owed).then_some(sum)
}

/// Numbers an option shows (its worth, the pool after it): derived from the position, so they are not part
/// of the option's identity. Two options are equal when they name the same thing; `Shown` compares equal,
/// orders equal and hashes to nothing, so an answer parsed from an id equals the option that was offered.
#[derive(Debug, Clone, Copy, Default)]
pub struct Shown {
    pub worth: i32,
    pub pool_after: i32,
}

impl PartialEq for Shown {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for Shown {}
impl PartialOrd for Shown {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Shown {
    fn cmp(&self, _: &Self) -> core::cmp::Ordering {
        core::cmp::Ordering::Equal
    }
}
impl core::hash::Hash for Shown {
    fn hash<H: core::hash::Hasher>(&self, _: &mut H) {}
}

/// Where a produced unit may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Spot {
    Space,
    Planet(Planet),
    /// A place in another system a module offers (`production_destinations`).
    Remote(System, Option<Planet>),
    /// The Crimson hero card ("place any of those ships onto this card"): not a place on the board.
    Card,
}

/// What this use of PRODUCTION has done with its ships so far, for the Last Bastion's Icon ("place those ships in a system that
/// contains ..."): every ship of one use goes to one system, so the first remote ship fixes the spot and a ship placed in the
/// producing system closes the Icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Icon {
    /// No ship placed yet.
    Fresh,
    /// A ship went to this remote spot: it is the only one left.
    Remote(Spot),
    /// A ship was placed in the producing system.
    Local,
}

/// The placement question: where `made` bought copies of `unit` go, when there is more than one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaceAsk {
    pub who: Seat,
    pub system: System,
    pub unit: Unit,
    pub made: u8,
    pub fleet_limit: i32,
    pub icon: Icon,
    /// The units come from an ability, not from the PRODUCTION of a unit (package W5).
    pub ability: bool,
}

/// The production question: what to build now. The window's own numbers (the string `ProductionWindow`
/// fields the options depend on); the rest an option shows is read from the position by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChooseAsk {
    pub who: Seat,
    pub system: System,
    /// Full PRODUCTION limit of this use and what is left of it.
    pub limit: i32,
    pub remaining: i32,
    /// Capacity ships opened for small units (Bellum Gloriosum) and the Cabal commander's exemptions.
    pub free_capacity: i32,
    pub small_exemptions: i32,
    /// Resources already paid in advance and the cost discount still available.
    pub credit: i32,
    pub discount_remaining: i32,
    /// The seat's fleet limit as `fleet::limit` works it out (pool, laws, abilities, the lifted-limit round).
    pub fleet_limit: i32,
    /// Harrugh Gefhara: every build of this use is free.
    pub free_this_use: bool,
    /// A Nomad commander waives flagship resources.
    pub flagship_free: bool,
    pub only_unit: Option<Unit>,
    pub max_unit_cost: Option<i32>,
    /// Units produced so far in this use, per unit base type (`UnitBaseType` order).
    pub produced: [u8; 11],
    /// Letnev's Armada, already inside the fleet limit (display only).
    pub armada: i32,
    /// What the seat may produce at all (`buildable_for`), in option order.
    pub buildable: [Option<Unit>; 9],
    /// What the ships of this use have done so far (the Last Bastion's Icon).
    pub icon: Icon,
    /// The units come from an ability, not from the PRODUCTION of a unit: ground forces go to the planets of the system the seat
    /// controls, no producer bars a kind, and the window has no capacity of its own (package W5).
    pub ability: bool,
}

/// Sling Relay's first question: in which dock system to produce (only asked with two or more).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlingAsk {
    pub who: Seat,
}

/// Sling Relay's second question: which ship to produce in `system` (only asked with two or more).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlingShipAsk {
    pub who: Seat,
    pub system: System,
}

/// Integrated Economy's question: what to produce on `planet` with `budget` of cost left. The window's own
/// numbers; the options come from the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EconomyAsk {
    pub who: Seat,
    pub system: System,
    pub planet: Planet,
    pub budget: i32,
    /// The seat's fleet limit as `fleet::limit` works it out.
    pub fleet_limit: i32,
    /// What the seat may produce at all (`buildable_for`), in option order.
    pub buildable: [Option<Unit>; 9],
}

/// Harrugh Gefhara's offer at the start of a use of PRODUCTION: make every unit of this use free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroAsk {
    pub who: Seat,
}

/// "produce one unit in `system`" (package W12, lane 7: Chaos Mapping, Freelancers, the single units of the Minister of Industry before
/// its window): a "may" for one unit, outside a use of PRODUCTION. `name` and `alias` are the interned printed name of the effect that
/// grants it and its content alias (`None`: an effect that names itself nowhere, Chaos Mapping's single unit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OneAsk {
    pub who: Seat,
    pub system: System,
    pub name: Option<Code>,
    pub alias: Option<Code>,
    /// What the seat may produce at all (`buildable_for`), in option order, and its fleet limit: the position gives the rest.
    pub buildable: [Option<Unit>; 9],
    pub fleet_limit: i32,
}

/// What the area asks. Parameters only; the option list is derived by `rules::production::pay_opts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ask {
    /// Pay the rest of a bill (one set of atoms).
    Pay(PayAsk),
    /// Leadership: how many command tokens to buy, and the atoms that pay for them (one set).
    Purchase(PurchaseAsk),
    /// Choose what to build.
    Choose(ChooseAsk),
    /// Choose where to put what was produced.
    Place(PlaceAsk),
    /// Sling Relay: choose the dock system.
    SlingSystem(SlingAsk),
    /// Sling Relay: choose the ship.
    SlingShip(SlingShipAsk),
    /// Integrated Economy: choose what to produce.
    Economy(EconomyAsk),
    /// Harrugh Gefhara: free production, or not.
    Hero(HeroAsk),
    /// Xander Alexin Victori III (a Keleres agent): let `payer` spend commodities as trade goods.
    AgentOffer { payer: Seat },
    /// Produce one unit in a system outside a use of PRODUCTION (`rules::production::one::ProduceOne`).
    One(OneAsk),
    /// Where the one unit goes (more than one spot): "place the {unit}".
    OnePlace { unit: Unit, system: System },
}

/// One answer the area offers (not stored in the state).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Opt {
    /// Exhaust a planet card for `worth` through its `source` face; the pool falls to `pool_after`.
    Exhaust { planet: Planet, source: Currency, shown: Shown },
    /// Spend a trade good (worth `shown.worth` against the bill).
    TradeGood { shown: Shown },
    /// Spend a commodity as a trade good (Keleres' agent).
    Commodity { shown: Shown },
    /// The answer to a payment question: these atoms of its options ([`PaySet`], [`set_worth`]). Never an offered option.
    Set(PaySet),
    /// Build `made` copies of `unit`, paying for it (`build|unit|made`).
    Build { unit: Unit, made: u8 },
    /// Produce one `unit` by returning a captured one (`exchange|unit`).
    Exchange { unit: Unit },
    /// Place the produced unit at a spot (`place|spot`).
    Place { spot: Spot },
    /// Produce nothing further (`done_producing`).
    Stop,
    /// Winnu: exhaust Berekar Berekon for -2 on the use's combined cost.
    Agent,
    /// Sling Relay: produce in this dock system (the system's id).
    SlingSystem { system: System },
    /// Sling Relay: produce one `unit` for `shown.worth` (`build|unit|1`).
    SlingShip { unit: Unit, shown: Shown },
    /// Integrated Economy: produce `made` copies of `unit` for `shown.worth` (`build|unit|made`).
    Economy { unit: Unit, made: u8, shown: Shown },
    /// Harrugh Gefhara: purge the hero, every unit of this use costs 0 (`use`).
    HeroUse,
    /// Decline the offer (`decline`).
    Decline,
    /// Use the leader the question offers (the Keleres agent); declining is the generic option.
    Use,
    /// Produce one `unit` for `shown.worth` (`build|unit|1`).
    One { unit: Unit, shown: Shown },
}

/// The machine identifier of the question (`DecisionContext.subtype`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subtype {
    Pay(Currency),
    Choose,
    Place,
    SlingSystem,
    SlingShip,
    Economy,
    HeroFree,
    /// `keleres_agent_commodities`.
    AgentOffer,
    /// `produce_one_unit` (only for an effect that names itself).
    One,
}
