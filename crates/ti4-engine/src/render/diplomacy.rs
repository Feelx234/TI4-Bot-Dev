//! Renderer of the `diplomacy` area: `impl Family` for `ti4_engine_core::choice::diplomacy` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`). Package B4d.
//!
//! Every option is written as the string engine writes it (`diplomacy::{builder, window}`): its id, kind, label and
//! payload. Three facts the question does not carry are read from the [`View`] of the step being rendered (set by
//! `diplomacy_window_dual` around one step): the round (a pledge names its deadline in its id), what the giver
//! holds (the `now` labels), and the draft (the prompt's summary, the `remove` labels, the draft payload of every
//! building option, the bundle the accept option reports). They are the window's own values, converted to the
//! string's types; nothing here decides a rule.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;
use ti4_engine_core::choice::diplomacy::{Ask, Counted, Item, Opt, Step, Subtype};
use ti4_engine_core::ids::Seat;
use ti4_model::diplomacy as md;

use super::trade::note_id;
use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::core_sync::Ctx;
use crate::diplomacy::builder::{
    AMOUNT_KIND, CANCEL_ID, Counted as SCounted, DONE_ID, Draft, EDIT_ID, ITEM_KIND, PROPOSE_ID, Pending, REVIEW_KIND, describe,
};
use crate::diplomacy::window::{ACCEPT_ID, COUNTER_ID, COUNTER_KIND, DECLINE_ID, RESPONSE_KIND, signal_option};

/// What a step is rendered in (see the module docs).
#[derive(Debug, Clone)]
pub struct View {
    pub round: u32,
    /// How many of each counted kind the giver of the side being edited holds (`Counted::ALL` order).
    pub held: [u8; 6],
    pub cx: Ctx,
    pub draft: Option<Draft>,
    /// The bundle on the table as the string serializes it.
    pub bundle: Option<Value>,
    pub signals: Vec<md::SignalStatement>,
    pub proposer: Seat,
}

thread_local! {
    static VIEW: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
}

/// Run `f` with `view` as the view of every question rendered in it.
pub fn with_view<T>(view: View, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Rc<View>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            VIEW.with(|v| *v.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(VIEW.with(|v| v.borrow_mut().replace(Rc::new(view))));
    f()
}

fn view() -> Rc<View> {
    VIEW.with(|v| v.borrow().clone()).expect("a diplomacy question is rendered inside a view")
}

/// The id of a note in a diplomacy option: `{alias}:{faction}`, and a Support for the Throne is `support:{faction}`
/// (`promissory::support`), which sorts among the others by that text.
#[must_use]
pub fn dip_note_id(key: ti4_engine_core::state::NoteKey) -> String {
    if key.note == ti4_engine_core::generated::PromissoryNote::SupportForTheThrone {
        format!("support:{}", crate::generated::IdStr::as_str(key.faction))
    } else {
        note_id(key)
    }
}

/// The string's counted kind.
#[must_use]
pub const fn counted(kind: Counted) -> SCounted {
    match kind {
        Counted::TradeGoods => SCounted::TradeGoods,
        Counted::Commodities => SCounted::Commodities,
        Counted::Cultural => SCounted::Cultural,
        Counted::Hazardous => SCounted::Hazardous,
        Counted::Industrial => SCounted::Industrial,
        Counted::Unknown => SCounted::Unknown,
    }
}

/// The window's draft as the string's `Draft` (the payload of every building option).
///
/// # Errors
/// A seat or a term the tables cannot write.
pub fn string_draft(cx: &Ctx, d: &ti4_engine_core::query::diplomacy_items::Draft) -> Result<Draft, crate::core_sync::Unmapped> {
    let seat = |s: Seat| cx.owners.id(s).map(ti4_model::id::PlayerId::new).ok_or_else(|| crate::core_sync::Unmapped { slice: ti4_engine_core::state::Slice::Trade, what: "diplomacy: a seat the table does not know", text: String::new() });
    let terms = |l: &[ti4_engine_core::state::DealTerm]| l.iter().map(|t| crate::core_sync::codec::trade::term_back(cx, t)).collect::<Result<Vec<_>, _>>();
    Ok(Draft {
        builder: seat(d.builder)?,
        other: seat(d.other)?,
        give: terms(d.give.as_slice())?,
        take: terms(d.take.as_slice())?,
        asking: d.asking,
        reviewing: d.reviewing,
        pending: d.pending.map(|(kind, later)| Pending { kind: counted(kind), later }),
        steps: d.steps,
    })
}

fn list(terms: &[md::DealTerm]) -> String {
    if terms.is_empty() { "nothing".to_owned() } else { terms.iter().map(describe).collect::<Vec<_>>().join(", ") }
}

fn summary(v: &View) -> String {
    v.draft.as_ref().map_or_else(String::new, |d| format!("you give: {} · you ask: {}", list(&d.give), list(&d.take)))
}

fn with_draft(v: &View, option: ChoiceOption) -> ChoiceOption {
    match &v.draft {
        Some(d) => option.with("draft", serde_json::to_value(d).expect("a draft serializes")),
        None => option,
    }
}

/// The renderer of the structured negotiation window.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        let v = view();
        match a {
            Ask::Build { step: Step::Give, .. } => format!("Offer what? ({})", summary(&v)),
            Ask::Build { step: Step::Take, .. } => format!("Ask for what? ({})", summary(&v)),
            Ask::Amount { .. } => format!("How many? ({})", summary(&v)),
            Ask::Review { .. } => format!("Review: {}", summary(&v)),
            Ask::Respond { .. } => "Respond to the structured diplomatic offer".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::OfferItem => "diplomacy_offer_item",
            Subtype::AskItem => "diplomacy_ask_item",
            Subtype::Amount => "diplomacy_amount",
            Subtype::Review => "diplomacy_review",
            Subtype::Response => "diplomacy_response",
        }
        .to_owned()
    }

    #[allow(clippy::too_many_lines)]
    fn option(_: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        let v = view();
        let id_of = |k: Counted| counted(k).id();
        let option = match (a, o) {
            (Ask::Build { builder, other, step }, Opt::Item(item)) => {
                let asking = *step == Step::Take;
                let receiver = if asking { *builder } else { *other };
                let verb = if asking { "ask for" } else { "give" };
                match *item {
                    Item::Now(k) => ChoiceOption::labelled(
                        format!("diplomacy|now|{}", id_of(k)),
                        ITEM_KIND,
                        format!("{verb} {} now ({} held)", counted(k).label(), v.held[Counted::ALL.iter().position(|c| *c == k).unwrap_or(0)]),
                    ),
                    Item::Later(k) => ChoiceOption::labelled(
                        format!("diplomacy|later|{}", id_of(k)),
                        ITEM_KIND,
                        format!("{verb} {} by the end of round {}", counted(k).label(), v.round.saturating_add(1)),
                    ),
                    Item::Note(key) => {
                        let n = dip_note_id(key);
                        ChoiceOption::labelled(format!("diplomacy|note|{n}"), ITEM_KIND, format!("{verb} promissory note {n}"))
                    }
                    Item::Card(card) => {
                        let c = crate::generated::IdStr::as_str(card);
                        ChoiceOption::labelled(format!("diplomacy|card|{c}"), ITEM_KIND, format!("give action card {c}"))
                    }
                    Item::Pledge(p) => {
                        let term = ti4_engine_core::query::diplomacy_items::term_of(p, v.round, receiver);
                        let term = crate::core_sync::codec::trade::term_back(&v.cx, &term).expect("a pledge names seats of the table");
                        let id = format!("diplomacy|promise|{}", serde_json::to_string(&term).expect("a deal term serializes"));
                        let label = format!("{} {}", if asking { "ask:" } else { "promise:" }, describe(&term));
                        ChoiceOption::labelled(id, ITEM_KIND, label).with("term", serde_json::to_value(&term).expect("a deal term serializes"))
                    }
                }
            }
            (Ask::Build { step, .. }, Opt::Remove(i)) => {
                let d = v.draft.as_ref().expect("a build has a draft");
                let terms = if *step == Step::Take { &d.take } else { &d.give };
                let term = terms.get(usize::from(*i)).expect("a removal names a term of the side");
                ChoiceOption::labelled(format!("diplomacy|remove|{i}"), ITEM_KIND, format!("remove: {}", describe(term)))
            }
            (Ask::Build { step, .. }, Opt::Done) => {
                ChoiceOption::labelled(DONE_ID, ITEM_KIND, if *step == Step::Take { "done asking" } else { "done offering" })
            }
            (Ask::Build { .. }, Opt::Signal(i)) => {
                let statement = v.signals.get(usize::from(*i)).expect("a signal of the contact");
                signal_option(statement, v.round)
            }
            (Ask::Build { .. }, Opt::Cancel) => ChoiceOption::labelled(CANCEL_ID, REVIEW_KIND, "Make no offer"),
            (Ask::Amount { kind, .. }, Opt::Amount(n)) => {
                ChoiceOption::labelled(format!("diplomacy|amount|{n}"), AMOUNT_KIND, format!("{n} {}", counted(*kind).label())).with("amount", i64::from(*n))
            }
            (Ask::Review { .. }, Opt::Propose) => ChoiceOption::labelled(PROPOSE_ID, REVIEW_KIND, "Propose these terms"),
            (Ask::Review { .. }, Opt::Edit) => ChoiceOption::labelled(EDIT_ID, REVIEW_KIND, "Edit the terms"),
            (Ask::Review { back, .. }, Opt::Cancel) => {
                ChoiceOption::labelled(CANCEL_ID, REVIEW_KIND, if *back { "Back to the offer on the table" } else { "Make no offer" })
            }
            (Ask::Respond { actor, .. }, Opt::Accept) => {
                let accept = ChoiceOption::labelled(ACCEPT_ID, RESPONSE_KIND, "Accept");
                match &v.bundle {
                    Some(b) => accept.with("bundle", b.clone()).with("actor_is_proposer", *actor == v.proposer),
                    None => accept,
                }
            }
            (Ask::Respond { .. }, Opt::Decline) => ChoiceOption::labelled(DECLINE_ID, RESPONSE_KIND, "Decline"),
            (Ask::Respond { .. }, Opt::Counter) => ChoiceOption::labelled(COUNTER_ID, COUNTER_KIND, "Counter: change the terms"),
            _ => panic!("option {o:?} in the question {a:?}"),
        };
        // Every option of a build, an amount and a review carries the draft.
        if matches!(a, Ask::Respond { .. }) { option } else { with_draft(&v, option) }
    }

    fn parse(_: &Render, _: &Ask, _: &str) -> Option<Opt> {
        // An id is read back by matching it against the options of the question (`dual_window::drive`).
        None
    }
}

/// The bundle payload of a revision on the table, as `window::bundle_payload` serializes it.
#[must_use]
pub fn bundle_value(revision: md::DealRevision) -> Value {
    serde_json::to_value(crate::diplomacy::candidates::built_bundle(revision)).expect("candidate serializes")
}
