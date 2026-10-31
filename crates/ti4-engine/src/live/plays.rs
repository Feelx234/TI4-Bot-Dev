//! The play facts of the live loop (`AbilityTable::facts.plays`): what an action card played from inside a window reads of the
//! position, computed from the CORE position and the kernel alone. The replay path reads them from the string `GameState`
//! (`reactions_core::play_facts`); this is its live counterpart, and `ti4_engine_core_bridge::check_play_facts` compares the two over
//! the template positions and their variants (the sides must agree wherever the string gives an answer the product keeps).
//!
//! Position-dependent facts are never kept in the `Env`: the core's `driver::live` asks the [`PlaysProvider`] every time it builds a
//! driver (once per `advance`/`ask` call), so the units a seat places after a technology, the Mahact pools and the research a card
//! opens are those of the position the call starts from. (Within one call they are as fresh as the replay path's, which reads them
//! when the window is armed.)
//!
//! Departures from the string side, by decision (dropped content, `docs/D2_LF_RESUME.md` section 0):
//!
//! * no Crimson card: the ships held on the Crimson hero card are 0 (`on_card`, `fighter_on_card`), the faction is refused at creation;
//! * no Ral Nel: the Data Skimmer cannot take a discard (`skimmer` false);
//! * `researches` (Reveal Prototype's set) is not declined for a Deepwrought, Keleres or Mentak-Corsair seat at the table or for a seat
//!   whose research asks more questions (`strategy_cards_core::research_unported`): the core windows answer those through the hook
//!   table of the live kernel, and a purged technology (Thunder's Edge, out of scope) does not exist.
//!
//! `core-w-l11` (cards played inside another area's window) grew the facts by `SeatPlay::more` (the forms of destroyer, dreadnought, mech,
//! pds and space dock, the Keleres agent, Deepwrought) and `PlayFacts::shared` (the neutral owner, replenish events, the Fracture, the
//! breakthroughs, the texts of the hit flows): both are filled from the core here ([`seat_more`], [`shared`]). `replenish_events` and
//! `fracture_in_play` are false by decision (see [`shared`]).

use ti4_engine_core::generated::{ActionCard, Breakthrough, Faction, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::{PlayFacts, PlaysProvider, SeatPlay, SeatPlayMore, SharedPlay};
use ti4_engine_core::kernel::children::HitTexts;
use ti4_engine_core::kernel::fx_strategy::placed_unit;
use ti4_engine_core::kernel::Kernel;
use ti4_engine_core::query::economy::Spend;
use ti4_engine_core::query::tech::{self, UnitRoster};
use ti4_engine_core::rules::production::strategy_cards::KernelHooks;
use ti4_engine_core::rules::production::{self as prod, Cx as ProdCx, Held};
use ti4_engine_core::state::{Code, CoreState, TechSet};
use ti4_model::content_types::SourceSet;

use crate::reactions_core::{card_effects, card_groups};
use crate::render::Render;

/// The texts and tables a play reads that are not in the position: the interned codes, the content scope and its unit roster.
/// Built once per game (by [`provider`]), never per call.
#[derive(Clone)]
pub struct Texts {
    pub action_card: Code,
    pub supply_base: Code,
    /// The seat of the neutral owner in this table's owner list (`Render::seat("neutral")`), when the table has one.
    pub neutral: Option<Seat>,
    courageous_subtype: Code,
    courageous_cause: Code,
    commander: Code,
    casualty: Code,
    effect_hit: Code,
    pub sources: SourceSet,
    pub roster: Option<&'static UnitRoster>,
    name_group: Vec<u8>,
    has_effect: Vec<bool>,
}

impl Texts {
    #[must_use]
    pub fn new(r: &Render, sources: SourceSet) -> Self {
        Self {
            action_card: r.code("action_card"),
            supply_base: r.code("forward_supply_base"),
            neutral: r.seat(crate::neutral_units::NEUTRAL),
            courageous_subtype: r.code("courageous_to_the_end_assign_casualty"),
            courageous_cause: r.code("action_card:courageous"),
            commander: r.code("letnevcommander"),
            casualty: r.code("assign_casualty"),
            effect_hit: r.code("effect_hit"),
            sources,
            roster: crate::technology_core::roster_for(ti4_content::ContentStore::embedded(), sources),
            name_group: card_groups(),
            has_effect: card_effects(),
        }
    }
}

/// The provider the live `Env` carries (`table.facts.plays_provider`).
#[must_use]
pub fn provider(r: &Render, sources: SourceSet) -> PlaysProvider {
    let texts = Texts::new(r, sources);
    PlaysProvider(std::sync::Arc::new(move |core, kernel| play_facts(core, kernel, &texts)))
}

/// The unit a seat places for `base` (`action_cards_view::placed` on the core): its upgrade, its faction's own unit or the plain one,
/// with the faction override of the hook table; nothing where a faction effect forbids placing it. The second part is the ships of that
/// type on the Crimson card (always 0, see the module header).
#[must_use]
pub fn placed(core: &CoreState, kernel: &Kernel, texts: &Texts, who: Seat, base: UnitBaseType) -> (Option<Unit>, i64) {
    let scope = ti4_engine_core::query::reach::scope_of(kernel);
    let Some(roster) = texts.roster else { return (None, 0) };
    let unit = placed_unit(core, kernel, scope, roster, who, base).filter(|u| !kernel.hooks.economy.effect_placement_forbidden(core, kernel, who, *u));
    (unit, 0)
}

/// What a card played from inside a window reads of a seat besides its infantry and cruiser (`reactions_core::seat_more` on the core): the
/// forms it places, no readied Keleres agent and no Deepwrought seat (both dropped).
#[must_use]
pub fn seat_more(core: &CoreState, kernel: &Kernel, texts: &Texts, who: Seat) -> SeatPlayMore {
    let naaz = core.players().by_seat(who).is_some_and(|p| p.faction == Faction::TheNaazRokhaAlliance);
    SeatPlayMore {
        destroyer: placed(core, kernel, texts, who, UnitBaseType::Destroyer),
        dreadnought: (Some(Unit::DreadnoughtI), 0),
        // The plain mech; a Naaz seat's is a unit whose placement the string announces (no unit here: the card declines).
        mech: ((!naaz).then_some(Unit::FactionlessMech), 0),
        pds: placed(core, kernel, texts, who, UnitBaseType::Pds),
        spacedock: placed(core, kernel, texts, who, UnitBaseType::Spacedock),
        agent_offered: false,
        holder_deepwrought: false,
    }
}

/// What the cards that read the table read (`reactions_core::play_facts`'s `shared` on the core). Cabal's replenish note and the Trade
/// Agreement are the core's (package W10) and the siphoning plot is a dropped faction's, so a replenishment causes no event the core does
/// not hold; the Fracture is Thunder's Edge content, out of the agreed scope.
#[must_use]
pub fn shared(core: &CoreState, texts: &Texts) -> SharedPlay {
    let content = ti4_content::ContentStore::embedded();
    let mut own_breakthrough = [None; 16];
    for p in core.players().iter() {
        let faction = crate::generated::IdStr::as_str(p.faction);
        let breakthrough = crate::breakthroughs::for_faction(content, faction).and_then(|b| <Breakthrough as crate::generated::IdStr>::from_id(b.as_str()));
        if let Some(slot) = own_breakthrough.get_mut(usize::from(p.seat.0)) {
            *slot = breakthrough;
        }
    }
    SharedPlay {
        neutral: texts.neutral,
        replenish_events: false,
        fracture_in_play: false,
        own_breakthrough,
        unit_order: |a, b| crate::generated::IdStr::as_str(a).cmp(crate::generated::IdStr::as_str(b)),
        courageous_subtype: texts.courageous_subtype,
        courageous_cause: texts.courageous_cause,
        hits: HitTexts { staging: staging_enabled(core), commander: texts.commander, casualty: texts.casualty, effect_hit: texts.effect_hit },
    }
}

/// `supply::staging_enabled` on the core: some seat's faction has a registered module (or is the Obsidian, never seated).
#[must_use]
pub fn staging_enabled(core: &CoreState) -> bool {
    core.players().iter().any(|p| {
        let faction = crate::generated::IdStr::as_str(p.faction);
        crate::factions::MODULES.iter().any(|module| module.alias == faction)
    })
}

/// `GameState::tokens_held_by_mahact`: how many of `who`'s command tokens sit in a Mahact seat's fleet pool.
#[must_use]
pub fn mahact_held(core: &CoreState, kernel: &Kernel, who: Seat) -> i32 {
    let held: usize = core
        .players()
        .iter()
        .map(|p| kernel.hooks.direct.mahact_fleet_pool_owners(core, p.seat).as_slice().iter().filter(|owner| **owner == who).count())
        .sum();
    i32::try_from(held).unwrap_or(i32::MAX)
}

/// `action_cards_view::reveal_set`: the unit upgrades of the line of a unit of `who` in the active system (its space area, as the string
/// reads it). The lines are read from the core board; the matching of a technology to a line is the string side's own
/// (`reveal_set_of_lines`), quirks included, so the two sides agree by construction wherever the board does.
#[must_use]
pub fn reveal_set(core: &CoreState, who: Seat, sources: &SourceSet) -> TechSet {
    let Some(system) = core.meta().active_system else { return TechSet::default() };
    let lines: Vec<&'static str> = core
        .board()
        .sys(system)
        .units
        .as_slice()
        .iter()
        .filter(|p| p.owner() == who)
        .map(|p| crate::generated::IdStr::as_str(p.kind().data().base_type))
        .collect();
    crate::action_cards_view::reveal_set_of_lines(&lines, sources)
}

/// Reveal Prototype is payable: the seat holds it, has the four resources, and a unit upgrade is open to research.
#[must_use]
pub fn reveal_payable(core: &CoreState, kernel: &Kernel, who: Seat) -> bool {
    let holds = core.players().by_seat(who).is_some_and(|p| p.cards.hand.as_slice().contains(&ActionCard::RevealPrototype));
    if !holds {
        return false;
    }
    let scope = ti4_engine_core::query::reach::scope_of(kernel);
    let blanked = ti4_engine_core::query::reach::blanked(core);
    let cx = ProdCx::new(core, kernel, scope, blanked);
    let held = Held::of(&cx, who);
    prod::available(&cx, who, Spend::Resources, &held.extras()) >= 4
        && tech::researchable(core, scope, blanked, who, &KernelHooks::new(core, kernel, who)).iter().any(tech::is_unit_upgrade)
}

/// The play facts of the position, or `None` when the seats cannot be told (no roster for the scope).
#[must_use]
pub fn play_facts(core: &CoreState, kernel: &Kernel, texts: &Texts) -> Option<PlayFacts> {
    let roster = texts.roster?;
    let researches = core.meta().rules_at_least(ti4_model::state::RULES_TECH_PAYMENT_CHOICE);
    let seats = core
        .players()
        .iter()
        .map(|p| {
            let who = p.seat;
            SeatPlay {
                who,
                mahact_held: mahact_held(core, kernel, who),
                infantry: placed(core, kernel, texts, who, UnitBaseType::Infantry),
                cruiser: placed(core, kernel, texts, who, UnitBaseType::Cruiser),
                fighter_on_card: 0,
                more: seat_more(core, kernel, texts, who),
                reveal: researches.then(|| reveal_set(core, who, &texts.sources)),
                reveal_payable: researches && reveal_payable(core, kernel, who),
            }
        })
        .collect();
    Some(PlayFacts {
        scope: u32::from(kernel.sources.0),
        blanked: ti4_engine_core::query::reach::blanked(core),
        action_card: texts.action_card,
        supply_base: texts.supply_base,
        note_order: |a, b| crate::render::cards::note_text(a).cmp(&crate::render::cards::note_text(b)),
        seats,
        name_group: texts.name_group.clone(),
        has_effect: texts.has_effect.clone(),
        skimmer: false,
        roster: Some(roster),
        rules_version: core.meta().rules_version,
        tech_order: |a, b| crate::generated::IdStr::as_str(a).cmp(crate::generated::IdStr::as_str(b)),
        shared: shared(core, texts),
    })
}


#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use ti4_engine_core::choice::{Ask, Opt, Pick};
    use ti4_engine_core::driver::live::{advance, open, Live, LiveError};
    use ti4_engine_core::driver::NoSeeds;
    use ti4_engine_core::generated::{ActionCard, Faction};
    use ti4_engine_core::kernel::stage::{Illegal, Sx, Window};
    use ti4_engine_core::kernel::{fx, Kernel, Next};
    use ti4_engine_core::state::{CoreState, EventKind, Key, Payload, ProductionUse, Val, Window as OpenWindow, WindowKind};
    use ti4_model::content_types::POK;

    use super::*;
    use crate::core_sync::{CoreView, Ctx, Sources};

    /// A window that activates a system (seat 0) and ends: the event is where Flank Speed may be played.
    #[derive(Clone)]
    struct Activate {
        done: bool,
    }

    impl Window for Activate {
        type Stage = bool;
        fn stage(&self) -> bool {
            self.done
        }
        fn step(&mut self, sx: &mut Sx<'_>) -> Next {
            if !self.done {
                self.done = true;
                let rec = fx::event(EventKind::SystemActivated, Payload::new().with(Key::Player, Val::Seat(Seat(0))).with(Key::System, Val::System(ti4_engine_core::generated::System::ALL[0])));
                if sx.emit(rec).is_err() {
                    return Next::Done;
                }
            }
            Next::Done
        }
        fn ask_of(&self, _: &CoreState, _: &Kernel) -> Option<ti4_engine_core::choice::Choice> {
            None
        }
        fn answer(&mut self, _: &mut Sx<'_>, _: Pick) -> Result<(), Illegal> {
            Err(Illegal)
        }
    }

    impl ti4_engine_core::kernel::stage::Held for Activate {
        const KIND: WindowKind = WindowKind::Production;
        fn from_pending(core: &CoreState, _: &Kernel) -> Option<Self> {
            match core.pending().window(WindowKind::Production)? {
                OpenWindow::Production(p) => Some(Self { done: p.phase == ti4_engine_core::state::UsePhase::Using }),
                _ => None,
            }
        }
        fn to_pending(&self, core: &mut CoreState) {
            let (who, system) = (ti4_engine_core::ids::Seat(0), ti4_engine_core::generated::System::ALL[0]);
            let phase = if self.done { ti4_engine_core::state::UsePhase::Using } else { ti4_engine_core::state::UsePhase::Start };
            core.pending_mut().open(OpenWindow::Production(Box::new(ProductionUse::observed(who, system))));
            if let Some(OpenWindow::Production(p)) = core.pending_mut().window_mut(WindowKind::Production) {
                p.phase = phase;
            }
        }
    }

    /// Two seated players (Mentak a, Letnev b) as a core position; seat 0 holds Flank Speed.
    fn position() -> (CoreState, Render) {
        let gs = crate::fixtures::seated_dealt(&[("a", "mentak"), ("b", "letnev")], POK);
        let (mut cx, r) = crate::dual_window::tables_for_tests(&gs).expect("tables");
        let reads: Vec<ti4_engine_core::state::Slice> = ti4_engine_core::state::Slice::ALL.iter().copied().filter(|s| *s != ti4_engine_core::state::Slice::Map && *s != ti4_engine_core::state::Slice::Rng).collect();
        let view = CoreView::of(&gs, &mut cx, Sources { galaxy: None, rng: None }, ti4_engine_core::state::SliceSet::of(&reads)).expect("a position the codecs carry");
        let mut core = view.into_state();
        core.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(ActionCard::FlankSpeed1);
        (core, r)
    }

    /// Walk the window to its end: take the first ability of the reaction window (a single printed name asks no card question), decline the rest.
    fn walk(core: &mut CoreState, env: &ti4_engine_core::driver::live::Env<'_>) -> Result<Vec<Ask>, LiveError> {
        let mut asked = Vec::new();
        let mut pick = None;
        for _ in 0..12 {
            let (live, _) = advance::<Activate>(core, env, pick.take())?;
            let Live::Asks(a) = live else { return Ok(asked) };
            asked.push(a.choice.ask.clone());
            let ability = a.choice.opts.iter().position(|o| matches!(o, Opt::Reaction(ti4_engine_core::choice::reaction::Opt::Ability(_))));
            let card = a.choice.opts.iter().position(|o| matches!(o, Opt::Cards(_)));
            let at = card.or(ability);
            pick = Some(match at {
                Some(i) => a.choice.pick(i).unwrap(),
                None => Pick::Opt(Opt::Decline),
            });
        }
        panic!("the window did not end: {asked:?}");
    }

    /// A card played from inside a live window: with the provider of this module it is played (it leaves the hand and lands on the
    /// discard pile); with the bare `env_for` the step stops with `Unknown(PlayFacts)`, which is what the playthrough counted as a bail
    /// before the facts came from the core.
    #[test]
    fn a_card_played_inside_a_live_window_is_played_and_without_the_facts_stops_the_step() {
        let kernel = crate::live::kernel::kernel_for(POK);
        let (core, r) = position();
        let with = crate::live::env::env_with_plays(&r, &core, kernel, Rc::new(NoSeeds), POK);
        let mut played = core.clone();
        open(&mut played, &Activate { done: false });
        let asked = walk(&mut played, &with).expect("the card is played");
        assert!(!asked.is_empty(), "the reaction window asked");
        assert!(played.players().by_seat(Seat(0)).unwrap().cards.hand.is_empty(), "the card left the hand");
        assert_eq!(played.cards().discarded_action.as_slice(), &[ActionCard::FlankSpeed1], "and lies on the discard pile");
        // The same window in an Env without the facts.
        let bare = crate::live::env::env_for(&r, &core, kernel, Rc::new(NoSeeds));
        let mut stopped = core;
        open(&mut stopped, &Activate { done: false });
        assert_eq!(walk(&mut stopped, &bare), Err(LiveError::Unknown(ti4_engine_core::kernel::ability::Unknown::PlayFacts)));
    }

    /// The facts follow the position: the unit a seat places for infantry changes with a technology the seat gains between two calls of
    /// the same Env (it is computed per call, not once per game).
    #[test]
    fn the_facts_follow_the_position_the_env_was_not_built_for() {
        let kernel = crate::live::kernel::kernel_for(POK);
        let (mut core, r) = position();
        let texts = Texts::new(&r, POK);
        let before = play_facts(&core, kernel, &texts).expect("facts");
        assert_eq!(before.seats[0].infantry.0, Some(Unit::InfantryI));
        let upgrade = tech_of_unit(Unit::InfantryII);
        core.players_mut().by_seat_mut(Seat(0)).unwrap().tech.owned.insert(upgrade);
        let after = play_facts(&core, kernel, &texts).expect("facts");
        assert_eq!(after.seats[0].infantry.0, Some(Unit::InfantryII), "the upgrade the seat owns now");
        assert_eq!(after.seats[1].infantry.0, before.seats[1].infantry.0, "another seat is untouched");
        let _ = Faction::from_index(0);
    }

    fn tech_of_unit(unit: Unit) -> ti4_engine_core::generated::Technology {
        match unit.data().required_technology {
            ti4_engine_core::generated::Requirement::Known(t) => t,
            other => panic!("{unit:?}: {other:?}"),
        }
    }

    #[test]
    fn a_mahact_pool_and_reveal_prototype_are_read_from_the_core() {
        let kernel = crate::live::kernel::kernel_for(POK);
        let (mut core, r) = position();
        let _ = &r;
        // No Mahact seat at the table: nobody holds a token of another seat.
        assert_eq!(mahact_held(&core, kernel, Seat(0)), 0);
        // Reveal Prototype: needs an active system with units of the line.
        assert!(reveal_set(&core, Seat(0), &POK).iter().next().is_none(), "no active system");
        let home = ti4_engine_core::generated::System::ALL[0];
        core.meta_mut().active_system = Some(home);
        core.board_mut().sys_mut(home).units.push(ti4_engine_core::state::Piece::new(Unit::CruiserI, Seat(0)));
        let set = reveal_set(&core, Seat(0), &POK);
        assert!(set.contains(tech_of_unit(Unit::CruiserII)), "the cruiser line");
        assert!(!set.contains(tech_of_unit(Unit::DestroyerII)), "not the destroyer line");
        // Another seat's ships do not open a line for this seat.
        assert!(reveal_set(&core, Seat(1), &POK).iter().next().is_none());
        assert!(!reveal_payable(&core, kernel, Seat(0)), "the card is not in hand");
    }

    fn two_seats(a: Faction, b: Faction) -> (CoreState, Render) {
        let (mut core, r) = position();
        use ti4_engine_core::state::{Player, Players};
        core.players = Players::from_vec(vec![Player::new(Seat(0), a), Player::new(Seat(1), b)]);
        (core, r)
    }

    /// The forms a seat places: the plain unit, nothing where a faction effect forbids it or the scope has no roster, never a card ship.
    #[test]
    fn the_unit_a_seat_places_is_the_plain_form_unless_forbidden_or_without_a_roster() {
        let (core, r) = two_seats(Faction::TheNaazRokhaAlliance, Faction::TheEmbersOfMuaat);
        let texts = Texts::new(&r, POK);
        let kernel = crate::live::kernel::kernel_for(POK);
        assert_eq!(placed(&core, kernel, &texts, Seat(1), UnitBaseType::Infantry), (Some(Unit::InfantryI), 0));
        let mut banned = Kernel::plain();
        banned.hooks.economy.effect_placement_forbidden.push(|_, _, _, _| true);
        assert_eq!(placed(&core, &banned, &texts, Seat(1), UnitBaseType::Infantry), (None, 0), "forbidden: no unit");
        let bare = Texts { roster: None, ..texts };
        assert_eq!(placed(&core, kernel, &bare, Seat(1), UnitBaseType::Infantry), (None, 0), "no roster for the scope");
        assert!(play_facts(&core, kernel, &bare).is_none());
    }

    /// What a card reads of a seat beyond infantry and cruiser: the Naaz mech is a unit the card declines, the others place the plain mech.
    #[test]
    fn the_extra_facts_of_a_seat_name_its_forms_and_the_naaz_mech_is_declined() {
        let (core, r) = two_seats(Faction::TheNaazRokhaAlliance, Faction::TheEmbersOfMuaat);
        let texts = Texts::new(&r, POK);
        let kernel = crate::live::kernel::kernel_for(POK);
        let naaz = seat_more(&core, kernel, &texts, Seat(0));
        let muaat = seat_more(&core, kernel, &texts, Seat(1));
        assert_eq!(naaz.mech, (None, 0));
        assert_eq!(muaat.mech, (Some(Unit::FactionlessMech), 0));
        for more in [naaz, muaat] {
            assert_eq!(more.dreadnought, (Some(Unit::DreadnoughtI), 0));
            assert_eq!(more.destroyer.0, Some(Unit::DestroyerI));
            assert_eq!(more.pds.0, Some(Unit::PDSI));
            assert_eq!(more.spacedock.0, Some(Unit::SpaceDockI));
            assert!(!more.agent_offered && !more.holder_deepwrought);
            assert_eq!((more.destroyer.1, more.pds.1, more.spacedock.1), (0, 0, 0));
        }
    }

    /// The table-wide facts: the breakthrough of each seat's faction, the neutral owner, the texts, and nothing the core does not hold.
    #[test]
    fn the_table_wide_facts_carry_the_texts_the_breakthroughs_and_no_unheld_events() {
        let (core, r) = two_seats(Faction::TheNaazRokhaAlliance, Faction::TheEmbersOfMuaat);
        let texts = Texts::new(&r, POK);
        let shared = shared(&core, &texts);
        assert_eq!(shared.neutral, texts.neutral);
        assert_eq!(r.code_text(shared.courageous_subtype), "courageous_to_the_end_assign_casualty");
        assert_eq!(r.code_text(shared.courageous_cause), "action_card:courageous");
        assert_eq!(r.code_text(shared.hits.commander), "letnevcommander");
        assert_eq!(r.code_text(shared.hits.casualty), "assign_casualty");
        assert_eq!(r.code_text(shared.hits.effect_hit), "effect_hit");
        assert!(!shared.replenish_events && !shared.fracture_in_play);
        assert!(shared.own_breakthrough[2..].iter().all(Option::is_none), "no seat, no breakthrough");
        let expect = |f: Faction| crate::breakthroughs::for_faction(ti4_content::ContentStore::embedded(), crate::generated::IdStr::as_str(f)).and_then(|b| <Breakthrough as crate::generated::IdStr>::from_id(b.as_str()));
        assert_eq!(shared.own_breakthrough[0], expect(Faction::TheNaazRokhaAlliance));
        assert_eq!(shared.own_breakthrough[1], expect(Faction::TheEmbersOfMuaat));
        assert!(shared.own_breakthrough[0].is_some() && shared.own_breakthrough[1].is_some(), "both factions have a breakthrough");
        assert_ne!(shared.own_breakthrough[0], shared.own_breakthrough[1]);
    }

    /// Staging follows the seating: a faction with a string module turns it on, a table of factions without one does not.
    #[test]
    fn staging_is_on_exactly_when_a_seated_faction_has_a_module() {
        let without = Faction::ALL.iter().copied().find(|f| !crate::factions::MODULES.iter().any(|m| m.alias == crate::generated::IdStr::as_str(*f))).expect("a faction without a module");
        let with = Faction::ALL.iter().copied().find(|f| crate::factions::MODULES.iter().any(|m| m.alias == crate::generated::IdStr::as_str(*f))).expect("a faction with a module");
        assert!(staging_enabled(&two_seats(without, without).0) == false);
        assert!(staging_enabled(&two_seats(without, with).0));
        assert!(staging_enabled(&two_seats(with, without).0));
    }

    /// The tokens of a seat sitting in a Mahact seat's fleet pool, counted per owner.
    #[test]
    fn the_command_tokens_in_a_mahact_fleet_pool_are_counted_per_owner() {
        use ti4_engine_core::keys::{Atom, MarkKey, MarkVal};
        let (mut core, _) = two_seats(Faction::TheMahactGeneSorcerers, Faction::TheEmbersOfMuaat);
        let kernel = crate::live::kernel::kernel_for(POK);
        let pool = MarkVal::List(vec![MarkVal::Atom(Atom::Seat(Seat(1))), MarkVal::Atom(Atom::Seat(Seat(1)))].into());
        core.marks_mut().tail.edit().insert(MarkKey::MahactFleet(Seat(0)), pool);
        assert_eq!(mahact_held(&core, kernel, Seat(1)), 2);
        assert_eq!(mahact_held(&core, kernel, Seat(0)), 0, "the Mahact's own tokens are not in its pool");
    }

    /// Reveal Prototype is the facts of a game from the technology payment choice on; below it nothing is carried, whatever the cards say.
    #[test]
    fn reveal_prototype_is_carried_only_from_the_technology_payment_choice() {
        let (mut core, r) = two_seats(Faction::TheEmbersOfMuaat, Faction::TheNaazRokhaAlliance);
        let texts = Texts::new(&r, POK);
        let kernel = crate::live::kernel::kernel_for(POK);
        core.players_mut().by_seat_mut(Seat(0)).unwrap().cards.hand.push(ActionCard::RevealPrototype);
        core.players_mut().by_seat_mut(Seat(0)).unwrap().econ.trade_goods = 12;
        core.meta_mut().rules_version = ti4_model::state::RULES_TECH_PAYMENT_CHOICE - 1;
        let old = play_facts(&core, kernel, &texts).unwrap();
        assert!(old.seats.iter().all(|s| s.reveal.is_none() && !s.reveal_payable));
        core.meta_mut().rules_version = ti4_model::state::RULES_TECH_PAYMENT_CHOICE;
        let new = play_facts(&core, kernel, &texts).unwrap();
        assert!(new.seats.iter().all(|s| s.reveal.is_some()));
        assert!(new.seats.iter().all(|s| s.fighter_on_card == 0));
    }
}
