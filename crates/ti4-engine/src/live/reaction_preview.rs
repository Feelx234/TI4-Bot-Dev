//! What playing a reaction card would do right now, on the native path (lane H4): `reaction_preview::preview_for` over the core position and the
//! event of the window (`NestedFacts`) instead of the string `GameState`. Display only, in `context.previews`, attached to the window's single-card
//! slot option and to the options of the "play an action card" question, exactly where the string's `timing` and `reactions` put them.
//!
//! Every arm is the string arm read from the typed position: the transients the string keeps (`last_sustain`, `last_combat_sides`, ...) are the
//! core's `transient().combat`, the event fields are the typed payload keys. The text orders the string gets from its maps (`BTreeMap` by id text)
//! are the integer orders of the core (seats and generated ids are in text order), so a tally is sorted by `(seat, unit)`.

use std::collections::BTreeMap;

use ti4_content::ContentStore;
use ti4_engine_core::choice::cards::{Ask as CAsk, Opt as COpt};
use ti4_engine_core::choice::reaction::Ask as RAsk;
use ti4_engine_core::choice::{Ask, Choice as CoreChoice, Opt};
use ti4_engine_core::generated::{ActionCard, Planet, System, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::ability::PlaysProvider;
use ti4_engine_core::kernel::read::Rd;
use ti4_engine_core::kernel::{Kernel, NestedFacts};
use ti4_engine_core::query::{board, reach};
use ti4_engine_core::state::{CoreState, EventRec, Key, Piece, Val};
use ti4_model::id::{ActionCardId, PlanetId, PlayerId, SystemId};

use crate::choice::Choice;
use crate::decision_context::{EffectPreview, TriggerUnits};
use crate::generated::IdStr;
use crate::render::Render;

/// Attach the previews of the cards the question offers to its context: the cards of the window's single-card slot (an option with a `card`
/// payload) and the cards of the "play an action card" question.
pub fn complete(r: &Render, core: &CoreState, k: &Kernel, plays: Option<&PlaysProvider>, facts: &NestedFacts, choice: &CoreChoice, rendered: &mut Choice) {
    let cards: Vec<(ActionCard, String)> = match &choice.ask {
        Ask::Reaction(RAsk::Window { .. }) => rendered
            .options
            .iter()
            .filter_map(|o| Some((ActionCard::from_id(o.payload.get("card")?.as_str()?)?, o.id.clone())))
            .collect(),
        Ask::Cards(CAsk::PlayCard { .. }) => choice
            .opts
            .iter()
            .zip(&rendered.options)
            .filter_map(|(o, rendered)| match o {
                Opt::Cards(COpt::Card(card)) => Some((*card, rendered.id.clone())),
                _ => None,
            })
            .collect(),
        _ => return,
    };
    let cx = Pcx { r, core, k, plays, event: &facts.event, who: choice.actor };
    let previews: Vec<EffectPreview> = cards.into_iter().filter_map(|(card, option)| cx.preview(card, &option)).collect();
    if let Some(context) = rendered.context.as_mut() {
        context.previews.extend(previews);
    }
}

/// The position and the event a preview reads.
struct Pcx<'a> {
    r: &'a Render,
    core: &'a CoreState,
    k: &'a Kernel,
    plays: Option<&'a PlaysProvider>,
    event: &'a EventRec,
    who: Seat,
}

fn blank(option: &str, card: ActionCard, effect: &str) -> EffectPreview {
    EffectPreview {
        option: option.to_owned(),
        card: IdStr::as_str(card).to_owned(),
        effect: effect.to_owned(),
        seats: Vec::new(),
        system: None,
        planets: Vec::new(),
        units: Vec::new(),
        amount: None,
        of: None,
        refs: Vec::new(),
        fizzles: None,
    }
}

impl Pcx<'_> {
    fn player(&self, seat: Seat) -> PlayerId {
        PlayerId::new(self.r.seat_text(seat))
    }

    fn system_id(system: System) -> SystemId {
        SystemId::new(IdStr::as_str(system))
    }

    /// The seat a payload field names (`Event::text` of a seat field).
    fn seat_field(&self, key: Key) -> Option<Seat> {
        self.event.payload.seat(key)
    }

    /// The text of a payload field naming a card (an action card, a strategy card or a text outside the tables).
    fn card_field(&self) -> Option<String> {
        match self.event.payload.get(Key::Card)? {
            Val::Action(c) => Some(IdStr::as_str(c).to_owned()),
            Val::Strategy(c) => Some(IdStr::as_str(c).to_owned()),
            Val::Code(c) => Some(self.r.code_text(c)),
            _ => None,
        }
    }

    fn agenda_refs(&self) -> Vec<String> {
        self.event.payload.agenda(Key::Agenda).map(|a| IdStr::as_str(a).to_owned()).into_iter().collect()
    }

    /// The system the window is about: the event's own, else the active one (`reaction_preview::system_of`).
    fn system_of_window(&self) -> Option<System> {
        self.event.payload.system(Key::System).or(self.core.meta().active_system)
    }

    /// Count `pieces` by owner and type, in the string's order (`BTreeMap` by owner and type text).
    fn tally<'p>(&self, pieces: impl IntoIterator<Item = &'p Piece>) -> Vec<TriggerUnits> {
        let mut counts: BTreeMap<(Seat, Unit), u32> = BTreeMap::new();
        for piece in pieces {
            *counts.entry((piece.owner(), piece.kind())).or_insert(0) += 1;
        }
        counts.into_iter().map(|((owner, unit), count)| TriggerUnits { owner: self.player(owner), unit_type: IdStr::as_str(unit).to_owned(), count }).collect()
    }

    fn seats_of(&self, units: &[TriggerUnits]) -> Vec<PlayerId> {
        let mut seats: Vec<PlayerId> = units.iter().map(|entry| entry.owner.clone()).collect();
        seats.sort();
        seats.dedup();
        seats
    }

    /// Every unit in a system: the space area and the planets (planet text order).
    fn all_units(&self, system: System) -> Vec<Piece> {
        let mut planets: Vec<Planet> = board::planets_of(self.core, system).collect();
        planets.sort_unstable();
        let mut units: Vec<Piece> = board::space_units(self.core, system).to_vec();
        for planet in planets {
            units.extend(board::planet_units(self.core, planet).iter().copied());
        }
        units
    }

    fn space_of(&self, system: System, mut keep: impl FnMut(&Piece) -> bool) -> Vec<Piece> {
        board::space_units(self.core, system).iter().copied().filter(|p| keep(p)).collect()
    }

    fn preview(&self, card: ActionCard, option: &str) -> Option<EffectPreview> {
        let content = ContentStore::embedded();
        let name = crate::action_cards::name_of(content, &ActionCardId::new(IdStr::as_str(card)));
        let preview = |effect: &str| blank(option, card, effect);
        let (core, who) = (self.core, self.who);
        let combat = &core.transient().combat;
        let active_system = core.meta().active_system;
        let result = match name.as_str() {
            "Sabotage" => {
                let mut p = preview("cancel_card");
                p.seats = self.seat_field(Key::Player).map(|s| self.player(s)).into_iter().collect();
                p.refs = self.card_field().into_iter().collect();
                p
            }
            "Direct Hit" => {
                let mut p = preview("destroy_unit");
                let (system, victim, unit, _, _) = combat.last_sustain?;
                p.seats = vec![self.player(victim)];
                p.units = vec![TriggerUnits { owner: self.player(victim), unit_type: IdStr::as_str(unit).to_owned(), count: 1 }];
                let rd = Rd::new(core, self.k).with_blanked(reach::blanked(core));
                if !ti4_engine_core::rules::combat::direct_hittable_at(rd, victim, system, unit) {
                    p.fizzles = Some("immune".to_owned());
                }
                p.system = Some(Self::system_id(system));
                p
            }
            "Shields Holding" | "Maneuvering Jets" => {
                let mut p = preview("cancel_hits");
                p.amount = Some(if name == "Shields Holding" { 2 } else { 1 });
                p.of = self.event.payload.int(Key::Hits);
                p.system = self.system_of_window().map(Self::system_id);
                if name == "Maneuvering Jets" {
                    p.seats = self.seat_field(Key::Gunner).map(|s| self.player(s)).into_iter().collect();
                }
                p
            }
            "Reflective Shielding" => {
                let mut p = preview("produce_hits");
                let (system, _, _, producer, _) = combat.last_sustain?;
                p.seats = vec![self.player(producer)];
                p.system = Some(Self::system_id(system));
                p.amount = Some(2);
                p
            }
            "Salvage" => {
                let mut p = preview("take_commodities");
                let (system, sides) = combat.last_combat_sides.as_ref()?;
                let opponents: Vec<Seat> = sides.as_slice().iter().copied().filter(|s| *s != who).collect();
                let total: i64 = opponents.iter().filter_map(|s| core.players().by_seat(*s)).map(|p| i64::from(p.econ.commodities)).sum();
                p.seats = opponents.iter().map(|s| self.player(*s)).collect();
                p.amount = Some(total);
                p.system = Some(Self::system_id(*system));
                if total == 0 {
                    p.fizzles = Some("nothing_there".to_owned());
                }
                p
            }
            "Parley" => {
                let mut p = preview("return_committed_unit");
                let (owner, system, planet, piece) = combat.last_committed_unit?;
                p.seats = vec![self.player(owner)];
                p.units = self.tally([&piece]);
                p.system = Some(Self::system_id(system));
                p.planets = vec![PlanetId::new(IdStr::as_str(planet))];
                p
            }
            "Infiltrate" => self.infiltrate(preview("replace_structures"))?,
            "Deadly Plot" => {
                let mut p = preview("discard_agenda");
                let planets = board::controlled_planets(core, who).as_slice().iter().filter(|(_, planet)| !board::exhausted(core, *planet)).count();
                p.amount = i64::try_from(planets).ok();
                p.refs = self.agenda_refs();
                p
            }
            "Counterstroke" => {
                let mut p = preview("return_command_token");
                p.system = active_system.map(Self::system_id);
                p.amount = Some(1);
                p
            }
            "Rout" | "Intercept" => {
                let mut p = preview(if name == "Rout" { "force_retreat" } else { "bar_retreat" });
                if let Some((system, attacker, defender)) = combat.active_space_combat {
                    p.seats = [attacker, defender].into_iter().filter(|s| *s != who).map(|s| self.player(s)).collect();
                    p.system = Some(Self::system_id(system));
                }
                p
            }
            "Waylay" => {
                let mut p = preview("barrage_hits_all_ships");
                p.system = self.system_of_window().map(Self::system_id);
                p
            }
            "Emergency Repairs" => {
                let mut p = preview("repair_units");
                let system = active_system?;
                let hurt: Vec<Piece> = self.all_units(system).into_iter().filter(|u| u.owner() == who && u.sustained()).collect();
                p.units = self.tally(&hurt);
                if hurt.is_empty() {
                    p.fizzles = Some("nothing_there".to_owned());
                }
                p.system = Some(Self::system_id(system));
                p
            }
            "Disable" => {
                let mut p = preview("disable_pds");
                let system = active_system?;
                let guns: Vec<Piece> = self.all_units(system).into_iter().filter(|u| u.owner() != who && u.kind().data().base_type == UnitBaseType::Pds).collect();
                p.units = self.tally(&guns);
                p.seats = self.seats_of(&p.units);
                if guns.is_empty() {
                    p.fizzles = Some("nothing_there".to_owned());
                }
                p.system = Some(Self::system_id(system));
                p
            }
            "Bunker" => {
                let mut p = preview("bombardment_penalty");
                let system = active_system?;
                let mut planets: Vec<Planet> = board::planets_of(core, system).filter(|planet| board::controls(core, who, *planet)).collect();
                planets.sort_unstable();
                p.planets = planets.into_iter().map(|planet| PlanetId::new(IdStr::as_str(planet))).collect();
                p.amount = Some(-4);
                p.system = Some(Self::system_id(system));
                p
            }
            "Blitz" => {
                let mut p = preview("grant_bombardment");
                let system = active_system?;
                let ships = self.space_of(system, |u| {
                    let data = u.kind().data();
                    u.owner() == who && data.ship && data.base_type != UnitBaseType::Fighter && data.bombardment.is_none()
                });
                p.units = self.tally(&ships);
                if ships.is_empty() {
                    p.fizzles = Some("nothing_there".to_owned());
                }
                p.amount = Some(6);
                p.system = Some(Self::system_id(system));
                p
            }
            "Fighter Prototype" => {
                let mut p = preview("fighter_bonus");
                let system = active_system?;
                let fighters = self.space_of(system, |u| u.owner() == who && u.kind().data().base_type == UnitBaseType::Fighter);
                p.units = self.tally(&fighters);
                if fighters.is_empty() {
                    p.fizzles = Some("nothing_there".to_owned());
                }
                p.amount = Some(2);
                p.system = Some(Self::system_id(system));
                p
            }
            "Rally" => {
                let mut p = preview("gain_fleet_tokens");
                p.amount = Some(2);
                if let Some(system) = active_system {
                    let others = self.tally(&self.space_of(system, |u| u.owner() != who));
                    p.seats = self.seats_of(&others);
                    p.system = Some(Self::system_id(system));
                }
                p
            }
            "Harness Energy" => {
                let mut p = preview("replenish_commodities");
                let player = core.players().by_seat(who)?;
                let limit = ti4_engine_core::rules::strategy::commodity_limit(player.faction);
                let held = i32::try_from(player.econ.commodities).unwrap_or(i32::MAX);
                p.amount = Some(i64::from((limit - held).max(0)));
                p.of = Some(i64::from(limit));
                p.system = active_system.map(Self::system_id);
                p
            }
            "Hack Election" => {
                let mut p = preview("vote_last");
                p.refs = self.agenda_refs();
                p
            }
            "Distinguished Councilor" => {
                let mut p = preview("extra_votes");
                p.amount = Some(5);
                p
            }
            "Political Stability" => {
                let mut p = preview("keep_strategy_cards");
                p.refs = core.players().by_seat(who).map(|player| player.strat.as_slice().iter().map(|c| IdStr::as_str(*c).to_owned()).collect()).unwrap_or_default();
                p
            }
            "Coup d'Etat" => {
                let mut p = preview("cancel_strategic_action");
                p.seats = self.seat_field(Key::Player).map(|s| self.player(s)).into_iter().collect();
                p.refs = self.card_field().into_iter().collect();
                p
            }
            "Confounding Legal Text" => {
                let mut p = preview("become_elected");
                p.seats = self.seat_field(Key::ElectedPlayer).or_else(|| self.seat_field(Key::Player)).map(|s| self.player(s)).into_iter().collect();
                p.refs = self.agenda_refs();
                p
            }
            "Reverse Engineer" => {
                let mut p = preview("take_discarded_card");
                let (by, discarded) = combat.last_action_discarded?;
                p.seats = vec![self.player(by)];
                p.refs = vec![IdStr::as_str(discarded).to_owned()];
                if !ti4_engine_core::rules::action_cards::is_component_action(discarded) {
                    p.fizzles = Some("no_component_action".to_owned());
                }
                p
            }
            "Scramble Frequency" => {
                let mut p = preview("reroll_all_dice");
                p.seats = combat.last_reroll_player.map(|s| self.player(s)).into_iter().collect();
                p.system = self.system_of_window().map(Self::system_id);
                p
            }
            "Veto" => {
                let mut p = preview("replace_agenda");
                p.refs = self.agenda_refs();
                p
            }
            "Morale Boost" => {
                let mut p = preview("combat_roll_bonus");
                p.amount = Some(1);
                p.system = self.system_of_window().map(Self::system_id);
                p
            }
            "Flank Speed" => {
                let mut p = preview("move_bonus");
                p.amount = Some(1);
                p.system = self.system_of_window().map(Self::system_id);
                p
            }
            "Lost Star Chart" => preview("wormholes_adjacent"),
            "Nav Suite" => preview("ignore_anomalies"),
            "Solar Flare" => {
                let mut p = preview("no_space_cannon");
                p.system = self.system_of_window().map(Self::system_id);
                p
            }
            "War Machine" => {
                let mut p = preview("production_boost");
                p.amount = Some(4);
                p
            }
            "Master Plan" | "Puppets on a String" => preview(if name == "Master Plan" { "extra_action" } else { "puppet_action" }),
            "Extreme Duress" => {
                let mut p = preview("duress");
                p.seats = core.meta().active.map(|s| self.player(s)).into_iter().collect();
                p
            }
            "Crisis" => preview("skip_next_turn"),
            "Black Market Dealings" => {
                let mut p = preview("black_market");
                p.seats = [self.seat_field(Key::Player), self.seat_field(Key::Partner)].into_iter().flatten().filter(|s| *s != who).map(|s| self.player(s)).collect();
                p
            }
            _ => return None,
        };
        Some(result)
    }

    /// Infiltrate: the rival PDS and space docks the card would swap out, as the effect reads them (`infiltrate_swaps` with the forms the seat would
    /// place); below `RULES_INFILTRATE_FIX` the card replaced the player's own structures.
    fn infiltrate(&self, mut p: EffectPreview) -> Option<EffectPreview> {
        let (core, k, who) = (self.core, self.k, self.who);
        let (system, planet, _, _) = core.transient().combat.last_control_gained?;
        let standing: Vec<TriggerUnits> = if core.meta().rules_version >= ti4_model::state::RULES_INFILTRATE_FIX {
            let scope = reach::scope_of(k);
            let forbidden = |unit| k.hooks.economy.effect_placement_forbidden(core, k, who, unit);
            // The forms the seat would place (the swaps' replacement), as the play facts hand them to the card.
            let own = self
                .plays
                .and_then(|plays| (plays.0)(core, k))
                .and_then(|facts| facts.seats.iter().find(|s| s.who == who).map(|s| (s.more.pds.0, s.more.spacedock.0)))
                .unwrap_or((None, None));
            let swaps = ti4_engine_core::rules::action_cards::infiltrate_swaps(core, scope, who, planet, own, &forbidden);
            let pieces: Vec<Piece> = swaps.as_slice().iter().map(|(piece, _)| *piece).collect();
            self.tally(&pieces)
        } else {
            let own: Vec<Piece> = board::planet_units(core, planet)
                .iter()
                .copied()
                .filter(|u| u.owner() == who && matches!(u.kind().data().base_type, UnitBaseType::Pds | UnitBaseType::Spacedock))
                .collect();
            self.tally(&own)
        };
        p.fizzles = standing.is_empty().then(|| "nothing_there".to_owned());
        p.units = standing;
        p.system = Some(Self::system_id(system));
        p.planets = vec![PlanetId::new(IdStr::as_str(planet))];
        Some(p)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use ti4_engine_core::state::Slice;
    use ti4_model::content_types::POK;
    use ti4_model::id::UnitTypeId;
    use ti4_model::units::Unit;
    use ti4_model::state::GameState;

    use super::*;
    use crate::core_sync::{with_core, Footprint, Sources};
    use crate::event::Event;

    const READS: Footprint = Footprint::new(
        &[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Transient, Slice::Marks, Slice::Agenda, Slice::Cards, Slice::Pending, Slice::Strategy, Slice::Trade, Slice::Explore, Slice::Decks, Slice::Objectives],
        &[],
    );

    fn pid(t: &str) -> PlayerId {
        PlayerId::new(t)
    }

    /// A table in the middle of a space combat and an invasion of a planet: units of all three seats on both areas, damage on some, and every
    /// "last ..." fact of the window set.
    fn world() -> (GameState, SystemId, PlanetId) {
        let mut state = crate::fixtures::seated_dealt(&[("a", "sol"), ("b", "hacan"), ("c", "arborec")], POK);
        state.promissory_notes.clear();
        state.promissory_faceup.clear();
        state.phase = ti4_model::state::Phase::Action;
        state.active = Some(pid("a"));
        let (system, planet) = crate::fixtures::a_placed_planet();
        state.active_system = Some(system.clone());
        for (who, kind, n) in [("a", "dreadnought", 2), ("a", "fighter", 3), ("a", "carrier", 1), ("b", "cruiser", 2), ("b", "fighter", 1), ("c", "destroyer", 1)] {
            crate::fixtures::put(&mut state, &system, kind, &pid(who), n);
        }
        for (who, kind, n) in [("a", "infantry", 2), ("b", "pds", 2), ("b", "infantry", 1), ("c", "spacedock", 1)] {
            crate::fixtures::put_on_planet(&mut state, &system, &planet, kind, &pid(who), n);
        }
        state.system_mut(&system).set_control(planet.clone(), pid("a"));
        if let Some(first) = state.system_mut(&system).units.iter_mut().find(|u| u.owner == pid("a") && u.type_id.as_str() == "dreadnought") {
            first.sustained_damage = true;
        }
        state.player_mut(&pid("a")).unwrap().commodities = 1;
        state.player_mut(&pid("b")).unwrap().commodities = 2;
        state.player_mut(&pid("c")).unwrap().commodities = 1;
        state.last_sustain = Some((system.clone(), pid("b"), UnitTypeId::new("dreadnought"), pid("a"), true));
        state.last_combat_sides = Some((system.clone(), vec![pid("a"), pid("b"), pid("c")]));
        state.last_committed_unit = Some((pid("b"), system.clone(), planet.clone(), Unit::new(UnitTypeId::new("infantry"), pid("b"))));
        state.last_control_gained = Some((system.clone(), planet.clone(), pid("a"), Some(pid("b"))));
        state.last_action_discarded = Some((pid("b"), ti4_model::id::ActionCardId::new("reparations")));
        state.last_reroll_player = Some(pid("b"));
        state.active_space_combat = Some((system.clone(), pid("a"), pid("b")));
        (state, system, planet)
    }

    /// The event each card's window opens, with the fields its preview reads (kind, payload).
    fn event_of(name: &str, system: &SystemId, planet: &PlanetId) -> (&'static str, serde_json::Value) {
        match name {
            "Sabotage" => ("ACTION_CARD_PLAYED", json!({"player": "b", "card": "reparations"})),
            "Direct Hit" | "Reflective Shielding" => ("SUSTAIN_DAMAGE_USED", json!({})),
            "Shields Holding" | "Maneuvering Jets" => ("HITS_TO_ASSIGN", json!({"system": system.as_str(), "hits": 3, "gunner": "b", "player": "a"})),
            "Coup d'Etat" => ("STRATEGIC_ACTION_BEGAN", json!({"player": "b", "card": "pok5trade"})),
            "Hack Election" | "Deadly Plot" | "Veto" => ("AGENDA_REVEALED", json!({"agenda": "mutiny"})),
            "Confounding Legal Text" => ("AGENDA_RESOLVED", json!({"agenda": "mutiny", "elected_player": "c", "player": "b"})),
            "Black Market Dealings" => ("TRANSACTION", json!({"player": "b", "partner": "a"})),
            "Scramble Frequency" | "Morale Boost" | "Flank Speed" | "Waylay" | "Solar Flare" => ("COMBAT_ROUND_STARTED", json!({"system": system.as_str()})),
            "Infiltrate" => ("PLANET_CONTROL_GAINED", json!({"system": system.as_str(), "planet": planet.as_str(), "player": "a"})),
            _ => ("SYSTEM_ACTIVATED", json!({"system": system.as_str(), "player": "b"})),
        }
    }

    /// The previews of the string (`reaction_preview::preview_for` over the `GameState`) and of the native path (the same card over the core
    /// position and the typed event) agree for every card that has one, in a position that has something for each of them to say, and again in
    /// the same position with nothing standing (the "nothing there" fizzles) for the cards that read the board.
    #[test]
    fn every_previewed_card_reads_the_core_position_as_the_string_reads_its_own() {
        let content = ContentStore::embedded();
        let (full, system, planet) = world();
        let mut empty = full.clone();
        for board in empty.board.values_mut() {
            board.units.clear();
            board.planet_units.clear();
        }
        empty.last_sustain = None;
        let names: Vec<&str> = crate::reaction_preview::PREVIEWED.to_vec();
        let mut compared = 0;
        let mut some = 0;
        for (label, state) in [("full", &full), ("empty", &empty)] {
            let (mut cx, r) = crate::dual_window::tables_for_tests(state).expect("a table the codecs carry");
            let mut gs = state.clone();
            for name in &names {
                let alias = content
                    .records(ti4_model::content_types::ContentType::ActionCards)
                    .iter()
                    .filter(|record| record.text("name") == Some(*name))
                    .filter_map(|record| record.text("alias"))
                    .map(ActionCardId::new)
                    .find(|alias| ActionCard::from_id(alias.as_str()).is_some())
                    .unwrap_or_else(|| panic!("a card named {name}"));
                let card = ActionCard::from_id(alias.as_str()).unwrap_or_else(|| panic!("{name} ({alias}) is a card of the tables"));
                let (kind, payload) = event_of(name, &system, &planet);
                let payload: std::collections::BTreeMap<String, serde_json::Value> = payload.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                let event = Event::new(7, kind, payload);
                let option = format!("reaction:{name}");
                let string = crate::reaction_preview::preview_for(state, content, POK, &pid("a"), &event, &alias, &option);
                let rec = crate::kernel_sync::convert::event_rec(&r, &event).unwrap_or_else(|| panic!("{name}: the event converts"));
                let seat = r.seat("a").unwrap();
                let native = with_core(&mut gs, &mut cx, Sources::default(), READS, |core, _| {
                    let kernel = crate::live::kernel::kernel_for(POK);
                    let cx = Pcx { r: &r, core, k: kernel, plays: None, event: &rec, who: seat };
                    cx.preview(card, &option)
                })
                .expect("the position lends");
                assert_eq!(native, string, "{name} ({label})");
                compared += 1;
                some += usize::from(string.is_some());
            }
        }
        assert_eq!(compared, names.len() * 2);
        assert!(some > names.len(), "the positions give most cards something to say: {some}");
    }
}
