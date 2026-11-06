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
use ti4_engine_core::choice::strategy::{Ask, Opt, Stack, Subtype, Waiver};
use ti4_engine_core::choice::Opt as AnyOpt;
use ti4_engine_core::generated::{Agenda, Faction, Planet, PublicObjective, StrategyCard, StrategyCardKind, System, Technology, Unit, UnitBaseType};
use ti4_engine_core::ids::Seat;

use super::{Family, Render, Wording};
use crate::wire::choice::{ChoiceOption, DECLINE_KIND};
use crate::generated::IdStr;
use crate::wire::preview::{Delta, Preview, Quantity};

/// The card of a follower's question.
fn follow_card(a: &Ask) -> StrategyCard {
    match a {
        Ask::Follow { card, .. } | Ask::CrisisPact { card } => *card,
        _ => unreachable!("a follow option is asked by Ask::Follow or Ask::CrisisPact"),
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
    let index = crate::ties::factions::waiver_module_index(crate::generated::IdStr::as_str(module)).unwrap_or(0);
    let (id, label) = match module {
        Faction::TheWinnu => ("acq".to_owned(), "resolve the secondary using Acquiescence".to_owned()),
        Faction::TheMahactGeneSorcerers => {
            let system = System::from_index(usize::from(code)).map_or("?", |s| s.as_str());
            (
                format!("{}{system}", crate::ties::factions::MAHACT_AGENT_WAIVER_PREFIX),
                format!("exhaust Jae Mir Kan: use {primary}'s command token from {} instead of a token", crate::ties::factions::mahact_system_name_of(system)),
            )
        }
        _ => {
            let note = ti4_engine_core_factions::cd_registry::naalu::query::mindsieve_parts(code).map_or_else(String::new, |(note, faction)| crate::render::cards::note_text(ti4_engine_core::state::NoteKey { note, faction }));
            (format!("{}{note}", crate::ties::factions::NAALU_MINDSIEVE_PREFIX), format!("resolve the secondary by giving {primary} the {note} promissory note"))
        }
    };
    ChoiceOption::labelled(format!("{}{index}|{id}", crate::ties::strategy::WAIVED_SECONDARY_PREFIX), crate::ties::strategy::STRATEGY_KIND, label)
}

/// The renderer of the strategy card questions.
pub struct Fam;

/// The seat whose faction text is `id` (the options naming seats by faction).
fn seat_named(r: &Render, id: &str) -> Option<Seat> {
    r.factions.iter().position(|f| !f.is_empty() && f == id).and_then(|i| u8::try_from(i).ok()).map(Seat)
}

/// The option kind of a token gain split (`{tactic}|{fleet}|{strategic}`, the tokens each pool gains).
pub const GAIN_KIND: &str = "gain";

/// The three counts of a `{tactic}|{fleet}|{strategic}` id (a gain or a redistribution).
fn split_id(id: &str) -> Option<[u8; 3]> {
    let mut parts = id.split('|');
    let mut next = || parts.next()?.parse::<u8>().ok();
    let counts = [next()?, next()?, next()?];
    parts.next().is_none().then_some(counts)
}

/// "1 tactic + 2 fleet": the pools a gain split puts tokens in.
fn gain_label(tactic: u8, fleet: u8, strategic: u8) -> String {
    let parts: Vec<String> = [(tactic, "tactic"), (fleet, "fleet"), (strategic, "strategy")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, pool)| format!("{n} {pool}"))
        .collect();
    if parts.is_empty() { "no token".to_owned() } else { parts.join(" + ") }
}

/// The id of Hecatoncheires' alternative at a "place a PDS" site (`factions::titans`).
const HECATONCHEIRES: &str = "titans|hecatoncheires";

impl Family for Fam {
    type Ask = Ask;
    type Opt = Opt;
    type Subtype = Subtype;

    fn prompt(r: &Render, a: &Ask) -> String {
        match a {
            Ask::TradeReplenish => "let another player replenish commodities".to_owned(),
            Ask::GainPool { left: 1 } => "gain a command token into which pool".to_owned(),
            Ask::GainPool { left } => format!("gain {left} command tokens into which pools"),
            Ask::WarfareRecall => "recall a command token".to_owned(),
            Ask::WarfareRedistribute { .. } => "Warfare: redistribute your command tokens".to_owned(),
            Ask::StatusRedistribute { .. } => "redistribute your command tokens".to_owned(),
            Ask::DiplomacySystem => "choose a system for Diplomacy".to_owned(),
            Ask::ReadyPlanet => "ready which planet".to_owned(),
            Ask::PoliticsSpeaker => "who becomes speaker".to_owned(),
            Ask::PoliticsAgenda { agenda } => format!("place {} where", agenda.as_str()),
            Ask::PoliticsAgendas { first, second } => format!("place {} and {} on the top or the bottom of the agenda deck", first.as_str(), second.as_str()),
            Ask::PlaceStructure { .. } => "place a structure".to_owned(),
            Ask::PdsAlternative { .. } => "place a PDS or an alternative".to_owned(),
            Ask::ImperialScore => "score a public objective with Imperial".to_owned(),
            Ask::PeaceAccords { .. } => "Peace Accords: annex a planet".to_owned(),
            // `strategy::crisis_pact_choice`: the card's ordinary follower question, Leadership's purchase question unchanged.
            Ask::CrisisPact { card } if card.data().kind == StrategyCardKind::Leadership => follow_words(*card).0.to_owned(),
            Ask::CrisisPact { card } => {
                let name = crate::ties::strategy_cards::card_name(ti4_content::ContentStore::embedded(), card.as_str()).unwrap_or_default();
                format!("Galactic Crisis Pact: perform the {name} secondary without spending a command token")
            }
            Ask::ConstructionAbility => "Construction: a structure or a production".to_owned(),
            Ask::TeWarfareRedistribute { .. } => "Warfare: redistribute your command tokens before the tactical action".to_owned(),
            Ask::TeWarfareRedistributeAfter { .. } => "Warfare: redistribute your command tokens after the tactical action".to_owned(),
            Ask::TeWarfareTactical => "Warfare: a tactical action without a command token".to_owned(),
            Ask::ResearchTechnology { .. } => "research a technology".to_owned(),
            Ask::SucabanOffer { researcher, source, copied } => {
                if *copied {
                    format!("Ssruu: copy {}'s Doctor Sucaban to trade infantry for research", r.seat_text(*source))
                } else {
                    format!("Doctor Sucaban: exhaust to let {} trade infantry for research", r.seat_text(*researcher))
                }
            }
            Ask::SucabanTrade => "remove an infantry to reduce the cost by 1".to_owned(),
            Ask::Follow { card, .. } => follow_words(*card).0.to_owned(),
            Ask::Draft => "choose a strategy card".to_owned(),
            Ask::ArbiterGive => "give away which of your strategy cards".to_owned(),
            Ask::ArbiterTake => "and take which".to_owned(),
            Ask::ResearchWaiver { tech, .. } => format!("research {}: choose a prerequisite waiver", tech.as_str()),
            Ask::ResearchWaiverPayment { waiver, .. } => format!("{}: choose the payment", waiver_label(*waiver)),
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
            Subtype::PlaceStructurePdsAlternative => "place_structure_pds_alternative",
            Subtype::ImperialScoreObjective => "imperial_score_objective",
            Subtype::PeaceAccordsAnnex => "peace_accords_annex",
            Subtype::ResearchTechnology => "research_technology",
            Subtype::SucabanExhaust => "doctor_sucaban_exhaust",
            Subtype::SucabanBorrowedExhaust => "doctor_sucaban_borrowed_exhaust",
            Subtype::SucabanRemoveInfantry => "doctor_sucaban_remove_infantry",
            Subtype::FollowSecondary => "strategy_secondary",
            Subtype::ConstructionChooseAbility => "construction_choose_ability",
            Subtype::WarfareFreeTactical => "warfare_free_tactical",
            Subtype::Draft => "draft_strategy_card",
            Subtype::ArbiterGive => "arbiter_give",
            Subtype::ArbiterTake => "arbiter_take",
            Subtype::ResearchWaiver => "research_waiver",
            Subtype::ResearchWaiverPayment => "research_waiver_payment",
        }
        .to_owned()
    }

    fn option(r: &Render, a: &Ask, o: &Opt) -> ChoiceOption {
        match o {
            Opt::Replenish(seat) => {
                let name = r.faction_text(*seat);
                ChoiceOption::labelled(name, "replenish", format!("{name} replenishes commodities"))
            }
            Opt::Gain { tactic, fleet, strategic } => ChoiceOption::labelled(
                format!("{tactic}|{fleet}|{strategic}"),
                GAIN_KIND,
                gain_label(*tactic, *fleet, *strategic),
            )
            .with("tactic", i64::from(*tactic))
            .with("fleet", i64::from(*fleet))
            .with("strategic", i64::from(*strategic)),
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
            Opt::Stack(stack) => {
                let (first, second) = match a {
                    Ask::PoliticsAgendas { first, second } => (*first, *second),
                    _ => (Agenda::ALL[0], Agenda::ALL[0]),
                };
                let (top, bottom) = stack.piles(first, second);
                let names = |pile: &[Agenda]| pile.iter().map(|x| x.as_str()).collect::<Vec<_>>();
                let (top, bottom) = (names(top.as_slice()), names(bottom.as_slice()));
                let label = match (top.as_slice(), bottom.as_slice()) {
                    ([t], [b]) => format!("{t} on top, {b} on the bottom"),
                    ([t1, t2], []) => format!("both on top: {t1} first, then {t2}"),
                    ([], [b1, b2]) => format!("both on the bottom: {b1}, then {b2} as the bottom card"),
                    _ => String::new(),
                };
                ChoiceOption::labelled(format!("{}/{}", top.join(","), bottom.join(",")), "agenda_order", label)
                    .with("top", serde_json::Value::from(top))
                    .with("bottom", serde_json::Value::from(bottom))
            }
            Opt::Structure { pds, system, planet } => {
                let (kind, planet, system) = (if *pds { "pds" } else { "spacedock" }, planet.as_str(), system.as_str());
                ChoiceOption::labelled(format!("{kind}|{system}|{planet}"), "build", format!("place {kind} on {planet}"))
                    .with_planet(planet, Some(system))
                    .with("unit", kind)
            }
            Opt::PlacePds => {
                let planet = match a {
                    Ask::PdsAlternative { planet, .. } => planet.as_str(),
                    _ => "",
                };
                ChoiceOption::labelled("pds", "build", format!("place pds on {planet}"))
            }
            Opt::Alternative => {
                let planet = match a {
                    Ask::PdsAlternative { planet, .. } => planet.as_str(),
                    _ => "",
                };
                ChoiceOption::labelled(HECATONCHEIRES, "build", format!("place 1 mech and 1 infantry on {planet} instead"))
            }
            Opt::Research(t) => {
                // The label is the printed name, which only the content store has: the caller completes it.
                let (cost, owned) = match a {
                    Ask::ResearchTechnology { cost, owned, .. } => (i64::from(*cost), i64::from(*owned)),
                    _ => (0, 0),
                };
                let mut option = ChoiceOption::labelled(t.as_str(), crate::ties::strategy_cards::RESEARCH_KIND, t.as_str()).previewed(Preview::certain(vec![
                    Delta::new(Quantity::TechnologiesOwned, owned, owned + 1),
                ]));
                option.payload.insert("cost".to_owned(), serde_json::Value::from(cost));
                option.payload.insert("cost_tokens".to_owned(), serde_json::Value::from(0));
                option
            }
            Opt::Exhaust => ChoiceOption::labelled("yes", "leader", "exhaust the agent"),
            Opt::Infantry { system, planet } => match planet {
                Some(planet) => ChoiceOption::labelled(format!("{}:{}", system.as_str(), planet.as_str()), "unit", format!("an infantry on {}", planet.as_str())),
                None => ChoiceOption::labelled(format!("{}:", system.as_str()), "unit", format!("an infantry in space at {}", system.as_str())),
            },
            Opt::ResearchWaiver(index) => {
                let waiver = match a {
                    Ask::ResearchWaiver { offered, .. } => offered.get(usize::from(*index)).copied().flatten(),
                    _ => None,
                };
                ChoiceOption::labelled(format!("waiver|{index}"), "research_waiver", waiver.map_or("ignore its prerequisites", waiver_label))
            }
            Opt::ResearchPayment(payment) => {
                let (id, label) = payment_words(r, *payment);
                ChoiceOption::labelled(id, "research_waiver_payment", label)
            }
            Opt::Objective(o) => ChoiceOption::labelled(o.as_str(), "objective", o.as_str()),
            Opt::LeakedSecret(x) => ChoiceOption::labelled(x.as_str(), "objective", x.as_str()),
            Opt::Card(card) if matches!(a, Ask::ArbiterGive) => ChoiceOption::labelled(card.as_str(), crate::ties::draft::STRATEGY_CARD_KIND, format!("give {}", card.as_str())),
            Opt::Annex(system, planet) => {
                // `faction_abilities::strategy_resolved`: the planet's id, "annex", the system and the planet as details, and the gain of one planet.
                let controlled = match a {
                    Ask::PeaceAccords { controlled } => i64::from(*controlled),
                    _ => 0,
                };
                ChoiceOption::labelled(planet.as_str(), "annex", format!("gain control of {}", planet.as_str()))
                    .with("system", system.as_str())
                    .with("planet", planet.as_str())
                    .previewed(Preview::certain(vec![Delta::new(Quantity::PlanetsControlled, controlled, controlled + 1)]))
            }
            Opt::Take(holder, card) => {
                let (holder, card) = (r.seat_text(*holder), card.as_str());
                ChoiceOption::labelled(format!("{holder}|{card}"), crate::ties::draft::STRATEGY_CARD_KIND, format!("take {card} from {holder}"))
            }
            Opt::Card(card) => ChoiceOption::labelled(card.as_str(), crate::ties::draft::STRATEGY_CARD_KIND, crate::ties::draft::strategy_card_label(ti4_content::ContentStore::embedded(), card.as_str())),
            Opt::Build => ChoiceOption::labelled("structure", "build", "place a structure"),
            Opt::Produce(system) => ChoiceOption::labelled(format!("produce|{}", system.as_str()), "build", format!("use PRODUCTION in {}", system.as_str())),
            Opt::Skip => ChoiceOption::labelled("no", crate::ties::strategy::STRATEGY_KIND, follow_words(follow_card(a)).1),
            Opt::Follow => ChoiceOption::labelled("yes", crate::ties::strategy::STRATEGY_KIND, follow_words(follow_card(a)).2),
            Opt::Waive(shape) => r.shape_option(*shape),
            Opt::WaiveBy(module, code) => waiver_option(r, a, *module, *code),
            Opt::PrimaryInstead => ChoiceOption::labelled(
                crate::ties::factions::PRIMARY_INSTEAD_ID,
                crate::ties::strategy::STRATEGY_KIND,
                "perform the primary ability instead (Enervate)",
            ),
        }
    }

    fn parse(r: &Render, a: &Ask, id: &str) -> Option<Opt> {
        match a {
            Ask::TradeReplenish => seat_named(r, id).map(Opt::Replenish),
            Ask::GainPool { left } => {
                let [tactic, fleet, strategic] = split_id(id)?;
                (u16::from(tactic) + u16::from(fleet) + u16::from(strategic) == u16::from(*left)).then_some(Opt::Gain { tactic, fleet, strategic })
            }
            Ask::WarfareRecall | Ask::DiplomacySystem | Ask::TeWarfareTactical => System::from_id(id).map(Opt::System),
            Ask::ConstructionAbility => match id.strip_prefix("produce|") {
                Some(system) => System::from_id(system).map(Opt::Produce),
                None => (id == "structure").then_some(Opt::Build),
            },
            Ask::ReadyPlanet => Planet::from_id(id).map(|p| Opt::Ready(p, Shown::default())),
            Ask::PoliticsSpeaker => seat_named(r, id).map(Opt::Speaker),
            Ask::WarfareRedistribute { .. } | Ask::TeWarfareRedistribute { .. } | Ask::TeWarfareRedistributeAfter { .. } | Ask::StatusRedistribute { .. } => {
                let [tactic, fleet, strategic] = split_id(id)?;
                Some(Opt::Split { tactic, fleet, strategic })
            }
            Ask::PoliticsAgendas { first, second } => {
                Stack::ALL.into_iter().find(|stack| {
                    let (top, bottom) = stack.piles(*first, *second);
                    let names = |pile: &[Agenda]| pile.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(",");
                    format!("{}/{}", names(top.as_slice()), names(bottom.as_slice())) == id
                }).map(Opt::Stack)
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
            Ask::PdsAlternative { .. } => match id {
                "pds" => Some(Opt::PlacePds),
                HECATONCHEIRES => Some(Opt::Alternative),
                _ => None,
            },
            Ask::Draft | Ask::ArbiterGive => StrategyCard::from_id(id).map(Opt::Card),
            Ask::ArbiterTake => {
                let (holder, card) = id.split_once('|')?;
                Some(Opt::Take(r.seat(holder)?, StrategyCard::from_id(card)?))
            }
            Ask::ImperialScore => PublicObjective::from_id(id).map(Opt::Objective).or_else(|| ti4_engine_core::generated::SecretObjective::from_id(id).map(Opt::LeakedSecret)),
            Ask::PeaceAccords { .. } => Planet::from_id(id).and_then(|planet| planet.data().system.map(|system| Opt::Annex(system, planet))),
            Ask::ResearchTechnology { .. } => Technology::from_id(id).map(Opt::Research),
            Ask::SucabanOffer { .. } => (id == "yes").then_some(Opt::Exhaust),
            Ask::SucabanTrade => {
                let (system, planet) = id.split_once(':')?;
                let planet = if planet.is_empty() { None } else { Some(Planet::from_id(planet)?) };
                Some(Opt::Infantry { system: System::from_id(system)?, planet })
            }
            Ask::ResearchWaiver { offered, .. } => {
                let index = id.strip_prefix("waiver|")?.parse::<u8>().ok()?;
                offered.get(usize::from(index)).copied().flatten().map(|_| Opt::ResearchWaiver(index))
            }
            Ask::ResearchWaiverPayment { waiver, .. } => parse_payment(r, *waiver, id).map(Opt::ResearchPayment),
            Ask::CrisisPact { .. } => match id {
                "no" => Some(Opt::Skip),
                "yes" => Some(Opt::Follow),
                _ => None,
            },
            Ask::Follow { .. } => match id {
                "no" => Some(Opt::Skip),
                "yes" => Some(Opt::Follow),
                crate::ties::factions::PRIMARY_INSTEAD_ID => Some(Opt::PrimaryInstead),
                _ if id.starts_with(crate::ties::strategy::WAIVED_SECONDARY_PREFIX) => { // mutants:skip: the prefix only spares the scan of the shape table for an id that is no waiver (the scan finds nothing either way)
                    let count = r.shape_count();
                    (0..count).find_map(|i| {
                        let code = ti4_engine_core::state::Code(u16::try_from(i).ok()?);
                        (r.shape_option(code).id == id).then_some(Opt::Waive(code))
                    })
                }
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
        if let Ask::ResearchWaiver { tech, .. } = a {
            // `strategy_cards::resolve_research`: the offer card of the waiver question.
            d = crate::wire::choice::offer_details(
                crate::wire::choice::offer_card(
                    "Research without prerequisites",
                    "faction ability",
                    Some("A faction ability lets you research this technology without its prerequisites"),
                    Some("Choose how, and pay its cost on the next step. Declining researches nothing."),
                ),
                vec![crate::wire::choice::offer_fact_technology("Technology", tech.as_str())],
                &[("decline", crate::wire::choice::offer_caption("Don't use a waiver", Some("Nothing is researched")))],
            );
        }
        if let Ask::ResearchWaiverPayment { tech, waiver } = a {
            let label = waiver_label(*waiver);
            d = crate::wire::choice::offer_details(
                crate::wire::choice::offer_card("Pay for the waiver", "faction ability", Some(label), Some("Choose what pays for it. Declining researches nothing and pays nothing.")),
                vec![crate::wire::choice::offer_fact_technology("Technology", tech.as_str())],
                &[("decline", crate::wire::choice::offer_caption("Don't pay", Some("Nothing is researched or paid")))],
            );
        }
        if let Ask::CrisisPact { card } = a {
            // `strategy::secondary_choice` without a token (the pact has no `played_by` and no `tokens_left`; `live::strategic` adds the agenda card).
            d.insert("kind".to_owned(), "strategy_secondary".into());
            d.insert("card".to_owned(), card.as_str().into());
            d.insert("costs_token".to_owned(), false.into());
        }
        if let Ask::PdsAlternative { system, planet } = a {
            // `strategy_cards::construction_structure`: the offer card of the alternative.
            d = crate::wire::choice::offer_details(
                crate::wire::choice::offer_card(
                    "Place a PDS",
                    "construction",
                    Some("A faction ability can replace this PDS"),
                    Some("You may place something else on this planet instead of the PDS."),
                ),
                vec![crate::wire::choice::offer_fact_planet("Planet", planet.as_str(), system.as_str())],
                &[("pds", crate::wire::choice::offer_caption("Place the PDS", Some("As planned")))],
            );
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

// -- the research window's faction waivers (`strategy_cards::resolve_research`) --------------------------------------------------------------

/// What a waiver says (`ResearchWaiver::label` of the string module that offers it: the Cabal's Riftmeld, Yin's Brother Omar).
fn waiver_label(waiver: Waiver) -> &'static str {
    match waiver.module {
        Faction::TheVuilraithCabal => "return 1 captured unit of that type to ignore all of its prerequisites",
        Faction::TheYinBrotherhood => "return 1 infantry to reinforcements to ignore its prerequisites",
        _ => "ignore its prerequisites",
    }
}

/// The text of a unit base type (`UnitKind::base_type`).
fn base_text(base: UnitBaseType) -> &'static str {
    match base {
        UnitBaseType::Carrier => "carrier",
        UnitBaseType::Cruiser => "cruiser",
        UnitBaseType::Destroyer => "destroyer",
        UnitBaseType::Dreadnought => "dreadnought",
        UnitBaseType::Fighter => "fighter",
        UnitBaseType::Flagship => "flagship",
        UnitBaseType::Infantry => "infantry",
        UnitBaseType::Mech => "mech",
        UnitBaseType::Pds => "pds",
        UnitBaseType::Spacedock => "spacedock",
        UnitBaseType::Warsun => "warsun",
    }
}

/// The system a planet stands in, as the position has it (the tile it is printed on where there is no position).
fn system_of(r: &Render, planet: Planet) -> Option<System> {
    if let Some(position) = r.position() {
        let state: &ti4_engine_core::state::CoreState = &position.state;
        if let Some((system, _)) = state.board().systems().find(|(system, _)| ti4_engine_core::query::board::planets_of(state, *system).any(|p| p == planet)) {
            return Some(system);
        }
    }
    planet.data().system
}

/// The id and the label of a payment of a research waiver (`riftmeld_payments`, `yin::infantry_payments`).
fn payment_words(r: &Render, payment: Waiver) -> (String, String) {
    use ti4_engine_core_factions::cb_economy::cabal::offer::payment_model;
    use ti4_engine_core_factions::cd_registry::yin::query::{payment_place, PaymentPlace};
    match payment.module {
        Faction::TheVuilraithCabal => match payment_model(payment.code) {
            Some((owner, unit)) => {
                let owner = r.seats.get(usize::from(owner.0)).map_or("", String::as_str);
                (format!("captured|{owner}|{}", unit.as_str()), format!("return 1 captured {} taken from {owner}", base_text(unit.data().base_type)))
            }
            None => (String::new(), String::new()),
        },
        Faction::TheYinBrotherhood => match payment_place(payment.code) {
            Some(PaymentPlace::Planet(planet)) => {
                let system = system_of(r, planet).map_or("", System::as_str);
                (format!("ground|{system}|{}", planet.as_str()), format!("return 1 infantry from {} in {system}", planet.as_str()))
            }
            Some(PaymentPlace::Space(system)) => (format!("space|{}", system.as_str()), format!("return 1 infantry from space in {}", system.as_str())),
            None => (String::new(), String::new()),
        },
        _ => (String::new(), String::new()),
    }
}

/// The payment of the waiver the id names (the inverse of [`payment_words`]).
fn parse_payment(r: &Render, waiver: Waiver, id: &str) -> Option<Waiver> {
    use ti4_engine_core_factions::cb_economy::cabal::offer::payment_code;
    use ti4_engine_core_factions::cd_registry::yin::query::{place_code, PaymentPlace};
    let mut parts = id.split('|');
    let code = match (waiver.module, parts.next()?) {
        (Faction::TheVuilraithCabal, "captured") => {
            let (owner, unit) = (r.seat(parts.next()?)?, Unit::from_id(parts.next()?)?);
            payment_code(owner, unit)
        }
        (Faction::TheYinBrotherhood, "ground") => {
            let (_, planet) = (System::from_id(parts.next()?)?, Planet::from_id(parts.next()?)?);
            place_code(PaymentPlace::Planet(planet))
        }
        (Faction::TheYinBrotherhood, "space") => place_code(PaymentPlace::Space(System::from_id(parts.next()?)?)),
        _ => return None,
    };
    parts.next().is_none().then_some(Waiver { module: waiver.module, code })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ti4_engine_core::generated::Unit;

    fn r() -> Render {
        Render::new(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])
    }

    /// The captured model of a Cabal payment is named by the string's unit id and base type: every unit of the catalogue.
    #[test]
    fn the_unit_ids_and_base_types_of_a_captured_model_are_the_strings() {
        let content = ti4_content::ContentStore::embedded();
        let catalogue = ti4_content::units::catalogue(content, ti4_model::content_types::FULL);
        let mut seen = 0;
        for (id, kind) in catalogue.iter() {
            let Some(unit) = Unit::from_id(id) else { panic!("the catalogue's unit {id} is not a typed unit") };
            seen += 1;
            assert_eq!(<Unit as crate::generated::IdStr>::as_str(unit), *id, "the id of {id}");
            assert_eq!(base_text(unit.data().base_type), kind.base_type(), "the base type of {id}");
        }
        assert!(seen >= 40, "the catalogue's units are the typed ones: {seen}");
    }

    /// Every payment code prints an id that parses back to it, and the words are the string's.
    #[test]
    fn the_payments_of_both_modules_print_and_parse_back() {
        use ti4_engine_core_factions::cb_economy::cabal::offer::payment_code;
        use ti4_engine_core_factions::cd_registry::yin::query::{place_code, PaymentPlace};
        let render = r();
        let cabal = Waiver { module: Faction::TheVuilraithCabal, code: 0 };
        let model = Waiver { module: Faction::TheVuilraithCabal, code: payment_code(Seat(1), Unit::CruiserI) };
        assert_eq!(payment_words(&render, model), ("captured|b|cruiser".to_owned(), "return 1 captured cruiser taken from b".to_owned()));
        assert_eq!(parse_payment(&render, cabal, "captured|b|cruiser"), Some(model));
        assert_eq!(parse_payment(&render, cabal, "captured|z|cruiser"), None, "no such seat");
        assert_eq!(parse_payment(&render, cabal, "captured|b|cruiser|x"), None, "trailing text");
        assert_eq!(parse_payment(&render, cabal, "space|01"), None, "another module's id");
        let yin = Waiver { module: Faction::TheYinBrotherhood, code: 0 };
        let planet_of = Planet::ALL.into_iter().find(|p| p.data().system.is_some()).expect("a printed planet");
        let home = planet_of.data().system.expect("printed");
        let space = Waiver { module: Faction::TheYinBrotherhood, code: place_code(PaymentPlace::Space(home)) };
        assert_eq!(payment_words(&render, space), (format!("space|{}", home.as_str()), format!("return 1 infantry from space in {}", home.as_str())));
        assert_eq!(parse_payment(&render, yin, &format!("space|{}", home.as_str())), Some(space));
        let ground = Waiver { module: Faction::TheYinBrotherhood, code: place_code(PaymentPlace::Planet(planet_of)) };
        let (id, label) = payment_words(&render, ground);
        let (planet, system) = (planet_of.as_str(), home.as_str());
        assert_eq!((id.as_str(), label.as_str()), (format!("ground|{system}|{planet}").as_str(), format!("return 1 infantry from {planet} in {system}").as_str()), "the printed tile of the planet without a position");
        assert_eq!(parse_payment(&render, yin, &id), Some(ground));
        assert_eq!(parse_payment(&render, yin, &format!("ground|{system}")), None);
        assert_eq!(parse_payment(&render, yin, "captured|b|cruiser"), None);
    }

    /// The waiver question's words, options and offer card (`strategy_cards::resolve_research`).
    #[test]
    fn the_waiver_question_words_are_the_strings() {
        let render = r();
        let tech = Technology::from_id("cr2").expect("a technology");
        let cabal = Waiver { module: Faction::TheVuilraithCabal, code: 0 };
        let yin = Waiver { module: Faction::TheYinBrotherhood, code: 0 };
        let ask = Ask::ResearchWaiver { tech, offered: [Some(cabal), Some(yin)] };
        assert_eq!(Fam::prompt(&render, &ask), "research cr2: choose a prerequisite waiver");
        let first = Fam::option(&render, &ask, &Opt::ResearchWaiver(0));
        assert_eq!((first.id.as_str(), first.kind.as_str(), first.label.as_str()), ("waiver|0", "research_waiver", "return 1 captured unit of that type to ignore all of its prerequisites"));
        let second = Fam::option(&render, &ask, &Opt::ResearchWaiver(1));
        assert_eq!((second.id.as_str(), second.label.as_str()), ("waiver|1", "return 1 infantry to reinforcements to ignore its prerequisites"));
        assert_eq!(Fam::parse(&render, &ask, "waiver|1"), Some(Opt::ResearchWaiver(1)));
        assert_eq!(Fam::parse(&render, &ask, "waiver|2"), None, "only two were offered");
        let single = Ask::ResearchWaiver { tech, offered: [Some(cabal), None] };
        assert_eq!(Fam::parse(&render, &single, "waiver|1"), None);
        let pay = Ask::ResearchWaiverPayment { tech, waiver: yin };
        assert_eq!(Fam::prompt(&render, &pay), "return 1 infantry to reinforcements to ignore its prerequisites: choose the payment");
        assert_eq!(Fam::subtype(&render, &Subtype::ResearchWaiver), "research_waiver");
        assert_eq!(Fam::subtype(&render, &Subtype::ResearchWaiverPayment), "research_waiver_payment");
        let details = Fam::details(&render, &pay);
        assert_eq!(details["card"]["title"], "Pay for the waiver");
        assert_eq!(details["card"]["window"], "return 1 infantry to reinforcements to ignore its prerequisites");
        assert_eq!(details["captions"]["decline"]["label"], "Don't pay");
        assert_eq!(Fam::details(&render, &ask)["captions"]["decline"]["hint"], "Nothing is researched");
    }
}
