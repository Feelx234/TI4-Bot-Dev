//! Renderer of the `trade` area: `impl Family` for `ti4_engine_core::choice::trade` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Packages B4 and B4c.
//!
//! The proposing question is `"transaction with {faction}"` (subtype `propose_transaction`, free form
//! `x;`); each deal is a [`Shape`] the core derived, written as the string engine writes it: its id, its
//! label and its payload (the notes and cards it names, and the prices `net` and `their_net`, which are the core's
//! `query::trade_deals::nets`). The answering question is `"{deal read aloud} -- accept?"` (subtype
//! `answer_transaction`) with the options `accept` (payload `net`: the partner's net of the deal), `refuse` and
//! `counter`.

use serde_json::Value;
use ti4_engine_core::choice::trade::{Ask, Opt, Shape, Subtype};
use ti4_engine_core::generated::{ExploreType, Planet};
use ti4_engine_core::query::trade_deals::nets;
use ti4_engine_core::state::NoteKey;

use super::{Family, Render};
use crate::wire::choice::{ChoiceOption, DECLINE_KIND};
use crate::core_sync::keys::text::note_text;
use crate::generated::IdStr;
use crate::ties::transactions::{ANSWER_KIND, FREE_FORM_PREFIX, OFFER_KIND};

/// The renderer of the transaction window.
pub struct Fam;

/// `{alias}:{faction}`, the id of a note in the string state.
pub fn note_id(key: NoteKey) -> String {
    format!("{}:{}", note_text(key.note), key.faction.as_str())
}

/// The key a relic fragment is held under (`CULTURAL`).
fn kind_text(kind: ExploreType) -> &'static str {
    match kind {
        ExploreType::Cultural => "CULTURAL",
        ExploreType::Frontier => "FRONTIER",
        ExploreType::Hazardous => "HAZARDOUS",
        ExploreType::Industrial => "INDUSTRIAL",
    }
}

/// The worth of every planet card (`transactions::planet_value`, as whole numbers: resources plus influence, at
/// least one), indexed by `Planet::index`, from the embedded content.
#[must_use]
pub fn planet_values() -> &'static [u8] {
    static VALUES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    VALUES.get_or_init(|| {
        let content = ti4_content::ContentStore::embedded();
        Planet::ALL.iter().map(|p| crate::ties::transactions::planet_value(content, &ti4_model::id::PlanetId::new(p.as_str())) as u8).collect()
    })
}

/// The shape a deal option id names (the inverse of [`deal_option`]'s id), or `None` for an id the core has no shape
/// for (a free-form deal: the renderer's table names it, a Pride of Kenara planet card, an unknown note). `swap` names the two factions of the
/// Support swap, whose id does not.
#[must_use]
pub fn shape_of(id: &str, swap: Option<(ti4_engine_core::generated::Faction, ti4_engine_core::generated::Faction)>) -> Option<Shape> {
    use crate::ties::core_adapter::typed;
    use crate::ties::transactions::FREE_FORM_PREFIX;
    use ti4_engine_core::generated::{ActionCard, SecretObjective};
    let key = |text: &str| crate::ties::trade_write::note_key(text);
    let price = |text: &str| text.parse::<u8>().ok();
    let count = |text: &str| text.parse::<u16>().ok();
    let pair = |text: &str| {
        let (l, r) = text.split_once(':')?;
        Some((count(l)?, count(r)?))
    };
    if id.starts_with(FREE_FORM_PREFIX) {
        return None;
    }
    if let Some(rest) = id.strip_prefix("kp").or_else(|| id.strip_prefix("kq")) {
        let (cards, p) = rest.rsplit_once(':')?;
        let (planet, to) = cards.split_once('>')?;
        let (planet, to) = (typed::<Planet>(planet)?, typed::<Planet>(to)?);
        let value = planet_values().get(planet.index()).copied()?;
        let price = price(p)?;
        return Some(if id.starts_with("kp") { Shape::PlanetSale { planet, to, price, value } } else { Shape::PlanetBuy { planet, to, price, value } });
    }
    if id == "ss" {
        let (from, to) = swap?;
        return Some(Shape::SupportSwap { from, to });
    }
    if let Some(rest) = id.strip_prefix("ac") {
        let (card, p) = rest.split_once(':')?;
        return (p == "1").then_some(Shape::SellCard { card: typed::<ActionCard>(card)? });
    }
    if let Some(rest) = id.strip_prefix("pn") {
        let (note, p) = rest.rsplit_once(':')?;
        return Some(Shape::Sell { note: key(note)?, price: price(p)? });
    }
    if let Some(rest) = id.strip_prefix("np") {
        let (note, p) = rest.rsplit_once(':')?;
        return Some(Shape::BuyNote { note: key(note)?, price: price(p)? });
    }
    if let Some(rest) = id.strip_prefix("cp") {
        let (note, p) = rest.rsplit_once(':')?;
        return Some(Shape::BuyNoteWithCommodities { note: key(note)?, price: price(p)? });
    }
    if let Some(rest) = id.strip_prefix("pc") {
        let (note, p) = rest.rsplit_once(':')?;
        return Some(Shape::NoteForCommodities { note: key(note)?, paid: price(p)? });
    }
    if let Some(rest) = id.strip_prefix("nn") {
        let (owed, want) = rest.split_once('>')?;
        return Some(Shape::NoteForNote { owed: key(owed)?, want: key(want)? });
    }
    if let Some(rest) = id.strip_prefix("cn") {
        let (card, note) = rest.split_once('>')?;
        return Some(Shape::CardForNote { card: typed::<ActionCard>(card)?, note: key(note)? });
    }
    if let Some(rest) = id.strip_prefix("so") {
        let (secret, p) = rest.split_once(':')?;
        return (p == "1").then_some(Shape::SellSecret { secret: typed::<SecretObjective>(secret)? });
    }
    if let Some(rest) = id.strip_prefix("fr") {
        let (kind, p) = rest.split_once(':')?;
        let kind = [ExploreType::Cultural, ExploreType::Frontier, ExploreType::Hazardous, ExploreType::Industrial].into_iter().find(|k| kind_text(*k) == kind)?;
        return (p == "1").then_some(Shape::SellFragment { kind });
    }
    if let Some(rest) = id.strip_prefix("cc") {
        return Some(Shape::Swap { n: count(rest)? });
    }
    if let Some(rest) = id.strip_prefix("ct") {
        let (give, want) = pair(rest)?;
        return Some(Shape::CommoditiesForGoods { give, want });
    }
    if let Some(rest) = id.strip_prefix("tc") {
        let (give, want) = pair(rest)?;
        return Some(Shape::GoodsForCommodities { give, want });
    }
    if let Some(rest) = id.strip_prefix('c') {
        let (n, zero) = pair(rest)?;
        return (zero == 0).then_some(Shape::GiftCommodities { n });
    }
    let (give, want) = pair(id)?;
    Some(Shape::Goods { give, want })
}

/// [`deal_option`] with the renderer's table: a free-form deal reads its id from it (it is never listed; the option is what the
/// proposer's answer is compared as).
#[must_use]
pub fn deal_option_in(r: &Render, shape: Shape) -> ChoiceOption {
    match shape {
        Shape::Free(i) => {
            let id = r.free_id(i);
            ChoiceOption::labelled(id.clone(), OFFER_KIND, id)
        }
        _ => deal_option(shape),
    }
}

/// The option the string engine lists for a deal (`offer_options`): id, label, payload and prices.
#[must_use]
pub fn deal_option(shape: Shape) -> ChoiceOption {
    let mut payload = std::collections::BTreeMap::<String, Value>::new();
    let text = |s: &str| Value::String(s.to_owned());
    let (id, label) = match shape {
        Shape::SupportSwap { .. } => ("ss".to_owned(), "exchange Support for the Throne notes".to_owned()),
        Shape::Sell { note, price } => {
            let n = note_id(note);
            payload.insert("note".to_owned(), text(&n));
            payload.insert("alias".to_owned(), text(note_text(note.note)));
            if price == 0 {
                payload.insert("gift".to_owned(), Value::Bool(true));
                (format!("pn{n}:0"), format!("give {n}"))
            } else {
                (format!("pn{n}:{price}"), format!("sell {n} for {price} trade goods"))
            }
        }
        Shape::SellCard { card } => {
            let c = card.as_str();
            payload.insert("action_card".to_owned(), text(c));
            (format!("ac{c}:1"), format!("sell the action card {c} for 1 trade good"))
        }
        Shape::CardForNote { card, note } => {
            let (c, n) = (card.as_str(), note_id(note));
            payload.insert("action_card".to_owned(), text(c));
            payload.insert("promissory".to_owned(), text(&n));
            (format!("cn{c}>{n}"), format!("give the action card {c} for the note {n}"))
        }
        Shape::SellSecret { secret } => {
            let s = secret.as_str();
            payload.insert("secret".to_owned(), text(s));
            (format!("so{s}:1"), format!("sell the secret objective {s} for 1 trade good"))
        }
        Shape::SellFragment { kind } => {
            let k = kind_text(kind);
            payload.insert("fragment".to_owned(), text(k));
            (format!("fr{k}:1"), format!("sell a {k} relic fragment for 1 trade good"))
        }
        Shape::Swap { n } => (format!("cc{n}"), format!("swap {n} commodities each -- both gain")),
        Shape::SwapOne => ("cc1".to_owned(), "swap 1 commodity each".to_owned()),
        Shape::CommoditiesForGoods { give, want } => (format!("ct{give}:{want}"), format!("give {give} commodities for {want} trade goods")),
        Shape::GoodsForCommodities { give, want } => (format!("tc{give}:{want}"), format!("give {give} trade goods for {want} commodities")),
        Shape::Goods { give, want } => (format!("{give}:{want}"), format!("give {give} for {want}")),
        Shape::GiftCommodities { n } => (format!("c{n}:0"), format!("gift {n} commodities")),
        Shape::BuyNote { note, price } => {
            let n = note_id(note);
            payload.insert("received_promissory".to_owned(), text(&n));
            (format!("np{n}:{price}"), format!("pay {price} trade goods for the note {n}"))
        }
        Shape::BuyNoteWithCommodities { note, price } => {
            let n = note_id(note);
            payload.insert("received_promissory".to_owned(), text(&n));
            (format!("cp{n}:{price}"), format!("pay {price} commodities for the note {n}"))
        }
        Shape::NoteForCommodities { note, paid } => {
            let n = note_id(note);
            payload.insert("promissory".to_owned(), text(&n));
            payload.insert("alias".to_owned(), text(note_text(note.note)));
            (format!("pc{n}:{paid}"), format!("give the note {n} for {paid} commodity"))
        }
        Shape::NoteForNote { owed, want } => {
            let (o, w) = (note_id(owed), note_id(want));
            payload.insert("promissory".to_owned(), text(&o));
            payload.insert("received_promissory".to_owned(), text(&w));
            (format!("nn{o}>{w}"), format!("give the note {o} for the note {w}"))
        }
        Shape::PlanetSale { planet, to, price, .. } => {
            let (p, t) = (planet.as_str(), to.as_str());
            payload.insert("planet".to_owned(), text(p));
            payload.insert("moves_to".to_owned(), text(t));
            if price == 0 {
                (format!("kp{p}>{t}:0"), format!("give the planet {p}, moving the units to {t}"))
            } else {
                (format!("kp{p}>{t}:{price}"), format!("sell the planet {p} for {price} trade goods, units to {t}"))
            }
        }
        Shape::PlanetBuy { planet, to, price, .. } => {
            let (p, t) = (planet.as_str(), to.as_str());
            payload.insert("received_planet".to_owned(), text(p));
            (format!("kq{p}>{t}:{price}"), format!("pay {price} trade goods for the planet {p}, their units to {t}"))
        }
        // A free-form deal is named by its proposer, never listed: `deal_option_in` writes its id from the renderer's table.
        Shape::Free(_) => (FREE_FORM_PREFIX.to_owned(), "a free-form deal".to_owned()),
    };
    let (net, their_net) = nets(shape);
    payload.insert("net".to_owned(), Value::from(net));
    payload.insert("their_net".to_owned(), Value::from(their_net));
    let mut option = ChoiceOption::labelled(id, OFFER_KIND, label);
    option.payload.extend(payload);
    option
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::Propose { name, .. } => format!("transaction with {}", name.as_str()),
            Ask::Answer { proposer, from, deal, .. } => {
                // "{faction} gives A for B", read back from the deal's own id so the two cannot drift.
                let me = r.player(*proposer);
                let other = ti4_model::id::PlayerId::new("\u{0}partner");
                // The partner's faction matters only to the Support swap, which names both.
                let to = match deal {
                    Shape::SupportSwap { to, .. } => to.as_str(),
                    _ => "",
                };
                let names = |who: &ti4_model::id::PlayerId| if *who == me { from.as_str().to_owned() } else { to.to_owned() };
                let offer = crate::ties::transactions::offer_from_with(&names, &deal_option_in(r, *deal).id, &me, &other)
                    .expect("a core deal is an id the string engine reads");
                format!("{} -- accept?", offer.describe_as(from.as_str()))
            }
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::Propose => "propose_transaction",
            Subtype::Answer => "answer_transaction",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Deal(shape) => deal_option_in(r, *shape),
            Opt::Accept => {
                let Ask::Answer { deal, .. } = a else { panic!("accept in a proposal") };
                let net = match deal {
                    // The string's own number: a free-form deal is priced where it was parsed.
                    Shape::Free(i) => r.free_net(*i),
                    _ => nets(*deal).1,
                };
                ChoiceOption::labelled("accept", ANSWER_KIND, "accept").with("net", net)
            }
            Opt::Refuse => ChoiceOption::labelled("refuse", DECLINE_KIND, "refuse"),
            Opt::Counter => ChoiceOption::labelled("counter", ANSWER_KIND, "counter-offer"),
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::Answer { .. } => match id {
                "refuse" => Some(Opt::Refuse),
                "counter" => Some(Opt::Counter),
                "accept" => Some(Opt::Accept),
                _ => None,
            },
            // A listed deal is read back by matching the id against the options of this very question
            // (`dual_window::drive_positioned` does); a free-form one by the table the adapter registered it in.
            Ask::Propose { .. } => id.starts_with(FREE_FORM_PREFIX).then(|| r.free_index(id)).flatten().map(|i| Opt::Deal(Shape::Free(i))),
        }
    }

    fn details(_: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        if matches!(a, Ask::Propose { .. }) {
            out.insert("free_form".to_owned(), FREE_FORM_PREFIX.into());
        }
        out
    }

    fn free_form(a: &Ask) -> Option<String> {
        matches!(a, Ask::Propose { .. }).then(|| FREE_FORM_PREFIX.to_owned())
    }
}
