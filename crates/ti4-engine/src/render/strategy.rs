//! Renderer of the `strategy` area: `impl Family` for `ti4_engine_core::choice::strategy` (prompt, context,
//! option, parse) and its entry in the registry (`render/mod.rs`).
//!
//! Owner: package B8 (docs/ENGINE_CORE_STATE.md 4.4). Ported: the Trade primary's "let another player
//! replenish commodities" (subtype `trade_choose_replenish`, option id the seat's faction text, kind
//! `replenish`, then the generic done worded "nobody else replenishes"); the command token gain and
//! Warfare's recall and redistribution; Diplomacy's system and ready-planet questions; Politics' speaker and
//! agenda questions; Leadership's purchase question. The display details (`seats`, `commodities`, pool counts,
//! `purchase`, `ready_hints`, `agenda`) and the pool previews come from the state and are added by the caller
//! of the window (`strategy_core`, `strategy_cards_core`).

use ti4_engine_core::choice::agenda::Shown;
use ti4_engine_core::choice::strategy::{Ask, Opt, Pool, Subtype};
use ti4_engine_core::choice::Opt as AnyOpt;
use ti4_engine_core::generated::{Planet, PublicObjective, StrategyCard, StrategyCardKind, System, Technology};
use ti4_engine_core::ids::Seat;

use super::{Family, Render, Wording};
use crate::choice::{ChoiceOption, DECLINE_KIND};
use crate::generated::IdStr;
use crate::preview::{Delta, Preview, Quantity};

/// The card of a follower's question.
fn follow_card(a: &Ask) -> StrategyCard {
    match a {
        Ask::Follow { card, .. } => *card,
        _ => unreachable!("a follow option is asked by Ask::Follow"),
    }
}

/// `strategy::secondary_question`: the prompt, the decline label and the follow label of a card's secondary question.
fn follow_words(card: StrategyCard) -> (&'static str, &'static str, &'static str) {
    if card == StrategyCard::ConstructionThundersEdge {
        return ("spend a strategy token to place a structure", "decline", "place");
    }
    match card.data().kind {
        StrategyCardKind::Leadership => ("spend 3 influence for a command token", "spend nothing further", "spend 3 influence"),
        StrategyCardKind::Trade => ("spend a strategy token to replenish commodities", "decline", "replenish"),
        StrategyCardKind::Construction => ("spend a strategy token to build a structure", "decline", "build"),
        StrategyCardKind::Warfare => ("spend a strategy token to produce at home", "decline", "produce"),
        StrategyCardKind::Technology => ("spend a strategy token and 4 resources to research", "decline", "spend"),
        StrategyCardKind::Imperial => ("spend a strategy token to draw a secret objective", "decline", "draw"),
        StrategyCardKind::Diplomacy => ("spend a strategy token to ready two planets", "decline", "ready"),
        StrategyCardKind::Politics => ("spend a strategy token to draw two action cards", "decline", "draw"),
    }
}

/// A token-free way to follow the core's hook table offers (`hooks_strategy::secondary_waivers` of the module that offered it): the id carries
/// the position of the module among the string modules that have the hook, the label is the module's text.
fn waiver_option(r: &Render, a: &Ask, module: ti4_engine_core::generated::Faction, code: u16) -> ChoiceOption {
    use ti4_engine_core::generated::Faction;
    let primary = match a {
        Ask::Follow { primary, .. } => r.seat_text(*primary).to_owned(),
        _ => unreachable!("a waiver is offered by Ask::Follow"),
    };
    let index = crate::factions::hooks_strategy::waiver_module_index(crate::generated::IdStr::as_str(module)).unwrap_or(0);
    let (id, label) = match module {
        Faction::TheWinnu => ("acq".to_owned(), "resolve the secondary using Acquiescence".to_owned()),
        Faction::TheMahactGeneSorcerers => {
            let system = System::from_index(usize::from(code)).map_or("?", |s| s.as_str());
            (
                format!("{}{system}", crate::factions::mahact::AGENT_WAIVER_PREFIX),
                format!("exhaust Jae Mir Kan: use {primary}'s command token from {} instead of a token", crate::factions::mahact::system_name_of(system)),
            )
        }
        _ => {
            let note = ti4_engine_core_factions::cd_registry::naalu::query::mindsieve_parts(code).map_or_else(String::new, |(note, faction)| crate::render::cards::note_text(ti4_engine_core::state::NoteKey { note, faction }));
            (format!("{}{note}", crate::factions::naalu::MINDSIEVE_PREFIX), format!("resolve the secondary by giving {primary} the {note} promissory note"))
        }
    };
    ChoiceOption::labelled(format!("{}{index}|{id}", crate::strategy::WAIVED_SECONDARY_PREFIX), crate::strategy::STRATEGY_KIND, label)
}

/// The renderer of the strategy card questions.
pub struct Fam;

/// The seat whose faction text is `id` (the options naming seats by faction).
fn seat_named(r: &Render, id: &str) -> Option<Seat> {
    r.factions.iter().position(|f| !f.is_empty() && f == id).and_then(|i| u8::try_from(i).ok()).map(Seat)
}

fn pool_id(p: Pool) -> &'static str {
    match p {
        Pool::Tactic => "tactic_tokens",
        Pool::Fleet => "fleet_tokens",
        Pool::Strategic => "strategic_tokens",
    }
}

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(_: &Render, a: &Ask) -> String {
        match a {
            Ask::TradeReplenish => "let another player replenish commodities".to_owned(),
            Ask::GainPool { .. } => "gain a command token into which pool".to_owned(),
            Ask::WarfareRecall => "recall a command token".to_owned(),
            Ask::WarfareRedistribute { .. } => "Warfare: redistribute your command tokens".to_owned(),
            Ask::StatusRedistribute { .. } => "redistribute your command tokens".to_owned(),
            Ask::DiplomacySystem => "choose a system for Diplomacy".to_owned(),
            Ask::ReadyPlanet => "ready which planet".to_owned(),
            Ask::PoliticsSpeaker => "who becomes speaker".to_owned(),
            Ask::PoliticsAgenda { agenda } => format!("place {} where", agenda.as_str()),
            Ask::LeadershipBuy { owed, more } => {
                format!("spend {owed}{} influence for a command token", if *more { " more" } else { "" })
            }
            Ask::PlaceStructure { .. } => "place a structure".to_owned(),
            Ask::ImperialScore => "score a public objective with Imperial".to_owned(),
            Ask::ConstructionAbility => "Construction: a structure or a production".to_owned(),
            Ask::TeWarfareRedistribute { .. } => "Warfare: redistribute your command tokens before the tactical action".to_owned(),
            Ask::TeWarfareTactical => "Warfare: a tactical action without a command token".to_owned(),
            Ask::ResearchTechnology { .. } => "research a technology".to_owned(),
            Ask::Follow { card, .. } => follow_words(*card).0.to_owned(),
            Ask::Draft => "choose a strategy card".to_owned(),
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::TradeReplenish => "trade_choose_replenish",
            Subtype::GainCommandToken => "gain_command_token",
            Subtype::WarfareRecallToken => "warfare_recall_token",
            Subtype::WarfareRedistributeTokens => "warfare_redistribute_tokens",
            Subtype::StatusRedistributeTokens => "status_redistribute_tokens",
            Subtype::DiplomacyChooseSystem => "diplomacy_choose_system",
            Subtype::ReadyPlanet => "ready_planet",
            Subtype::PoliticsChooseSpeaker => "politics_choose_speaker",
            Subtype::PoliticsPlaceAgenda => "politics_place_agenda",
            Subtype::BuyTokenWithInfluence => "buy_token_with_influence",
            Subtype::PlaceStructure => "place_structure",
            Subtype::ImperialScoreObjective => "imperial_score_objective",
            Subtype::ResearchTechnology => "research_technology",
            Subtype::FollowSecondary => "strategy_secondary",
            Subtype::ConstructionChooseAbility => "construction_choose_ability",
            Subtype::WarfareFreeTactical => "warfare_free_tactical",
            Subtype::Draft => "draft_strategy_card",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Replenish(seat) => {
                let name = r.faction_text(*seat);
                ChoiceOption::labelled(name, "replenish", format!("{name} replenishes commodities"))
            }
            Opt::Pool(p) => ChoiceOption::labelled(
                pool_id(*p),
                "pool",
                match p {
                    Pool::Tactic => "tactic pool",
                    Pool::Fleet => "fleet pool",
                    Pool::Strategic => "strategy pool",
                },
            ),
            Opt::System(system) => {
                let id = system.as_str();
                match a {
                    Ask::TeWarfareTactical => ChoiceOption::labelled(id, "activate", format!("free tactical action in {id}")),
                    _ => ChoiceOption::labelled(id, if matches!(a, Ask::WarfareRecall) { "recall" } else { "system" }, id),
                }
            }
            Opt::Ready(planet, shown) => {
                let id = planet.as_str();
                ChoiceOption::labelled(id, "ready", format!("ready {id}")).with_planet(id, shown.system.map(|s| s.as_str()))
            }
            Opt::Speaker(seat) => {
                let name = r.faction_text(*seat);
                ChoiceOption::labelled(name, "speaker", format!("{name} becomes speaker"))
            }
            Opt::Split { tactic, fleet, strategic } => ChoiceOption::labelled(
                format!("{tactic}|{fleet}|{strategic}"),
                "redistribute",
                format!("tactic {tactic} / fleet {fleet} / strategy {strategic}"),
            ),
            Opt::Top => ChoiceOption::labelled("top", "agenda", "on top of the deck"),
            Opt::Bottom => ChoiceOption::labelled("bottom", "agenda", "on the bottom"),
            Opt::Structure { pds, system, planet } => {
                let (kind, planet, system) = (if *pds { "pds" } else { "spacedock" }, planet.as_str(), system.as_str());
                ChoiceOption::labelled(format!("{kind}|{system}|{planet}"), "build", format!("place {kind} on {planet}"))
                    .with_planet(planet, Some(system))
                    .with("unit", kind)
            }
            Opt::Research(t) => {
                // The label is the printed name, which only the content store has: the caller completes it.
                let (cost, owned) = match a {
                    Ask::ResearchTechnology { cost, owned, .. } => (i64::from(*cost), i64::from(*owned)),
                    _ => (0, 0),
                };
                let mut option = ChoiceOption::labelled(t.as_str(), crate::strategy_cards::RESEARCH_KIND, t.as_str()).previewed(Preview::certain(vec![
                    Delta::new(Quantity::TechnologiesOwned, owned, owned + 1),
                ]));
                option.payload.insert("cost".to_owned(), serde_json::Value::from(cost));
                option.payload.insert("cost_tokens".to_owned(), serde_json::Value::from(0));
                option
            }
            Opt::Objective(o) => ChoiceOption::labelled(o.as_str(), "objective", o.as_str()),
            Opt::Card(card) => ChoiceOption::labelled(card.as_str(), crate::draft::STRATEGY_CARD_KIND, crate::draft::strategy_card_label(ti4_content::ContentStore::embedded(), card.as_str())),
            Opt::Build => ChoiceOption::labelled("structure", "build", "place a structure"),
            Opt::Produce(system) => ChoiceOption::labelled(format!("produce|{}", system.as_str()), "build", format!("use PRODUCTION in {}", system.as_str())),
            Opt::Skip => ChoiceOption::labelled("no", crate::strategy::STRATEGY_KIND, follow_words(follow_card(a)).1),
            Opt::Follow => ChoiceOption::labelled("yes", crate::strategy::STRATEGY_KIND, follow_words(follow_card(a)).2),
            Opt::Waive(shape) => r.shape_option(*shape),
            Opt::WaiveBy(module, code) => waiver_option(r, a, *module, *code),
            Opt::PrimaryInstead => ChoiceOption::labelled(
                crate::factions::firmament_plots::PRIMARY_INSTEAD_ID,
                crate::strategy::STRATEGY_KIND,
                "perform the primary ability instead (Enervate)",
            ),
            Opt::Buy(false) => ChoiceOption::labelled("no", crate::strategy::STRATEGY_KIND, "spend nothing further"),
            Opt::Buy(true) => {
                let (owed, more) = match a {
                    Ask::LeadershipBuy { owed, more } => (*owed, *more),
                    _ => (0, false),
                };
                ChoiceOption::labelled("yes", crate::strategy::STRATEGY_KIND, format!("spend {owed}{} influence", if more { " more" } else { "" }))
            }
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::TradeReplenish => seat_named(r, id).map(Opt::Replenish),
            Ask::GainPool { .. } => [Pool::Tactic, Pool::Fleet, Pool::Strategic].into_iter().find(|p| pool_id(*p) == id).map(Opt::Pool),
            Ask::WarfareRecall | Ask::DiplomacySystem | Ask::TeWarfareTactical => System::from_id(id).map(Opt::System),
            Ask::ConstructionAbility => match id.strip_prefix("produce|") {
                Some(system) => System::from_id(system).map(Opt::Produce),
                None => (id == "structure").then_some(Opt::Build),
            },
            Ask::ReadyPlanet => Planet::from_id(id).map(|p| Opt::Ready(p, Shown::default())),
            Ask::PoliticsSpeaker => seat_named(r, id).map(Opt::Speaker),
            Ask::WarfareRedistribute { .. } | Ask::TeWarfareRedistribute { .. } | Ask::StatusRedistribute { .. } => {
                let mut parts = id.split('|');
                let mut next = || parts.next()?.parse::<u8>().ok();
                let (tactic, fleet, strategic) = (next()?, next()?, next()?);
                (parts.next().is_none()).then_some(Opt::Split { tactic, fleet, strategic })
            }
            Ask::PoliticsAgenda { .. } => match id {
                "top" => Some(Opt::Top),
                "bottom" => Some(Opt::Bottom),
                _ => None,
            },
            Ask::PlaceStructure { only_pds, .. } => {
                let mut parts = id.split('|');
                let (kind, system, planet, rest) = (parts.next()?, parts.next()?, parts.next()?, parts.next());
                let pds = match kind {
                    "pds" => true,
                    "spacedock" if !only_pds => false,
                    _ => return None,
                };
                rest.is_none().then_some(())?;
                Some(Opt::Structure { pds, system: System::from_id(system)?, planet: Planet::from_id(planet)? })
            }
            Ask::Draft => StrategyCard::from_id(id).map(Opt::Card),
            Ask::ImperialScore => PublicObjective::from_id(id).map(Opt::Objective),
            Ask::ResearchTechnology { .. } => Technology::from_id(id).map(Opt::Research),
            Ask::Follow { .. } => match id {
                "no" => Some(Opt::Skip),
                "yes" => Some(Opt::Follow),
                crate::factions::firmament_plots::PRIMARY_INSTEAD_ID => Some(Opt::PrimaryInstead),
                _ if id.starts_with(crate::strategy::WAIVED_SECONDARY_PREFIX) => {
                    let count = r.shape_count();
                    (0..count).find_map(|i| {
                        let code = ti4_engine_core::state::Code(u16::try_from(i).ok()?);
                        (r.shape_option(code).id == id).then_some(Opt::Waive(code))
                    })
                }
                _ => None,
            },
            Ask::LeadershipBuy { .. } => match id {
                "no" => Some(Opt::Buy(false)),
                "yes" => Some(Opt::Buy(true)),
                _ => None,
            },
        }
    }

    fn details(r: &Render, a: &Ask) -> serde_json::Map<String, serde_json::Value> {
        let mut d = serde_json::Map::new();
        if let Ask::Follow { card, primary, costs_token } = a {
            // `strategy::secondary_choice`: `tokens_left` is the follower's pool now (the caller completes it) and the
            // purchase, pool, hint and commodity facts are the string's display texts (copied from the recorded question).
            d.insert("kind".to_owned(), "strategy_secondary".into());
            d.insert("card".to_owned(), card.as_str().into());
            d.insert("played_by".to_owned(), r.seat_text(*primary).into());
            d.insert("costs_token".to_owned(), (*costs_token).into());
        }
        if let Ask::PlaceStructure { step, only_pds } = a {
            if *step > 0 {
                d.insert("step".to_owned(), (*step).into());
                d.insert("of".to_owned(), 2.into());
            }
            if *only_pds {
                d.insert("only_pds".to_owned(), true.into());
            }
        }
        d
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
