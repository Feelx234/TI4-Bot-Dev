//! Renderer of the `trade` area: `impl Family` for `ti4_engine_core::choice::trade` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package B4.
//!
//! The proposing question is `"transaction with {faction}"` (subtype `propose_transaction`, free form
//! `x;`); its deals are interned option shapes (`serde_json` of the string `ChoiceOption`). The
//! answering question is `"{deal read aloud} -- accept?"` (subtype `answer_transaction`) with the
//! options `accept` (payload `net`), `refuse` and `counter`.

use ti4_engine_core::choice::trade::{Ask, Opt, Subtype};

use super::{Family, Render};
use crate::choice::{ChoiceOption, DECLINE_KIND};
use crate::transactions::{ANSWER_KIND, FREE_FORM_PREFIX};

/// The renderer of the transaction window.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::Propose { name, .. } => format!("transaction with {}", r.code_text(*name)),
            Ask::Answer { said, .. } => format!("{} -- accept?", r.code_text(*said)),
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
            Opt::Deal(code) => serde_json::from_str(&r.code_text(*code)).expect("an interned deal is a ChoiceOption"),
            Opt::Accept => {
                let Ask::Answer { net, .. } = a else { panic!("accept in a proposal") };
                let net: f64 = r.code_text(*net).parse().expect("an interned net is a number");
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
            Ask::Propose { .. } => {
                let count = r.codes.borrow().len();
                (0..count).find_map(|i| {
                    let code = ti4_engine_core::state::Code(u16::try_from(i).ok()?);
                    let option: ChoiceOption = serde_json::from_str(&r.code_text(code)).ok()?;
                    (option.id == id).then_some(Opt::Deal(code))
                })
            }
        }
    }

    fn free_form(a: &Ask) -> Option<String> {
        matches!(a, Ask::Propose { .. }).then(|| FREE_FORM_PREFIX.to_owned())
    }
}
