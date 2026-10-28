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
use crate::generated::{ActionCard, Agenda, Planet, PlanetTrait, System, UnitBaseType};
use crate::ids::Seat;
use crate::keys::{Prediction, PredictionTag, VoteBar};
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{Kernel, Next, fx};
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
    /// Tactical Bombardment: the planets of rivals in a chosen system exhaust.
    Bombard,
    /// Diplomatic Pressure: a chosen seat gives the holder a promissory note of its choice.
    Pressure,
}

impl Fx {
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
            C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4 => Self::Pressure,
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
    Done,
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
}

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

fn has_base(env: Env, cell: &crate::state::PlanetCell, base: UnitBaseType) -> bool {
    cell.units.as_slice().iter().any(|p| env.scope.unit(p.kind()).map(|d| d.base_type) == Some(base))
}

impl Play {
    /// The play of `card` by `who`; `None` when the card is not ported or the position holds something the
    /// window cannot express (a prediction on no agenda).
    #[must_use]
    pub fn of(core: &CoreState, card: ActionCard, who: Seat, env: Env) -> Option<Self> {
        let fx = Fx::of(card)?;
        // The deck reserve (a replay's forced draws) is the host's: the string draws through it.
        if fx == Fx::Veto && core.cards().reserve.is_some() {
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
        Some(Self { card, fx, who, env, frame, victim: None, stage: Stage::Start, opts: SmallList::new() })
    }

    fn what(&self) -> Option<What> {
        Some(match self.fx {
            Fx::Insubordination | Fx::Assassinate | Fx::BurialSites | Fx::SupplyBase => What::Player,
            Fx::Confusing => What::Elect,
            Fx::Recall => What::Recall,
            Fx::Silence => What::Silence,
            Fx::Bombard => What::Bombard,
            Fx::Pressure if self.victim.is_some() => What::Note,
            Fx::Pressure => What::Player,
            Fx::Mining | Fx::Uprising | Fx::Cripple | Fx::Meltdown => What::Planet,
            _ => return None,
        })
    }

    fn seat_opts(seats: impl Iterator<Item = Seat>) -> SmallList<Opt, 16> {
        seats.map(Opt::Seat).collect()
    }

    /// The options of the card's question, in the string order.
    fn options(&self, core: &CoreState) -> SmallList<Opt, 16> {
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
                    held.sort_by_cached_key(|k| (self.env.note_text)(*k));
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
            Fx::Mining => board::controlled_planets(core, who).as_slice().iter().map(|(s, p)| Opt::Spot(*s, *p)).collect(),
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

    fn goods(&self, core: &mut CoreState, who: Seat, amount: i64) {
        let amount = i32::try_from(amount).unwrap_or(0);
        fx::gain_trade_goods_staged(core, who, amount, self.env.action_card);
    }

    /// The effect that needs no pick (and the part of a pick effect that precedes the question).
    fn before(&self, core: &mut CoreState) {
        match self.fx {
            Fx::SupplyBase => {
                fx::gain_trade_goods_staged(core, self.who, 3, self.env.supply_base);
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
    fn apply(&mut self, core: &mut CoreState, opt: Opt) -> bool {
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
    fn advance(&mut self, core: &mut CoreState) {
        loop {
            self.opts = self.options(core);
            match self.opts.as_slice() {
                [] => {
                    self.stage = Stage::Done;
                    return;
                }
                [only] => {
                    let only = *only;
                    if !self.apply(core, only) {
                        self.stage = Stage::Done;
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
        if self.fx == Fx::Pressure { self.victim.unwrap_or(self.who) } else { self.who }
    }

    fn ask(&self) -> Ask {
        match (self.what(), self.frame) {
            (Some(what), _) => Ask::Pick { card: self.card, what },
            (None, Some(frame)) => Ask::Predict { card: self.card, frame },
            (None, None) => Ask::Pick { card: self.card, what: What::Player },
        }
    }
}

impl Window for Play {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        if self.stage == Stage::Start {
            self.before(sx.state);
            if self.what().is_none() && !matches!(self.fx, Fx::Rider(_)) {
                self.stage = Stage::Done;
                return Next::Done;
            }
            self.advance(sx.state);
        }
        match self.stage {
            Stage::Asking => Next::Continue(crate::choice::Ask::Cards(self.ask())),
            Stage::Start | Stage::Done => Next::Done,
        }
    }

    fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
        (self.stage == Stage::Asking).then(|| Choice {
            actor: self.actor(),
            ask: crate::choice::Ask::Cards(self.ask()),
            ctx: Ctx {
                origin: if self.frame.is_some() { Origin::Rule(Rule::R8) } else { Origin::ActionCard(self.card) },
                subtype: crate::choice::Subtype::Cards(match self.what() {
                    Some(what) => Subtype::Pick { card: self.card, what },
                    None => Subtype::PredictOutcome,
                }),
                optional: false,
                target: None,
                outstanding: SmallList::new(),
                space_battle: false,
                invasion_seq: None,
            },
            opts: self.opts.as_slice().iter().map(|o| crate::choice::Opt::Cards(*o)).collect::<Opts>(),
        })
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        if self.stage != Stage::Asking {
            return Err(Illegal);
        }
        let Pick::Opt(crate::choice::Opt::Cards(opt)) = pick else { return Err(Illegal) };
        if !self.opts.as_slice().contains(&opt) {
            return Err(Illegal);
        }
        if self.apply(sx.state, opt) {
            self.advance(sx.state);
        } else {
            self.stage = Stage::Done;
        }
        Ok(())
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
            (C::Spy, "the seeded roller"),
            (C::Plague, "the seeded roller, staged ground-force deaths"),
            (C::UnstablePlanet, "staged ground-force deaths"),
            (C::RepealLaw, "a law leaving play is B9's"),
            (C::DirectHit1, "the exact sustain mark and the ship-lost observers"),
            (C::DirectHit4, "the exact sustain mark and the ship-lost observers"),
            (C::Intercept, "combatants include planet-based ships"),
            (C::PirateContract1, "the neutral destroyer has no seat"),
            (C::Summit, "the pool question has previews and details of the tokens area"),
            (C::SkilledRetreat1, "destinations and previews"),
            (C::LuckyShot, "the per-unit details of the options"),
            (C::CourageousToTheEnd, "dice and a casualty choice"),
            (C::Strategize1, "performs another area's ability"),
            (C::Bribery, "the trade-good payment"),
            (C::WarEffort, "unit placement"),
            (C::InsiderInformation, "the string-only peek mark"),
        ] {
            assert_eq!(Fx::of(card), None, "{card:?}: {why}");
        }
        for card in [C::Sabotage3, C::Veto4, C::DiplomaticPressure2, C::Sanction, C::Uprising, C::Counterstroke] {
            assert!(Fx::of(card).is_some(), "{card:?}");
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
            note_text: |_| String::new(),
        };
        let who = Seat(0);
        assert!(Play::of(&core, ActionCard::Veto, who, env).is_some());
        core.cards_mut().reserve = Some(crate::containers::Cow::new(Default::default()));
        assert!(Play::of(&core, ActionCard::Veto, who, env).is_none(), "the string draws through the reserve");
    }
}
