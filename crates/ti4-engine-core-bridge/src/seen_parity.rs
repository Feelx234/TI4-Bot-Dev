//! Parity of the typed view with the string view (docs/todo/D3-bots.md D3-1 item 4): at a decider call, every accessor
//! of [`ti4_engine_core::seen::Seen`] must give the answer of the matching `Observed` / `SeatObservation` method, after
//! converting ids to text. Run by `bot_shadow --seen-parity` at every decider call of the golden games (the positions
//! the bot really sees) and by the tests of this crate.
//!
//! Float accessors would be compared bitwise; the accessors of this view are integers and ids.

use std::collections::BTreeSet;

use ti4_engine::choice::SeatObservation;
use ti4_engine::generated::IdStr;
use ti4_engine::seen_load::SeenOwned;
use ti4_engine_core::generated::{Agenda, StrategyCard, System};
use ti4_engine_core::keys::{Outcome, Prediction};
use ti4_engine_core::query::economy::Spend as CoreSpend;
use ti4_engine_core::seen::{ProgressRow, RowFamily, Seen};
use ti4_engine_core::state::ObjectiveRef;
use ti4_engine_core::Seat;
use ti4_model::id::{PlanetId, SystemId};

/// What a parity check of one position found.
#[derive(Debug, Default)]
pub struct Found {
    /// Accessor results compared (one per accessor call).
    pub compared: u64,
    /// `accessor: string answer vs typed answer`.
    pub mismatches: Vec<String>,
    /// Accessor answers the typed side bails on (it returns `None`; the string side falls back to its own engine).
    pub skipped: std::collections::BTreeMap<&'static str, u64>,
}

impl Found {
    fn skip(&mut self, accessor: &'static str) {
        *self.skipped.entry(accessor).or_default() += 1;
    }
}

struct Cmp<'a> {
    found: &'a mut Found,
    owned: &'a SeenOwned,
}

impl Cmp<'_> {
    fn eq<T: PartialEq + std::fmt::Debug>(&mut self, what: &str, string: T, typed: T) {
        self.found.compared += 1;
        if string != typed {
            self.found.mismatches.push(format!("{what}: string {string:?} vs typed {typed:?}"));
        }
    }

    fn pid(&self, seat: Seat) -> String {
        self.owned.player(seat).map_or_else(|| format!("<seat {}>", seat.0), |p| p.as_str().to_owned())
    }

    fn outcome(&self, o: Outcome) -> String {
        match o {
            Outcome::Empty => String::new(),
            Outcome::For => "for".to_owned(),
            Outcome::Against => "against".to_owned(),
            Outcome::Seat(s) => self.pid(s),
            Outcome::Planet(p) => p.as_str().to_owned(),
            Outcome::StrategyCard(c) => c.as_str().to_owned(),
            Outcome::Law(a) => a.as_str().to_owned(),
            Outcome::Secret(s) => s.as_str().to_owned(),
        }
    }
}

fn texts<T: IdStr>(items: impl IntoIterator<Item = T>) -> Vec<String> {
    items.into_iter().map(|t| t.as_str().to_owned()).collect()
}

/// Compare every accessor of the typed view with the string view of the same position, for the bound seat of `seen`.
/// `owned` is the loaded typed copy of that position.
pub fn check(seen: &SeatObservation<'_>, owned: &SeenOwned) -> Found {
    let mut found = Found::default();
    let me = seen.bound_seat().clone();
    let Some(typed) = owned.seen(&me) else {
        found.mismatches.push(format!("the bound seat {me} has no seat in the owner table"));
        return found;
    };
    let mut c = Cmp { found: &mut found, owned };
    let obs = seen.observed();
    meta(&mut c, obs, &typed);
    seats(&mut c, obs, &typed);
    board(&mut c, obs, &typed);
    agenda(&mut c, obs, &typed);
    hand(&mut c, seen, &typed);
    objectives(&mut c, seen, &typed);
    movement(&mut c, seen, &typed);
    perturbation(&mut c, seen, &typed);
    found
}

fn meta(c: &mut Cmp<'_>, obs: &ti4_engine::choice::Observed<'_>, t: &Seen<'_>) {
    c.eq("round", obs.round(), t.round());
    c.eq("phase", format!("{:?}", obs.phase()), format!("{:?}", t.phase()));
    c.eq("active", obs.active_player().map(|p| p.as_str().to_owned()), t.active().map(|s| c.pid(s)));
    c.eq("speaker", obs.speaker().as_str().to_owned(), c.pid(t.speaker()));
    c.eq("custodians_removed", obs.custodians_removed(), t.custodians_removed());
    c.eq(
        "players",
        obs.players().iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
        t.players().iter().map(|s| c.pid(*s)).collect::<Vec<_>>(),
    );
    c.eq(
        "initiative_order",
        obs.initiative_order().iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
        t.initiative_order().as_slice().iter().map(|s| c.pid(*s)).collect::<Vec<_>>(),
    );
}

fn seats(c: &mut Cmp<'_>, obs: &ti4_engine::choice::Observed<'_>, t: &Seen<'_>) {
    for player in obs.players() {
        let Some(seat) = c.owned.seat(player) else {
            c.found.mismatches.push(format!("player {player} has no seat"));
            continue;
        };
        let (Some(s), Some(v)) = (obs.seat(player), t.seat(seat)) else {
            c.eq(&format!("seat({player}).is_some"), obs.seat(player).is_some(), t.seat(seat).is_some());
            continue;
        };
        let w = |f: &str| format!("seat({player}).{f}");
        c.eq(&w("faction"), s.faction.as_str().to_owned(), v.faction.as_str().to_owned());
        c.eq(&w("victory_points"), s.victory_points, v.victory_points);
        c.eq(&w("trade_goods"), s.trade_goods, v.trade_goods);
        c.eq(&w("commodities"), s.commodities, v.commodities);
        c.eq(&w("tactic_tokens"), s.tactic_tokens, v.tactic_tokens);
        c.eq(&w("fleet_tokens"), s.fleet_tokens, v.fleet_tokens);
        c.eq(&w("strategic_tokens"), s.strategic_tokens, v.strategic_tokens);
        c.eq(
            &w("strategy_cards"),
            s.strategy_cards.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
            texts(v.strategy_cards.iter().copied()),
        );
        c.eq(
            &w("exhausted_strategy_cards"),
            s.exhausted_strategy_cards.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
            texts(v.exhausted_strategy_cards.iter()),
        );
        c.eq(
            &w("technologies"),
            s.technologies.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
            texts(v.technologies.iter()),
        );
        c.eq(
            &w("exhausted_technologies"),
            s.exhausted_technologies.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
            texts(v.exhausted_technologies.iter()),
        );
        c.eq(&w("action_cards_held"), s.action_cards_held, v.action_cards_held);
        c.eq(&w("secret_objectives_held"), s.secret_objectives_held, v.secret_objectives_held);
        c.eq(&w("passed"), s.passed, v.passed);
        c.eq(&w("relics"), s.relics.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(), texts(v.relics.iter().copied()));
        c.eq(
            &w("exhausted_relics"),
            s.exhausted_relics.iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
            texts(v.exhausted_relics.iter()),
        );
        c.eq(
            &w("exploration_cards"),
            s.exploration_cards.to_vec(),
            texts(v.exploration_cards.iter().copied()),
        );
        // Fragments: the string map keys are the trait names; compare the multiset of counts and the key count.
        let mut string_counts: Vec<i32> = s.relic_fragments.values().copied().collect();
        string_counts.sort_unstable();
        let mut typed_counts: Vec<i32> = (0..4u8)
            .filter(|i| v.relic_fragments.present & (1 << i) != 0)
            .map(|i| i32::from(v.relic_fragments.counts[usize::from(i)]))
            .collect();
        typed_counts.sort_unstable();
        c.eq(&w("relic_fragments"), string_counts, typed_counts);
        c.eq(
            &w("breakthrough"),
            s.breakthrough.map(|b| b.as_str().to_owned()),
            v.breakthrough.map(|b| b.as_str().to_owned()),
        );
        c.eq(
            &w("leaders"),
            s.leaders.iter().map(|(k, st)| (k.as_str().to_owned(), format!("{st:?}"))).collect::<Vec<_>>(),
            v.leaders.status.iter().map(|(k, st)| (k.as_str().to_owned(), format!("{st:?}"))).collect::<Vec<_>>(),
        );
        // the economy of the seat
        for (kind, core) in [(ti4_engine::production::Spend::Resources, CoreSpend::Resources), (ti4_engine::production::Spend::Influence, CoreSpend::Influence)] {
            c.eq(&w(&format!("available_spend {core:?}")), obs.available_spend(player, kind), t.available_spend(seat, core));
        }
        c.eq(
            &w("controlled_planets"),
            obs.controlled_planets(player).iter().map(|(s, p)| (s.as_str().to_owned(), p.as_str().to_owned())).collect::<Vec<_>>(),
            t.controlled_planets(seat).as_slice().iter().map(|(s, p)| (s.as_str().to_owned(), p.as_str().to_owned())).collect::<Vec<_>>(),
        );
        c.eq(
            &w("systems_with_token"),
            obs.systems_with_token(player).iter().map(|s| s.as_str().to_owned()).collect::<Vec<_>>(),
            t.systems_with_token(seat).iter().map(|i| ti4_engine_core::ids::system_at(i).as_str().to_owned()).collect::<Vec<_>>(),
        );
        c.eq(
            &w("systems_with_units_of"),
            obs.systems_with_units_of(player).iter().map(|s| s.as_str().to_owned()).collect::<Vec<_>>(),
            t.systems_with_units_of(seat).iter().map(|i| ti4_engine_core::ids::system_at(i).as_str().to_owned()).collect::<Vec<_>>(),
        );
    }
    for typed in StrategyCard::ALL {
        let card = ti4_model::id::StrategyCardId::new(typed.as_str());
        c.eq(&format!("card_initiative({card})"), obs.card_initiative(&card), t.card_initiative(typed));
        c.eq(&format!("strategy_card_goods({card})"), obs.strategy_card_goods(&card), t.strategy_card_goods(typed));
    }
}

fn unit_texts_string(units: &[ti4_model::units::Unit]) -> Vec<(String, String, bool, bool)> {
    units.iter().map(|u| (u.type_id.as_str().to_owned(), u.owner.as_str().to_owned(), u.sustained_damage, u.galvanized)).collect()
}

fn board(c: &mut Cmp<'_>, obs: &ti4_engine::choice::Observed<'_>, t: &Seen<'_>) {
    c.eq("active_system", obs.active_system().map(|s| s.as_str().to_owned()), t.active_system().map(|s| s.as_str().to_owned()));
    // Every touched system, and the first systems of the table (absent ones are empty).
    let mut systems: BTreeSet<SystemId> = obs.board().keys().cloned().collect();
    systems.extend(System::ALL.iter().take(8).map(|s| SystemId::new(s.as_str())));
    for id in systems {
        let Some(typed) = System::from_id(id.as_str()) else {
            c.found.mismatches.push(format!("system {id} is not in the typed tables"));
            continue;
        };
        let (string, view) = (obs.system(&id), t.system(typed));
        let w = |f: &str| format!("system({id}).{f}");
        c.eq(
            &w("units"),
            unit_texts_string(&string.units),
            view.units().iter().map(|p| (p.kind().as_str().to_owned(), c.pid(p.owner()), p.sustained(), p.galvanized())).collect(),
        );
        // the string model of the system, built from the typed view, for the shared string helpers
        let model = c.owned.to_model_system(typed);
        c.eq(
            &w("to_model_system (units, tokens, control, planet units)"),
            (unit_texts_string(&string.units), string.command_tokens.iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
             string.planet_control.iter().map(|(p, o)| (p.as_str().to_owned(), o.as_str().to_owned())).collect::<Vec<_>>(),
             string.planet_units.iter().filter(|(_, u)| !u.is_empty()).map(|(p, u)| (p.as_str().to_owned(), unit_texts_string(u))).collect::<Vec<_>>()),
            (unit_texts_string(&model.units), model.command_tokens.iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
             model.planet_control.iter().map(|(p, o)| (p.as_str().to_owned(), o.as_str().to_owned())).collect::<Vec<_>>(),
             model.planet_units.iter().map(|(p, u)| (p.as_str().to_owned(), unit_texts_string(u))).collect::<Vec<_>>()),
        );
        c.eq(
            &w("command_tokens"),
            string.command_tokens.iter().map(|p| p.as_str().to_owned()).collect::<Vec<_>>(),
            {
                let mut v: Vec<String> = view.tokens().iter().map(|s| c.pid(s)).collect();
                v.sort();
                v
            },
        );
        let mut control = Vec::new();
        let mut units = Vec::new();
        for planet in view.planets().as_slice() {
            let pv = view.planet(*planet);
            if let Some(owner) = pv.control {
                control.push((planet.as_str().to_owned(), c.pid(owner)));
            }
            if !pv.units.is_empty() {
                units.push((
                    planet.as_str().to_owned(),
                    pv.units.iter().map(|p| (p.kind().as_str().to_owned(), c.pid(p.owner()), p.sustained(), p.galvanized())).collect::<Vec<_>>(),
                ));
            }
            c.eq(
                &format!("planet_is_ready({planet:?})"),
                obs.planet_is_ready(&PlanetId::new(planet.as_str())),
                t.planet_is_ready(*planet),
            );
        }
        control.sort();
        units.sort();
        c.eq(
            &w("planet_control"),
            string.planet_control.iter().map(|(p, o)| (p.as_str().to_owned(), o.as_str().to_owned())).collect::<Vec<_>>(),
            control,
        );
        c.eq(
            &w("planet_units"),
            string
                .planet_units
                .iter()
                .filter(|(_, u)| !u.is_empty())
                .map(|(p, u)| (p.as_str().to_owned(), unit_texts_string(u)))
                .collect::<Vec<_>>(),
            units,
        );
    }
}

fn agenda(c: &mut Cmp<'_>, obs: &ti4_engine::choice::Observed<'_>, t: &Seen<'_>) {
    for (alias, outcome) in obs.laws() {
        match Agenda::from_id(alias) {
            Some(law) => {
                let typed = t.law_outcome(law).map(|o| c.outcome(o));
                c.eq(&format!("law_outcome({alias})"), Some(outcome.clone()), typed);
            }
            None => c.found.mismatches.push(format!("law {alias} is not in the typed tables")),
        }
    }
    // A law not in play answers none on both sides.
    for law in Agenda::ALL.iter().take(12) {
        if !obs.laws().contains_key(law.as_str()) {
            c.eq(&format!("law_outcome({})", law.as_str()), obs.law_outcome(law.as_str()).map(str::to_owned), t.law_outcome(*law).map(|o| c.outcome(o)));
        }
    }
    c.eq("agenda_in_play", obs.agenda_in_play().map(str::to_owned), t.agenda_in_play().map(|a| a.as_str().to_owned()));
    c.eq("agendas_resolved_this_round", obs.agendas_resolved_this_round(), t.agendas_resolved_this_round());
    for player in obs.players() {
        let Some(seat) = c.owned.seat(player) else { continue };
        let string = obs.agenda_prediction_of(player).map(str::to_owned);
        let typed = t.agenda_prediction_of(seat);
        // The tag spelling of the string text is not rebuilt: the outcome part (before `|`) and the kind are compared.
        let render = |p: Prediction| match p {
            Prediction::Plain(o) => (c.outcome(o), "plain".to_owned()),
            Prediction::Tagged(o, _) => (c.outcome(o), "tagged".to_owned()),
            Prediction::Barred(_) => ("none".to_owned(), "barred".to_owned()),
        };
        let string_parts = string.map(|text| match text.split_once('|') {
            None => (text, "plain".to_owned()),
            Some(("none", _)) => ("none".to_owned(), "barred".to_owned()),
            Some((o, _)) => (o.to_owned(), "tagged".to_owned()),
        });
        c.eq(&format!("agenda_prediction_of({player})"), string_parts, typed.map(render));
    }
    c.eq(
        "invasion",
        obs.invasion().map(|i| {
            (i.system.as_str().to_owned(), i.invader.as_str().to_owned(), i.seq, i.planet.as_ref().map(|p| p.as_str().to_owned()), i.defender.as_ref().map(|p| p.as_str().to_owned()), i.ground_round)
        }),
        t.invasion().map(|i| (i.system.as_str().to_owned(), c.pid(i.invader), i.seq, i.planet.map(|p| p.as_str().to_owned()), i.defender.map(|d| c.pid(d)), i.ground_round)),
    );
    c.eq(
        "space_battle",
        obs.space_battle().map(|(s, a, d)| (s.as_str().to_owned(), a.as_str().to_owned(), d.as_str().to_owned())),
        t.space_battle().map(|(s, a, d)| (s.as_str().to_owned(), c.pid(a), c.pid(d))),
    );
    let mut string_holders: Vec<String> = obs.promissory_notes().values().map(|p| p.as_str().to_owned()).collect();
    string_holders.sort();
    let mut typed_holders: Vec<String> = t.promissory_notes_in_play().iter().map(|(_, h)| c.pid(*h)).collect();
    typed_holders.sort();
    c.eq("promissory_notes (holders)", string_holders, typed_holders);
    c.eq(
        "support_holders",
        obs.support_holders().iter().map(|(o, h)| (o.as_str().to_owned(), h.as_str().to_owned())).collect::<Vec<_>>(),
        {
            let mut v: Vec<(String, String)> = t.support_holders().iter().map(|(o, h)| (c.pid(*o), c.pid(*h))).collect();
            v.sort();
            v
        },
    );
}

fn hand(c: &mut Cmp<'_>, seen: &SeatObservation<'_>, t: &Seen<'_>) {
    c.eq(
        "held_action_cards",
        seen.held_action_cards().iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
        texts(t.my_action_cards().iter().copied()),
    );
    c.eq(
        "held_secrets",
        seen.held_secrets().iter().map(|x| x.as_str().to_owned()).collect::<Vec<_>>(),
        texts(t.my_secrets().iter().copied()),
    );
    c.eq("held_promissory_notes (count)", seen.held_promissory_notes().len(), t.my_notes().len());
}


fn card_rows(rows: &[ti4_engine::objectives::CardProgress]) -> Vec<(String, String, u64, u64, bool, Option<u8>)> {
    rows.iter()
        .map(|r| (r.alias.clone(), r.family_token.clone(), r.have.to_bits(), r.threshold.to_bits(), r.satisfied, r.stage))
        .collect()
}

fn typed_rows(rows: &[ProgressRow]) -> Vec<(String, String, u64, u64, bool, Option<u8>)> {
    use ti4_engine::objectives_core as oc;
    rows.iter()
        .map(|r| {
            let alias = match r.objective {
                ObjectiveRef::Public(o) => o.as_str().to_owned(),
                ObjectiveRef::Secret(o) => o.as_str().to_owned(),
            };
            let token = match r.family {
                RowFamily::Count(f) => ti4_engine::objectives::family_token(&oc::family(f)),
                RowFamily::Cost(f) => {
                    let p = oc::cost_progress(ti4_engine_core::query::objectives::CostProgress { family: f, have: r.have, target: r.threshold });
                    ti4_engine::objectives::cost_family_token(&p.family)
                }
            };
            (alias, token, (r.have as f64).to_bits(), (r.threshold as f64).to_bits(), r.satisfied, r.stage)
        })
        .collect()
}

fn objectives(c: &mut Cmp<'_>, seen: &SeatObservation<'_>, t: &Seen<'_>) {
    use ti4_engine::objectives::Imagined as SImagined;
    use ti4_engine_core::query::objectives::Imagined as TImagined;
    let obs = seen.observed();
    c.eq(
        "revealed_objectives",
        obs.revealed_objectives().iter().map(|o| o.as_str().to_owned()).collect::<Vec<_>>(),
        t.revealed_objectives()
            .iter()
            .map(|r| match r {
                ObjectiveRef::Public(o) => o.as_str().to_owned(),
                ObjectiveRef::Secret(o) => o.as_str().to_owned(),
            })
            .collect::<Vec<_>>(),
    );
    for player in obs.players() {
        let Some(seat) = c.owned.seat(player) else { continue };
        let scored = t.scored_by(seat);
        let mut typed_scored: Vec<String> = scored.public.iter().map(|o| o.as_str().to_owned()).chain(scored.secret.iter().map(|o| o.as_str().to_owned())).collect();
        typed_scored.sort();
        match t.scoreable_public(seat) {
            Some(n) => c.eq(&format!("scoreable_public({player})"), obs.scoreable_public(player), n),
            None => c.found.skip("scoreable_public"),
        }
        c.eq(&format!("scored_by({player})"), obs.scored_by(player).iter().map(|o| o.as_str().to_owned()).collect::<Vec<_>>(), typed_scored);

        // Planets the option would take: the first two planets other seats control (the counterfactual of an invasion).
        let theirs: Vec<PlanetId> = obs
            .board()
            .values()
            .flat_map(|s| s.planet_control.iter())
            .filter(|(_, owner)| *owner != player)
            .map(|(p, _)| p.clone())
            .take(2)
            .collect();
        let typed_planets: Vec<ti4_engine_core::generated::Planet> = theirs.iter().filter_map(|p| IdStr::from_id(p.as_str())).collect();
        let tech = ti4_engine_core::generated::Technology::ALL[3];
        let systems = vec!["18".to_owned()];
        let techs = vec![tech.as_str().to_owned()];
        let string_options = [
            SImagined::NONE,
            SImagined { planets: &theirs, systems: &systems, technologies: &techs, structures: &[] },
        ];
        let typed_options = [
            TImagined::NONE,
            TImagined { planets: &typed_planets, systems: &[System::from_id("18").unwrap_or(System::ALL[0])], technologies: &[tech], structures: &[] },
        ];
        for (n, (si, ti)) in string_options.iter().zip(typed_options).enumerate() {
            let string = obs.revealed_objective_progress_imagining(player, si);
            match t.revealed_objective_progress(seat, ti) {
                Some(rows) => c.eq(&format!("revealed_objective_progress({player}, imagined {n})"), card_rows(&string), typed_rows(&rows)),
                None => c.found.skip("revealed_objective_progress"),
            }
        }
    }
    // The bound seat's secrets.
    let me = c.owned.seat(seen.bound_seat());
    if me.is_some() {
        let string = seen.held_secret_progress_imagining(&ti4_engine::objectives::Imagined::NONE);
        match t.my_secret_progress(ti4_engine_core::query::objectives::Imagined::NONE) {
            Some(rows) => c.eq("held_secret_progress", card_rows(&string), typed_rows(&rows)),
            None => c.found.skip("held_secret_progress"),
        }
    }
    // Systems the map-shaped objectives name.
    if let Some(me) = me {
        let mut systems: BTreeSet<SystemId> = obs.board().keys().cloned().collect();
        systems.extend(System::ALL.iter().take(40).map(|s| SystemId::new(s.as_str())));
        for id in systems {
            let Some(typed) = System::from_id(id.as_str()) else { continue };
            c.eq(
                &format!("objectives_implicating_system({id})"),
                obs.objectives_implicating_system(seen.bound_seat(), &id).iter().map(|o| o.as_str().to_owned()).collect::<Vec<_>>(),
                t.objectives_implicating_system(me, typed).as_slice().iter().map(|o| o.as_str().to_owned()).collect::<Vec<_>>(),
            );
        }
    }
}

/// Destinations and origins worth asking about: the active system, a few systems with units of any seat, and Mecatol.
fn movement(c: &mut Cmp<'_>, seen: &SeatObservation<'_>, t: &Seen<'_>) {
    let obs = seen.observed();
    let mut dests: Vec<SystemId> = Vec::new();
    if let Some(active) = obs.active_system() {
        dests.push(active.clone());
    }
    dests.extend(obs.board().iter().filter(|(_, s)| !s.units.is_empty()).map(|(id, _)| id.clone()).take(4));
    dests.push(SystemId::new("18"));
    dests.sort();
    dests.dedup();
    let me = seen.bound_seat();
    let Some(my_seat) = c.owned.seat(me) else { return };
    let origins: Vec<SystemId> = obs.systems_with_units_of(me).into_iter().take(4).cloned().collect();
    for dest in &dests {
        let Some(typed_dest) = System::from_id(dest.as_str()) else { continue };
        let string = obs.movable_into(me, dest);
        let typed = t.movable_into(my_seat, typed_dest);
        c.eq(
            &format!("movable_into({dest})"),
            string.iter().map(|m| (m.origin.as_str().to_owned(), m.index, m.unit.type_id.as_str().to_owned(), m.capacity, m.gravity_drive, m.ionian)).collect::<Vec<_>>(),
            typed.iter().map(|m| (m.origin.as_str().to_owned(), usize::from(m.index), m.kind.as_str().to_owned(), m.capacity, m.gravity_drive, m.ionian)).collect::<Vec<_>>(),
        );
        for m in string.iter() {
            let Some(kind) = ti4_engine_core::generated::Unit::from_id(m.unit.type_id.as_str()) else { continue };
            let Some(origin) = System::from_id(m.origin.as_str()) else { continue };
            for boosts in [(false, false), (m.gravity_drive, m.ionian)] {
                c.eq(
                    &format!("rift_exits_into({} -> {dest}, {boosts:?})", m.origin),
                    obs.rift_exits_into(me, &m.origin, m.unit.type_id.as_str(), boosts, dest),
                    t.rift_exits_into(my_seat, origin, kind, boosts, typed_dest),
                );
            }
        }
        for origin in &origins {
            let Some(typed_origin) = System::from_id(origin.as_str()) else { continue };
            for value in [1, 2, 3] {
                c.eq(
                    &format!("can_reach({origin} -> {dest}, {value})"),
                    obs.can_reach(me, origin, dest, value),
                    t.can_reach(my_seat, typed_origin, typed_dest, value),
                );
            }
        }
    }
    for origin in &origins {
        let Some(typed_origin) = System::from_id(origin.as_str()) else { continue };
        c.eq(
            &format!("loadable({origin})"),
            obs.loadable(me, origin)
                .iter()
                .map(|l| {
                    (
                        l.unit.type_id.as_str().to_owned(),
                        l.unit.owner.as_str().to_owned(),
                        match &l.source {
                            ti4_engine::transit::CargoSource::Space => None,
                            ti4_engine::transit::CargoSource::Planet(p) => Some(p.as_str().to_owned()),
                        },
                        l.system.as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>(),
            t.loadable(my_seat, typed_origin)
                .iter()
                .map(|l| (l.piece.kind().as_str().to_owned(), c.pid(l.piece.owner()), l.source.map(|p| p.as_str().to_owned()), l.system.as_str().to_owned()))
                .collect::<Vec<_>>(),
        );
    }
}

/// A hash of every public accessor of the view, for the bound seat and every other seat (typed side only).
fn digest(owned: &SeenOwned, me: &ti4_model::id::PlayerId, obs: &ti4_engine::choice::Observed<'_>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let Some(t) = owned.seen(me) else { return 0 };
    let mut put = |what: &str, value: String| {
        what.hash(&mut h);
        value.hash(&mut h);
    };
    put("meta", format!("{} {:?} {:?} {:?} {} {:?} {:?}", t.round(), t.phase(), t.active(), t.speaker(), t.custodians_removed(), t.players(), t.initiative_order()));
    let mut systems: BTreeSet<SystemId> = obs.board().keys().cloned().collect();
    systems.extend(System::ALL.iter().take(8).map(|s| SystemId::new(s.as_str())));
    for seat in t.players().to_vec() {
        let v = t.seat(seat).map(|v| format!("{v:?}"));
        put("seat", format!("{v:?}"));
        for kind in [CoreSpend::Resources, CoreSpend::Influence] {
            put("spend", format!("{}", t.available_spend(seat, kind)));
        }
        put("planets", format!("{:?} {:?} {:?}", t.controlled_planets(seat), t.systems_with_token(seat), t.systems_with_units_of(seat)));
        put("scored", format!("{:?}", t.scored_by(seat)));
        put("progress", format!("{:?}", t.revealed_objective_progress(seat, ti4_engine_core::query::objectives::Imagined::NONE)));
        put("prediction", format!("{:?}", t.agenda_prediction_of(seat)));
    }
    for id in systems {
        let Some(s) = System::from_id(id.as_str()) else { continue };
        let view = t.system(s);
        let planets: Vec<String> = view.planets().as_slice().iter().map(|p| format!("{:?}", view.planet(*p))).collect();
        put("system", format!("{:?} {:?} {planets:?}", view.units(), view.tokens()));
    }
    put("agenda", format!("{:?} {} {:?} {:?} {:?}", t.agenda_in_play(), t.agendas_resolved_this_round(), t.invasion(), t.space_battle(), t.support_holders()));
    put("notes", format!("{:?}", t.promissory_notes_in_play()));
    put("hand", format!("{:?} {:?} {:?} {:?}", t.my_action_cards(), t.my_secrets(), t.my_notes(), t.my_secret_progress(ti4_engine_core::query::objectives::Imagined::NONE)));
    if let Some(seat) = owned.seat(me) {
        for dest in obs.board().iter().filter(|(_, s)| !s.units.is_empty()).map(|(id, _)| id).take(3) {
            if let Some(d) = System::from_id(dest.as_str()) {
                put("movable", format!("{:?}", t.movable_into(seat, d)));
            }
        }
    }
    h.finish()
}

/// The perturbation test (docs/todo/D3-bots.md 2.2 rule 4): scramble the hidden fields of the loaded state and require
/// every accessor to answer as before.
fn perturbation(c: &mut Cmp<'_>, seen: &SeatObservation<'_>, _t: &Seen<'_>) {
    let me = seen.bound_seat();
    let scrambled = c.owned.scrambled(me);
    let (a, b) = (digest(c.owned, me, seen.observed()), digest(&scrambled, me, seen.observed()));
    c.eq("perturbation: digest of every accessor with other seats' hands scrambled", a, b);
}
