//! The announce/resolve flow of a played action card (package B6; the string `reactions::announce`):
//!
//! 1. `ACTION_CARD_PLAYED` is announced through the windows around it (Sabotage and Instinct Training cancel it);
//! 2. a cancelled play is discarded and announced as discarded (the card stays spent, 1.15);
//! 3. otherwise the effect resolves: a stamp or flag ([`Simple`]), a card with questions ([`Play`]) or, for a
//!    card the string side has no effect for, `ACTION_CARD_UNRESOLVED`;
//! 4. the card is discarded (Data Skimmer aside) and `ACTION_CARD_DISCARDED` is announced.
//!
//! One machine, three runners. [`Announce`] is a [`Window`] for the component action (its effect questions are
//! stages), and [`Announce::run_inline`] runs the same machine inside an ability effect (the reaction slot's play)
//! or at the close of a window (the cards declared in it), where every question goes to the tape. The machine
//! is written against [`Runner`], so there is one place per rule.
//!
//! Bails (each answers `Unknown`, the adapter falls back to the string run, one test each in `tests`):
//! * destructions staged by the played card (Direct Hit): the announcement of the removals is not ported
//!   (`Unknown::PendingDestructions`);
//! * a Data Skimmer that could take the discard (`Unknown::DataSkimmer`);
//! * Reflective Shielding played inside a window (`Unknown::ReflectiveShielding`: the sustain step that opened the
//!   window drains the hits it stages; found by the wide golden dual, 12 differences in `combat::answer`);
//! * a card whose effect the core does not play (`Announce::of` is `None`), or a play inside a window whose
//!   adapter facts are missing (`Unknown::PlayFacts`).

use crate::choice::{Choice, Pick};
use crate::generated::ActionCard;
use crate::ids::Seat;
use crate::kernel::ability::{AbilityTable, Inline, Runner, Unknown};
use crate::kernel::resolver::TimingError;
use crate::kernel::stage::{Illegal, Sx, Window};
use crate::kernel::{fx, Kernel, Next};
use crate::state::{CoreState, EventKind, EventRec, Key, Payload, Val};

use super::effects::{Fx, Play};
use super::{Env, Placed, Simple};

/// What the card's effect is.
#[derive(Debug, Clone)]
enum Effect {
    /// A stamp or flag.
    Simple(Simple),
    /// A card with questions.
    Play(Box<Play>),
    /// The string side has no effect for the card: announced unresolved.
    Unresolved,
    /// A rules package registered the effect (`AbilityTable::add_card_play`: the cards that read the combat or invasion
    /// area, Sabotage, Parley, Direct Hit, Skilled Retreat); it runs on the host of the window, so only inline.
    Registered,
}

/// The suspension points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Nothing announced yet.
    Start,
    /// The play stood; the effect may wait on a pick.
    Resolving,
    Done,
}

/// One announced card.
#[derive(Debug, Clone)]
pub struct Announce {
    who: Seat,
    card: ActionCard,
    env: Env,
    effect: Effect,
    stage: Stage,
    played: bool,
}

fn card_event(kind: EventKind, who: Option<Seat>, card: ActionCard) -> EventRec {
    let mut payload = Payload::new();
    if let Some(who) = who {
        payload.set(Key::Player, Val::Seat(who));
    }
    payload.set(Key::Card, Val::Action(card));
    fx::event(kind, payload)
}

impl Announce {
    /// The announcement of `card` played by `who`; `has_effect` is whether the string side has an effect for it
    /// (`action_cards::effect_for`); `registered` whether a rules package registered the effect of `card` (only the inline
    /// runner has the host to run it). `None` when the core does not play the effect (the bail: the string runs).
    #[must_use]
    pub fn of(core: &CoreState, card: ActionCard, who: Seat, env: Env, has_effect: bool, registered: bool) -> Option<Self> {
        let effect = if !has_effect {
            Effect::Unresolved
        } else if let Some(simple) = Simple::of(card) {
            Effect::Simple(simple)
        } else if let Some(play) = Play::of(core, card, who, env) {
            Effect::Play(Box::new(play))
        } else if registered && Fx::of(card).is_none_or(|fx| !fx.nested()) {
            // A card the Fx table owns that refuses the position (a bail of its own) stays the string's. A card whose window is not
            // played from inside another window (`Fx::nested`: it reads facts only its own window supplies) is played by the
            // registration there.
            Effect::Registered
        } else {
            return None;
        };
        Some(Self { who, card, env, effect, stage: Stage::Start, played: false })
    }

    /// Whether the play stood (`false`: it was cancelled while announced). Meaningful once finished.
    #[must_use]
    pub const fn played(&self) -> bool {
        self.played
    }

    /// The system the Diplomacy primary performed by Overrule placed tokens in and the seats that got one (empty for any other play).
    #[must_use]
    pub fn placed_tokens(&self) -> Option<(crate::generated::System, Vec<Seat>)> {
        match &self.effect {
            Effect::Play(play) => play.placed_tokens().map(|(system, seats)| (system, seats.to_vec())),
            _ => None,
        }
    }

    /// The dice the effect rolled, in order.
    #[must_use]
    pub fn faces(&self) -> Vec<u32> {
        match &self.effect {
            Effect::Play(play) => play.faces().to_vec(),
            _ => Vec::new(),
        }
    }

    /// Exploration Probe: the card the frontier deck gave and what the seat held around it (see [`Play::explored`]).
    #[must_use]
    pub fn explored(&self) -> Option<super::effects::ProbeRow> {
        match &self.effect {
            Effect::Play(play) => play.explored(),
            _ => None,
        }
    }

    /// Go on until the effect asks (the question) or the flow is finished (`None`).
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn advance<R: Runner>(&mut self, rt: &mut R) -> Result<Option<Choice>, TimingError> {
        if self.stage == Stage::Start {
            self.stage = Stage::Resolving;
            rt.state().transient_mut().misc.action_card_plays.push((self.who, self.card));
            let announced = rt.emit(card_event(EventKind::ActionCardPlayed, Some(self.who), self.card))?;
            if announced.cancelled {
                // The card is still spent when cancelled: 1.15 lets a WHEN ability cancel the event, not un-spend the card.
                self.discard(rt)?;
                self.stage = Stage::Done;
                return Ok(None);
            }
            match &mut self.effect {
                Effect::Simple(simple) => super::play_simple(rt.state(), self.who, *simple, self.env),
                Effect::Play(play) => rt.with_sx(|sx| play.begin(sx)),
                Effect::Registered => rt.run_registered_play(self.who, self.card)?,
                Effect::Unresolved => {
                    rt.emit(card_event(EventKind::ActionCardUnresolved, None, self.card))?;
                }
            }
        }
        if self.stage == Stage::Resolving {
            if let Effect::Play(play) = &self.effect
                && play.asking()
            {
                let (s, k) = rt.read();
                return Ok(play.ask_of(s, k));
            }
            if let Effect::Play(play) = &self.effect
                && play.unsupported()
            {
                rt.unknown(Unknown::MissingFact);
            }
            self.discard(rt)?;
            // An effect may have staged a destruction (Direct Hit, Courageous to the End): a rules package that registered
            // the announcement of the removals gives them their windows; otherwise it is the string's.
            rt.after_play()?;
            if !rt.state().transient().combat.pending_destructions.is_empty() {
                rt.unknown(Unknown::PendingDestructions);
            }
            self.played = true;
            self.stage = Stage::Done;
        }
        Ok(None)
    }

    /// `announce_discard`: the card is on the pile (Data Skimmer aside), `last_action_discarded` names it, and the
    /// discard is announced.
    fn discard<R: Runner>(&mut self, rt: &mut R) -> Result<(), TimingError> {
        if rt.facts().plays.as_ref().is_some_and(|p| p.skimmer) {
            rt.unknown(Unknown::DataSkimmer);
        }
        let s = rt.state();
        s.cards_mut().discarded_action.edit().push(self.card);
        s.transient_mut().combat.last_action_discarded = Some((self.who, self.card));
        rt.emit(card_event(EventKind::ActionCardDiscarded, Some(self.who), self.card))?;
        Ok(())
    }

    /// Run the flow with every question going to the tape (inside an effect, or at the close of a window); whether
    /// the play stood.
    ///
    /// # Errors
    /// What the resolver refuses with.
    pub fn run_inline<R: Runner>(&mut self, rt: &mut R) -> Result<bool, TimingError> {
        while let Some(choice) = self.advance(rt)? {
            let pick = rt.ask(&choice);
            let Effect::Play(play) = &mut self.effect else { break };
            if rt.with_sx(|sx| play.choose(sx, pick)).is_err() {
                break;
            }
        }
        Ok(self.played)
    }
}

impl Window for Announce {
    type Stage = Stage;

    fn stage(&self) -> Stage {
        self.stage
    }

    fn step(&mut self, sx: &mut Sx<'_>) -> Next {
        // A refused announcement (the nesting limit) ends the flow, as the string run unwinds.
        match self.advance(sx) {
            Ok(Some(_)) => match &self.effect {
                Effect::Play(play) => Next::Continue(play.any_ask()),
                _ => Next::Done,
            },
            Ok(None) | Err(_) => Next::Done,
        }
    }

    fn ask_of(&self, s: &CoreState, k: &Kernel) -> Option<Choice> {
        match &self.effect {
            Effect::Play(play) if self.stage == Stage::Resolving => play.ask_of(s, k),
            _ => None,
        }
    }

    fn answer(&mut self, sx: &mut Sx<'_>, pick: Pick) -> Result<(), Illegal> {
        match &mut self.effect {
            Effect::Play(play) => play.choose(sx, pick),
            _ => Err(Illegal),
        }
    }
}

/// The unit form and the facts a seat's plays read, from the adapter's facts (`None`: the adapter computed none).
fn env_of(rt: &impl Runner, who: Seat) -> Option<Env> {
    let f = rt.facts().plays.as_ref()?;
    let seat = f.seats.iter().find(|s| s.who == who)?;
    let env = Env {
        mahact_held: seat.mahact_held,
        scope: crate::tables::Scope(f.scope),
        blanked: f.blanked,
        action_card: f.action_card,
        supply_base: f.supply_base,
        note_order: f.note_order,
        infantry: Placed { unit: seat.infantry.0, on_card: seat.infantry.1 },
        cruiser: Placed { unit: seat.cruiser.0, on_card: seat.cruiser.1 },
        fighter: Placed { unit: Some(crate::generated::Unit::FighterI), on_card: seat.fighter_on_card },
        facts: super::Facts::default(),
    };
    Some(env)
}

/// Announce and resolve a card inside an effect or at the close of a window. `false`: the play was cancelled
/// (or the core cannot play it, in which case the run is marked unknown and the adapter falls back).
///
/// # Errors
/// What the resolver refuses with.
pub fn play_inline<R: Runner>(rt: &mut R, who: Seat, card: ActionCard) -> Result<bool, TimingError> {
    // Its hits are staged for the sustain step that opened the window, which drains them after the emission.
    if card == ActionCard::ReflectiveShielding {
        rt.unknown(Unknown::ReflectiveShielding);
        return Ok(false);
    }
    let Some(env) = env_of(rt, who) else {
        rt.unknown(Unknown::PlayFacts);
        return Ok(false);
    };
    let has_effect = rt.facts().plays.as_ref().is_some_and(|f| f.has_effect.get(card as usize).copied().unwrap_or(false));
    let registered = rt.has_registered_play(card);
    let Some(mut announce) = Announce::of(rt.state(), card, who, env, has_effect, registered) else {
        rt.unknown(Unknown::PlayFacts);
        return Ok(false);
    };
    announce.run_inline(rt)
}

/// The close of a window (rules version 3): the cards declared in it are announced now, once every declaration is
/// in, each in declaration order (`AbilityTable::on_close`).
///
/// # Errors
/// What the resolver refuses with.
pub fn announce_declared(rt: &mut Inline<'_, '_>) -> Result<(), TimingError> {
    if rt.state().transient().misc.declared_card_plays.is_empty() {
        return Ok(());
    }
    let declared = std::mem::take(&mut rt.state().transient_mut().misc.declared_card_plays);
    for (who, card) in declared.as_slice() {
        play_inline(rt, *who, *card)?;
    }
    Ok(())
}

/// Install the close hook in the table.
pub fn register(t: &mut AbilityTable) {
    t.on_close = Some(announce_declared);
}

/// Whether the core plays the effect of `card` (a stamp, or a card with a window).
#[must_use]
pub fn plays(card: ActionCard) -> bool {
    Simple::of(card).is_some() || Fx::of(card).is_some()
}

/// Whether the core plays `card` from a reaction window: [`plays`], or an effect a rules package registered in the table.
/// Not Reflective Shielding: it stages hits that the sustain window of the string drains after the window closes (the
/// census names the card; [`play_inline`] answers `Unknown::ReflectiveShielding` if it gets there anyway).
#[must_use]
pub fn plays_in(t: &AbilityTable, card: ActionCard) -> bool {
    card != ActionCard::ReflectiveShielding && (Simple::of(card).is_some() || Fx::of(card).is_some_and(Fx::nested) || t.card_play(card).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::{Driver, Status};
    use crate::generated::{Faction, System, Unit};
    use crate::kernel::Resolver;
    use crate::state::{Code, PendingDestruction, Player, Players};

    fn env() -> Env {
        Env {
            mahact_held: 0,
            scope: crate::tables::Scope(u32::MAX),
            blanked: false,
            action_card: Code(0),
            supply_base: Code(0),
            note_order: |a, b| a.cmp(&b),
            infantry: Placed::NONE,
            cruiser: Placed::NONE,
            fighter: Placed::NONE,
            facts: crate::rules::action_cards::Facts::default(),
        }
    }

    fn two_seats() -> CoreState {
        let mut s = CoreState::all_loaded();
        s.players = Players::from_vec(vec![Player::new(Seat(0), Faction::TheArborec), Player::new(Seat(1), Faction::TheEmiratesOfHacan)]);
        s
    }

    fn run(a: Announce, s: &mut CoreState, table: AbilityTable) -> Driver<Announce> {
        let k = Kernel::plain();
        let mut d = Driver::new(a, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
        let mut status = d.run(s, &k).unwrap();
        while let Status::Asking(choice) = status {
            let pick = choice.pick(0).unwrap();
            status = d.resume(s, &k, pick).unwrap();
        }
        d
    }

    fn plays_table() -> AbilityTable {
        let mut t = AbilityTable::default();
        t.facts.plays = Some(crate::kernel::ability::PlayFacts {
            scope: u32::MAX,
            blanked: false,
            action_card: Code(0),
            supply_base: Code(0),
            note_order: |a, b| a.cmp(&b),
            seats: Vec::new(),
            name_group: Vec::new(),
            has_effect: Vec::new(),
            skimmer: false,
        });
        t
    }

    #[test]
    fn a_stamp_card_is_announced_resolved_and_discarded_in_order() {
        let mut s = two_seats();
        let a = Announce::of(&s, ActionCard::MoraleBoost1, Seat(0), env(), true, false).unwrap();
        let d = run(a, &mut s, plays_table());
        let kinds: Vec<EventKind> = d.emitted.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::ActionCardPlayed, EventKind::ActionCardDiscarded]);
        assert!(d.window.played());
        assert_eq!(s.cards().discarded_action.as_slice(), &[ActionCard::MoraleBoost1]);
        assert_eq!(s.transient().combat.last_action_discarded, Some((Seat(0), ActionCard::MoraleBoost1)));
        assert_eq!(s.transient().misc.action_card_plays.as_slice(), &[(Seat(0), ActionCard::MoraleBoost1)]);
        assert!(s.players().by_seat(Seat(0)).unwrap().card_timers.combat_bonus_round.is_some(), "the stamp was applied");
    }

    #[test]
    fn a_card_the_string_side_has_no_effect_for_is_announced_unresolved() {
        let mut s = two_seats();
        let a = Announce::of(&s, ActionCard::Spy, Seat(0), env(), false, false).unwrap();
        let d = run(a, &mut s, plays_table());
        let kinds: Vec<EventKind> = d.emitted.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::ActionCardPlayed, EventKind::ActionCardUnresolved, EventKind::ActionCardDiscarded]);
    }

    /// A rules package registers the effect of a card (`AbilityTable::add_card_play`); the component action's window plays it
    /// on the host built from its own services, as an inline play does.
    #[test]
    fn a_registered_effect_runs_in_the_component_actions_window() {
        use crate::kernel::ability::CardPlay;
        fn bump(host: &mut crate::kernel::ability::RulesHost<'_>, _: &mut Resolver, who: Seat, _: ActionCard) -> Result<(), TimingError> {
            host.state.players_mut().by_seat_mut(who).unwrap().econ.fleet_tokens += 7;
            Ok(())
        }
        let mut s = two_seats();
        let before = s.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens;
        let a = Announce::of(&s, ActionCard::Parley, Seat(0), env(), true, true).unwrap();
        let mut table = plays_table();
        table.add_card_play(CardPlay { covers: |c| c == ActionCard::Parley, effect: bump });
        let d = run(a, &mut s, table);
        assert!(d.window.played());
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().econ.fleet_tokens, before + 7, "the registered effect ran");
        let kinds: Vec<EventKind> = d.emitted.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::ActionCardPlayed, EventKind::ActionCardDiscarded]);
    }

    /// Bail: a galvanized ground force that falls cannot be staged (the string row has a `|g` tail the typed row cannot
    /// hold), so the run is unknown and the adapter keeps the string's.
    #[test]
    fn bail_a_galvanized_ground_force_that_falls_makes_the_run_unknown() {
        use crate::generated::{Planet, PlanetTrait};
        let mut s = two_seats();
        let planet = *Planet::ALL.iter().find(|p| p.data().system.is_some() && p.data().traits.contains(&PlanetTrait::Hazardous)).unwrap();
        let system = planet.data().system.unwrap();
        fx::place_unit(&mut s, Seat(1), system, Some(planet), Unit::InfantryI);
        s.board_mut().planet_mut(planet).units.as_mut_slice()[0] = crate::state::Piece::new(Unit::InfantryI, Seat(1)).with_galvanized(true);
        let a = Announce::of(&s, ActionCard::UnstablePlanet, Seat(0), env(), true, false).unwrap();
        let d = run(a, &mut s, plays_table());
        assert_eq!(d.unknown(), Some(Unknown::MissingFact));
        // Without the galvanized token the same fall is staged as a ground-force row.
        let mut s = two_seats();
        fx::place_unit(&mut s, Seat(1), system, Some(planet), Unit::InfantryI);
        let a = Announce::of(&s, ActionCard::UnstablePlanet, Seat(0), env(), true, false).unwrap();
        let d = run(a, &mut s, plays_table());
        assert_eq!(d.unknown(), None);
        assert!(s.marks().tail.iter().any(|(k, _)| matches!(k, crate::keys::MarkKey::PrivateStagedGround(_))));
    }

    /// The cards of package B6d read facts only the adapter's play window supplies (forms, roster, map, traders): from inside
    /// another window they are the string's, and the census says so (found by the first run of the slot test of Reveal
    /// Prototype, which played with default facts).
    #[test]
    fn the_b6d_cards_are_not_played_from_inside_another_window() {
        let t = AbilityTable::default();
        for card in [ActionCard::Upgrade, ActionCard::DecoyOperation, ActionCard::RevealPrototype, ActionCard::LieInWait, ActionCard::Bribery, ActionCard::HarnessEnergy, ActionCard::Infiltrate, ActionCard::CrashLanding, ActionCard::GhostSquad, ActionCard::ExplorationProbe, ActionCard::Strategize2, ActionCard::Overrule, ActionCard::FireTeam, ActionCard::ScrambleFrequency] {
            assert!(plays(card), "{card:?} has a window of its own");
            assert!(!plays_in(&t, card), "{card:?}");
        }
        assert!(plays_in(&t, ActionCard::Rally) && plays_in(&t, ActionCard::Intercept));
    }

    /// A card whose window is not played from inside another window (it reads facts only its own window supplies) but that a rules
    /// package registered is played by the registration there (`core-b12` registers Fire Team for the ground roll's tail); a card
    /// the window plays nested stays the window's, registered or not.
    #[test]
    fn a_registered_effect_wins_over_a_window_that_is_not_played_nested() {
        let s = two_seats();
        assert!(Announce::of(&s, ActionCard::FireTeam, Seat(0), env(), true, false).is_none(), "no facts and nothing registered");
        let fire_team = Announce::of(&s, ActionCard::FireTeam, Seat(0), env(), true, true).expect("a registered play");
        assert!(matches!(fire_team.effect, Effect::Registered));
        let spy = Announce::of(&s, ActionCard::Spy, Seat(0), env(), true, true).expect("a play");
        assert!(matches!(spy.effect, Effect::Play(_)), "a card the window plays nested is the window's");
    }

    /// Bail: a card with an effect the core does not play has no announcement (the string runs).
    #[test]
    fn bail_a_card_with_an_unported_effect_has_no_announcement() {
        let s = two_seats();
        assert!(Announce::of(&s, ActionCard::CourageousToTheEnd, Seat(0), env(), true, false).is_none());
        assert!(Announce::of(&s, ActionCard::ExperimentalBattlestation, Seat(0), env(), true, false).is_none());
        assert!(Announce::of(&s, ActionCard::DirectHit1, Seat(0), env(), true, false).is_none());
    }

    /// Bail: destructions staged when the card is done (Direct Hit's removals are announced by the string).
    #[test]
    fn bail_a_play_that_leaves_staged_destructions_is_unknown() {
        let mut s = two_seats();
        s.transient_mut().combat.pending_destructions.push(PendingDestruction {
            system: System::ALL[0],
            owner: Seat(1),
            unit: Unit::ALL[0],
            reason: Code(0),
            during_space_combat: false,
        });
        let a = Announce::of(&s, ActionCard::MoraleBoost1, Seat(0), env(), true, false).unwrap();
        let d = run(a, &mut s, plays_table());
        assert_eq!(d.unknown(), Some(Unknown::PendingDestructions));
    }

    /// Bail: the Data Skimmer could take the discard (a breakthrough's pile, not the core's).
    #[test]
    fn bail_a_discard_the_data_skimmer_could_take_is_unknown() {
        let mut s = two_seats();
        let a = Announce::of(&s, ActionCard::MoraleBoost1, Seat(0), env(), true, false).unwrap();
        let mut table = plays_table();
        table.facts.plays.as_mut().unwrap().skimmer = true;
        let d = run(a, &mut s, table);
        assert_eq!(d.unknown(), Some(Unknown::DataSkimmer));
    }

    struct Emit;
    impl Window for Emit {
        type Stage = ();
        fn stage(&self) {}
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            let _ = sx.emit(fx::event(EventKind::ActionCardDiscarded, Payload::new()));
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }
    impl Clone for Emit {
        fn clone(&self) -> Self {
            Self
        }
    }

    /// Bail: Reflective Shielding played inside a window stages hits that the string sustain step drains after the
    /// window closes; the core windows do not, so the run is unknown (regression of the wide golden dual).
    #[test]
    fn bail_reflective_shielding_played_inside_a_window_is_unknown() {
        let mut s = two_seats();
        s.transient_mut().misc.declared_card_plays.push((Seat(0), ActionCard::ReflectiveShielding));
        let mut table = plays_table();
        register(&mut table);
        let k = Kernel::plain();
        let mut d = Driver::new(Emit, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(d.unknown(), Some(Unknown::ReflectiveShielding));
    }

    /// Bail: a play from inside a window needs the adapter's facts; without them the run is unknown.
    #[test]
    fn bail_a_play_inside_a_window_without_the_adapter_facts_is_unknown() {
        let mut s = two_seats();
        s.transient_mut().misc.declared_card_plays.push((Seat(0), ActionCard::MoraleBoost1));
        // The close hook runs through the resolver: an emission of any event closes its windows.
        let mut table = AbilityTable::default();
        register(&mut table);
        let k = Kernel::plain();
        let mut d = Driver::new(Emit, Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), table, 1);
        assert_eq!(d.run(&mut s, &k).unwrap(), Status::Done);
        assert_eq!(d.unknown(), Some(Unknown::PlayFacts));
    }
}
