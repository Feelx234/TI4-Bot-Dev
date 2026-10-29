//! The old seams on the new ones (docs/ENGINE_DISENTANGLE.md 2.9, risk D12).
//!
//! * **A5 `Hooks` slots** keep their names and their registrations. [`bridge_legacy`] registers *one* provider per
//!   legacy slot on the matching seam, so a window can switch to `seam.<x>.eval/decide/collect` while the factions
//!   are still registered on the old slots; when a faction is ported natively it registers on the seam and the
//!   slot's bridge keeps answering for the rest. Same answer by construction (each bridge calls the dispatch
//!   method of the slot), so the hook-level dual stays at 0 differences.
//! * **B1 `rules::combat::Faction`** (13 methods) and **B3 `rules::production::Env`** (10 methods) become
//!   implementations of nothing: their methods turn into seam calls, table below. The dual-checked bodies on the
//!   string side are reused unchanged.
//!
//! | B1 `Faction` method | seam |
//! |---|---|
//! | `combat_modifier`, `unit_roll_modifier`, `mahact_flagship_bonus`, `nekro_mech_bonus`, `assail_space`, `law_fighter_bonus` | `hit_shift.eval` (the law term is the Ixth row of [`COMBAT_ROWS`](super::data::COMBAT_ROWS)) |
//! | `unit_dice`, `bastion_extra_die`, `round_agent_extra_die` | `dice.eval` |
//! | `may_sustain_modules` | `may_sustain.decide(.., true)` |
//! | `grants_sustain` (the module hook; Nomad's mech and The Cavalry) | `grants_sustain.decide(.., false)` (the allow side, added by CE) |
//! | `borrowed_stats` | `borrowed_stats.first` |
//! | `alastor_participants` | `alastor_participants.collect` |
//! | `flagship_has_text(.., FlagshipText)` | not a seam: generated `Unit -> FlagshipTextSet` (string step S5) plus the Nekro lender as a `silenced`-style predicate; the `FlagshipText` enum and its three neutral stubs are deleted |
//!
//! | B3 `Env` method | seam |
//! |---|---|
//! | `planet_spend_value`, `resource_bonus` | `planet_spend.eval` (resource bonus is the `bastion_units_resource_bonus` provider) |
//! | `extra_production`, `extra_production_planet` | `extra_production.eval` |
//! | `has_cannot_produce`, `cannot_produce` | `cannot_produce.decide` (and `.is_empty()` for the probe) |
//! | `silenced_in_space`, `abilities_lost`, `abilities_usable` | `silenced.decide` (`abilities_usable` is an `Allow`-side Entropic Scars rule from the technology provider) |
//! | `is_space_station` | not a seam: a generated planet property |
//!
//! The asking half (B3 `produce_one_from`, Keleres and Winnu agents, hero) is P6: `offers` at
//! `Point::{PaymentWindow, ProductionStart, ProductionEnd}`; the string `Table::ask` calls become stages.

use super::{
    CannonSite, CombatCtx, CommitSite, DeploySite, DestSite, Delta, ExchangeSite, Layer, Obs, ObsKind, Phase, ProdSite, ProduceSite, Q,
    Replacement, RollKind, RollSite, RuleRef, Seams, SeatSystemSite, SetWhy, SilenceCtx, SilenceSite, Source, SpendSite, Step, Verdict,
};
use super::gather::Pass;
use crate::kernel::Next;
use crate::state::Piece;

/// `Source::Rule` code of a bridge (the legacy slot is not one faction).
pub const LEGACY: Source = Source::Rule(RuleRef(9000));

fn add(n: i64) -> Delta {
    if n == 0 { Delta::None } else { Delta::Add(n) }
}

fn combat_only(site: &RollSite) -> bool {
    site.kind == RollKind::Combat
}

/// Register one bridge per legacy slot. Call once per kernel, before the faction modules.
pub fn bridge_legacy(seams: &mut Seams) {
    // P1 hit_shift: the summed slots and direct bonuses of today's `effective_from`.
    seams.hit_shift.push_add(LEGACY, Layer::Faction, |q, site, _| {
        if !combat_only(site) {
            return Delta::None;
        }
        let (h, u) = (&q.k.hooks, &site.unit);
        add(h.general.combat_modifier(q.s, q.k, u.owner, u.ctx) + h.general.unit_roll_modifier(q.s, q.k, *u))
    });
    seams.hit_shift.push_add(LEGACY, Layer::Faction, |q, site, _| {
        if !combat_only(site) {
            return Delta::None;
        }
        let (d, u) = (&q.k.hooks.direct, &site.unit);
        add(d.mahact_units_flagship_roll_bonus(q.s, q.k, u.owner, u.unit)
            + d.nekro_units_mech_roll_bonus(q.s, q.k, u.owner, u.unit, u.system, None)
            + d.firmament_plots_assail_space(q.s, q.k, u.owner, u.system))
    });
    // P1 dice: unit_dice is a fold (it may set), then the two extra-die providers.
    seams.dice.push_set(LEGACY, Layer::Faction, |q, site, acc| {
        let to = q.k.hooks.general.unit_dice(q.s, q.k, site.unit, acc);
        if to == acc { Delta::None } else { Delta::Set { to, why: SetWhy::LegacyFold } }
    });
    seams.dice.push_add(LEGACY, Layer::Faction, |q, site, _| {
        let u = &site.unit;
        let bastion = u.system.map_or(0, |_| {
            // The string hook takes the unit as the board holds it; the site names its type and owner.
            i64::try_from(q.k.hooks.direct.bastion_extra_die(q.s, Piece::new(u.unit, u.owner))).unwrap_or(0)
        });
        let agent = u.system.map_or(0, |sys| {
            q.k.hooks.direct.borrowed_round_agents_extra_die_for(q.s, q.k, sys, u.planet, u.owner, u.unit, usize::from(site.unit_index), site.round)
        });
        add(bastion + agent)
    });
    // P1 production and payment.
    seams.planet_spend.push_set(LEGACY, Layer::Faction, |q, site: &SpendSite, acc| {
        let to = q.k.hooks.economy.planet_spend_value(q.s, q.k, site.who, site.planet, site.spend, acc);
        if to == acc { Delta::None } else { Delta::Set { to, why: SetWhy::LegacyFold } }
    });
    seams.planet_spend.push_add(LEGACY, Layer::Faction, |q, site: &SpendSite, _| add(q.k.hooks.direct.bastion_units_resource_bonus(q.s, site.planet)));
    seams.extra_production.push_add(LEGACY, Layer::Faction, |q, site: &ProdSite, _| {
        let e = &q.k.hooks.economy;
        add(e.extra_production(q.s, q.k, site.who, site.system) + site.planet.map_or(0, |p| e.extra_production_planet(q.s, q.k, site.who, site.system, p)))
    });
    seams.production_discount.push_add(LEGACY, Layer::Faction, |q, site: &ProdSite, _| {
        add(q.k.hooks.economy.production_cost_reduction(q.s, site.who, site.system))
    });
    // P2 predicates.
    seams.may_sustain.push(LEGACY, |q, u| if q.k.hooks.combat.may_sustain(q.s, q.k, *u) { Verdict::Abstain } else { Verdict::Forbid });
    seams.grants_sustain.push(LEGACY, |q, u| if q.k.hooks.combat.grants_sustain(q.s, q.k, *u) { Verdict::Allow } else { Verdict::Abstain });
    seams.hit_immune.push(LEGACY, |q, u| if q.k.hooks.combat.direct_hit_immune(q.s, q.k, *u) { Verdict::Allow } else { Verdict::Abstain });
    seams.cannon_barred.push(LEGACY, |q, c: &CannonSite| {
        if q.k.hooks.combat.space_cannon_barred(q.s, q.k, c.shooter, c.target, c.system) { Verdict::Allow } else { Verdict::Abstain }
    });
    seams.cannot_produce.push(LEGACY, |q, c: &ProduceSite| {
        if q.k.hooks.economy.cannot_produce(q.s, q.k, c.who, c.unit, c.producer) { Verdict::Allow } else { Verdict::Abstain }
    });
    seams.silenced.push(LEGACY, |q, c: &SilenceSite| {
        let d = &q.k.hooks.direct;
        let piece = c.unit.map(|u| Piece::new(u, c.owner));
        let lost = d.crimson_abilities_lost(q.s, c.owner, c.system);
        let ralnel = c.ctx == SilenceCtx::Space && piece.is_some_and(|p| d.ralnel_silenced_in_space(q.s, q.k, p));
        let ctx = match c.ctx {
            SilenceCtx::Space | SilenceCtx::Production => Some(CombatCtx::Space),
            SilenceCtx::Ground => Some(CombatCtx::Ground),
        };
        let by_ctx = piece.zip(ctx).is_some_and(|(p, x)| d.context_silenced(q.s, q.k, p, x));
        if lost || ralnel || by_ctx { Verdict::Allow } else { Verdict::Abstain }
    });
    seams.commit_blocked.push(LEGACY, |q, c: &CommitSite| {
        if q.k.hooks.direct.keleres_units_commit_blocked(q.s, q.k, c.who, c.system, c.planet) { Verdict::Allow } else { Verdict::Abstain }
    });
    // P3 gathers.
    seams.alastor_participants.push(LEGACY, Pass::Add, |q, c: &SeatSystemSite, out| {
        out.extend_from_slice(&q.k.hooks.direct.nekro_units_alastor_participants(q.s, c.who, c.system));
    });
    seams.production_destinations.push(LEGACY, Pass::Add, |q, c: &DestSite, out| {
        out.extend_from_slice(&q.k.hooks.economy.production_destinations(q.s, q.k, c.who, c.system, c.base));
    });
    seams.deploy_planets.push(LEGACY, Pass::Add, |q, c: &DeploySite, out| {
        out.extend_from_slice(&q.k.hooks.direct.crimson_deploy_planets(q.s, q.k, c.who, c.system, &c.landable));
    });
    // P4 replacements.
    seams.borrowed_stats.push(Replacement {
        source: LEGACY,
        step: Step::BorrowedStats,
        offer: |q, c: &SeatSystemSite| q.k.hooks.combat.borrowed_stats(q.s, q.k, c.who, c.system),
        perform: |_, _, _| Next::Done,
    });
    seams.unit_exchange.push(Replacement {
        source: LEGACY,
        step: Step::ProductionExchange,
        offer: |q, c: &ExchangeSite| q.k.hooks.economy.production_unit_exchange(q.s, q.k, c.who, c.system, c.unit).then_some(()),
        perform: |cx, c, ()| {
            let k = cx.kernel;
            k.hooks.economy.production_unit_exchange_performed(cx, c.who, c.system, c.unit);
            Next::Done
        },
    });
    // P5 observers.
    seams.observers.push(ObsKind::ShipLost, Phase::After, |cx, o| {
        if let Obs::ShipLost { system, piece } = *o {
            let k = cx.kernel;
            k.hooks.direct.bastion_note_ship_lost(cx, system, piece);
        }
    });
    seams.observers.push(ObsKind::ProductionWindowClosed, Phase::After, |cx, o| {
        if let Obs::ProductionWindowClosed { who, system } = *o {
            let k = cx.kernel;
            k.hooks.direct.argent_clear_agent_production_destinations(cx, who, system);
            // Keleres' agent window is not closed here: a placement does not end the payment window, the caller of
            // the production does (`keleres::close_agent_window`, string), after the use.
        }
    });
    seams.observers.push(ObsKind::Replenished, Phase::After, |cx, o| {
        if let Obs::Replenished { who } = *o {
            let k = cx.kernel;
            k.hooks.direct.cabal_note_replenished(cx, who);
        }
    });
    seams.observers.push(ObsKind::CombatRoundStarted, Phase::Before, |cx, o| {
        if let Obs::CombatRoundStarted { who, .. } = *o {
            let k = cx.kernel;
            for seat in who.iter() {
                k.hooks.general.space_combat_round_started(cx, seat);
            }
        }
    });
}

/// A `Q`-taking helper for windows that still hold a `(&CoreState, &Kernel)` pair.
#[must_use]
pub const fn q<'a>(s: &'a crate::state::CoreState, k: &'a crate::kernel::Kernel) -> Q<'a> {
    Q::new(s, k)
}
