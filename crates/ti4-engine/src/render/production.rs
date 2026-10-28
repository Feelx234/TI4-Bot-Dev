//! Renderer of the `production` area: `impl Family` for `ti4_engine_core::choice::production` (prompt, option,
//! parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B3. Ported: the payment question (`pay` options: exhaust a planet face, spend a trade good,
//! spend a commodity), byte for byte as `production::payment_options` lists them.

use ti4_engine_core::choice::production::{Ask, ChooseAsk, Currency, Opt, PayAsk, PlaceAsk, Shown, Subtype};
use ti4_engine_core::generated::{System, Unit};
use ti4_engine_core::rules::production::Spot;
use ti4_engine_core::query::fleet::Standing;
use ti4_engine_core::generated::Planet;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;
use crate::preview::{Delta, Preview, Quantity};

/// The renderer of the production questions.
pub struct Fam;

const fn name(c: Currency) -> &'static str {
    match c {
        Currency::Resources => "resources",
        Currency::Influence => "influence",
    }
}

const fn quantity(c: Currency) -> Quantity {
    match c {
        Currency::Resources => Quantity::Resources,
        Currency::Influence => Quantity::Influence,
    }
}

fn planet_id(p: Planet) -> &'static str {
    <Planet as IdStr>::as_str(p)
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::Pay(p) => format!("pay {} more {}", p.owed, name(p.kind)),
            Ask::Choose(c) => format!("produce in {} ({} left)", <System as IdStr>::as_str(c.system), c.remaining),
            Ask::Place(p) => format!("place the {}", unit_id(p.unit)),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::Pay(kind) => format!("pay_{}", name(*kind)),
            Subtype::Choose => "produce_unit".to_owned(),
            Subtype::Place => "place_unit".to_owned(),
        }
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let PayAsk { kind, owed, pool, goods } = match *a {
            Ask::Pay(p) => p,
            Ask::Choose(c) => return build_option(r, &c, o),
            Ask::Place(p) => return place_option(r, &p, o),
        };
        match *o {
            Opt::Exhaust { planet, source, shown } => {
                let (worth, pool_after) = (i64::from(shown.worth), i64::from(shown.pool_after));
                let id = if source == kind {
                    format!("exhaust|{}", planet_id(planet))
                } else {
                    format!("exhaust|{}|{}", planet_id(planet), name(source))
                };
                let mut label = format!("exhaust {} for {worth} {}", planet_id(planet), name(kind));
                if source != kind {
                    label.push_str(" using its ");
                    label.push_str(name(source));
                }
                ChoiceOption::labelled(id, crate::production::PAY_KIND, label)
                    .with("worth", worth)
                    .with("owed", owed)
                    .with("kind", name(kind))
                    .with("source", name(source))
                    .previewed(Preview::certain(vec![
                        Delta::new(quantity(kind), pool, pool_after),
                        Delta::new(Quantity::TradeGoods, goods, goods),
                    ]))
            }
            Opt::TradeGood { shown } => {
                let worth = i64::from(shown.worth);
                ChoiceOption::labelled("trade_good", crate::production::PAY_KIND, "spend a trade good")
                    .with("worth", worth)
                    .with("owed", owed)
                    .with("kind", name(kind))
                    .previewed(Preview::certain(vec![
                        Delta::new(quantity(kind), pool, pool - worth),
                        Delta::new(Quantity::TradeGoods, goods, goods - 1),
                    ]))
            }
            Opt::Build { .. } | Opt::Exchange { .. } | Opt::Place { .. } | Opt::Stop | Opt::Agent => unreachable!("builds are asked by Ask::Choose"),
            Opt::Commodity { shown } => {
                let worth = i64::from(shown.worth);
                ChoiceOption::labelled("commodity", crate::production::PAY_KIND, "spend a commodity as a trade good")
                    .with("worth", worth)
                    .with("owed", owed)
                    .with("kind", name(kind))
                    .previewed(Preview::certain(vec![Delta::new(quantity(kind), pool, pool - worth)]))
            }
        }
    }

    fn details(r: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
        match a {
            Ask::Choose(c) => choose_details(r, c).unwrap_or_default(),
            _ => serde_json::Map::new(),
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        let kind = match *a {
            Ask::Pay(p) => p.kind,
            Ask::Choose(_) => return parse_build(id),
            Ask::Place(p) => return parse_place(&p, id),
        };
        match id {
            "trade_good" => Some(Opt::TradeGood { shown: Shown::default() }),
            "commodity" => Some(Opt::Commodity { shown: Shown::default() }),
            _ => {
                let rest = id.strip_prefix("exhaust|")?;
                let (planet, source) = match rest.split_once('|') {
                    Some((p, "resources")) => (p, Currency::Resources),
                    Some((p, "influence")) => (p, Currency::Influence),
                    Some(_) => return None,
                    None => (rest, kind),
                };
                Some(Opt::Exhaust { planet: <Planet as IdStr>::from_id(planet)?, source, shown: Shown::default() })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::choice::{Ask as CoreAsk, Opt as CoreOpt};
    use ti4_engine_core::choice::production::Shown;

    #[test]
    fn every_payment_option_parses_back_from_its_id() {
        let r = Render::default();
        let ask = CoreAsk::Production(Ask::Pay(PayAsk { kind: Currency::Resources, owed: 3, pool: 5, goods: 1 }));
        let planet = <Planet as IdStr>::from_id("abyz").expect("abyz is a planet");
        let opts = [
            Opt::Exhaust { planet, source: Currency::Resources, shown: Shown { worth: 3, pool_after: 2 } },
            Opt::Exhaust { planet, source: Currency::Influence, shown: Shown { worth: 1, pool_after: 2 } },
            Opt::TradeGood { shown: Shown { worth: 1, pool_after: 4 } },
            Opt::Commodity { shown: Shown { worth: 1, pool_after: 4 } },
        ];
        for o in opts {
            let rendered = crate::render::option(&r, &ask, &CoreOpt::Production(o));
            assert_eq!(crate::render::parse(&r, &ask, &rendered.id), Some(CoreOpt::Production(o)), "{}", rendered.id);
        }
        let influence = crate::render::option(&r, &ask, &CoreOpt::Production(opts[1]));
        assert_eq!(influence.id, "exhaust|abyz|influence");
        assert_eq!(influence.label, "exhaust abyz for 1 resources using its influence");
    }
}

fn unit_id(u: Unit) -> &'static str {
    <Unit as IdStr>::as_str(u)
}

#[must_use]
pub fn spot_text(s: Spot) -> String {
    match s {
        Spot::Space => crate::production::SPACE.to_owned(),
        Spot::Planet(p) => planet_id(p).to_owned(),
        Spot::Remote(system, at) => format!(
            "{}{}{}",
            <System as IdStr>::as_str(system),
            crate::production::REMOTE_SEPARATOR,
            at.map_or(crate::production::SPACE, planet_id)
        ),
    }
}

fn parse_build(id: &str) -> Option<Opt> {
    if id == "done_producing" {
        return Some(Opt::Stop);
    }
    if id == crate::production::AGENT_OPTION_ID {
        return Some(Opt::Agent);
    }
    if let Some(rest) = id.strip_prefix("build|") {
        let (unit, made) = rest.split_once('|')?;
        return Some(Opt::Build { unit: <Unit as IdStr>::from_id(unit)?, made: made.parse().ok()? });
    }
    Some(Opt::Exchange { unit: <Unit as IdStr>::from_id(id.strip_prefix("exchange|")?)? })
}

fn limit_deltas(before: &Standing, after: &Standing) -> [Delta; 2] {
    [
        Delta::new(Quantity::FleetSupplyHeadroom, before.fleet_headroom(), after.fleet_headroom()),
        Delta::new(Quantity::CapacityFree, before.capacity_free(), after.capacity_free()),
    ]
}

fn placement_facts(option: ChoiceOption, before: &Standing, after: &Standing) -> ChoiceOption {
    option
        .with("capacity_used", after.consumed - before.consumed)
        .with("fleet_headroom_after", after.fleet_headroom())
        .with("capacity_free_after", after.capacity_free())
        .with("fleet_excess_after", after.fleet_excess())
        .with("capacity_excess_after", after.capacity_excess)
}

fn capacity_facts(option: ChoiceOption, unit: Unit, placed: usize, spots: &[String]) -> ChoiceOption {
    use ti4_engine_core::generated::UnitBaseType;
    use ti4_engine_core::query::fleet as fl;
    let placed = i64::try_from(placed).unwrap_or(0);
    let space = spots.iter().filter(|s| s.as_str() == crate::production::SPACE).count();
    let placement = if spots.is_empty() || space == 0 {
        "never"
    } else if space == spots.len() {
        "always"
    } else {
        "choice"
    };
    let role = if unit.data().base_type == UnitBaseType::Fighter {
        "fighter"
    } else if fl::consumes_capacity(unit) {
        "ground"
    } else {
        "none"
    };
    option
        .with("capacity_provided", fl::capacity(unit) * placed)
        .with("capacity_needed", if role == "none" { 0 } else { fl::capacity_cost(unit) * placed })
        .with("capacity_role", role)
        .with("space_placement", placement)
}

/// A Choosing option, with the numbers the position gives (see `production_view::choosing_info`).
fn build_option(r: &Render, ask: &ChooseAsk, o: &Opt) -> ChoiceOption {
    let (unit, exchange) = match *o {
        Opt::Stop => return ChoiceOption::labelled("done_producing", "decline", "produce nothing further"),
        Opt::Agent => {
            return ChoiceOption::labelled(
                crate::production::AGENT_OPTION_ID,
                crate::production::AGENT_OPTION_KIND,
                "Exhaust Berekar Berekon: -2 cost",
            )
            .with("leader", "winnuagent")
            .with("discount_offered", 2_i64);
        }
        Opt::Build { unit, .. } => (unit, false),
        Opt::Exchange { unit } => (unit, true),
        _ => unreachable!("payment options are asked by Ask::Pay"),
    };
    let info = crate::production_view::choosing_info(r, ask, unit).expect("a listed option is offered in its position");
    let id = unit_id(unit);
    let system = <System as IdStr>::as_str(ask.system);
    let spots: Vec<String> = info.spots.as_slice().iter().map(|s| spot_text(*s)).collect();
    let ships = |placed: usize| if ti4_engine_core::query::fleet::counts_against_supply(unit) { i64::try_from(placed).unwrap_or(0) } else { 0 };
    let (remaining_after, free_after) = if exchange { info.exchange_after } else { (info.remaining_after, info.free_after) };
    let count = if exchange { 1 } else { info.placed };
    let (rem, free0) = (i64::from(ask.remaining), i64::from(ask.free_capacity) + i64::from(ask.small_exemptions));
    let mut deltas = vec![
        Delta::new(Quantity::ProductionRemaining, rem, remaining_after),
        Delta::new(Quantity::ProductionFreeCapacity, free0, free_after),
    ];
    let credit = i64::from(ask.credit);
    let mut option;
    let after = if exchange { info.after_exchange } else { info.after_paid };
    if exchange {
        option = ChoiceOption::labelled(format!("exchange|{id}"), crate::production::PRODUCE_KIND, format!("produce 1x {id} by returning a captured {id}"))
            .with("cost", 0_i64)
            .with("printed_cost", info.printed)
            .with("discount", 0_i64)
            .with("count", 1_i64)
            .with("placed", 1_i64)
            .with("credit", credit)
            .with("credit_used", 0_i64)
            .with("owed", 0_i64)
            .with("production_spent", rem - remaining_after)
            .with("unit", id)
            .with("system", system)
            .with("fleet_ships", ships(1))
            .with("exchange", true);
    } else {
        let credit_used = credit.min(info.cost);
        option = ChoiceOption::labelled(
            format!("build|{id}|{}", info.made),
            crate::production::PRODUCE_KIND,
            format!("produce {}x {id} for {}", info.made, info.cost),
        )
        .with("cost", info.cost)
        .with("printed_cost", info.printed)
        .with("discount", info.discount_used)
        .with("count", i64::try_from(info.made).unwrap_or(1))
        .with("placed", i64::try_from(info.placed).unwrap_or(1))
        .with("yield", i64::try_from(info.pair).unwrap_or(1))
        .with("credit", credit)
        .with("available_resources", info.spendable_resources)
        .with("free_this_use", ask.free_this_use)
        .with("credit_used", credit_used)
        .with("owed", info.cost - credit_used)
        .with("production_spent", rem - remaining_after)
        .with("unit", id)
        .with("system", system)
        .with("fleet_ships", ships(info.placed));
    }
    option = capacity_facts(option, unit, count, &spots);
    if let (Some(after), [only]) = (after.as_ref(), spots.as_slice()) {
        deltas.extend(limit_deltas(&info.before, after));
        option = placement_facts(option.with("destination", only.clone()), &info.before, after);
    } else {
        option = option.with("placement_pending", i64::try_from(spots.len()).unwrap_or(2));
    }
    option.previewed(Preview::certain(deltas))
}

fn parse_place(ask: &PlaceAsk, id: &str) -> Option<Opt> {
    let at = id.strip_prefix("place|")?;
    let spot = match at.split_once(crate::production::REMOTE_SEPARATOR) {
        Some((system, rest)) => Spot::Remote(
            <System as IdStr>::from_id(system)?,
            if rest == crate::production::SPACE { None } else { Some(<Planet as IdStr>::from_id(rest)?) },
        ),
        None if at == crate::production::SPACE => Spot::Space,
        None => Spot::Planet(<Planet as IdStr>::from_id(at)?),
    };
    let _ = ask;
    Some(Opt::Place { spot })
}

fn place_option(r: &Render, ask: &PlaceAsk, o: &Opt) -> ChoiceOption {
    let Opt::Place { spot } = *o else { unreachable!("only placements are asked by Ask::Place") };
    let info = crate::production_view::placing_info(r, ask, spot).expect("a listed spot is offered in its position");
    let text = spot_text(spot);
    placement_facts(
        ChoiceOption::labelled(format!("place|{text}"), crate::production::PLACE_KIND, format!("place on {text}"))
            .with("system", <System as IdStr>::as_str(ask.system))
            .with("unit", unit_id(ask.unit))
            .with("destination", text.clone())
            .with("count", i64::from(ask.made))
            .with("placed", i64::try_from(info.placed).unwrap_or(0)),
        &info.before,
        &info.after,
    )
    .previewed(Preview::certain(limit_deltas(&info.before, &info.after).to_vec()))
}

/// `Choice.details` of the Choosing question: fleet supply, this system's capacity, the box limits.
fn choose_details(r: &Render, c: &ChooseAsk) -> Option<serde_json::Map<String, serde_json::Value>> {
    use ti4_engine_core::generated::UnitBaseType;
    use ti4_engine_core::query::{board, fleet as fl};
    let pos = r.position()?;
    let core: &ti4_engine_core::state::CoreState = &pos.state;
    let scope = ti4_engine_core::tables::Scope(u32::from(pos.kernel.sources.0));
    let cx = ti4_engine_core::rules::production::Cx::new(core, pos.kernel, scope, ti4_engine_core::query::reach::blanked(core));
    let standing = ti4_engine_core::rules::production::standing_before(&cx, c.who, c);
    let mut fleet_supply = serde_json::json!({ "used": standing.fleet_charged, "limit": standing.fleet_limit });
    if fl::is_unlimited(core, c.who) {
        fleet_supply["unlimited"] = serde_json::Value::Bool(true);
    }
    if c.armada > 0 {
        fleet_supply["armada"] = serde_json::json!(c.armada);
    }
    let fighters_here: i64 = board::space_units(core, c.system)
        .iter()
        .filter(|p| p.owner() == c.who && p.kind().data().base_type == UnitBaseType::Fighter)
        .map(|p| fl::capacity_cost(p.kind()))
        .sum();
    let support: i64 = board::planets_of(core, c.system)
        .map(|pl| board::planet_units(core, pl).iter().filter(|p| p.owner() == c.who).map(|p| fl::fighter_support(p.kind())).sum::<i64>())
        .sum();
    let capacity = serde_json::json!({
        "transport": standing.transport,
        "consumed": standing.consumed,
        "fighters": fighters_here,
        "unused_support": (support - fighters_here).max(0),
    });
    let mut limits = serde_json::Map::new();
    for unit in c.buildable.iter().flatten() {
        let base = unit.data().base_type;
        let Some(limit) = fl::plastic(base) else { continue };
        let id = unit_id(*unit);
        let base_text = <UnitBaseType as IdStr>::as_str(base);
        if let Some(entry) = limits.get_mut(base_text) {
            entry["units"].as_array_mut().expect("units array").push(serde_json::json!(id));
            continue;
        }
        let held = fl::held_on_board(core, scope, c.who, base);
        limits.insert(
            base_text.to_owned(),
            serde_json::json!({ "units": [id], "limit": limit, "held": held, "remaining": (limit - held).max(0) }),
        );
    }
    let mut out = serde_json::Map::new();
    out.insert("fleet_supply".to_owned(), fleet_supply);
    out.insert("capacity".to_owned(), capacity);
    out.insert("unit_limits".to_owned(), serde_json::Value::Object(limits));
    Some(out)
}

/// The spot a report text (`space`, a planet id or `system@at`) names.
#[must_use]
pub fn spot_of(text: &str) -> Option<Spot> {
    match text.split_once(crate::production::REMOTE_SEPARATOR) {
        Some((system, rest)) => Some(Spot::Remote(
            <System as IdStr>::from_id(system)?,
            if rest == crate::production::SPACE { None } else { Some(<Planet as IdStr>::from_id(rest)?) },
        )),
        None if text == crate::production::SPACE => Some(Spot::Space),
        None => Some(Spot::Planet(<Planet as IdStr>::from_id(text)?)),
    }
}
