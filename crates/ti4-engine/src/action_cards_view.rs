//! `Area::ActionCards` (package B6): the typed action-card reads of `ti4_engine_core::rules::action_cards`
//! behind the string interface. One [`ActionView`] is the typed view of one position (the Meta, Seats,
//! Players, Board and Agenda slices). The faction answers are passed to the rules as arguments (they come
//! from the string modules until package C). A card or player id the tables do not know makes a method
//! answer `None` and the caller keeps the string answer.

use ti4_engine_core::generated::{ActionCard, System};
use ti4_engine_core::rules::action_cards::Window;
use ti4_engine_core::rules::action_cards as rules;
use ti4_engine_core::state::{Slice, SliceSet};
use ti4_model::id::{ActionCardId, PlayerId};
use ti4_model::state::GameState;

use crate::core_sync::{CoreView, Ctx, Sources};
use crate::generated::IdStr;

/// What the action-card reads need.
pub const READS: SliceSet = SliceSet::of(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Agenda]);

/// The typed view of one position.
pub struct ActionView<'a> {
    view: CoreView,
    cx: Ctx,
    state: &'a GameState,
    blanked: bool,
}

/// `action_cards::is_component_action` of a card id (needs no view).
#[must_use]
pub fn is_component_action(alias: &ActionCardId) -> Option<bool> {
    Some(rules::is_component_action(ActionCard::from_id(alias.as_str())?))
}

impl<'a> ActionView<'a> {
    /// The view of a position; `None` when something is not mapped.
    #[must_use]
    pub fn of(state: &'a GameState) -> Option<Self> {
        let mut cx = Ctx::of(state).ok()?;
        let view = CoreView::of(state, &mut cx, Sources { galaxy: None, rng: None }, READS).ok()?;
        Some(Self { view, cx, state, blanked: crate::laws::blanked(state) })
    }

    /// The view when the area runs on the core or both (the offline check), else `None`.
    #[must_use]
    pub fn open(state: &'a GameState) -> Option<Self> {
        if crate::dual::backend(crate::dual::Area::ActionCards) == crate::dual::Backend::String {
            return None;
        }
        Self::of(state)
    }

    /// `action_cards::hand_limit`.
    #[must_use]
    pub fn hand_limit(&self, player: &PlayerId, content: &ti4_content::ContentStore) -> Option<usize> {
        self.cx.owners.seat(player.as_str())?;
        Some(rules::hand_limit(&self.view, self.blanked, |base| {
            crate::factions::hooks_economy::action_card_limit(self.state, content, player, base)
        }))
    }

    /// The head gates of `action_cards::is_playable`: whether `player` may play action cards at all.
    #[must_use]
    pub fn may_play(&self, player: &PlayerId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        Some(rules::may_play(
            &self.view,
            self.blanked,
            seat,
            crate::factions::hooks_economy::action_cards_forbidden(self.state, player),
            crate::laws::political_secret_bars(self.state, player),
        ))
    }

    /// `action_cards::has_legal_target` for the cards the core answers; `None` is the documented bail.
    #[must_use]
    pub fn has_legal_target(&self, player: &PlayerId, alias: &ActionCardId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        rules::has_legal_target(&self.view, self.blanked, crate::core_adapter::scope(ti4_model::content_types::SourceSet::all()), seat, ActionCard::from_id(alias.as_str())?)
    }

    /// `action_cards::reaction_can_matter` after the cost check, for a window naming `system`; `None` is
    /// the documented bail. `actor` is the event's `player` text.
    #[must_use]
    pub fn reaction_can_matter(
        &self,
        player: &PlayerId,
        alias: &ActionCardId,
        system: &str,
        actor: Option<&str>,
    ) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let actor = match actor {
            None => None,
            Some(a) => Some(self.cx.owners.seat(a)?),
        };
        let window = Window { system: System::from_id(system)?, actor };
        rules::reaction_can_matter(
            &self.view,
            crate::core_adapter::scope(ti4_model::content_types::SourceSet::all()),
            seat,
            ActionCard::from_id(alias.as_str())?,
            window,
        )
    }

    /// The hand positions `action_cards::available_actions` offers; `None` is the documented bail.
    #[must_use]
    pub fn playable_hand(&self, player: &PlayerId) -> Option<Vec<usize>> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let gates = rules::Gates {
            faction_forbids: crate::factions::hooks_economy::action_cards_forbidden(self.state, player),
            political_secret_bars: crate::laws::political_secret_bars(self.state, player),
            goods: &|| crate::supply::potential_goods(self.state, player),
        };
        let scope = crate::core_adapter::scope(ti4_model::content_types::SourceSet::all());
        let hand = rules::playable_hand(&self.view, self.blanked, scope, seat, &gates)?;
        Some(hand.as_slice().iter().map(|i| usize::from(*i)).collect())
    }

    /// `action_cards::cost_is_payable`; `None` is the documented bail.
    #[must_use]
    pub fn cost_is_payable(&self, player: &PlayerId, alias: &ActionCardId) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let card = ActionCard::from_id(alias.as_str())?;
        // The goods are asked only for the card that reads them (the string supply rules answer).
        let goods = if card == ActionCard::Bribery { crate::supply::potential_goods(self.state, player) } else { 0 };
        rules::cost_is_payable(&self.view, seat, card, goods)
    }
}

/// The own-hand question as the core renders it, for `choose_from_own_hand`: the options are the distinct
/// printed cards of `hand` (the caller derived them: names are text), `None` for a reason the core has no
/// word for (the documented bail: any other prompt or subtype) or an unmapped seat or card.
#[must_use]
pub fn own_hand_choice(
    state: &GameState,
    player: &PlayerId,
    source: &str,
    subtype: &str,
    prompt: &str,
    firsts: &[ActionCardId],
    optional: bool,
) -> Option<crate::choice::Choice> {
    use ti4_engine_core::choice as core;
    use ti4_engine_core::choice::cards as cards;
    let cx = Ctx::of(state).ok()?;
    let r = crate::render::Render::new(cx.owners.ids().to_vec());
    let seat = cx.owners.seat(player.as_str())?;
    let reason = match subtype {
        "stall_tactics_discard" => cards::Reason::StallTactics,
        "deepgloom_stall_tactics_discard" => cards::Reason::DeepgloomStallTactics,
        "malleon_discard" => cards::Reason::MalleonDiscard,
        "c4r_discard" => cards::Reason::C4rDiscard,
        "show_action_card" => {
            let viewer = prompt.strip_prefix("show ")?.strip_suffix(" 1 action card from your hand")?;
            cards::Reason::Show { viewer: cx.owners.seat(viewer)? }
        }
        _ => return None,
    };
    let mut opts = core::Opts::default();
    for alias in firsts {
        opts.push(core::Opt::Cards(cards::Opt::Card(ActionCard::from_id(alias.as_str())?)));
    }
    if optional {
        opts.push(core::Opt::Decline);
    }
    let choice = core::Choice {
        actor: seat,
        ask: core::Ask::Cards(cards::Ask::OwnHand { reason }),
        ctx: core::Ctx {
            origin: core::Origin::FactionAbility(crate::render::generic::content_of(&r, source)),
            subtype: core::Subtype::Cards(reason.subtype()),
            optional: false,
            target: None,
            outstanding: ti4_engine_core::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts,
    };
    let (phase, round) = (crate::render::generic::core_phase(state.phase), state.round);
    let rendered = crate::render::Typed::new(&r, choice, phase, round).rendered;
    // Bail: a caller with wording the core does not know (tests, new callers) keeps the string question.
    (rendered.prompt == prompt).then_some(rendered)
}

/// Play a stamp-or-flag effect (`rules::action_cards::Simple`, by name) on the core's copy of the position
/// and write the result back; `None` when the position is outside the tables (the string effect then runs).
#[must_use]
pub fn play_simple(state: &mut GameState, player: &PlayerId, effect: &str, sources: &ti4_model::content_types::SourceSet) -> Option<()> {
    use rules::Simple;
    let effect = match effect {
        "morale_boost" => Simple::MoraleBoost,
        "flank_speed" => Simple::FlankSpeed,
        "solar_flare" => Simple::SolarFlare,
        "coup" => Simple::Coup,
        "crisis" => Simple::Crisis,
        "master_plan" => Simple::MasterPlan,
        "lost_star" => Simple::LostStar,
        "hack_election" => Simple::HackElection,
        "political_stability" => Simple::PoliticalStability,
        "rout" => Simple::Rout,
        "waylay" => Simple::Waylay,
        "f_prototype" => Simple::FighterPrototype,
        "bunker" => Simple::Bunker,
        "war_machine" => Simple::WarMachine,
        "blitz" => Simple::Blitz,
        "nav_suite" => Simple::NavSuite,
        "black_market" => Simple::BlackMarket,
        "rally" => Simple::Rally,
        "maneuvering_jets" => Simple::ManeuveringJets,
        "shields_holding" => Simple::ShieldsHolding,
        "reflective" => Simple::Reflective,
        "confounding" => Simple::Confounding,
        "deadly_plot" => Simple::DeadlyPlot,
        "salvage" => Simple::Salvage,
        "reverse_engineer" => Simple::ReverseEngineer,
        "economic_initiative" => Simple::EconomicInitiative,
        "disable" => Simple::Disable,
        "councilor" => Simple::Councilor,
        "emergency_repairs" => Simple::EmergencyRepairs,
        _ => return None,
    };
    let (mut cx, r) = crate::dual_window::tables(state)?;
    let seat = r.seat(player.as_str())?;
    let fp = crate::core_sync::Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Transient, Slice::Board, Slice::Agenda, Slice::Cards],
        &[Slice::Players, Slice::Transient, Slice::Board, Slice::Agenda, Slice::Cards],
    );
    let env = env_of(state, player, sources, &r);
    crate::core_sync::with_core(state, &mut cx, Sources::default(), fp, |core, _| rules::play_simple(core, seat, effect, env)).ok()
}

/// What the played effects read from outside the core.
fn env_of(state: &GameState, player: &PlayerId, sources: &ti4_model::content_types::SourceSet, r: &crate::render::Render) -> rules::Env {
    rules::Env {
        mahact_held: state.tokens_held_by_mahact(player),
        scope: crate::core_adapter::scope(sources.clone()),
        blanked: crate::laws::blanked(state),
        action_card: r.code("action_card"),
        supply_base: r.code("forward_supply_base"),
        note_order: |a, b| crate::render::cards::note_text(a).cmp(&crate::render::cards::note_text(b)),
        infantry: rules::Placed::NONE,
        cruiser: rules::Placed::NONE,
        fighter: rules::Placed { unit: Some(ti4_engine_core::generated::Unit::FighterI), on_card: fighter_on_card(state, player, sources) },
    }
}

/// The fighters on the Crimson hero card (Fighter Conscription places the plain fighter against the supply).
pub(crate) fn fighter_on_card(state: &GameState, player: &PlayerId, sources: &ti4_model::content_types::SourceSet) -> i64 {
    crate::factions::crimson_cards::held_on_card(state, ti4_content::ContentStore::embedded(), sources.clone(), player, "fighter")
}

/// Whether the card's effect rolls the game's dice (`Fx::rolls`).
#[must_use]
pub fn rolls(alias: &ActionCardId) -> bool {
    ActionCard::from_id(alias.as_str()).and_then(rules::effects::Fx::of).is_some_and(rules::effects::Fx::rolls)
}

/// The window lent the streams when the card rolls: the footprint, the sources and the seeds of the dice stream. `None`
/// when the card rolls and the dice are not ones the core can reproduce (the adapter bails).
fn lend_rolls(
    alias: &ActionCardId,
    before: Option<&crate::rng::RngPositions>,
    label: &str,
) -> Option<Option<crate::dual_window::Lent>> {
    if !rolls(alias) {
        return Some(None);
    }
    let Some(before) = before else {
        crate::dual::note_bail(label, "irreproducible dice (test faces or a stream restore)");
        return None;
    };
    Some(Some(crate::dual_window::Lent { before: before.clone() }))
}

/// Whether the core has a window for the card (`rules::action_cards::effects::Fx`).
#[must_use]
pub fn plays(alias: &ActionCardId) -> bool {
    ActionCard::from_id(alias.as_str()).and_then(rules::effects::Fx::of).is_some()
}

const PLAY: crate::core_sync::Footprint = crate::core_sync::Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger],
    &[Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger],
);

/// The core half of a played card effect (`action_cards::effect_for`), as a lockstep window. `None` when the
/// card has no window or the position holds something the core cannot express.
#[must_use]
pub fn play_window(
    gs: &mut GameState,
    player: &PlayerId,
    alias: &ActionCardId,
    sources: &ti4_model::content_types::SourceSet,
    script: &crate::dual_window::Recording,
    before_rolls: Option<&crate::rng::RngPositions>,
) -> Option<crate::dual::CoreWindow<crate::action_cards_typed::Rolled>> {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::stage::AbilityTable;
    use ti4_engine_core::kernel::Kernel;
    let card = ActionCard::from_id(alias.as_str())?;
    let (mut cx, r) = crate::dual_window::tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let mut env = env_of(gs, player, sources, &r);
    env.infantry = placed(gs, player, sources, "infantry")?;
    env.cruiser = placed(gs, player, sources, "cruiser")?;
    let lent = lend_rolls(alias, before_rolls, "action_cards::play")?;
    let before = gs.clone();
    let kernel = Kernel::plain();
    let mut staged = Vec::new();
    let mut emitted = Vec::new();
    let mut faces = Vec::new();
    let mut unsupported = false;
    let mut rng_after = None;
    let fp = if lent.is_some() { PLAY.union(crate::core_sync::Footprint::new(&[Slice::Rng], &[Slice::Rng])) } else { PLAY };
    let src = Sources { galaxy: None, rng: lent.as_ref().map(|l| &l.before) };
    let mut driver_seeds = None;
    if lent.is_some() {
        driver_seeds = Some(std::rc::Rc::new(crate::dual_window::DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?)));
    }
    let details = |c: &ti4_engine_core::choice::Choice, _: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
        recorded_details(c, script, rendered);
    };
    let driven = crate::core_sync::with_core(gs, &mut cx, src, fp, |core, _| {
        let play = rules::effects::Play::of(core, card, seat, env)?;
        let mut driver = Driver::new(play, ti4_engine_core::kernel::Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        if let Some(seeds) = driver_seeds {
            driver = driver.with_seeds(seeds);
        }
        let driven = crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &details);
        staged = std::mem::take(&mut core.pending_mut().staged);
        emitted = std::mem::take(&mut driver.emitted);
        faces = driver.window.faces().to_vec();
        unsupported = driver.window.unsupported();
        if lent.is_some() {
            rng_after = Some(core.rng().clone());
        }
        Some(driven)
    })
    .ok()??
    .ok()?;
    if unsupported {
        return None;
    }
    crate::dual_window::apply_staged(gs, &r, &staged)?;
    let events: Vec<crate::event::Event> = emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
    let positions = match rng_after {
        Some(after) => Some(crate::core_sync::codec::rng::positions(&cx, &after).ok()?),
        None => None,
    };
    Some(crate::dual::CoreWindow { run: crate::dual::Run::new((faces, positions)).with_events_vec(events), questions: driven.rendered })
}

/// The display facts only the card's text knows (`Choice.details` of Repeal Law's `laws`): taken from the recorded question
/// with the same prompt.
fn recorded_details(c: &ti4_engine_core::choice::Choice, script: &crate::dual_window::Recording, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::cards::What;
    if matches!(c.ask, ti4_engine_core::choice::Ask::Cards(ti4_engine_core::choice::cards::Ask::Pick { what: What::Law | What::Hull | What::Scuttle { .. }, .. }))
        && let Some(asked) = script.questions.iter().find(|q| q.choice.prompt == rendered.prompt)
    {
        rendered.details.clone_from(&asked.choice.details);
    }
}

/// `action_cards::discard` on the core: remove the card at `index` from `player`'s hand. `Some(card)` is the
/// card taken (`None` inside: no such position), `None` outside is the bail (the string runs).
#[must_use]
pub fn discard(state: &mut GameState, player: &PlayerId, index: usize) -> Option<Option<ActionCardId>> {
    let (mut cx, r) = crate::dual_window::tables(state)?;
    let seat = r.seat(player.as_str())?;
    let fp = crate::core_sync::Footprint::new(&[Slice::Meta, Slice::Seats, Slice::Players], &[Slice::Players]);
    let taken = crate::core_sync::with_core(state, &mut cx, Sources::default(), fp, |core, _| rules::discard_from_hand(core, seat, index)).ok()?;
    Some(taken.map(|card| ActionCardId::new(card.as_str())))
}

/// What `action_cards::place_units` would place of a base type for `player`: the unit form (upgrades, faction
/// overrides), nothing when the faction forbids effect placement, and the ships on the Crimson card. `None`
/// when the form is not a unit the core knows.
pub(crate) fn placed(state: &GameState, player: &PlayerId, sources: &ti4_model::content_types::SourceSet, base: &str) -> Option<rules::Placed> {
    use ti4_engine_core::generated::Unit;
    let content = ti4_content::ContentStore::embedded();
    let on_card = crate::factions::crimson_cards::held_on_card(state, content, sources.clone(), player, base);
    let Some(id) = crate::action_cards::placed_unit_id(state, content, sources.clone(), player, base) else {
        return Some(rules::Placed { unit: None, on_card });
    };
    if crate::factions::hooks_economy::effect_placement_forbidden(state, content, sources.clone(), player, &id) {
        return Some(rules::Placed { unit: None, on_card });
    }
    Some(rules::Placed { unit: Some(Unit::from_id(id.as_str())?), on_card })
}

/// Why a play cannot be compared: the Data Skimmer could take the discard (the string moves the card to a
/// breakthrough, which the core does not hold).
const SKIMMER_BAIL: &str = "a discard the Data Skimmer could take";

/// The core half of `reactions::announce`: the announcement of a played card, its effect and its discard, as one
/// lockstep window (the reaction windows around `ACTION_CARD_PLAYED` and `ACTION_CARD_DISCARDED` are followed by
/// the ability table). `None` when the card has no core effect or the position holds something the core cannot
/// express; the value is whether the play stood.
#[must_use]
pub fn announce_window(
    gs: &mut GameState,
    player: &PlayerId,
    alias: &ActionCardId,
    sources: &ti4_model::content_types::SourceSet,
    script: &crate::dual_window::Recording,
    before_rolls: Option<&crate::rng::RngPositions>,
) -> Option<crate::dual::CoreWindow<(Option<bool>, crate::action_cards_typed::Rolled)>> {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::stage::AbilityTable;
    let card = ActionCard::from_id(alias.as_str())?;
    if !rules::announce::plays(card) && crate::action_cards::effect_for(alias).is_some() {
        return None;
    }
    let (mut cx, r) = crate::dual_window::tables(gs)?;
    let seat = r.seat(player.as_str())?;
    let mut env = env_of(gs, player, sources, &r);
    env.infantry = placed(gs, player, sources, "infantry")?;
    env.cruiser = placed(gs, player, sources, "cruiser")?;
    if crate::factions::ralnel_cards::skimmer_holder(gs).is_some() {
        crate::dual::note_bail("action_cards::announce", SKIMMER_BAIL);
        return None;
    }
    let has_effect = crate::action_cards::effect_for(alias).is_some();
    let lent = lend_rolls(alias, before_rolls, "action_cards::announce")?;
    let before = gs.clone();
    let window = crate::core_combat::window_env(gs, &cx, sources.clone())?;
    let kernel = &window.kernel;
    let mut staged = Vec::new();
    let mut emitted = Vec::new();
    let mut played = None;
    let mut faces = Vec::new();
    let mut rng_after = None;
    let mut fp = crate::dual_window::nested_fp(PLAY, script);
    if lent.is_some() {
        fp = fp.union(crate::core_sync::Footprint::new(&[Slice::Rng], &[Slice::Rng]));
    }
    let src = Sources { galaxy: None, rng: lent.as_ref().map(|l| &l.before) };
    let seeds = match lent {
        Some(_) => Some(std::rc::Rc::new(crate::dual_window::DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?))),
        None => None,
    };
    let rolled = seeds.is_some();
    let driven = crate::core_sync::with_core(gs, &mut cx, src, fp, |core, _| {
        let announce = rules::announce::Announce::of(core, card, seat, env, has_effect, false)?;
        let mut driver = Driver::new(announce, ti4_engine_core::kernel::Resolver::new(&[seat], Some(seat)), AbilityTable::default(), 1);
        if let Some(seeds) = seeds {
            driver = driver.with_seeds(seeds);
        }
        let battle = |_c: &ti4_engine_core::choice::Choice, _: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
            // The effect's questions are asked through `ask_seeing`: the battle or invasion the position is in.
            if let Some(ctx) = rendered.context.take() {
                rendered.context = Some(ctx.about_battle(&before).about_invasion(&before));
            }
            recorded_details(_c, script, rendered);
        };
        let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &battle);
        played = Some(driver.window.played());
        faces = driver.window.faces();
        if rolled {
            rng_after = Some(core.rng().clone());
        }
        staged = std::mem::take(&mut core.pending_mut().staged);
        emitted = std::mem::take(&mut driver.emitted);
        Some(driven)
    })
    .ok()??
    .ok()?;
    crate::dual_window::apply_staged(gs, &r, &staged)?;
    let events: Vec<crate::event::Event> = emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>()?;
    let positions = match rng_after {
        Some(after) => Some(crate::core_sync::codec::rng::positions(&cx, &after).ok()?),
        None => None,
    };
    Some(crate::dual::CoreWindow { run: crate::dual::Run::new((played, (faces, positions))).with_events_vec(events), questions: driven.rendered })
}
