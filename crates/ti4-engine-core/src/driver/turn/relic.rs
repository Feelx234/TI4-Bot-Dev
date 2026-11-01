//! The relic component actions of the action menu (`relics::perform`): purging fragments for a relic, and using a relic with an ACTION (every one
//! the string resolves: the Dynamis Core, the Book of Latvinia, the Codex, the Enigmatic Device, the Maw of Worlds, the Stellar Converter, the
//! Titan Prototype and the Silver Flame). The Enigmatic Device exploration cards are the counted gap
//! [`crate::kernel::turn_codes::MenuGap::Unrouted`] until their question is native.

use super::component::{conclude_quiet, Verdict};
use super::inline::{gap_flow, run as run_window, Flow};
use crate::generated::{ExploreType, Relic, Technology};
use crate::ids::Seat;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::{MenuGap, TurnGap};

/// "Purge 3 relic fragments of a trait to gain a relic" (`relics::perform`, the `purge|` option): three fragments (frontier ones make up a
/// shortfall) for the top relic. The Shard of the Throne scores at once and its record says so. A purge that cannot be paid (too few fragments, an
/// empty relic deck) fails and is not offered again this turn.
pub fn purge(sx: &mut Sx<'_>, who: Seat, key: u32, kind: ExploreType) -> Flow {
    let gained = crate::rules::explore::purge_for_relic_scored(sx.state, who, kind);
    conclude_quiet(sx, who, key, if gained.is_some() { Verdict::Resolved } else { Verdict::Failed { withhold: true } })
}

/// Whether using the relic has a native body ([`use_relic`] never ends it in the counted gap).
#[must_use]
pub const fn routed(relic: Relic) -> bool {
    matches!(
        relic,
        Relic::DynamisCore
            | Relic::BookOfLatvinia
            | Relic::TheCodex
            | Relic::EnigmaticDevice
            | Relic::MawOfWorlds
            | Relic::StellarConverter
            | Relic::JRXS455O
            | Relic::TheSilverFlame
    )
}

/// The ten the Silver Flame needs, on its own die.
const SILVER_FLAME_SIDES: u32 = 10;

/// Using a relic with its printed ACTION (`relics::use_relic`, then the purge). A relic whose use cannot resolve (the Enigmatic Device that cannot
/// be paid, a Stellar Converter with nothing in range) fails and is withheld from the menu for the turn (`relics::perform` answers `false`).
pub fn use_relic(sx: &mut Sx<'_>, who: Seat, key: u32, relic: Relic) -> Flow {
    let resolved = match relic {
        Relic::DynamisCore => {
            // "Gain trade goods equal to your commodity value, +2": the faction's printed number, not what the seat holds.
            let value = sx.state.players().by_seat(who).map_or(0, |p| i32::from(p.faction.data().commodities)) + 2; // mutants:skip: the seat exists
            crate::kernel::fx::gain_trade_goods_staged(sx.state, who, value, sx.table.codes.turn.relic);
            true
        }
        Relic::BookOfLatvinia => {
            crate::rules::explore::book_of_latvinia(sx.state, who, crate::query::reach::scope_of(sx.kernel));
            true
        }
        Relic::TheCodex => {
            let mut window = crate::rules::relic_windows::Codex::new(who);
            if run_window(sx, &mut window) == Flow::Halt {
                return Flow::Halt;
            }
            true
        }
        Relic::EnigmaticDevice => match enigmatic_device(sx, who) {
            Ok(resolved) => resolved,
            Err(flow) => return flow,
        },
        Relic::MawOfWorlds => match maw_of_worlds(sx, who) {
            Ok(resolved) => resolved,
            Err(flow) => return flow,
        },
        Relic::StellarConverter => match stellar_converter(sx, who) {
            Ok(resolved) => resolved,
            Err(flow) => return flow,
        },
        Relic::JRXS455O => match titan_prototype(sx, who) {
            Ok(resolved) => resolved,
            Err(flow) => return flow,
        },
        Relic::TheSilverFlame => {
            silver_flame(sx, who);
            true
        }
        other => return gap_flow(sx, TurnGap::Menu(MenuGap::Unrouted(7, crate::choice::turn::Opt::UseRelic(other).key().unwrap_or(0)))), // mutants:skip (the key default): every option on the menu has a key
    };
    if !resolved {
        return conclude_quiet(sx, who, key, Verdict::Failed { withhold: true });
    }
    // A relic used is purged (the Silver Flame purges itself in its roll, and `purge_relic` of a card that is gone changes nothing).
    crate::rules::explore::purge_relic(sx.state, who, relic);
    conclude_quiet(sx, who, key, Verdict::Resolved)
}

/// The technologies a seat may be granted by a card that names no research: those it does not hold, of the game's sources, its faction's own among
/// the faction ones (90.11), in the order of the corpus.
fn grantable(sx: &Sx<'_>, who: Seat) -> Vec<Technology> {
    let Some(p) = sx.state.players().by_seat(who) else { return Vec::new() };
    let scope = crate::query::reach::scope_of(sx.kernel);
    let mut out: Vec<Technology> = Technology::ALL
        .iter()
        .copied()
        .filter(|t| scope.0 & crate::tables::source_bit(t.data().source) != 0)
        .filter(|t| t.data().faction.is_none_or(|tag| crate::query::tech::tag_is(tag, p.faction)))
        .filter(|t| !p.tech.owned.contains(*t))
        .collect();
    out.sort_by(|a, b| (sx.table.codes.agenda.tech_corpus)(*a, *b));
    out
}

/// "You may spend 6 resources and purge this card to research 1 technology": the bill first (an unpayable use resolves nothing and costs nothing),
/// then the technology among those the seat could research by prerequisites, then the research.
fn enigmatic_device(sx: &mut Sx<'_>, who: Seat) -> Result<bool, Flow> {
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else { return Err(gap_flow(sx, TurnGap::NoRoster)) };
    let mut bill = crate::rules::production::pay::PayBill::new(who, crate::query::economy::Spend::Resources, 6);
    if run_window(sx, &mut bill) == Flow::Halt {
        return Err(Flow::Halt);
    }
    if bill.outcome != Some(true) {
        return Ok(false);
    }
    let scope = crate::query::reach::scope_of(sx.kernel);
    let hooks = crate::rules::production::strategy_cards::KernelHooks::new(sx.state, sx.kernel, who);
    let open = crate::query::tech::researchable_by_prerequisites(sx.state, scope, crate::query::reach::blanked(sx.state), who, &hooks);
    let options: Vec<Technology> = grantable(sx, who).into_iter().filter(|t| open.contains(*t)).collect();
    let mut window = crate::rules::relic_windows::GainTechnology::new(who, Relic::EnigmaticDevice, &options);
    if run_window(sx, &mut window) == Flow::Halt {
        return Err(Flow::Halt);
    }
    if let Some(chosen) = window.chosen {
        crate::rules::production::strategy_cards::Research::research_chosen(sx.state, sx.kernel, scope, *roster, who, chosen);
    }
    Ok(true)
}

/// "Purge this card and exhaust all of your planets to gain any 1 technology": the planets are exhausted first (the cost, paid whatever is chosen), then
/// the technology is gained without prerequisites.
fn maw_of_worlds(sx: &mut Sx<'_>, who: Seat) -> Result<bool, Flow> {
    let planets: Vec<_> = crate::query::board::controlled_planets(sx.state, who).as_slice().iter().map(|(_, planet)| *planet).collect();
    for planet in planets {
        crate::kernel::fx::exhaust_planet(sx.state, planet);
    }
    let options = grantable(sx, who);
    let mut window = crate::rules::relic_windows::GainTechnology::new(who, Relic::MawOfWorlds, &options);
    if run_window(sx, &mut window) == Flow::Halt {
        return Err(Flow::Halt);
    }
    if let Some(chosen) = window.chosen {
        crate::kernel::fx::grant_technology(sx.state, who, chosen);
    }
    Ok(true)
}

/// "Choose 1 non-home, non-legendary planet other than Mecatol Rex in a system adjacent to one of your units with BOMBARDMENT": the owner picks and the
/// planet is destroyed. Nothing in range resolves nothing (`false`).
fn stellar_converter(sx: &mut Sx<'_>, who: Seat) -> Result<bool, Flow> {
    let targets = crate::driver::menu_cards::stellar_targets(sx.state, crate::query::reach::scope_of(sx.kernel), who);
    if targets.is_empty() {
        return Ok(false);
    }
    let mut window = crate::rules::relic_windows::StellarConverter::new(who, &targets);
    if run_window(sx, &mut window) == Flow::Halt {
        return Err(Flow::Halt);
    }
    Ok(window.destroyed.is_some())
}

/// "Choose a player, that player may spend 3 resources to place a structure on a planet they control. If they do not, they gain 1 trade good": the
/// owner names the builder, then the builder's own build window runs with the structure rules read for that seat.
fn titan_prototype(sx: &mut Sx<'_>, who: Seat) -> Result<bool, Flow> {
    use crate::rules::production::strategy_cards::TitanBuild;
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else { return Err(gap_flow(sx, TurnGap::NoRoster)) };
    let mut choose = crate::rules::relic_windows::TitanPrototype::new(who);
    if run_window(sx, &mut choose) == Flow::Halt {
        return Err(Flow::Halt);
    }
    let Some(builder) = choose.chosen else { return Ok(false) };
    let Some(facts) = crate::driver::windows::strategic::facts::structure_facts(sx.state, sx.kernel, roster, builder, &sx.table.codes.turn) else {
        return Err(gap_flow(sx, TurnGap::StrategyAbility));
    };
    let mut build = TitanBuild::new(builder, facts, sx.table.codes.turn.relic);
    if run_window(sx, &mut build) == Flow::Halt {
        return Err(Flow::Halt);
    }
    Ok(true)
}

/// The Silver Flame: "roll 1 die; on a 10 gain 1 victory point, otherwise purge your home system and bar the seat from public objectives". The card
/// is purged either way (`rules::explore::silver_flame`).
fn silver_flame(sx: &mut Sx<'_>, who: Seat) {
    use crate::state::{RollPurpose, RollRec};
    let face = u8::try_from(sx.die(crate::state::RngDomain::Dice, SILVER_FLAME_SIDES)).unwrap_or(u8::MAX);
    sx.state.ledger_mut().dice.push(RollRec {
        purpose: RollPurpose::Other(sx.table.codes.turn.silver_flame),
        owner: Some(who),
        unit: None,
        planet: None,
        scene: None,
        hits_on: None,
        faces: std::iter::once(face).collect(),
        rerolled: 0,
    });
    crate::rules::explore::silver_flame(sx.state, who, u32::from(face) == SILVER_FLAME_SIDES);
}

/// An Enigmatic Device exploration card in the play area (`exploration::perform_action`): the technology is asked first (among those the seat could
/// research by prerequisites, in the order of the alias text), then the six resources are paid, the card is purged and the technology researched. A use
/// that cannot be paid, or with nothing to research, fails and is withheld from the menu for the turn.
pub fn use_device(sx: &mut Sx<'_>, who: Seat, key: u32, card: crate::generated::Explore) -> Flow {
    let Some(plays) = sx.table.facts.plays.as_ref() else { return gap_flow(sx, TurnGap::NoRoster) };
    let (Some(roster), alias_order) = (plays.roster, plays.tech_order) else { return gap_flow(sx, TurnGap::NoRoster) };
    let scope = crate::query::reach::scope_of(sx.kernel);
    let hooks = crate::rules::production::strategy_cards::KernelHooks::new(sx.state, sx.kernel, who);
    let open = crate::query::tech::researchable_by_prerequisites(sx.state, scope, crate::query::reach::blanked(sx.state), who, &hooks);
    let mut options: Vec<Technology> = Technology::ALL.iter().copied().filter(|t| open.contains(*t)).collect();
    options.sort_by(|a, b| alias_order(*a, *b));
    if options.is_empty() {
        return conclude_quiet(sx, who, key, Verdict::Failed { withhold: true });
    }
    let mut window = crate::rules::relic_windows::GainTechnology::for_device(who, card, &options);
    if run_window(sx, &mut window) == Flow::Halt {
        return Flow::Halt;
    }
    let Some(chosen) = window.chosen else { return conclude_quiet(sx, who, key, Verdict::Failed { withhold: true }) };
    let mut bill = crate::rules::production::pay::PayBill::new(who, crate::query::economy::Spend::Resources, 6);
    if run_window(sx, &mut bill) == Flow::Halt {
        return Flow::Halt;
    }
    if bill.outcome != Some(true) {
        return conclude_quiet(sx, who, key, Verdict::Failed { withhold: true });
    }
    if let Some(p) = sx.state.players_mut().by_seat_mut(who)
        && let Some(at) = p.cards.explore.as_slice().iter().position(|held| *held == card)
    {
        p.cards.explore.remove(at); // purged
    }
    if !crate::rules::production::strategy_cards::Research::research_chosen(sx.state, sx.kernel, scope, *roster, who, chosen) {
        crate::kernel::fx::grant_technology(sx.state, who, chosen);
    }
    conclude_quiet(sx, who, key, Verdict::Resolved)
}
