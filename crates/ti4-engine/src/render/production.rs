//! Renderer of the `production` area: `impl Family` for `ti4_engine_core::choice::production` (prompt, option,
//! parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B3. Ported: the payment question (`pay` options: exhaust a planet face, spend a trade good,
//! spend a commodity), byte for byte as `production::payment_options` lists them.

use ti4_engine_core::choice::production::{Ask, Currency, Opt, PayAsk, Shown, Subtype};
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
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::Pay(kind) => format!("pay_{}", name(*kind)),
        }
    }

    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let Ask::Pay(PayAsk { kind, owed, pool, goods }) = *a;
        match *o {
            Opt::Exhaust { planet, source, shown: Shown { worth, pool_after } } => {
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
            Opt::TradeGood { shown: Shown { worth, .. } } => ChoiceOption::labelled("trade_good", crate::production::PAY_KIND, "spend a trade good")
                .with("worth", worth)
                .with("owed", owed)
                .with("kind", name(kind))
                .previewed(Preview::certain(vec![
                    Delta::new(quantity(kind), pool, pool - worth),
                    Delta::new(Quantity::TradeGoods, goods, goods - 1),
                ])),
            Opt::Commodity { shown: Shown { worth, .. } } => ChoiceOption::labelled("commodity", crate::production::PAY_KIND, "spend a commodity as a trade good")
                .with("worth", worth)
                .with("owed", owed)
                .with("kind", name(kind))
                .previewed(Preview::certain(vec![Delta::new(quantity(kind), pool, pool - worth)])),
        }
    }

    fn parse(_: &Render, a: &Ask, id: &str) -> Option<Opt> {
        let Ask::Pay(PayAsk { kind, .. }) = *a;
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
