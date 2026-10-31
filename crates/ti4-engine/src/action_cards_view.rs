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
use crate::dual::Why;
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
    pub fn has_legal_target(&self, player: &PlayerId, alias: &ActionCardId, sources: ti4_model::content_types::SourceSet) -> Option<bool> {
        let seat = self.cx.owners.seat(player.as_str())?;
        // The scope of the GAME: a home system outside it is not a home system (the string's `is_home_system` reads the same
        // `sources`), so a seat of a faction of another expansion makes `all()` and the game's scope disagree (D2-5-find-02).
        rules::has_legal_target(&self.view, self.blanked, crate::core_adapter::scope(sources), seat, ActionCard::from_id(alias.as_str())?)
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
    pub fn playable_hand(&self, player: &PlayerId, sources: ti4_model::content_types::SourceSet) -> Option<Vec<usize>> {
        let seat = self.cx.owners.seat(player.as_str())?;
        let gates = rules::Gates {
            faction_forbids: crate::factions::hooks_economy::action_cards_forbidden(self.state, player),
            political_secret_bars: crate::laws::political_secret_bars(self.state, player),
            goods: &|| crate::supply::potential_goods(self.state, player),
        };
        // The scope of the game, as the string's `is_playable` reads it (see `has_legal_target`).
        let scope = crate::core_adapter::scope(sources);
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
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
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
        facts: rules::Facts {
            neutral: r.seat(crate::neutral_units::NEUTRAL),
            agent_offered: state.player(player).is_some_and(|p| p.commodities > 0) && crate::factions::keleres::agent_could_grant(state, player),
            destroyer: rules::Placed::NONE,
            dreadnought: rules::Placed::NONE,
            mech: rules::Placed::NONE,
            pds: rules::Placed::NONE,
            holder_deepwrought: state.player(player).is_some_and(|p| p.faction.as_str() == "deepwrought"),
            spacedock: rules::Placed::NONE,
            replenish_events: crate::leaders_core::replenish_causes_events(state),
            rules_version: state.rules_version,
            tech_order: |a, b| crate::generated::IdStr::as_str(a).cmp(crate::generated::IdStr::as_str(b)),
            roster: crate::technology_core::roster_for(ti4_content::ContentStore::embedded(), sources.clone()),
            reveal: reveal_set(state, player, sources),
            traders: [None; 2],
            fracture_in_play: state.fracture_in_play,
            own_breakthrough: own_breakthroughs(state, r),
            unit_order: |a, b| crate::generated::IdStr::as_str(a).cmp(crate::generated::IdStr::as_str(b)),
            map: false,
            explore_empty: ti4_engine_core::state::Code(0),
            action_draw_plain: false,
            carried: rules::perform::Carried::default(),
            reroll: None,
            courageous_subtype: ti4_engine_core::state::Code(0),
            courageous_cause: ti4_engine_core::state::Code(0),
            hits: ti4_engine_core::kernel::children::HitTexts::default(),
        },
    }
}

/// The owner table of a card's window: with the neutral owner when the card places its units (`Fx::needs_neutral`).
fn tables_of(gs: &GameState, card: ActionCard) -> Option<(Ctx, crate::render::Render)> {
    if rules::effects::Fx::of(card) == Some(rules::effects::Fx::Jam) {
        // The rival is named by faction: two seats of one faction would clash, which the string cannot tell apart either.
        let (cx, r) = crate::dual_window::tables(gs).why(crate::dual::reason::NO_TABLES)?;
        let factions: Vec<String> = r
            .seats
            .iter()
            .map(|id| gs.players.iter().find(|p| p.id.as_str() == id).map_or_else(String::new, |p| p.faction.as_str().to_owned()))
            .collect();
        let named: Vec<&String> = factions.iter().filter(|f| !f.is_empty()).collect();
        if named.iter().enumerate().any(|(i, f)| named[..i].contains(f)) {
            return None;
        }
        return Some((cx, r.with_factions(factions)));
    }
    if matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Perform { primary: true })) {
        // The primaries name their options by faction (the Trade primary's seats, the speaker of Politics): two seats of one faction
        // would clash, and the string cannot tell them apart either.
        return crate::strategy_core::named_tables(gs);
    }
    if rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::needs_neutral) {
        crate::dual_window::tables_with(gs, &[crate::neutral_units::NEUTRAL])
    } else {
        crate::dual_window::tables(gs)
    }
}

/// Fire Team and Scramble Frequency: the interned texts of the staged dice (`rules::RerollFacts`): the kinds of roll, in the state's
/// table, and the name of each group of the holder's dice, in the question renderer's. `None` when the table is full.
fn reroll_facts(gs: &GameState, cx: &mut Ctx, r: &crate::render::Render, player: &PlayerId) -> Option<rules::RerollFacts> {
    let mut kind = |text: &str| cx.texts.intern(Slice::Transient, text).ok();
    let ground = kind("ground")?;
    let forced = [kind("bombardment")?, kind("space_cannon")?, kind("anti_fighter_barrage")?];
    let mut shown = [ti4_engine_core::state::Code(0); rules::RerollFacts::GROUPS];
    let mut entries = 0_u8;
    if let Some(set) = gs.reroll_staging.get(player) {
        // A set of more groups than the facts hold is covered up to the last one: the window declines it.
        for (slot, entry) in shown.iter_mut().zip(&set.rolls) {
            *slot = r.code(&entry.unit);
            entries += 1;
        }
    }
    Some(rules::RerollFacts { ground, forced, shown, entries })
}

/// Fire Team's question about a die carries the whole staged roll and the place of the die in it (display only; the string builds the
/// same from the set it asks about, `combat::choose_reroll_dice`).
fn reroll_details(c: &ti4_engine_core::choice::Choice, before: &GameState, player: &PlayerId, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::combat::{Ask as CAsk, Opt as COpt, RerollSource};
    let ti4_engine_core::choice::Ask::Combat(CAsk::RerollDie { source: RerollSource::FireTeam, .. }) = &c.ask else { return };
    let Some(set) = before.reroll_staging.get(player) else { return };
    let at = c.opts.iter().find_map(|o| match o {
        ti4_engine_core::choice::Opt::Combat(COpt::RerollDie { entry, die, .. }) => Some((usize::from(*entry), usize::from(*die))),
        _ => None,
    });
    // The walk asks every die in order: the asked die's place is the number of dice before it.
    let current = at.map_or(0, |(entry, die)| set.rolls.iter().take(entry).map(|roll| roll.faces.len()).sum::<usize>() + die);
    rendered.details.insert("dice".to_owned(), serde_json::Value::Array(crate::combat::staged_dice_view(set)));
    rendered.details.insert("current".to_owned(), current.into());
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

/// Courageous To The End and Experimental Battlestation: the cards whose effect runs a hit flow of the combat area as a child window.
fn hits_card(card: ActionCard) -> bool {
    matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Courageous | rules::effects::Fx::Battlestation))
}

/// What the hit flows of Courageous To The End and Experimental Battlestation read that the core does not hold: the interned texts
/// of their questions and events, whether the map is lent (the adjacency of the dock), the staging flag. `announce` says whether
/// the play is announced (the staged destructions are given their events by the announcement, with the render codes of the events;
/// otherwise they stay in the state, with the texts of the state). `None` when the string position holds something the combat
/// windows do not carry: the string plays the card (a counted bail).
fn hit_facts(
    gs: &GameState,
    r: &crate::render::Render,
    cx: &mut Ctx,
    card: ActionCard,
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
    announce: bool,
    facts: &mut rules::Facts,
) -> Option<()> {
    use rules::effects::Fx;
    match Fx::of(card) {
        Some(Fx::Courageous) => {
            facts.courageous_subtype = r.code("courageous_to_the_end_assign_casualty");
            facts.courageous_cause = if announce { r.code("action_card:courageous") } else { cx.texts.intern(Slice::Transient, "action_card:courageous").ok()? };
        }
        Some(Fx::Battlestation) => {
            facts.map = galaxy.is_some();
            facts.hits = ti4_engine_core::kernel::children::HitTexts {
                staging: crate::supply::staging_enabled(gs),
                commander: r.code("letnevcommander"),
                casualty: r.code("assign_casualty"),
                effect_hit: r.code("effect_hit"),
            };
            // The sustain offer of the core pays the Letnev commander's offer only for an unlocked commander
            // (`dual_window::sustain_offer`): the seat that takes the hits is the active one.
            let label = if announce { "action_cards::announce" } else { "action_cards::play" };
            if let Some(target) = gs.active.as_ref() {
                let unlocked = gs
                    .player(target)
                    .is_some_and(|p| p.leaders.get(ti4_model::id::text("letnevcommander")) == Some(&ti4_model::state::LeaderStatus::Unlocked));
                if crate::leaders::pays_on_sustain(gs, ti4_content::ContentStore::embedded(), target) && !unlocked {
                    crate::dual::note_bail(label, "a sustain payment the core does not follow");
                    return None;
                }
            }
        }
        _ => {}
    }
    Some(())
}

/// The display facts of the questions of a hit flow: the hit a casualty question assigns (a Courageous To The End loss is a destruction
/// the holder produces; the dock's hits are an effect's, produced by the holder) and the sustainers of a sustain question.
fn hit_details(
    c: &ti4_engine_core::choice::Choice,
    core: &ti4_engine_core::state::CoreState,
    kernel: &ti4_engine_core::kernel::Kernel,
    card: ActionCard,
    holder: &PlayerId,
    rendered: &mut crate::choice::Choice,
) {
    use crate::decision_context::{HitCause, HitDetail, HitRestriction};
    use ti4_engine_core::choice::combat::Ask as CAsk;
    use ti4_engine_core::choice::Ask;
    match c.ask {
        Ask::Combat(CAsk::AssignHit) => {
            let courageous = card == ActionCard::CourageousToTheEnd;
            if let Some(context) = rendered.context.take() {
                rendered.context = Some(context.with_hit(HitDetail {
                    cause: if courageous { HitCause::CourageousToTheEnd } else { HitCause::Other },
                    destroy: courageous,
                    restriction: HitRestriction::Any,
                    producer: Some(holder.clone()),
                }));
            }
        }
        Ask::Combat(ask @ CAsk::CancelHit { .. }) => {
            rendered.details.insert("sustainers".to_owned(), crate::core_combat::sustainers_detail(kernel, core, &ask, c.actor));
        }
        _ => {}
    }
}

/// Whether the core has a window for the card (`rules::action_cards::effects::Fx`).
#[must_use]
pub fn plays(alias: &ActionCardId) -> bool {
    ActionCard::from_id(alias.as_str()).and_then(rules::effects::Fx::of).is_some()
}

/// The map slice, lent for the cards that read it.
const MAP: crate::core_sync::Footprint = crate::core_sync::Footprint::new(&[Slice::Map], &[]);

/// The exploration slice (the frontier tokens, the wormhole tokens and the log), for the card that explores.
const EXPLORE: crate::core_sync::Footprint = crate::core_sync::Footprint::new(&[Slice::Explore], &[Slice::Explore]);

/// Whether the card is Exploration Probe.
fn is_probe(card: ActionCard) -> bool {
    rules::effects::Fx::of(card) == Some(rules::effects::Fx::Probe)
}

/// What Exploration Probe reads that the core does not hold: whether the map is lent, the empty text of the exploration log's row (the
/// core opens the row; its words are written by [`close_row`]) and whether two action cards would be drawn plainly (a Lost Crew).
fn probe_facts(gs: &GameState, cx: &mut Ctx, player: &PlayerId, galaxy: Option<&ti4_content::galaxy::Galaxy>, facts: &mut rules::Facts) -> Option<()> {
    facts.map = galaxy.is_some();
    facts.explore_empty = cx.texts.intern(Slice::Explore, "").ok()?;
    facts.action_draw_plain = crate::strategy_cards_core::plain_draw_within_limit(gs, ti4_content::ContentStore::embedded(), player, 2);
    Some(())
}

/// The reward question of a Merchant Station carries the seat's commodity standing (display only, read where the string reads it).
pub(crate) fn reward_details(c: &ti4_engine_core::choice::Choice, before: &GameState, player: &PlayerId, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::explore::{Ask as EAsk, Step};
    if matches!(c.ask, ti4_engine_core::choice::Ask::Explore(EAsk::Reward(Step::MerchantStation))) {
        rendered.details.insert("commodities".to_owned(), crate::strategy_cards::commodity_standing(before, ti4_content::ContentStore::embedded(), player));
    }
}

/// Exploration Probe: the words of the log row the core opened (`explore_in` closes the row of the string with them). `None` when
/// the row is not the one the core's card opened.
fn close_row(gs: &mut GameState, player: &PlayerId, row: &rules::effects::ProbeRow) -> Option<()> {
    use crate::exploration::{Explored, Holdings};
    use rules::effects::Explored as How;
    let at = gs.exploration_log.len().checked_sub(1)?;
    let record = gs.exploration_log.get(at)?;
    if record.player != *player || record.card != row.card.as_str() || !record.outcome.is_empty() || record.deck != "FRONTIER" {
        return None;
    }
    let card = record.card.clone();
    let outcome = match row.outcome {
        How::Fragment(deck) => Explored::Fragment { trait_name: crate::core_sync::conv::explore_key(deck) },
        How::Resolved => Explored::Resolved { card },
        How::Unresolved => Explored::Unresolved { card },
    };
    let tokens: Vec<String> = gs
        .wormhole_tokens
        .iter()
        .filter(|(kind, system)| kind.eq_ignore_ascii_case("gamma") && Some(system.as_str()) == row.gamma.map(|g| g.as_str()))
        .map(|(kind, system)| format!("wormhole:{kind}:{system}"))
        .collect();
    let kept = gs.player(player).and_then(|seat| seat.exploration_cards.last().cloned());
    let words = crate::exploration::row_words(&Holdings::from_core(row.before), &Holdings::from_core(row.after), None, &outcome, tokens, kept.as_deref(), None);
    let record = gs.exploration_log.get_mut(at)?;
    record.outcome = words.text;
    record.result = words.result;
    record.gains = words.gains;
    record.placed = words.placed;
    Some(())
}
/// Conservator Procyon: the words of the log row the core opened at `at` for a frontier token (`explore_in` closes the row of the string with
/// them, token by token). `None` when the row is not the one the core's window opened.
pub(crate) fn close_frontier_row(gs: &mut GameState, player: &PlayerId, at: usize, row: &ti4_engine_core::rules::hero_windows::FrontierRow) -> Option<()> {
    use crate::exploration::{Explored, Holdings};
    use ti4_engine_core::rules::hero_windows::How;
    let record = gs.exploration_log.get(at)?;
    if record.player != *player || record.card != row.card.as_str() || !record.outcome.is_empty() || record.deck != "FRONTIER" || record.system.as_deref() != Some(row.system.as_str()) {
        return None;
    }
    let card = record.card.clone();
    let outcome = match row.how {
        How::Fragment(deck) => Explored::Fragment { trait_name: crate::core_sync::conv::explore_key(deck) },
        How::Resolved => Explored::Resolved { card: card.clone() },
        How::Unresolved => Explored::Unresolved { card: card.clone() },
    };
    let tokens: Vec<String> = gs
        .wormhole_tokens
        .iter()
        .filter(|(kind, system)| kind.eq_ignore_ascii_case("gamma") && Some(system.as_str()) == row.gamma.map(|g| g.as_str()))
        .map(|(kind, system)| format!("wormhole:{kind}:{system}"))
        .collect();
    let words = crate::exploration::row_words(&Holdings::from_core(row.before), &Holdings::from_core(row.after), None, &outcome, tokens, Some(card.as_str()), None);
    let record = gs.exploration_log.get_mut(at)?;
    record.outcome = words.text;
    record.result = words.result;
    record.gains = words.gains;
    record.placed = words.placed;
    Some(())
}
/// Strategize and Overrule: what the performed abilities of the strategy areas read beyond a card's play (the research reads the
/// trade and exploration slices) and write (the speaker of Politics).
pub(crate) const PERFORM: crate::core_sync::Footprint = crate::core_sync::Footprint::new(&[Slice::Trade, Slice::Explore], &[Slice::Meta]);

pub(crate) const PLAY: crate::core_sync::Footprint = crate::core_sync::Footprint::new(
    &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger, Slice::Objectives],
    &[Slice::Players, Slice::Board, Slice::Decks, Slice::Cards, Slice::Agenda, Slice::Marks, Slice::Transient, Slice::Pending, Slice::Strategy, Slice::Ledger],
);

/// The ability table of a play window: no rule of the reactions table (a play is compared on its own), only the links to the
/// areas whose windows a card runs as children (the payment, the hand limit, the research, the exploration reward, the
/// abilities of the strategy cards; `ti4_engine_core::driver::children`). A table without them makes the play decline.
pub(crate) fn play_table() -> ti4_engine_core::kernel::stage::AbilityTable {
    let mut table = ti4_engine_core::kernel::stage::AbilityTable::default();
    ti4_engine_core::driver::children::register(&mut table);
    table.facts.card_groups = crate::reactions_core::card_groups();
    // A card that destroys ships itself (Courageous To The End) stages the destructions; the announcement of the play gives them their
    // `SHIP_DESTROYED` windows after the discard (`ti4_engine_core_factions::cards`, as for the reactions table).
    ti4_engine_core_factions::cards::register_after_play(&mut table);
    table
}

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
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Option<crate::dual::CoreWindow<crate::action_cards_typed::Rolled>> {
    use ti4_engine_core::driver::Driver;
    use ti4_engine_core::kernel::Kernel;
    let card = ActionCard::from_id(alias.as_str())?;
    let (mut cx, r) = tables_of(gs, card)?;
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let mut env = env_of(gs, player, sources, &r);
    env.infantry = placed(gs, player, sources, "infantry")?;
    env.cruiser = placed(gs, player, sources, "cruiser")?;
    env.facts.destroyer = placed(gs, player, sources, "destroyer")?;
    env.facts.pds = placed(gs, player, sources, "pds")?;
    env.facts.spacedock = placed(gs, player, sources, "spacedock")?;
    if matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Reroll | rules::effects::Fx::Scramble)) {
        env.facts.reroll = Some(reroll_facts(gs, &mut cx, &r, player)?);
    }
    env.facts.traders = traders_of(gs, galaxy, player, &r);
    env.facts.mech = rules::Placed {
        // The plain mech; a Naaz seat's is a unit whose placement the string announces (a bail: no unit).
        unit: (gs.player(player)?.faction.as_str() != "naaz").then(|| <ti4_engine_core::generated::Unit as crate::generated::IdStr>::from_id("mech")).flatten(),
        on_card: crate::factions::crimson_cards::held_on_card(gs, ti4_content::ContentStore::embedded(), sources.clone(), player, "mech"),
    };
    env.facts.dreadnought = rules::Placed {
        unit: Some(ti4_engine_core::generated::Unit::DreadnoughtI),
        on_card: crate::factions::crimson_cards::held_on_card(gs, ti4_content::ContentStore::embedded(), sources.clone(), player, "dreadnought"),
    };
    let probe = is_probe(card);
    if probe {
        probe_facts(gs, &mut cx, player, galaxy, &mut env.facts)?;
    }
    hit_facts(gs, &r, &mut cx, card, galaxy, false, &mut env.facts)?;
    let lent = lend_rolls(alias, before_rolls, "action_cards::play")?;
    let before = gs.clone();
    let hitting = hits_card(card);
    let combat = if hitting { Some(crate::core_combat::window_env(gs, &cx, sources.clone()).why("the combat environment could not be built from the position (a value the codecs refuse)")?) } else { None };
    let technology = rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::technology);
    if technology && technology_unported(gs, sources, player) {
        crate::dual::note_bail("action_cards::play", "a technology gained at a table the research window does not carry");
        return None;
    }
    let performs = matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Perform { .. }));
    let plain = Kernel::plain();
    let kernel = if technology || performs {
        crate::production_view::kernel()
    } else if let Some(window) = &combat {
        &window.kernel
    } else {
        &plain
    };
    let mut staged = Vec::new();
    let mut emitted = Vec::new();
    let mut faces = Vec::new();
    let mut placed_tokens = None;
    let mut unsupported = false;
    let mut rng_after = None;
    let fp = if lent.is_some() { PLAY.union(crate::core_sync::Footprint::new(&[Slice::Rng], &[Slice::Rng])) } else { PLAY };
    let fp = if performs { fp.union(PERFORM) } else { fp };
    let wants_map = rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::needs_map);
    if wants_map && galaxy.is_none() {
        crate::dual::note_bail("action_cards::play", "a card that reads the map, played without one");
        return None;
    }
    // A card that reads the map when there is one (Exploration Probe) is lent the galaxy the string had, if any.
    let lends_map = galaxy.is_some() && rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::uses_map);
    let fp = if lends_map { fp.union(MAP) } else { fp };
    let fp = if probe { fp.union(EXPLORE) } else { fp };
    let src = Sources { galaxy: if lends_map { galaxy } else { None }, rng: lent.as_ref().map(|l| &l.before) };
    let mut driver_seeds = None;
    if lent.is_some() {
        driver_seeds = Some(std::rc::Rc::new(crate::dual_window::DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?)));
    }
    let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
    let performing = performing_of(&before, sources, player, seat, card, &r);
    let details = |c: &ti4_engine_core::choice::Choice, core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
        recorded_details(c, script, rendered);
        reward_details(c, &before, player, rendered);
        reroll_details(c, &before, player, rendered);
        crate::strategy_cards_core::complete_tokens(c, core, rendered, letnev);
        if performs {
            performing.complete(c, core, rendered);
        }
        if hitting {
            hit_details(c, core, kernel, card, player, rendered);
        }
    };
    let mut row = None;
    let driven = crate::core_sync::with_core(gs, &mut cx, src, fp, |core, _| {
        if lends_map && core.galaxy().is_none() {
            crate::dual::note_bail_here("the map could not be lent to the core");
            return None;
        }
        if technology && !crate::strategy_cards_core::research_hooks_agree(core, &before, ti4_content::ContentStore::embedded(), sources.clone(), player, seat) {
            crate::dual::note_bail_here("the research hooks of the string and the core disagree (a hook the core does not carry)");
            return None;
        }
        let mut env = env;
        if performs {
            env.facts.carried = carried_of(core, &before, sources, player, seat, &r);
        }
        let play = rules::effects::Play::of(core, card, seat, env)?;
        let mut driver = Driver::new(play, ti4_engine_core::kernel::Resolver::new(&[seat], Some(seat)), play_table(), 1);
        if let Some(seeds) = driver_seeds {
            driver = driver.with_seeds(seeds);
        }
        let driven = crate::dual_window::drive_with(&mut driver, core, &kernel, &r, &before, script, &details);
        staged = std::mem::take(&mut core.pending_mut().staged);
        emitted = std::mem::take(&mut driver.emitted);
        faces = driver.window.faces().to_vec();
        placed_tokens = driver.window.placed_tokens().map(|(system, seats)| (system, seats.to_vec()));
        unsupported = driver.window.unsupported();
        row = driver.window.explored();
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
    if let Some(row) = &row {
        close_row(gs, player, row)?;
    }
    crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    stage_placed_tokens(gs, &r, placed_tokens);
    let events: Vec<crate::event::Event> = emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let positions = match rng_after {
        Some(after) => Some(crate::core_sync::codec::rng::positions(&cx, &after).why("the random streams could not be read back")?),
        None => None,
    };
    Some(crate::dual::CoreWindow { run: crate::dual::Run::new((faces, positions)).with_events_vec(events), questions: driven.rendered })
}

/// The display facts only the card's text knows (`Choice.details` of Repeal Law's `laws`): taken from the recorded question
/// with the same prompt.
fn recorded_details(c: &ti4_engine_core::choice::Choice, script: &crate::dual_window::Recording, rendered: &mut crate::choice::Choice) {
    use ti4_engine_core::choice::cards::What;
    if matches!(c.ask, ti4_engine_core::choice::Ask::Cards(ti4_engine_core::choice::cards::Ask::Pick { what: What::Law | What::Hull | What::Scuttle { .. } | What::Cruiser | What::Troops { .. } | What::Pull { .. }, .. }))
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
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
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
    galaxy: Option<&ti4_content::galaxy::Galaxy>,
) -> Option<crate::dual::CoreWindow<(Option<bool>, crate::action_cards_typed::Rolled)>> {
    use ti4_engine_core::driver::Driver;
    let card = ActionCard::from_id(alias.as_str()).why("an action card the core tables do not know")?;
    if !rules::announce::plays(card) && crate::action_cards::effect_for(alias).is_some() {
        crate::dual::note_bail_here("a card the announce window does not play (it has no core effect, or the table plays it inside a window)");
        return None;
    }
    let (mut cx, r) = tables_of(gs, card).why(crate::dual::reason::NO_TABLES)?;
    let seat = r.seat(player.as_str()).why(crate::dual::reason::NO_SEAT)?;
    let mut env = env_of(gs, player, sources, &r);
    env.infantry = placed(gs, player, sources, "infantry").why("the unit a seat places for infantry is not a unit of the core tables")?;
    env.cruiser = placed(gs, player, sources, "cruiser").why("the unit a seat places for cruiser is not a unit of the core tables")?;
    env.facts.destroyer = placed(gs, player, sources, "destroyer").why("the unit a seat places for destroyer is not a unit of the core tables")?;
    env.facts.pds = placed(gs, player, sources, "pds").why("the unit a seat places for pds is not a unit of the core tables")?;
    env.facts.spacedock = placed(gs, player, sources, "spacedock").why("the unit a seat places for spacedock is not a unit of the core tables")?;
    if matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Reroll | rules::effects::Fx::Scramble)) {
        env.facts.reroll = Some(reroll_facts(gs, &mut cx, &r, player).why("the staged dice of the position could not be read for the reroll")?);
    }
    env.facts.traders = traders_of(gs, galaxy, player, &r);
    env.facts.mech = rules::Placed {
        // The plain mech; a Naaz seat's is a unit whose placement the string announces (a bail: no unit).
        unit: (gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() != "naaz").then(|| <ti4_engine_core::generated::Unit as crate::generated::IdStr>::from_id("mech")).flatten(),
        on_card: crate::factions::crimson_cards::held_on_card(gs, ti4_content::ContentStore::embedded(), sources.clone(), player, "mech"),
    };
    env.facts.dreadnought = rules::Placed {
        unit: Some(ti4_engine_core::generated::Unit::DreadnoughtI),
        on_card: crate::factions::crimson_cards::held_on_card(gs, ti4_content::ContentStore::embedded(), sources.clone(), player, "dreadnought"),
    };
    let probe = is_probe(card);
    if probe {
        probe_facts(gs, &mut cx, player, galaxy, &mut env.facts).why("the map facts of an Exploration Probe could not be computed")?;
    }
    hit_facts(gs, &r, &mut cx, card, galaxy, true, &mut env.facts).why("the facts of a hit card could not be computed")?;
    let hitting = hits_card(card);
    if crate::factions::ralnel_cards::skimmer_holder(gs).is_some() {
        crate::dual::note_bail("action_cards::announce", SKIMMER_BAIL);
        return None;
    }
    let has_effect = crate::action_cards::effect_for(alias).is_some();
    let lent = lend_rolls(alias, before_rolls, "action_cards::announce")?;
    let letnev = gs.player(player).why(crate::dual::reason::NO_SEAT)?.faction.as_str() == "letnev";
    let before = gs.clone();
    let window = crate::core_combat::window_env(gs, &cx, sources.clone()).why("the combat environment could not be built from the position (a value the codecs refuse)")?;
    let technology = rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::technology);
    if technology && technology_unported(gs, sources, player) {
        crate::dual::note_bail("action_cards::announce", "a technology gained at a table the research window does not carry");
        return None;
    }
    let performs = matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Perform { .. }));
    let kernel = if technology || performs { crate::production_view::kernel() } else { &window.kernel };
    let mut staged = Vec::new();
    let mut emitted = Vec::new();
    let mut played = None;
    let mut placed_tokens = None;
    let mut faces = Vec::new();
    let mut rng_after = None;
    let mut fp = crate::dual_window::nested_fp(PLAY, script);
    if lent.is_some() {
        fp = fp.union(crate::core_sync::Footprint::new(&[Slice::Rng], &[Slice::Rng]));
    }
    if performs {
        fp = fp.union(PERFORM);
    }
    let wants_map = rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::needs_map);
    if wants_map && galaxy.is_none() {
        crate::dual::note_bail("action_cards::announce", "a card that reads the map, played without one");
        return None;
    }
    // A card that reads the map when there is one (Exploration Probe) is lent the galaxy the string had, if any.
    let lends_map = galaxy.is_some() && rules::effects::Fx::of(card).is_some_and(rules::effects::Fx::uses_map);
    if lends_map {
        fp = fp.union(MAP);
    }
    if probe {
        fp = fp.union(EXPLORE);
    }
    let src = Sources { galaxy: if lends_map { galaxy } else { None }, rng: lent.as_ref().map(|l| &l.before) };
    let seeds = match lent {
        Some(_) => Some(std::rc::Rc::new(crate::dual_window::DiceSeeds(crate::core_sync::codec::rng::domain_text(&cx, ti4_engine_core::state::RngDomain::Dice).ok()?))),
        None => None,
    };
    let rolled = seeds.is_some();
    let mut row = None;
    let performing = performing_of(&before, sources, player, seat, card, &r);
    let driven = crate::core_sync::with_core(gs, &mut cx, src, fp, |core, _| {
        if lends_map && core.galaxy().is_none() {
            crate::dual::note_bail_here("the map could not be lent to the core");
            return None;
        }
        if technology && !crate::strategy_cards_core::research_hooks_agree(core, &before, ti4_content::ContentStore::embedded(), sources.clone(), player, seat) {
            crate::dual::note_bail_here("the research hooks of the string and the core disagree (a hook the core does not carry)");
            return None;
        }
        let mut env = env;
        if performs {
            env.facts.carried = carried_of(core, &before, sources, player, seat, &r);
        }
        let announce = rules::announce::Announce::of(core, card, seat, env, has_effect, false);
        if announce.is_none() {
            crate::dual::note_bail_here("a card the core announce window does not play");
        }
        let announce = announce?;
        let mut driver = Driver::new(announce, ti4_engine_core::kernel::Resolver::new(&[seat], Some(seat)), play_table(), 1);
        if let Some(seeds) = seeds {
            driver = driver.with_seeds(seeds);
        }
        let battle = |_c: &ti4_engine_core::choice::Choice, _core: &ti4_engine_core::state::CoreState, rendered: &mut crate::choice::Choice| {
            // The effect's questions are asked through `ask_seeing`: the battle or invasion the position is in.
            // A payment is asked of the table directly (`production::pay`), which adds neither (found by the golden dual: Reveal
            // Prototype, played at the start of a combat, paid with the battle's system as the target of the core question only).
            if !matches!(_c.ask, ti4_engine_core::choice::Ask::Production(_))
                && let Some(ctx) = rendered.context.take()
            {
                rendered.context = Some(ctx.about_battle(&before).about_invasion(&before));
            }
            recorded_details(_c, script, rendered);
            reward_details(_c, &before, player, rendered);
            reroll_details(_c, &before, player, rendered);
            crate::strategy_cards_core::complete_tokens(_c, _core, rendered, letnev);
            if performs {
                performing.complete(_c, _core, rendered);
            }
            if hitting {
                hit_details(_c, _core, kernel, card, player, rendered);
            }
        };
        let driven = crate::dual_window::drive_with(&mut driver, core, kernel, &r, &before, script, &battle);
        played = Some(driver.window.played());
        row = driver.window.explored();
        faces = driver.window.faces();
        placed_tokens = driver.window.placed_tokens();
        if rolled {
            rng_after = Some(core.rng().clone());
        }
        staged = std::mem::take(&mut core.pending_mut().staged);
        emitted = std::mem::take(&mut driver.emitted);
        Some(driven)
    })
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::NOT_LENDABLE)?
    .why(crate::dual::reason::DRIVER_REFUSED)?;
    if let Some(row) = &row {
        close_row(gs, player, row).why("the exploration row of the card could not be closed")?;
    }
    crate::dual_window::apply_staged(gs, &r, &staged).why(crate::dual::reason::STAGED_UNAPPLIED)?;
    stage_placed_tokens(gs, &r, placed_tokens);
    let events: Vec<crate::event::Event> = emitted.iter().map(|e| crate::dual_window::event_of(&r, e)).collect::<Option<_>>().why(crate::dual::reason::EVENT_UNTYPED)?;
    let positions = match rng_after {
        Some(after) => Some(crate::core_sync::codec::rng::positions(&cx, &after).why("the random streams could not be read back")?),
        None => None,
    };
    Some(crate::dual::CoreWindow { run: crate::dual::Run::new((played, (faces, positions))).with_events_vec(events), questions: driven.rendered })
}

/// The announcements of the command tokens the Diplomacy primary placed (Overrule), staged on the string state as
/// `strategy_cards::diplomacy` stages them (they are not a codec slice).
pub(crate) fn stage_placed_tokens(gs: &mut GameState, r: &crate::render::Render, placed: Option<(System, Vec<ti4_engine_core::Seat>)>) {
    let Some((system, seats)) = placed else { return };
    for seat in seats {
        let system = ti4_model::id::SystemId::new(crate::generated::IdStr::as_str(system));
        crate::tokens::stage_command_token_placed(gs, &r.player(seat), &system, crate::factions::hooks_cards::TokenPool::Reinforcements);
    }
}

/// The completer of the display facts of the questions a performed ability asks (Strategize and Overrule).
fn performing_of<'a>(
    before: &'a GameState,
    sources: &ti4_model::content_types::SourceSet,
    player: &'a PlayerId,
    seat: ti4_engine_core::Seat,
    card: ActionCard,
    r: &'a crate::render::Render,
) -> crate::strategy_cards_core::Performing<'a> {
    crate::strategy_cards_core::Performing {
        before,
        content: ti4_content::ContentStore::embedded(),
        sources: sources.clone(),
        player,
        seat,
        primary: matches!(rules::effects::Fx::of(card), Some(rules::effects::Fx::Perform { primary: true })),
        r,
        free: std::cell::Cell::new(0),
    }
}

/// What the adapter vouches for when Strategize or Overrule performs a strategy card's ability in the core: the guards of the
/// string windows that read the string position (`strategy_cards_core`).
pub(crate) fn carried_of(
    core: &ti4_engine_core::state::CoreState,
    gs: &GameState,
    sources: &ti4_model::content_types::SourceSet,
    player: &PlayerId,
    seat: ti4_engine_core::Seat,
    r: &crate::render::Render,
) -> rules::perform::Carried {
    let content = ti4_content::ContentStore::embedded();
    rules::perform::Carried {
        plain_draw: crate::strategy_cards_core::plain_draw(gs, content, player, 2),
        research: !technology_unported(gs, sources, player)
            && crate::strategy_cards_core::research_hooks_agree(core, gs, content, sources.clone(), player, seat),
        payments: !crate::production_view::unported_window(gs, player),
        structures: crate::strategy_cards_core::structure_facts(gs, content, sources.clone(), player, r),
        trade_source: r.code("trade_primary"),
    }
}

/// The breakthrough of each seat's faction, by seat (`breakthroughs::for_faction`).
fn own_breakthroughs(state: &GameState, r: &crate::render::Render) -> [Option<ti4_engine_core::generated::Breakthrough>; 16] {
    let content = ti4_content::ContentStore::embedded();
    let mut out = [None; 16];
    for p in &state.players {
        let Some(seat) = r.seat(p.id.as_str()) else { continue };
        let own = crate::breakthroughs::for_faction(content, p.faction.as_str());
        out[usize::from(seat.0)] = own.and_then(|b| <ti4_engine_core::generated::Breakthrough as crate::generated::IdStr>::from_id(b.as_str()));
    }
    out
}

/// Reveal Prototype's candidates: the unit upgrades "of the same type as 1 of your units that is participating in this combat"
/// (the string's line matching of `baseUpgrade` and the unnamed second-generation line by name).
pub(crate) fn reveal_set(state: &GameState, player: &PlayerId, sources: &ti4_model::content_types::SourceSet) -> ti4_engine_core::state::TechSet {
    use ti4_model::content_types::ContentType;
    let content = ti4_content::ContentStore::embedded();
    let mut out = ti4_engine_core::state::TechSet::default();
    let Some(system) = state.active_system.clone() else { return out };
    let types = ti4_content::units::catalogue(content, sources.clone());
    let units = state.system_state(&system).units.clone();
    let lines: Vec<&str> = units
        .iter()
        .filter(|u| &u.owner == player)
        .filter_map(|u| types.get(u.type_id.as_str()).map(ti4_content::units::UnitType::base_type))
        .collect();
    for tech in ti4_engine_core::generated::Technology::ALL {
        let alias = ti4_model::id::TechnologyId::new(crate::generated::IdStr::as_str(tech));
        if !crate::technology::is_unit_upgrade(content, &alias) {
            continue;
        }
        let Some(record) = content.get(ContentType::Technologies, alias.as_str()) else { continue };
        let base = record.text("baseUpgrade").unwrap_or_default().to_owned();
        let matches = if base.is_empty() {
            let name = record.text("name").unwrap_or_default().to_lowercase().replace(" ii", "").replace(' ', "_");
            lines.iter().any(|line| name == *line || name.ends_with(&format!("_{line}")))
        } else {
            let subject_line = types.get(base.as_str()).map(ti4_content::units::UnitType::base_type);
            lines.iter().any(|line| Some(*line) == subject_line) || subject_line.is_none() && lines.iter().any(|line| **line == base)
        };
        if matches {
            out.insert(tech);
        }
    }
    out
}

/// Whether a card that gains or researches a technology meets something the core windows do not carry: what the Technology
/// strategy card declines (`research_unported`) and a Deepwrought, Keleres or Mentak seat anywhere at the table (Doctor
/// Carrina's window opens around a research of any seat; Keleres and Deepwrought are dropped from the product, Mentak's
/// Corsair form is the string's).
pub(crate) fn technology_unported(gs: &GameState, sources: &ti4_model::content_types::SourceSet, player: &PlayerId) -> bool {
    let content = ti4_content::ContentStore::embedded();
    gs.players.iter().any(|p| p.faction.as_str() == "deepwrought" || p.faction.as_str().starts_with("keleres") || crate::factions::mentak::corsair_form(gs, &p.id))
        || crate::strategy_cards_core::research_unported(gs, content, sources.clone(), player)
}

/// Lie in Wait: the first two neighbours (by the map) who resolved a transaction this round; none without a map.
fn traders_of(gs: &GameState, galaxy: Option<&ti4_content::galaxy::Galaxy>, player: &PlayerId, r: &crate::render::Render) -> [Option<ti4_engine_core::Seat>; 2] {
    let mut out = [None; 2];
    let Some(galaxy) = galaxy else { return out };
    if gs.transactions_this_round.is_empty() {
        return out;
    }
    for (slot, who) in out.iter_mut().zip(crate::transactions::neighbours_who_transacted(gs, galaxy, player)) {
        *slot = r.seat(who.as_str());
    }
    out
}

#[cfg(test)]
mod hit_facts_tests {
    use super::*;
    use crate::action_cards::tests::closing_position;

    /// The guards of Experimental Battlestation's hit flow (`hit_facts`): against a seat that pays on a sustain without an unlocked
    /// commander the string plays the card; the contrast cases (a game below `RULES_FACTION_EFFECTS` too, since W12) are carried.
    #[test]
    fn the_sustain_guards_of_experimental_battlestation_decline_only_what_the_core_window_does_not_follow() {
        let _serial = crate::dual::tests::serial();
        let card = ActionCard::ExperimentalBattlestation;
        let b = PlayerId::new("b");
        let mut plain = closing_position();
        plain.active = Some(b.clone());
        let run = |state: &GameState| {
            let (mut cx, r) = crate::dual_window::tables(state).expect("tables");
            let mut facts = rules::Facts::default();
            let out = hit_facts(state, &r, &mut cx, card, None, true, &mut facts);
            (out.is_some(), facts)
        };
        let (carried, facts) = run(&plain);
        assert!(carried);
        assert!(!facts.map, "no map was lent");
        assert_eq!(facts.hits.staging, crate::supply::staging_enabled(&plain));
        let mut old = plain.clone();
        old.rules_version = ti4_model::state::RULES_FACTION_EFFECTS - 1;
        assert!(run(&old).0, "below the faction effects the core window pays unasked and is followed");
        // Held another way than as an unlocked leader: the string pays, the core window would not.
        let mut held = plain.clone();
        held.faction_marks.insert("commander_ability:b:letnevcommander".to_owned(), "1".to_owned());
        assert!(!run(&held).0, "a commander held another way");
        // Unlocked: the core's commander window pays the same.
        let mut unlocked = held.clone();
        unlocked.player_mut(&b).unwrap().leaders.insert(ti4_model::id::LeaderId::new("letnevcommander"), ti4_model::state::LeaderStatus::Unlocked);
        assert!(run(&unlocked).0, "an unlocked commander is followed");
        // The card that destroys ships has no such guard; its texts are the play's.
        let (mut cx, r) = crate::dual_window::tables(&held).expect("tables");
        let mut facts = rules::Facts::default();
        assert!(hit_facts(&held, &r, &mut cx, ActionCard::CourageousToTheEnd, None, true, &mut facts).is_some());
        assert_eq!(r.code_text(facts.courageous_subtype), "courageous_to_the_end_assign_casualty");
        assert_eq!(r.code_text(facts.courageous_cause), "action_card:courageous");
        let mut state_texts = rules::Facts::default();
        assert!(hit_facts(&held, &r, &mut cx, ActionCard::CourageousToTheEnd, None, false, &mut state_texts).is_some());
        assert_eq!(cx.texts.text(Slice::Transient, state_texts.courageous_cause).unwrap(), "action_card:courageous", "a destruction left in the state carries the text of the state");
    }
}
