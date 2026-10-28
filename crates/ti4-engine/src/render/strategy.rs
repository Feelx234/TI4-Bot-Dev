//! Renderer of the `strategy` area: `impl Family` for `ti4_engine_core::choice::strategy` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B8 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Trade primary's "let another player
//! replenish commodities" (subtype `trade_choose_replenish`, option id the seat's faction text, kind
//! `replenish`, then the generic done worded "nobody else replenishes"). The display details (`seats`,
//! `commodities`) come from the state and are added by the caller of the window (`strategy_core`).

use ti4_engine_core::choice::strategy::{Ask, Opt, Subtype};
use ti4_engine_core::choice::Opt as AnyOpt;

use super::{Family, Render, Wording};
use crate::choice::{ChoiceOption, DECLINE_KIND};

/// The renderer of the strategy card questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::TradeReplenish => "let another player replenish commodities".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::TradeReplenish => "trade_choose_replenish".to_owned(),
        }
    }

    fn option(r: &Render, _: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Replenish(seat) => {
                let name = r.faction_text(*seat);
                ChoiceOption::labelled(name, "replenish", format!("{name} replenishes commodities"))
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::TradeReplenish => r
                .factions
                .iter()
                .position(|f| !f.is_empty() && f == id)
                .and_then(|i| u8::try_from(i).ok())
                .map(|i| Opt::Replenish(ti4_engine_core::ids::Seat(i))),
        }
    }

    fn wording(_: &Render, a: &Ask, o: &AnyOpt) -> Wording {
        match (a, o) {
            (Ask::TradeReplenish, AnyOpt::Done) => Wording {
                kind: Some(DECLINE_KIND.to_owned()),
                label: Some("nobody else replenishes".to_owned()),
                ..Wording::default()
            },
            _ => Wording::default(),
        }
    }
}
