//! A seat's "never offer me this card" preference, applied where the question is asked.
//!
//! The preference is a *session* setting, not game state: it is not in [`GameState`], not in a
//! fingerprint and not in the decision log's inputs. It is applied by [`NeverOffer`], a decider
//! wrapper that sits between the table and the seat's real decider. A window whose every card the
//! seat has set to Never is declined without asking; a window with some other card left is asked
//! with the Never cards left out. Either way the table records an ordinary answer, so a replay
//! (which answers from the recorded log and never reaches this wrapper) reproduces the game
//! whatever the preference is at replay time.
//!
//! Cards are matched by printed name, so every copy of Sabotage is covered at once.
//!
//! [`GameState`]: ti4_model::state::GameState

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ti4_engine::choice::{Choice, ChoiceOption, Decider, IllegalChoice, SeatObservation};

/// The card names a seat has asked never to be offered. Shared with whoever changes it.
pub type NeverSet = Arc<Mutex<BTreeSet<String>>>;

/// Called with the declined choice and the card names that made it a skip.
pub type SkipObserver = Box<dyn FnMut(&Choice, &[String]) + Send>;

/// What the preference does to one question.
#[derive(Debug, Clone, PartialEq)]
pub enum Filtered {
    /// Nothing in it concerns a Never card.
    Unchanged,
    /// Some options were Never cards; the rest still need an answer.
    Narrowed(Choice),
    /// Every reaction option was a Never card and a decline exists: answer it, do not ask.
    Declined {
        /// The decline option of the original choice.
        option: ChoiceOption,
        /// The card names that were left out, for the note to the player.
        cards: Vec<String>,
    },
}

/// The card names an option stands for, when it offers an action card to play.
///
/// A window slot that holds several cards names them all in `card_names`; one holding a single
/// printed name carries it in `card_name`; the inner "which card" step has one name per option.
fn card_names(option: &ChoiceOption) -> Vec<String> {
    let listed: Vec<String> = option
        .payload
        .get("card_names")
        .and_then(serde_json::Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if !listed.is_empty() {
        return listed;
    }
    option
        .payload
        .get("card_name")
        .and_then(serde_json::Value::as_str)
        .map(|name| vec![name.to_owned()])
        .unwrap_or_default()
}

fn is_card_offer(option: &ChoiceOption) -> bool {
    (option.kind == "ability" && option.id.starts_with("reaction:"))
        || option.kind == ti4_engine::reactions::ACTION_CARD_KIND
}

/// Apply a seat's Never set to a question.
#[must_use]
pub fn filter(choice: &Choice, never: &BTreeSet<String>) -> Filtered {
    if never.is_empty() {
        return Filtered::Unchanged;
    }
    let mut dropped: Vec<String> = Vec::new();
    let mut kept: Vec<ChoiceOption> = Vec::new();
    for option in &choice.options {
        let names = card_names(option);
        if is_card_offer(option) && !names.is_empty() && names.iter().all(|n| never.contains(n)) {
            for name in names {
                if !dropped.contains(&name) {
                    dropped.push(name);
                }
            }
        } else {
            kept.push(option.clone());
        }
    }
    if dropped.is_empty() {
        return Filtered::Unchanged;
    }
    if kept.iter().all(ChoiceOption::is_decline) {
        return match kept.into_iter().next() {
            Some(option) => Filtered::Declined {
                option,
                cards: dropped,
            },
            // Nothing left and nothing to decline with: ask the question whole.
            None => Filtered::Unchanged,
        };
    }
    let mut narrowed = choice.clone();
    narrowed.options = kept;
    Filtered::Narrowed(narrowed)
}

/// A decider wrapper that honours a seat's Never set before delegating.
pub struct NeverOffer {
    inner: Box<dyn Decider>,
    never: NeverSet,
    on_skip: Option<SkipObserver>,
}

impl NeverOffer {
    #[must_use]
    pub fn new(inner: Box<dyn Decider>, never: NeverSet) -> Self {
        Self {
            inner,
            never,
            on_skip: None,
        }
    }

    /// Be told whenever a window is declined without asking.
    #[must_use]
    pub fn on_skip(mut self, observer: impl FnMut(&Choice, &[String]) + Send + 'static) -> Self {
        self.on_skip = Some(Box::new(observer));
        self
    }

    fn current(&self) -> BTreeSet<String> {
        self.never.lock().expect("never set lock").clone()
    }
}

impl Decider for NeverOffer {
    fn choose(&mut self, choice: &Choice) -> Result<ChoiceOption, IllegalChoice> {
        match filter(choice, &self.current()) {
            Filtered::Unchanged => self.inner.choose(choice),
            Filtered::Narrowed(narrowed) => self.inner.choose(&narrowed),
            Filtered::Declined { option, cards } => {
                if let Some(observer) = &mut self.on_skip {
                    observer(choice, &cards);
                }
                Ok(option)
            }
        }
    }

    fn choose_seeing(
        &mut self,
        choice: &Choice,
        seen: &SeatObservation<'_>,
    ) -> Result<ChoiceOption, IllegalChoice> {
        match filter(choice, &self.current()) {
            Filtered::Unchanged => self.inner.choose_seeing(choice, seen),
            Filtered::Narrowed(narrowed) => self.inner.choose_seeing(&narrowed, seen),
            Filtered::Declined { option, cards } => {
                if let Some(observer) = &mut self.on_skip {
                    observer(choice, &cards);
                }
                Ok(option)
            }
        }
    }

    fn stage_scores(&mut self, scores: Vec<(Option<f64>, Option<f64>)>) {
        self.inner.stage_scores(scores);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_model::id::PlayerId;

    fn card(id: &str, name: &str) -> ChoiceOption {
        ChoiceOption::labelled(id, ti4_engine::reactions::ACTION_CARD_KIND, format!("play {name}"))
            .with("card", id.to_owned())
            .with("card_name", name.to_owned())
    }

    fn slot(names: &[&str]) -> ChoiceOption {
        let mut option =
            ChoiceOption::labelled("reaction:x:EV:when", "ability", "reaction:x:EV:when");
        option
            .payload
            .insert("card_names".to_owned(), serde_json::json!(names));
        option
    }

    fn never(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    fn window(options: Vec<ChoiceOption>) -> Choice {
        Choice::new(PlayerId::new("b"), "when EV", options)
    }

    #[test]
    fn a_window_of_only_never_cards_is_declined_without_asking() {
        let choice = window(vec![slot(&["Sabotage"]), ChoiceOption::decline()]);
        match filter(&choice, &never(&["Sabotage"])) {
            Filtered::Declined { option, cards } => {
                assert!(option.is_decline());
                assert_eq!(cards, vec!["Sabotage".to_owned()]);
            }
            other => panic!("expected a skip, got {other:?}"),
        }
    }

    #[test]
    fn another_card_in_the_window_keeps_it_open_without_the_never_card() {
        let choice = window(vec![
            card("sab1", "Sabotage"),
            card("fs1", "Flank Speed"),
            ChoiceOption::decline(),
        ]);
        let Filtered::Narrowed(narrowed) = filter(&choice, &never(&["Sabotage"])) else {
            panic!("expected a narrowed question");
        };
        let ids: Vec<&str> = narrowed.options.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["fs1", "decline"]);
    }

    #[test]
    fn a_slot_holding_one_never_and_one_other_name_stays_offered() {
        let choice = window(vec![
            slot(&["Sabotage", "Flank Speed"]),
            ChoiceOption::decline(),
        ]);
        assert_eq!(filter(&choice, &never(&["Sabotage"])), Filtered::Unchanged);
    }

    #[test]
    fn non_card_options_and_empty_sets_are_untouched() {
        let other = ChoiceOption::labelled("instinct_training", "ability", "Instinct Training");
        let choice = window(vec![other, ChoiceOption::decline()]);
        assert_eq!(filter(&choice, &never(&["Sabotage"])), Filtered::Unchanged);
        let choice = window(vec![slot(&["Sabotage"]), ChoiceOption::decline()]);
        assert_eq!(filter(&choice, &never(&[])), Filtered::Unchanged);
    }

    #[test]
    fn the_wrapper_declines_and_reports_but_forwards_everything_else() {
        let skipped = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let sink = skipped.clone();
        let set: NeverSet = Arc::new(Mutex::new(never(&["Sabotage"])));
        let mut decider = NeverOffer::new(Box::new(ti4_engine::choice::FirstOption), set.clone())
            .on_skip(move |_, cards| sink.lock().unwrap().push(cards.to_vec()));
        let sabotage = window(vec![slot(&["Sabotage"]), ChoiceOption::decline()]);
        assert!(decider.choose(&sabotage).unwrap().is_decline());
        assert_eq!(skipped.lock().unwrap().len(), 1);
        // A different card is asked as usual: FirstOption takes the first option.
        let other = window(vec![slot(&["Flank Speed"]), ChoiceOption::decline()]);
        assert_eq!(decider.choose(&other).unwrap().id, "reaction:x:EV:when");
        assert_eq!(skipped.lock().unwrap().len(), 1);
        // Changing the set takes effect on the next question.
        set.lock().unwrap().clear();
        assert_eq!(decider.choose(&sabotage).unwrap().id, "reaction:x:EV:when");
    }
}
