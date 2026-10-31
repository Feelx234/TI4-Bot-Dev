//! The starting technology picks of setup (`starting_tech::{options, settle, choice, apply}`): Winnu pick any one technology without
//! prerequisites, Argent Flight two of a list, the Keleres (dropped) two that others own. They are asked one at a time at the start of
//! round 1's strategy phase, before the first card is drafted; a pick with nothing to choose is settled without a decision.
//!
//! The picks live in `Transient::misc.starting_tech_picks`; a pick is settled by [`settle_core`] (also run by every step's reconcile) and
//! answered by [`apply`], neither of which asks anything, so no job is needed.

use crate::choice::technology::{Ask as TAsk, StartRule, Subtype as TSub};
use crate::choice::{Ask, Choice, Opt, Opts, Origin};
use crate::choice::generic::Content;
use crate::generated::Technology;
use crate::kernel::fx;
use crate::kernel::fx_strategy::apply_unit_upgrades;
use crate::kernel::stage::Sx;
use crate::kernel::turn_codes::TurnGap;
use crate::kernel::Kernel;
use crate::query::tech::{self, UnitRoster};
use crate::query::reach;
use crate::rules::production::strategy_cards::KernelHooks;
use crate::state::{Code, CoreState, PlainKind, PlainRec, StartingTechPick, StartingTechSource};
use crate::tables::Scope;

use super::inline::{gap, Flow};

/// The technologies a pick offers now, in id order (`starting_tech::options`).
#[must_use]
pub fn options(core: &CoreState, k: &Kernel, pick: &StartingTechPick) -> Vec<Technology> {
    let scope = reach::scope_of(k);
    let Some(me) = core.players().by_seat(pick.player) else { return Vec::new() };
    let mut open: Vec<Technology> = match pick.source {
        StartingTechSource::Listed(list) => list.as_slice().to_vec(),
        StartingTechSource::OwnedByOthers => {
            let active = tech::active(scope);
            core.players()
                .iter()
                .filter(|p| p.seat != pick.player)
                .flat_map(|p| p.tech.owned.iter())
                .filter(|t| active.contains(*t) && t.data().faction.is_none())
                .collect()
        }
        StartingTechSource::Researchable => {
            let hooks = KernelHooks::new(core, k, pick.player);
            tech::researchable_by_prerequisites(core, scope, reach::blanked(core), pick.player, &hooks).iter().collect()
        }
    };
    open.retain(|t| !me.tech.owned.contains(*t));
    open.sort_unstable();
    open.dedup();
    open
}

fn gain(core: &mut CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster, who: crate::ids::Seat, tech: Technology) {
    fx::grant_technology(core, who, tech);
    apply_unit_upgrades(core, k, scope, roster, who);
}

/// `starting_tech::settle`: give out every pick that has nothing to choose, from the front of the line, until one is left that needs an
/// answer.
pub fn settle_core(core: &mut CoreState, k: &Kernel, scope: Scope, roster: &UnitRoster) {
    while let Some(pick) = core.transient().misc.starting_tech_picks.as_slice().first().copied() {
        let open = options(core, k, &pick);
        let forced: Vec<Technology> = match (pick.source, open.len()) {
            (_, 0) => Vec::new(),
            (StartingTechSource::Researchable, 1) => open.clone(),
            (StartingTechSource::Researchable, _) => break,
            (_, n) if n <= usize::from(pick.remaining) => open.clone(),
            _ => break,
        };
        for technology in &forced {
            gain(core, k, scope, roster, pick.player, *technology);
        }
        let given = u8::try_from(forced.len()).unwrap_or(u8::MAX);
        let left = if forced.is_empty() { 0 } else { pick.remaining.saturating_sub(given) };
        let picks = &mut core.transient_mut().misc.starting_tech_picks;
        if left == 0 {
            picks.remove(0);
        } else if let Some(front) = picks.as_mut_slice().first_mut() {
            front.remaining = left;
        }
    }
}

/// The reconcile's share (`Game::step_flow`: `starting_tech::settle` while picks are left).
pub fn settle(sx: &mut Sx<'_>) -> Flow {
    let Some(roster) = sx.table.facts.plays.as_ref().and_then(|p| p.roster) else {
        gap(sx, TurnGap::NoRoster);
        return Flow::Halt;
    };
    settle_core(sx.state, sx.kernel, reach::scope_of(sx.kernel), roster);
    Flow::Done
}

/// The question of the front pick (`starting_tech::choice`), `None` when nothing is asked.
#[must_use]
pub fn question(core: &CoreState, k: &Kernel, source: Code) -> Option<Choice> {
    let pick = *core.transient().misc.starting_tech_picks.as_slice().first()?;
    let open = options(core, k, &pick);
    if open.is_empty() {
        return None;
    }
    let rule = match pick.source {
        StartingTechSource::Listed(_) => StartRule::Listed,
        StartingTechSource::OwnedByOthers => StartRule::OwnedByOthers,
        StartingTechSource::Researchable => StartRule::Researchable,
    };
    let number = pick.of - pick.remaining + 1;
    Some(Choice {
        actor: pick.player,
        ask: Ask::Technology(TAsk::StartingTech { number, of: pick.of, rule }),
        ctx: crate::choice::Ctx {
            origin: Origin::Content(Content::Other(source)),
            subtype: crate::choice::Subtype::Technology(TSub::StartingTechnology),
            optional: false,
            target: None,
            outstanding: crate::containers::SmallList::new(),
            space_battle: false,
            invasion_seq: None,
        },
        opts: open.into_iter().map(|t| Opt::Technology(crate::choice::technology::Opt::Tech(t))).collect::<Opts>(),
    })
}

/// `starting_tech::apply`: the offered technology joins the seat, the pick is spent, the line settles, and `STARTING_TECHNOLOGY_CHOSEN`
/// is announced. `false`: `tech` is not offered (nothing changed).
pub fn apply(core: &mut CoreState, k: &Kernel, roster: &UnitRoster, technology: Technology) -> bool {
    let scope = reach::scope_of(k);
    let Some(pick) = core.transient().misc.starting_tech_picks.as_slice().first().copied() else { return false };
    if !options(core, k, &pick).contains(&technology) {
        return false;
    }
    gain(core, k, scope, roster, pick.player, technology);
    let picks = &mut core.transient_mut().misc.starting_tech_picks;
    if let Some(front) = picks.as_mut_slice().first_mut() {
        front.remaining = front.remaining.saturating_sub(1);
        if front.remaining == 0 {
            picks.remove(0);
        }
    }
    settle_core(core, k, scope, roster);
    fx::plain(core, PlainRec::bare(PlainKind::StartingTechnologyChosen));
    true
}
