//! mahact: tier W: offers, replacements and anything that asks (docs/ENGINE_CORE_FACTIONS_PLAN.md, package CC).
//! Owned by CC only.
//!
//! * `strategy.secondary_waived` (Jae Mir Kan, `mahact::secondary_waived`; lane G4): the waiver the follower took in place of a strategy token --
//!   exhaust the agent (`fx_strategy::exhaust_agent`: the kernel's leader exhaust, which tells the Nomad's Temporal Command Suite with a staged
//!   `AGENT_EXHAUSTED`) and take the active player's command token off the system the waiver names, back into their reinforcements.
//!
//! Not ported (window owners), with the reason:
//!
//! * `mahact_units_offer_starlancer` (asks each eligible holder, then `strategy_cards::gain_tokens`): B-window `Ask` family.
//! * `timing_abilities` (Edict, Imperia, Scepter, the Legionnaires), `commander_unlocked`, `component_actions`,
//!   `perform_component`, `leader_action`, `use_leader_timed` (Airo Shir Aur): B7.
//! * `add_token` / `remove_token` / `store` (the pool writers) come with the abilities that call them.

use ti4_engine_core::generated::{Faction, Leader, System};
use ti4_engine_core::hooks::{HookTable, Waiver};
use ti4_engine_core::ids::Seat;
use ti4_engine_core::kernel::{fx, fx_strategy, Cx};
use ti4_engine_core::query::board;
use ti4_engine_core::state::LeaderStatus;

use super::query::is_mahact;

/// Fill the slots of this tier.
pub fn register(t: &mut HookTable) {
    t.strategy.secondary_waived.push(secondary_waived);
}

/// Pay the waiver of Jae Mir Kan: `waiver.code` is the index of the system holding the active player's token (`secondary_waivers`). The payment
/// is made only if the token is still there, the follower is the Mahact and the agent is ready (a waiver of another module is not this one's).
pub fn secondary_waived(cx: &mut Cx<'_>, follower: Seat, primary: Seat, waiver: Waiver) {
    if waiver.module != Faction::TheMahactGeneSorcerers {
        return;
    }
    let Some(system) = System::from_index(usize::from(waiver.code)) else { return };
    let ready = cx.state.players().by_seat(follower).is_some_and(|p| p.leaders.status.get(&Leader::JaeMirKan) == Some(&LeaderStatus::Readied));
    if !board::has_token(cx.state, system, primary) || !is_mahact(cx.state, follower) || !ready {
        return;
    }
    if fx_strategy::exhaust_agent(cx.state, cx.kernel, follower, Leader::JaeMirKan) {
        fx::remove_command_token(cx.state, primary, system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::testkit::table;
    use ti4_engine_core::kernel::{Cx, FirstOption};
    use ti4_engine_core::kernel::Kernel;

    fn pay(s: &mut ti4_engine_core::state::CoreState, k: &Kernel, follower: u8, primary: u8, waiver: Waiver) {
        let mut asker = FirstOption;
        let mut cx = Cx::new(s, k, &mut asker);
        secondary_waived(&mut cx, Seat(follower), Seat(primary), waiver);
    }

    fn jae(system: System) -> Waiver {
        Waiver { module: Faction::TheMahactGeneSorcerers, code: u16::try_from(system.index()).unwrap() }
    }

    #[test]
    fn the_registered_hook_pays_the_waiver_the_follower_took() {
        // Through the table every faction fills (the slot the secondary window calls), not the function alone.
        let (mut s, mut k) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan]);
        let mut hooks = ti4_engine_core::hooks::HookTable::new();
        crate::register_all(&mut hooks);
        k.hooks = hooks;
        let system = System::ALL[3];
        fx::place_command_token(&mut s, Seat(1), system);
        s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Readied);
        let mut asker = FirstOption;
        let mut cx = Cx::new(&mut s, &k, &mut asker);
        k.hooks.strategy.secondary_waived(&mut cx, Seat(0), Seat(1), jae(system));
        assert!(!board::has_token(&s, system, Seat(1)));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&LeaderStatus::Exhausted));
    }

    #[test]
    fn the_agent_is_exhausted_and_the_active_players_token_leaves_the_system() {
        let (mut s, k) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan]);
        let system = System::ALL[3];
        fx::place_command_token(&mut s, Seat(1), system);
        s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Readied);
        pay(&mut s, &k, 0, 1, jae(system));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&LeaderStatus::Exhausted));
        assert!(!board::has_token(&s, system, Seat(1)), "the token is off the board");
    }

    #[test]
    fn nothing_is_paid_without_a_ready_agent_a_token_there_the_mahact_or_the_mahact_waiver() {
        let build = || {
            let (mut s, k) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan]);
            let system = System::ALL[3];
            fx::place_command_token(&mut s, Seat(1), system);
            s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Readied);
            (s, k, system)
        };
        // Exhausted already.
        let (mut s, k, system) = build();
        s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Exhausted);
        pay(&mut s, &k, 0, 1, jae(system));
        assert!(board::has_token(&s, system, Seat(1)));
        // No token of the active player in the system named.
        let (mut s, k, system) = build();
        pay(&mut s, &k, 0, 1, jae(System::ALL[4]));
        assert!(board::has_token(&s, system, Seat(1)));
        assert_eq!(s.players().by_seat(Seat(0)).unwrap().leaders.status.get(&Leader::JaeMirKan), Some(&LeaderStatus::Readied));
        // Another faction's waiver code.
        let (mut s, k, system) = build();
        pay(&mut s, &k, 0, 1, Waiver { module: Faction::TheNaaluCollective, code: u16::try_from(system.index()).unwrap() });
        assert!(board::has_token(&s, system, Seat(1)));
        // The follower is not the Mahact.
        let (mut s, k, system) = build();
        s.players_mut().edit(0).faction = Faction::TheEmiratesOfHacan;
        pay(&mut s, &k, 0, 1, jae(system));
        assert!(board::has_token(&s, system, Seat(1)));
    }

    #[test]
    fn the_nomads_temporal_command_suite_hears_the_exhaustion() {
        use ti4_engine_core::hooks::HookTable;
        let (mut s, mut k) = table(&[Faction::TheMahactGeneSorcerers, Faction::TheEmiratesOfHacan]);
        let mut hooks = HookTable::new();
        hooks.direct.nomad_watches_agent_exhaustion.push(|_| true);
        k.hooks = hooks;
        let system = System::ALL[3];
        fx::place_command_token(&mut s, Seat(1), system);
        s.players_mut().edit(0).leaders.status.insert(Leader::JaeMirKan, LeaderStatus::Readied);
        pay(&mut s, &k, 0, 1, jae(system));
        assert!(s.pending().staged.iter().any(|e| e.kind == ti4_engine_core::state::EventKind::AgentExhausted), "the staged event is the Nomad's to hear");
    }
}
