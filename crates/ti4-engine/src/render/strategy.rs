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
use ti4_engine_core::generated::{Planet, System};
use ti4_engine_core::ids::Seat;

use super::{Family, Render, Wording};
use crate::choice::{ChoiceOption, DECLINE_KIND};
use crate::generated::IdStr;

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
            Ask::DiplomacySystem => "choose a system for Diplomacy".to_owned(),
            Ask::ReadyPlanet => "ready which planet".to_owned(),
            Ask::PoliticsSpeaker => "who becomes speaker".to_owned(),
            Ask::PoliticsAgenda { agenda } => format!("place {} where", agenda.as_str()),
            Ask::LeadershipBuy { owed, more } => {
                format!("spend {owed}{} influence for a command token", if *more { " more" } else { "" })
            }
        }
    }

    fn subtype(_: &Render, s: &Subtype) -> String {
        match s {
            Subtype::TradeReplenish => "trade_choose_replenish",
            Subtype::GainCommandToken => "gain_command_token",
            Subtype::WarfareRecallToken => "warfare_recall_token",
            Subtype::WarfareRedistributeTokens => "warfare_redistribute_tokens",
            Subtype::DiplomacyChooseSystem => "diplomacy_choose_system",
            Subtype::ReadyPlanet => "ready_planet",
            Subtype::PoliticsChooseSpeaker => "politics_choose_speaker",
            Subtype::PoliticsPlaceAgenda => "politics_place_agenda",
            Subtype::BuyTokenWithInfluence => "buy_token_with_influence",
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
                ChoiceOption::labelled(id, if matches!(a, Ask::WarfareRecall) { "recall" } else { "system" }, id)
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
            Ask::WarfareRecall | Ask::DiplomacySystem => System::from_id(id).map(Opt::System),
            Ask::ReadyPlanet => Planet::from_id(id).map(|p| Opt::Ready(p, Shown::default())),
            Ask::PoliticsSpeaker => seat_named(r, id).map(Opt::Speaker),
            Ask::WarfareRedistribute { .. } => {
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
            Ask::LeadershipBuy { .. } => match id {
                "no" => Some(Opt::Buy(false)),
                "yes" => Some(Opt::Buy(true)),
                _ => None,
            },
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
