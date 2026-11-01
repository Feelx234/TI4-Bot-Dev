//! The `Env`s the rule areas ask of "the factions and rules not yet in the core" (package KF3): `leaders::Env`, `strategy::Env`
//! and `abilities::Env`, answered from the state and the hook table of the kernel alone. Until now they were implemented only by
//! the string engine's views (`LeaderView`, `StratView`), which read the string position; the live loop cannot use those.
//!
//! One struct implements the three traits (their method names do not collide). The pattern is `rules::trade::KernelEnv`: a
//! hook slot answers where the string called `factions::*`, a core query answers where it read the content store, the map or the
//! supply. The agreement with the string views is a lockstep, not an argument: `ti4-engine/src/kernel_env_tests.rs` asks both of
//! every seat of the positions of a bot game (trap T12: the oracle compares the core with itself otherwise).
//!
//! Documented departures (content outside the agreed scope): `has_specialty_research_planet` is the Jol-Nar breakthrough
//! (Thunder's Edge: refused), `other_action_resolves` for the Thunder's Edge Xxcha hero (`XxekirGromTe`) answers `false`, and
//! the Keleres agent term of `leadership_affordable`/`technology_secondary_affordable` is dropped with the faction.
//!
//! L5: it reads the rules of every area below.

use crate::generated::{Leader, Planet, StrategyCardKind, Technology};
use crate::ids::Seat;
use crate::kernel::hooks::CombatCtx;
use crate::kernel::Kernel;
use crate::query::economy::Spend;
use crate::query::{objectives, reach};
use crate::rules::{abilities, leaders, strategy};
use crate::state::CoreState;
use crate::SysSet;

/// Influence one Leadership token costs (`strategy_cards::INFLUENCE_PER_TOKEN`).
const INFLUENCE_PER_TOKEN: i64 = 3;
/// `strategy_cards::TECHNOLOGY_SECONDARY_COST`.
const TECHNOLOGY_SECONDARY_COST: i64 = 4;

/// The state and the kernel: what every method reads.
#[derive(Clone, Copy)]
pub struct KernelEnv<'a> {
    pub core: &'a CoreState,
    pub kernel: &'a Kernel,
}

impl<'a> KernelEnv<'a> {
    #[must_use]
    pub const fn new(core: &'a CoreState, kernel: &'a Kernel) -> Self {
        Self { core, kernel }
    }

    /// `production::available`: planets (their larger face), the Triad, trade goods at their worth, War Machine for resources.
    fn available(&self, who: Seat, spend: Spend) -> i64 {
        let cx = crate::rules::production::Cx::new(self.core, self.kernel, reach::scope_of(self.kernel), reach::blanked(self.core));
        crate::rules::production::available(&cx, who, spend, &crate::rules::production::Extras::default())
    }

    /// Z'eu: a system with state (the string `GameState::board` holds the systems anything happened in: `SysFlags::TOUCHED`) that is no
    /// player's home and holds none of its command tokens; with no such system at all the map is "all still open" and it resolves.
    fn zeu_resolves(&self, who: Seat) -> bool {
        let homes = |s: crate::generated::System| self.core.players().iter().any(|p| p.home_system == Some(s));
        let touched = || self.core.board().systems().filter(|(_, c)| c.flags.has(crate::state::SysFlags::TOUCHED));
        touched().next().is_none() || touched().any(|(s, c)| !homes(s) && !c.cmd.contains(who))
    }

    /// Doctor Sucaban's swap (Jol-Nar hero): a technology of a colour the seat has another of to swap for (`leaders::can_resolve_action`).
    fn sucaban_resolves(&self, who: Seat) -> bool {
        let Some(p) = self.core.players().by_seat(who) else { return false };
        p.tech.owned.iter().any(|t| {
            !crate::query::tech::is_unit_upgrade(t)
                && Technology::ALL.iter().any(|o| {
                    !p.tech.owned.contains(*o) && !crate::query::tech::is_unit_upgrade(*o) && o.data().types.first() == t.data().types.first()
                })
        })
    }
}

impl abilities::Env for KernelEnv<'_> {
    fn combat_hook(&self, who: Seat, ctx: CombatCtx) -> i64 {
        self.kernel.hooks.general.combat_modifier(self.core, self.kernel, who, ctx)
    }
    fn fleet_supply_hook(&self, who: Seat, limit: i32) -> i32 {
        self.kernel.hooks.general.fleet_supply(self.core, self.kernel, who, limit)
    }
    fn status_tokens_hook(&self, who: Seat, count: i32) -> i32 {
        self.kernel.hooks.general.status_tokens(self.core, self.kernel, who, count)
    }
    fn waived_hook(&self, who: Seat, tech: Technology) -> usize {
        self.kernel.hooks.general.waived_prerequisites(self.core, self.kernel, who, tech)
    }
    fn substitutes_primary_hook(&self, who: Seat, card: Option<StrategyCardKind>) -> bool {
        self.kernel.hooks.general.substitutes_primary(self.core, self.kernel, who, card)
    }
    fn trades_action_cards_hook(&self, who: Seat) -> bool {
        self.kernel.hooks.general.trades_action_cards(self.core, self.kernel, who)
    }
    fn ignores_neighbours_hook(&self, who: Seat) -> bool {
        self.kernel.hooks.general.ignores_neighbours(self.core, self.kernel, who)
    }
    fn secondary_is_free_hook(&self, who: Seat, card: Option<StrategyCardKind>) -> bool {
        self.kernel.hooks.general.secondary_is_free(self.core, self.kernel, who, card)
    }
}

impl strategy::Env for KernelEnv<'_> {
    /// Masters of Trade (Trade) or a module's hook (`rules::abilities::secondary_is_free`). The hook alone missed Hacan's printed ability.
    fn secondary_is_free(&self, who: Seat, card: Option<StrategyCardKind>) -> bool {
        // Hacan's Masters of Trade waive Trade, and the modules' hooks (`faction_abilities::secondary_is_free`).
        abilities::secondary_is_free(&abilities::Cx { core: self.core, env: self }, who, card)
    }
    fn substitutes_primary(&self, who: Seat, card: Option<StrategyCardKind>) -> bool {
        // Jol-Nar's Brilliant substitutes the primary of Technology (`faction_abilities::substitutes_primary`).
        abilities::substitutes_primary(&abilities::Cx { core: self.core, env: self }, who, card)
    }
    fn leadership_affordable(&self, who: Seat) -> bool {
        self.available(who, Spend::Influence) >= INFLUENCE_PER_TOKEN
    }
    fn can_research(&self, who: Seat) -> bool {
        !crate::rules::production::strategy_cards::Research::researchable_for(self.core, self.kernel, reach::scope_of(self.kernel), who).is_empty()
    }
    fn has_specialty_research_planet(&self, _: Seat) -> bool {
        false
    }
    /// `payment::affordable(TECHNOLOGY_SECONDARY_COST, Resources)`: the printed resources of the planets the seat can spend and its trade goods. NOT
    /// `production::available`, which adds what a production step may spend: War Machine's +5, the Triad and the larger face of a planet. Found by the shadow
    /// (sol 4p, arborec 4p, cabal 4p): a seat that had played War Machine in the activation was asked to follow the Technology card and could not pay.
    fn technology_secondary_affordable(&self, who: Seat) -> bool {
        let cx = crate::rules::production::Cx::new(self.core, self.kernel, reach::scope_of(self.kernel), reach::blanked(self.core));
        let planets: i64 = crate::rules::production::spendable_planets(&cx, who, &crate::rules::production::Extras::default())
            .as_slice()
            .iter()
            .map(|p| crate::rules::production::planet_value(&cx, who, *p, Spend::Resources))
            .sum();
        let goods = self.core.players().by_seat(who).map_or(0, |p| i64::from(p.econ.trade_goods)) * crate::rules::production::trade_good_worth(&cx, who); // mutants:skip (a seat that is not seated has no goods: the default is unreachable)
        planets + goods >= TECHNOLOGY_SECONDARY_COST
    }
    fn secret_hand_all_scored(&self, who: Seat) -> bool {
        let limit = objectives::HAND_LIMIT + crate::query::relic::secret_objective_bonus(self.core, who);
        self.core.players().by_seat(who).is_some_and(|p| p.cards.secrets.is_empty()) && self.core.objectives().scored[usize::from(who.0)].secret.len() >= limit
    }
    fn can_produce_in(&self, who: Seat, system: crate::generated::System) -> bool {
        let cx = crate::rules::production::Cx::new(self.core, self.kernel, reach::scope_of(self.kernel), reach::blanked(self.core));
        crate::rules::production::capacity(&cx, who, system) > 0
    }
}

impl leaders::Env for KernelEnv<'_> {
    fn faction_commander_unlocked(&self, who: Seat, leader: Leader) -> Option<bool> {
        self.kernel.hooks.general.commander_unlocked(self.core, self.kernel, who, leader)
    }
    fn resource_bonus(&self, planet: Planet) -> i64 {
        self.kernel.hooks.direct.bastion_units_resource_bonus(self.core, planet)
    }
    fn mecatol_area(&self) -> Option<SysSet> {
        // A position without the map slice is a game without a galaxy: the string's `galaxy: None`.
        if !self.core.loaded.has(crate::state::Slice::Map) {
            return None;
        }
        let g = self.core.galaxy()?;
        let mecatol = objectives::mecatol_in_galaxy(self.core);
        let mut set = g.adjacent(mecatol);
        set.insert(mecatol.index());
        Some(set)
    }
    fn leader_action(&self, who: Seat, leader: Leader) -> Option<bool> {
        self.kernel.hooks.general.leader_action(self.core, self.kernel, who, leader)
    }
    fn imperia_grants(&self, who: Seat, commander: Leader) -> bool {
        self.kernel.hooks.direct.mahact_imperia_grants(self.core, who, commander)
    }
    fn other_action_resolves(&self, who: Seat, leader: Leader) -> bool {
        match leader {
            Leader::Zeu => self.zeu_resolves(who),
            Leader::RinTheMastersLegacy => self.sucaban_resolves(who),
            // Xxekir Grom (Thunder's Edge Xxcha hero): a controlled planet to place on and a PDS in the box (the string also counts the mech; the
            // form of each is the route's, `driver::turn::leader`).
            Leader::XxekirGromTe => {
                !crate::query::board::controlled_planets(self.core, who).as_slice().is_empty()
                    && crate::query::fleet::remaining_all(self.core, crate::query::reach::scope_of(self.kernel), who, crate::generated::Unit::PDSI) > 0
            }
            _ => false,
        }
    }
}
