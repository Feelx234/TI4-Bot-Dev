//! Renderer of the `cards` area: `impl Family` for `ti4_engine_core::choice::cards` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B6 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the own-hand question of
//! `action_cards::choose_from_own_hand`, byte for byte (prompt, subtype, kind `own_hand_card`, label = the
//! printed name).

use ti4_content::ContentStore;
use ti4_engine_core::choice::cards::{Ask, Opt, Reason, Subtype, What};
use ti4_engine_core::generated::{ActionCard, Planet, System};
use ti4_model::id::ActionCardId;

use super::{Family, Render};
use crate::choice::ChoiceOption;
use crate::generated::IdStr;

/// The renderer of the card questions.
pub struct Fam;

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::OwnHand { reason } => match reason {
                Reason::StallTactics => "Stall Tactics: discard 1 action card from your hand".to_owned(),
                Reason::DeepgloomStallTactics => {
                    "Stall Tactics (allowed by Deepgloom Executable): discard 1 action card".to_owned()
                }
                Reason::MalleonDiscard => "Nekro Malleon: discard 1 action card".to_owned(),
                Reason::C4rDiscard => "???_ERROR_ERROR_???: discard 1 action card".to_owned(),
                Reason::Show { viewer } => format!("show {} 1 action card from your hand", r.seat_text(*viewer)),
            },
            Ask::Pick { what: What::Crash { system, second }, .. } => format!("Crash Landing: choose a {} in {}", if *second { "planet" } else { "ground force" }, system.as_str()),
            Ask::Pick { what: What::Squad { system }, .. } => format!("Ghost Squad: move ground forces in {}", system.as_str()),
            Ask::Pick { what: What::JamWho { system }, .. } => format!("Signal Jamming: whose token goes into {}", system.as_str()),
            Ask::Pick { card, what } => pick_prompt(*card, *what).to_owned(),
            Ask::Predict { card, .. } => format!("{}: predict the agenda outcome", rider_name(*card)),
            Ask::PlayCard { relation, event } => {
                format!("play an action card ({} {})", crate::kernel_sync::text::relation_text(*relation), crate::kernel_sync::event_text(r, *event))
            }
        }
    }

    fn subtype(r: &Render, s: &Subtype) -> String {
        match s {
            Subtype::PlayReaction { relation, event } => {
                return format!("play_reaction_{}_{}", crate::kernel_sync::text::relation_text(*relation), crate::kernel_sync::event_text(r, *event));
            }
            Subtype::StallTacticsDiscard => "stall_tactics_discard",
            Subtype::DeepgloomStallTacticsDiscard => "deepgloom_stall_tactics_discard",
            Subtype::MalleonDiscard => "malleon_discard",
            Subtype::C4rDiscard => "c4r_discard",
            Subtype::ShowActionCard => "show_action_card",
            Subtype::PredictOutcome => "predict_agenda_outcome",
            Subtype::Pick { card, what } => {
                return match what {
                    What::Elect => "confusing_legal_text_elect".to_owned(),
                    What::Squad { .. } => "ghost_squad_move".to_owned(),
                    What::Crash { second: false, .. } => "crashlanding_choose_ground".to_owned(),
                    What::Crash { second: true, .. } => "crashlanding_choose_planet".to_owned(),
                    What::Silence => "silence_choose_system".to_owned(),
                    What::Accept => "exchange_program_answer".to_owned(),
                    What::Exhaust => "reparations_exhaust".to_owned(),
                    What::Ready => "reparations_ready".to_owned(),
                    What::Redraft { .. } => "public_disgrace_choose_card".to_owned(),
                    _ => format!("{}_pick_{}", card.as_str(), pick_kind(*what)),
                };
            }
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Seat(seat) if matches!(pick_of(a).1, What::Gain) => {
                let id = r.seat_text(*seat);
                ChoiceOption::labelled(id, "player", format!("{id} gains their own breakthrough"))
            }
            Opt::Seat(seat) if matches!(pick_of(a).1, What::JamWho { .. }) => {
                let faction = r.faction_text(*seat);
                ChoiceOption::labelled(faction, "player", format!("{faction}'s command token"))
            }
            Opt::Seat(seat) => {
                let id = r.seat_text(*seat);
                let (card, what) = pick_of(a);
                ChoiceOption::labelled(id, pick_kind(what), format!("{}{id}", seat_label(card, what)))
            }
            Opt::System(system) => {
                let id = system.as_str();
                match pick_of(a).1 {
                    What::Silence => ChoiceOption::labelled(id, "silence", format!("ships in {id} may pass blockades")),
                    What::Bombard => ChoiceOption::labelled(id, "system", format!("bombard {id}")),
                    What::Rescue => ChoiceOption::labelled(id, "rescue_from", format!("a ship from {id}")).with("system", id),
                    What::Deploy => ChoiceOption::labelled(id, "system", format!("cruiser into {id}")),
                    What::Pirate => ChoiceOption::labelled(id, "system", id),
                    What::Ghost => ChoiceOption::labelled(id, "system", format!("destroyer into {id}")),
                    What::Jam => ChoiceOption::labelled(id, "system", format!("jam {id}")),
                    _ => ChoiceOption::labelled(id, "recall", format!("recall your token from {id}")),
                }
            }
            Opt::Law(law) => ChoiceOption::labelled(law.as_str(), "repeal", law.as_str()),
            Opt::Ship { system, index, kind, .. } if matches!(pick_of(a).1, What::Cruiser) => {
                ChoiceOption::labelled(index.to_string(), "cruiser", kind.as_str()).with("system", system.as_str())
            }
            Opt::Ship { system, index, owner, kind } => {
                let id = format!("{}|{index}", system.as_str());
                let label = match pick_of(a).1 {
                    What::Hull => format!("destroy {}'s {} in {}", r.seat_text(*owner), kind.as_str(), system.as_str()),
                    _ => format!("{} in {}", kind.as_str(), system.as_str()),
                };
                ChoiceOption::labelled(id, "ship", label).with("system", system.as_str())
            }
            Opt::Stop if matches!(pick_of(a).1, What::Pull { .. }) => ChoiceOption::labelled("stop", "unit", "stop after one"),
            Opt::Stop if matches!(pick_of(a).1, What::Troops { .. }) => ChoiceOption::labelled("stop", "infantry", "stop after one"),
            Opt::Stop => ChoiceOption::labelled("stop", "ship", "stop after one"),
            Opt::Kind(kind) => {
                let id = kind.as_str();
                let name = ContentStore::embedded()
                    .get(ti4_model::content_types::ContentType::Units, id)
                    .and_then(|record| record.text("name").map(str::to_owned))
                    .unwrap_or_else(|| id.to_owned());
                ChoiceOption::labelled(format!("ground|{id}"), "crashlanding_ground", name)
            }
            Opt::Spot(system, planet) if matches!(pick_of(a).1, What::Crash { .. }) => {
                let name = ti4_content::galaxy::planet(ContentStore::embedded(), planet.as_str(), ti4_model::content_types::FULL)
                    .and_then(|record| record.name())
                    .unwrap_or(planet.as_str())
                    .to_owned();
                ChoiceOption::labelled(format!("planet|{}", planet.as_str()), "crashlanding_planet", name).with_planet(planet.as_str(), Some(system.as_str()))
            }
            Opt::Count(n) => ChoiceOption::labelled(n.to_string(), "count", format!("spend {n} trade goods")),
            Opt::Tech(tech) => {
                let id = tech.as_str();
                let label = match pick_of(a).1 {
                    What::Focus => format!("research {id}"),
                    _ => crate::technology::name(ContentStore::embedded(), &ti4_model::id::TechnologyId::new(id)),
                };
                ChoiceOption::labelled(id, "technology", label)
            }
            Opt::Steal { from, tech } => {
                let owner = r.seat_text(*from);
                let name = crate::technology::name(ContentStore::embedded(), &ti4_model::id::TechnologyId::new(tech.as_str()));
                ChoiceOption::labelled(format!("{owner}|{}", tech.as_str()), "technology", format!("{owner}'s {name}"))
            }
            Opt::Move { from, to, kind, count } => {
                ChoiceOption::labelled(
                    format!("move|{}|{}|{}", from.as_str(), to.as_str(), kind.as_str()),
                    "move",
                    format!("move {count} {} from {} to {}", kind.as_str(), from.as_str(), to.as_str()),
                )
                .with("from", from.as_str())
                .with("to", to.as_str())
                .with("unit", kind.as_str())
                .with("count", usize::from(*count))
            }
            Opt::Troop { system, planet, index, kind } => {
                let id = format!("{}|{}|{index}", system.as_str(), planet.as_str());
                if matches!(pick_of(a).1, What::Pull { .. }) {
                    ChoiceOption::labelled(id, "unit", format!("{} in {}", kind.as_str(), system.as_str()))
                } else {
                    ChoiceOption::labelled(id, "infantry", format!("infantry in {}", system.as_str())).with("system", system.as_str())
                }
            }
            Opt::Answer(yes) => ChoiceOption::labelled(if *yes { "yes" } else { "no" }, "answer", if *yes { "accept" } else { "refuse" }),
            Opt::Fragment(kind) => {
                let id = kind.as_str().to_ascii_uppercase();
                ChoiceOption::labelled(id.clone(), "fragment", id)
            }
            Opt::Spot(system, planet) if matches!(pick_of(a).1, What::Offer | What::Landing) => ChoiceOption::labelled(
                format!("{}|{}", system.as_str(), planet.as_str()),
                "planet",
                format!("{} in {}", planet.as_str(), system.as_str()),
            )
            .with_planet(planet.as_str(), Some(system.as_str())),
            Opt::Spot(system, planet) if matches!(pick_of(a).1, What::Exhaust | What::Ready | What::Dig | What::Land) => {
                ChoiceOption::labelled(planet.as_str(), pick_kind(pick_of(a).1), planet.as_str()).with_planet(planet.as_str(), Some(system.as_str()))
            }
            Opt::Spot(system, planet) => ChoiceOption::labelled(format!("{}|{}", system.as_str(), planet.as_str()), "planet", planet.as_str())
                .with_planet(planet.as_str(), Some(system.as_str())),
            Opt::Strat(card) if matches!(pick_of(a).1, What::Redraft { .. }) => {
                let id = card.as_str();
                ChoiceOption::labelled(id, "strategy_card", crate::draft::strategy_card_label(ContentStore::embedded(), id))
            }
            Opt::Strat(card) => {
                let id = card.as_str();
                ChoiceOption::labelled(id, "strategy_card", format!("place a trade good on {id}"))
            }
            Opt::Note(key) => {
                let id = note_text(*key);
                ChoiceOption::labelled(id.clone(), "promissory note", id)
            }
            Opt::Outcome(outcome) => {
                let Ask::Predict { frame, .. } = a else { return ChoiceOption::labelled("", "", "") };
                let named = super::agenda::outcome_label(r, frame, *outcome);
                ChoiceOption::labelled(super::agenda::outcome_text(r, *outcome), "prediction", format!("predict {named}"))
            }
            Opt::Card(card) if matches!(pick_of(a).1, What::Lie { .. }) && matches!(a, Ask::Pick { .. }) => {
                let from = match pick_of(a).1 {
                    What::Lie { from } => r.seat_text(from).to_owned(),
                    _ => String::new(),
                };
                ChoiceOption::labelled(card.as_str(), "action_card", format!("take {} from {from}", card.as_str()))
            }
            Opt::Card(card) if matches!(a, Ask::PlayCard { .. }) => {
                let id = ActionCardId::new(card.as_str());
                let name = crate::action_cards::name_of(ContentStore::embedded(), &id);
                ChoiceOption::labelled(card.as_str(), crate::reactions::ACTION_CARD_KIND, format!("play {name}"))
                    .with("card", card.as_str())
                    .with("card_name", name)
            }
            Opt::Card(card) => {
                let id = ActionCardId::new(card.as_str());
                ChoiceOption::labelled(card.as_str(), crate::action_cards::OWN_HAND_KIND, crate::action_cards::name_of(ContentStore::embedded(), &id))
            }
        }
    }

    fn details(r: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
        let mut out = serde_json::Map::new();
        if let Ask::Pick { what: What::Redraft { first, by }, .. } = a {
            // Display only: the card that was taken away, and who played Public Disgrace.
            out.insert("first".to_owned(), first.as_str().into());
            out.insert("played_by".to_owned(), r.seat_text(*by).into());
        }
        if let Ask::Pick { what: What::Lie { from }, .. } = a {
            out.insert("from".to_owned(), r.seat_text(*from).into());
        }
        if let Ask::Pick { what: What::Invest { step, owed }, .. } = a {
            out.insert("step".to_owned(), (*step).into());
            out.insert("of".to_owned(), 5.into());
            out.insert("distinct_owed".to_owned(), (*owed).into());
        }
        out
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::OwnHand { .. } | Ask::PlayCard { .. } => ActionCard::from_id(id).map(Opt::Card),
            Ask::Pick { what: What::Player | What::Elect | What::Gain, .. } => r.seat(id).map(Opt::Seat),
            Ask::Pick { what: What::JamWho { .. }, .. } => (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(ti4_engine_core::Seat)).find(|s| r.faction_text(*s) == id).map(Opt::Seat),
            Ask::Pick { what: What::Recall | What::Silence | What::Bombard | What::Deploy | What::Rescue | What::Pirate | What::Ghost | What::Jam, .. } => System::from_id(id).map(Opt::System),
            Ask::Pick { what: What::Planet | What::Landing, .. } => {
                let (system, planet) = id.split_once('|')?;
                Some(Opt::Spot(System::from_id(system)?, Planet::from_id(planet)?))
            }
            Ask::Pick { what: What::Note, .. } => parse_note(id).map(Opt::Note),
            Ask::Pick { what: What::Invest { .. }, .. } => ti4_engine_core::generated::StrategyCard::from_id(id).map(Opt::Strat),
            Ask::Pick { what: What::Accept, .. } => match id {
                "yes" => Some(Opt::Answer(true)),
                "no" => Some(Opt::Answer(false)),
                _ => None,
            },
            Ask::Pick { what: What::Cruiser | What::Troops { .. } | What::Pull { .. } | What::Squad { .. } | What::Steal | What::Crash { .. }, .. } => None,
            Ask::Pick { what: What::Lie { .. }, .. } => ActionCard::from_id(id).map(Opt::Card),
            Ask::Pick { what: What::Spend, .. } => id.parse().ok().map(Opt::Count),
            Ask::Pick { what: What::Focus | What::Return | What::Funding | What::Prototype, .. } => ti4_engine_core::generated::Technology::from_id(id).map(Opt::Tech),
            Ask::Pick { what: What::Hull | What::Scuttle { .. }, .. } => {
                let (system, index) = id.split_once('|')?;
                // Only the id is read back; the owner and unit form are the question's.
                let _ = (System::from_id(system)?, index.parse::<u8>().ok()?);
                None
            }
            Ask::Pick { what: What::Law, .. } => ti4_engine_core::generated::Agenda::from_id(id).map(Opt::Law),
            Ask::Pick { what: What::Fragment, .. } => {
                ti4_engine_core::generated::ExploreType::ALL.iter().copied().find(|k| k.as_str().eq_ignore_ascii_case(id)).map(Opt::Fragment)
            }
            Ask::Pick { what: What::Exhaust | What::Ready | What::Dig | What::Offer | What::Land, .. } => {
                let planet = Planet::from_id(id)?;
                Some(Opt::Spot(planet.data().system?, planet))
            }
            Ask::Pick { what: What::Redraft { .. }, .. } => ti4_engine_core::generated::StrategyCard::from_id(id).map(Opt::Strat),
            Ask::Predict { frame, .. } => super::agenda::parse_outcome(r, frame, id).map(Opt::Outcome),
        }
    }
}

/// The `kind` of a pick question's options.
const fn pick_kind(what: What) -> &'static str {
    match what {
        What::Player => "player",
        What::Elect => "elect",
        What::Recall => "recall",
        What::Bombard | What::Deploy | What::Pirate | What::Ghost | What::Jam => "system",
        What::Silence => "silence",
        What::Note => "promissory note",
        What::Planet => "planet",
        What::Rescue => "rescue_from",
        What::Invest { .. } => "strategy_card",
        What::Law => "repeal",
        What::Hull | What::Scuttle { .. } => "ship",
        What::Cruiser => "cruiser",
        What::Troops { .. } => "infantry",
        What::Pull { .. } => "unit",
        What::Land => "planet",
        What::Squad { .. } => "move",
        What::Gain => "player",
        What::Crash { second: false, .. } => "crashlanding_ground",
        What::Crash { second: true, .. } => "crashlanding_planet",
        What::Spend => "count",
        What::Lie { .. } => "action_card",
        What::Steal | What::Focus | What::Return | What::Funding | What::Prototype => "technology",
        What::Offer | What::Landing => "planet",
        What::JamWho { .. } => "player",
        What::Accept => "answer",
        What::Fragment => "fragment",
        What::Exhaust => "reparations_exhaust",
        What::Ready => "reparations_ready",
        What::Dig => "planet",
        What::Redraft { .. } => "strategy_card",
    }
}

fn pick_of(a: &Ask) -> (ActionCard, What) {
    match a {
        Ask::Pick { card, what } => (*card, *what),
        Ask::Predict { card, .. } => (*card, What::Player),
        Ask::OwnHand { .. } | Ask::PlayCard { .. } => (ActionCard::Spy, What::Player),
    }
}

/// The prompt of a card's pick question, as `action_cards.rs` words it.
const fn pick_prompt(card: ActionCard, what: What) -> &'static str {
    use ActionCard as C;
    match (card, what) {
        (C::Insubordination, _) => "Insubordination: whose tactic pool",
        (C::AssassinateRepresentative, _) => "Assassinate Representative: choose 1 player",
        (C::AncientBurialSites, _) => "Ancient Burial Sites: choose 1 player",
        (C::ForwardSupplyBase, _) => "Forward Supply Base: choose another player",
        (C::ConfusingLegalText, _) => "Confusing Legal Text: who is the elected player instead",
        (C::UnexpectedAction, _) => "Unexpected Action: recall your token from where",
        (C::MiningInitiative, _) => "Mining Initiative: mine which planet",
        (C::WarEffort, _) => "War Effort: into which system",
        (C::FrontlineDeployment, _) => "Frontline Deployment: onto which planet",
        (C::TacticalBombardment, _) => "Tactical Bombardment: which system",
        (C::InTheSilenceOfSpace, _) => "In The Silence Of Space: whose ships ignore blockades",
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, What::Note) => {
            "Diplomatic Pressure: which note do you give"
        }
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, _) => {
            "Diplomatic Pressure: choose another player"
        }
        (C::Rescue, _) => "Rescue: bring a ship from where",
        (C::ManipulateInvestments, _) => "Manipulate Investments: place a trade good on which strategy card",
        (C::Uprising, _) => "Uprising: which planet",
        (C::CrippleDefenses, _) => "Cripple Defenses: which planet",
        (C::ReactorMeltdown, _) => "Reactor Meltdown: which space dock",
        (C::RepealLaw, _) => "repeal which law",
        (C::ExchangeProgram, What::Offer) => "Exchange Program: which planet to offer",
        (C::ExchangeProgram, What::Accept) => "Exchange Program: accept the exchange?",
        (C::ExchangeProgram, _) => "Exchange Program: which player to ask",
        (C::LuckyShot, _) => "Lucky Shot: destroy which ship",
        (C::Scuttle, What::Scuttle { second: true }) => "Scuttle: another ship or stop",
        (C::Scuttle, _) => "Scuttle: which ship to scuttle",
        (C::SeizeArtifact, What::Fragment) => "Seize Artifact: which fragment to take",
        (C::SeizeArtifact, _) => "Seize Artifact: which neighbor to take from",
        (C::Reparations, What::Ready) => "ready a planet (Reparations)",
        (C::Reparations, _) => "exhaust a planet (Reparations)",
        (C::ArchaeologicalExpedition, _) => "Archaeological Expedition: which planet's deck",
        (C::PublicDisgrace, _) => "choose a different strategy card",
        (C::Spy, _) => "Spy: rob which player",
        (C::Plague, _) => "Plague: which planet",
        (C::UnstablePlanet, _) => "Unstable Planet: which hazardous planet",
        (C::DecoyOperation, What::Pull { second: false }) => "Decoy Operation: which unit to pull",
        (C::DecoyOperation, What::Pull { second: true }) => "Decoy Operation: another unit or stop",
        (C::DecoyOperation, _) => "Decoy Operation: which planet they land on",
        (C::RefitTroops, What::Troops { second: true }) => "Refit Troops: another infantry or stop",
        (C::RefitTroops, _) => "Refit Troops: which infantry to replace",
        (C::Brilliance, _) => "Brilliance: which player gains their own breakthrough",
        (C::FocusedResearch, _) => "Focused Research: research a technology",
        (C::DivertFunding, What::Return) => "Divert Funding: which technology to return",
        (C::DivertFunding, _) => "Divert Funding: what to research with the funding",
        (C::RevealPrototype, _) => "Reveal Prototype: which prototype to reveal",
        (C::Bribery, _) => "Bribery: how many trade goods to spend?",
        (C::LieInWait, _) => "Lie in Wait: take which action card",
        (C::Plagiarize, _) => "Plagiarize: which technology to steal",
        (C::Upgrade, _) => "Upgrade: which cruiser becomes a dreadnought",
        (C::SignalJamming, _) => "Signal Jamming: jam which system",
        (C::GhostShip, _) => "Ghost Ship: into which wormhole system",
        (C::MercenaryContract, _) => "Mercenary Contract: which planet the infantry land on",
        (C::PirateFleet, _) => "Pirate Fleet: which system the fleet enters",
        (C::PirateContract1 | C::PirateContract2 | C::PirateContract3 | C::PirateContract4, _) => "Pirate Contract: which system the destroyer enters",
        _ => "",
    }
}

/// The label prefix of a seat option.
const fn seat_label(card: ActionCard, what: What) -> &'static str {
    use ActionCard as C;
    match (card, what) {
        (_, What::Elect) => "elect ",
        (C::Insubordination, _) => "take a tactic token from ",
        (C::AssassinateRepresentative, _) => "assassinate ",
        (C::AncientBurialSites, _) => "target ",
        (C::ForwardSupplyBase, _) => "pay ",
        (C::Spy, _) => "take a card from ",
        (C::DiplomaticPressure1 | C::DiplomaticPressure2 | C::DiplomaticPressure3 | C::DiplomaticPressure4, _) => "pressure ",
        _ => "",
    }
}

/// The name a Rider's prediction prompt starts with.
const fn rider_name(card: ActionCard) -> &'static str {
    use ActionCard as C;
    match card {
        C::ImperialRider => "Imperial Rider",
        C::ConstructionRider => "Construction Rider",
        C::DiplomacyRider => "Diplomacy Rider",
        C::LeadershipRider => "Leadership Rider",
        C::PoliticsRider => "Politics Rider",
        C::TechnologyRider => "Technology Rider",
        C::TradeRider => "Trade Rider",
        C::WarfareRider => "Warfare Rider",
        _ => "Sanction",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::ActionCard;

    #[test]
    fn prompts_subtypes_and_options_render_as_the_string_engine_words_them() {
        let r = Render::new(vec!["a".to_owned(), "b".to_owned()]);
        let ask = Ask::OwnHand { reason: Reason::DeepgloomStallTactics };
        assert_eq!(Fam::prompt(&r, &ask), "Stall Tactics (allowed by Deepgloom Executable): discard 1 action card");
        assert_eq!(Fam::subtype(&r, &Subtype::C4rDiscard), "c4r_discard");
        let opt = Fam::option(&r, &ask, &Opt::Card(ActionCard::Bribery));
        assert_eq!((opt.id.as_str(), opt.kind.as_str(), opt.label.as_str()), ("bribery", "own_hand_card", "Bribery"));
        assert_eq!(Fam::parse(&r, &ask, "bribery"), Some(Opt::Card(ActionCard::Bribery)));
        assert_eq!(Fam::parse(&r, &ask, "nonsense"), None);
    }

    /// The slot's card question renders as `reactions::slot` words it: the prompt, the subtype and one option per
    /// printed name (id, kind, label and the card payload of `reaction_card_options`).
    #[test]
    fn the_play_card_question_renders_as_the_reaction_slot_words_it() {
        use ti4_engine_core::choice::reaction::Relation;
        use ti4_engine_core::state::EventKind;
        let r = Render::new(vec!["a".to_owned(), "b".to_owned()]);
        let ask = Ask::PlayCard { relation: Relation::After, event: EventKind::SystemActivated };
        assert_eq!(Fam::prompt(&r, &ask), "play an action card (after SYSTEM_ACTIVATED)");
        assert_eq!(
            Fam::subtype(&r, &Subtype::PlayReaction { relation: Relation::When, event: EventKind::HitsToAssign }),
            "play_reaction_when_HITS_TO_ASSIGN"
        );
        let content = ContentStore::embedded();
        let hand = [ActionCardId::new("fs2"), ActionCardId::new("nav_suite"), ActionCardId::new("fs1")];
        let string = crate::reactions::reaction_card_options(content, &hand);
        assert_eq!(string.len(), 2);
        for (want, card) in string.iter().zip([ActionCard::FlankSpeed2, ActionCard::NavSuite]) {
            let got = Fam::option(&r, &ask, &Opt::Card(card));
            assert_eq!((&got.id, &got.kind, &got.label, &got.payload), (&want.id, &want.kind, &want.label, &want.payload));
            assert_eq!(Fam::parse(&r, &ask, &want.id), Some(Opt::Card(card)));
        }
    }
}

/// The state's text of a promissory note (`"{alias}:{faction}"`).
pub(crate) fn note_text(key: ti4_engine_core::state::NoteKey) -> String {
    format!("{}:{}", crate::core_sync::keys::text::note_text(key.note), key.faction.as_str())
}

/// The note a state text names.
fn parse_note(id: &str) -> Option<ti4_engine_core::state::NoteKey> {
    let (alias, faction) = id.split_once(':')?;
    let note = crate::core_sync::keys::text::parse_note(alias)?;
    let key = ti4_engine_core::state::NoteKey { note, faction: ti4_engine_core::generated::Faction::from_id(faction)? };
    (note_text(key) == id).then_some(key)
}
