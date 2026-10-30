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
use ti4_engine_core::seen::Seen;
use ti4_engine_core::Seat;
use ti4_model::id::{PlanetId, SystemId};

/// What a parity check of one position found.
#[derive(Debug, Default)]
pub struct Found {
    /// Accessor results compared (one per accessor call).
    pub compared: u64,
    /// `accessor: string answer vs typed answer`.
    pub mismatches: Vec<String>,
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

