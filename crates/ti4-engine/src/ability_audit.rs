//! The registry audit: every ability the string engine registers (any event, any relation) is either a rule of the core table, or listed below with what
//! becomes of it. A string ability without a core rule would be MISSING from a native game without any bail (`reactions_core::Recording::unported` exists
//! only while the string runs next to the core). A NEW string ability, or one that gets a rule, fails this test until the list says so.

#[cfg(test)]
mod tests {
    use crate::ability_gaps::GAP_IDS;
    use crate::reactions_core::slot_of;
    use ti4_engine_core_factions::silent::gaps::GAPS;

    /// What becomes of a string ability the core table has no rule for.
    #[derive(Clone, Copy)]
    enum Status {
        /// Out of the agreed content, by decision.
        Dropped(&'static str),
        /// Done another way (a hook, a mark the native windows read).
        Carried(&'static str),
        /// A rule on another lane's branch, not merged here yet: delete the line when that branch merges (the test fails when it does).
        Lane(&'static str),
        /// In scope and not ported: a LOUD gap. The row of `GAPS` (and `GAP_IDS`) makes a native game stop where a seat holds the card.
        Gap(usize),
    }

    /// The anonymised id of an ability (the faction of the owner left out).
    fn anonymous(id: &str) -> String {
        let mut parts: Vec<&str> = id.split(':').collect();
        if parts.len() > 2 {
            parts[1] = "*";
        }
        parts.join(":")
    }

    /// The string abilities of the 24 lobby factions (and a Yssaril third seat) without a core rule, sorted.
    fn missing() -> Vec<String> {
        let lobby = ["arborec", "argent", "cabal", "empyrean", "ghost", "hacan", "jolnar", "l1z1x", "letnev", "mahact", "mentak", "muaat", "naalu", "naaz", "nekro", "nomad", "saar", "sardakk", "sol", "titans", "winnu", "xxcha", "yin", "yssaril"];
        let mut missing = std::collections::BTreeSet::new();
        for faction in lobby {
            let other = if faction == "sol" { "hacan" } else { "sol" };
            for third in [None, Some("yssaril")] {
                let mut seats = vec![("a", faction), ("b", other)];
                if let Some(third) = third.filter(|t| *t != faction && *t != other) {
                    seats.push(("c", third));
                }
                let gs = crate::fixtures::seated_game(&seats, ti4_model::content_types::POK);
                let resolver = crate::fixtures::armed_resolver(&gs);
                for a in resolver.kernel_view().abilities.iter().filter(|a| slot_of(&a.id).is_none()) {
                    missing.insert(anonymous(&a.id));
                }
            }
        }
        missing.into_iter().collect()
    }

    const ACCOUNTED: &[(&str, Status)] = &[
        ("ability:*:aetherpassage:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("ability:*:awaken:SYSTEM_ACTIVATED:after", Status::Gap(9)),
        ("ability:*:council_patronage:STRATEGY_PHASE_BEGAN:after", Status::Dropped("a dropped faction")),
        ("ability:*:edict:GROUND_COMBAT_ENDED:when", Status::Lane("core-m6c-tactical")),
        ("ability:*:edict:SPACE_COMBAT_WON:when", Status::Lane("core-m6c-tactical")),
        ("ability:*:foresight:MOVEMENT_FINISHED:after", Status::Gap(0)),
        ("ability:*:incursion:SYSTEM_ACTIVATED:when", Status::Dropped("a dropped faction")),
        ("ability:*:incursion_remove:STATUS_PHASE_ENDED:after", Status::Dropped("a dropped faction")),
        ("ability:*:laws_order:TURN_BEGAN:after", Status::Dropped("a dropped faction")),
        ("ability:*:miniaturization:TACTICAL_ACTION_ENDED:after", Status::Dropped("a dropped faction")),
        ("ability:*:phoenixstandard:GROUND_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("ability:*:phoenixstandard:SPACE_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("ability:*:plot_extract:TECHNOLOGY_GAINED:after", Status::Dropped("a dropped faction")),
        ("ability:*:plot_seethe:STATUS_PHASE_BEGAN:after", Status::Dropped("a dropped faction")),
        ("ability:*:plot_siphon:COMMODITIES_GAINED:after", Status::Dropped("a dropped faction")),
        ("ability:*:plots_revealed:FACTION_FLIPPED:after", Status::Dropped("a dropped faction")),
        ("ability:*:reclamation_tactical:SYSTEM_ACTIVATED:after", Status::Carried("hooks::marks (the Winnu mark Reclamation reads)")),
        ("ability:*:sundered:MOVEMENT_FINISHED:after", Status::Dropped("a dropped faction")),
        ("ability:*:survivalinstinct:SYSTEM_ACTIVATED:after", Status::Dropped("a dropped faction")),
        ("breakthrough:*:arborecbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:argentbt:ACTION_COMPLETED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:argentbt:SYSTEM_ACTIVATED:when", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:argentbt_forget:TURN_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:argentbt_mark:SYSTEM_ACTIVATED:when", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:bastionbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:cabalbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:crimsonbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:deepwroughtbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:empyreanbt:SYSTEM_ACTIVATED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:empyreanbt_allow:SYSTEM_ACTIVATED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:firmamentbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:firmamentbt:STATUS_PHASE_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:mahactbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:mentakbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:muaatbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:muaatbt:SHIP_MOVED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:naazbt:ACTION_COMPLETED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:naazbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:naazbt:NAAZ_MECH_PLACED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:nomadbt:TURN_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:obsidianbt:GROUND_COMBAT_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:obsidianbt:SPACE_COMBAT_WON:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:obsidianbt:STATUS_PHASE_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:ralnelbt:PLAYER_PASSED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:saarbt:SAAR_DEORBIT_BARRAGE:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:saarbt_ready:STATUS_PHASE_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:sardakkbt:GROUND_COMBAT_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:sardakkbt:SPACE_COMBAT_ENDED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:titansbt_sleeper:TURN_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:winnubt:SYSTEM_ACTIVATED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:winnubt_clear:TURN_BEGAN:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:winnubt_consume:SHIP_MOVED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:yinbt:BREAKTHROUGH_GAINED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:yinbt:PUBLIC_OBJECTIVE_SCORED:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("breakthrough:*:yssarilbt:DEEPGLOOM_TRANSACTION:after", Status::Dropped("Thunder's Edge breakthrough")),
        ("leader:*:a:yssarilagent:b:saaragent:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:a:yssarilagent:c:saaragent:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:a:yssarilagent:ghostagent:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:a:yssarilagent:sardakkagent:TACTICAL_ACTION_ENDED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:arboreccommander:SYSTEM_ACTIVATED:after", Status::Gap(11)),
        ("leader:*:argentcommander:UNIT_ABILITY_ROLLED:when", Status::Gap(1)),
        ("leader:*:bastionagent:GROUND_FORCE_DESTROYED:when", Status::Dropped("a dropped faction")),
        ("leader:*:bastionagent:SHIP_DESTROYED:when", Status::Dropped("a dropped faction")),
        ("leader:*:bastionhero:GROUND_FORCE_DESTROYED:when", Status::Dropped("a dropped faction")),
        ("leader:*:bastionhero:SHIP_DESTROYED:when", Status::Dropped("a dropped faction")),
        ("leader:*:c:l1z1xagent:a:SYSTEM_ACTIVATED:after", Status::Gap(14)),
        ("leader:*:crimsoncommander:GROUND_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("leader:*:crimsoncommander:SPACE_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("leader:*:crimsonhero:SPACE_COMBAT_STARTED:after", Status::Dropped("a dropped faction")),
        ("leader:*:firmamentagent:SHIP_MOVED:after", Status::Dropped("a dropped faction")),
        ("leader:*:firmamentagent:SYSTEM_ACTIVATED:after", Status::Dropped("a dropped faction")),
        ("leader:*:ghostagent:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:keleresagent_clear:ACTION_COMPLETED:after", Status::Dropped("a dropped faction")),
        ("leader:*:kelerescommander:ACTION_COMPLETED:after", Status::Dropped("a dropped faction")),
        ("leader:*:kelerescommander_unlock:ACTION_CARD_PLAYED:after", Status::Dropped("a dropped faction")),
        ("leader:*:keleresherokuuasi:COMBAT_ROUND_STARTED:after", Status::Dropped("a dropped faction")),
        ("leader:*:keleresheroodlynn:AGENDA_RESOLVED:after", Status::Dropped("a dropped faction")),
        ("leader:*:keleresheroodlynn:AGENDA_REVEALED:after", Status::Dropped("a dropped faction")),
        ("leader:*:muaathero:MOVEMENT_FINISHED:after", Status::Gap(2)),
        ("leader:*:muaathero_moved:SHIP_MOVED:after", Status::Gap(3)),
        ("leader:*:nomadagentmercer:TACTICAL_ACTION_ENDED:after", Status::Gap(15)),
        ("leader:*:obsidianagent:SHIP_DESTROYED:when", Status::Dropped("a dropped faction")),
        ("leader:*:ralnelcommander:RETREAT_DECLARED:when", Status::Dropped("a dropped faction")),
        ("leader:*:ralnelhero:PLAYER_PASSED:after", Status::Dropped("a dropped faction")),
        ("leader:*:saaragent:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:sardakkagent:ACTION_COMPLETED:after", Status::Lane("core-m6c-tactical")),
        ("leader:*:sardakkagent_mark:SYSTEM_ACTIVATED:after", Status::Carried("hooks::marks (the T'ro mark of the end of the action)")),
        ("leader:*:yssarilcommander:SYSTEM_ACTIVATED:after", Status::Gap(13)),
        ("leader:*:yssarilcommander_clear:ACTION_COMPLETED:after", Status::Gap(4)),
        ("note:*:keleres_rider:AGENDA_REVEALED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:ambuscade:UNIT_ABILITY_ROLLED:when", Status::Gap(5)),
        ("promissory:*:blackops:TRANSACTION_RESOLVED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:crucible:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("promissory:*:malevolency:STATUS_PHASE_ENDED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:malevolency:TACTICAL_ACTION_ENDED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:nanolink:SYSTEM_ACTIVATED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:pop:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("promissory:*:raisethestandard:GROUND_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:raisethestandard:SPACE_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:sever_return:STATUS_PHASE_ENDED:after", Status::Dropped("a dropped faction")),
        ("promissory:*:share_knowledge_return:STATUS_PHASE_ENDED:after", Status::Dropped("a dropped faction")),
        ("technology:*:as:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("technology:*:asn_clear:ACTION_COMPLETED:after", Status::Dropped("a dropped faction (the Keleres technology Agency Supply Network)")),
        ("technology:*:hydrothermal:STATUS_PHASE_BEGAN:after", Status::Dropped("a dropped faction")),
        ("technology:*:nanomachines:STATUS_PHASE_ENDED:after", Status::Dropped("a dropped faction")),
        ("technology:*:nekroc4y:SHIP_DESTROYED:after", Status::Dropped("unclaimed by any faction (operator ruling 2026-10-07: an obscure variant, \"not part of the game\"; `nekro_units::TECHNOLOGIES`): no seat can gain it")),
        ("technology:*:ng:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("technology:*:parasite-firm:STATUS_PHASE_BEGAN:after", Status::Dropped("a dropped faction")),
        ("technology:*:parasite-obs:TURN_BEGAN:after", Status::Dropped("a dropped faction")),
        ("technology:*:planesplitter-firm:TECHNOLOGY_GAINED:after", Status::Dropped("a dropped faction")),
        ("technology:*:planesplitter-obs:STRATEGIC_ACTION_BEGAN:when", Status::Dropped("a dropped faction")),
        ("technology:*:proxima:GROUND_COMBAT_ROUND_BEGAN:after", Status::Dropped("a dropped faction")),
        ("technology:*:radical:STATUS_PHASE_BEGAN:after", Status::Dropped("a dropped faction")),
        ("technology:*:subatomic:SHIP_DESTROYED:after", Status::Dropped("a dropped faction")),
        ("technology:*:swa2:UNIT_ABILITY_ROLLED:after", Status::Gap(6)),
        ("technology:*:tcs:AGENT_EXHAUSTED:after", Status::Lane("core-m1-trade")),
        ("technology:*:yso:UNITS_PRODUCED:after", Status::Gap(7)),
        ("unit:*:arborec_flagship:SYSTEM_ACTIVATED:after", Status::Gap(12)),
        ("unit:*:bastion_mech:GROUND_FORCE_DESTROYED:after", Status::Dropped("a dropped faction")),
        ("unit:*:crimson_destroyer:GROUND_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("unit:*:crimson_destroyer:SPACE_COMBAT_ENDED:after", Status::Dropped("a dropped faction")),
        ("unit:*:deepwrought_mech:SYSTEM_ACTIVATED:when", Status::Dropped("a dropped faction")),
        ("unit:*:firmament_flagship:GROUND_COMBAT_ROUND_ENDED:after", Status::Dropped("a dropped faction")),
        ("unit:*:firmament_flagship:SPACE_COMBAT_ROUND_ENDED:after", Status::Dropped("a dropped faction")),
        ("unit:*:ghost_mech:SYSTEM_ACTIVATED:after", Status::Lane("core-m6c-tactical")),
        ("unit:*:keleres_flagship:SYSTEM_ACTIVATED:when", Status::Dropped("a dropped faction")),
        ("unit:*:naalu_mech_te:RELIC_GAINED:after", Status::Gap(8)),
        ("unit:*:obsidian_mech:FACTION_FLIPPED:after", Status::Dropped("a dropped faction")),
        ("unit:*:ralnel_flagship:FLAGSHIP_RETREATED:when", Status::Dropped("a dropped faction")),
        ("unit:*:ralnel_mech:GROUND_COMBAT_ROUND_ENDED:after", Status::Dropped("a dropped faction")),
        ("unit:*:titans_flagship:SYSTEM_ACTIVATED:after", Status::Gap(10)),
    ];

    #[test]
    fn every_ability_the_string_registers_is_ported_or_accounted_for() {
        let accounted: Vec<String> = ACCOUNTED.iter().map(|(id, _)| (*id).to_owned()).collect();
        let missing = missing();
        let unlisted: Vec<&String> = missing.iter().filter(|m| !accounted.contains(m)).collect();
        let gone: Vec<&String> = accounted.iter().filter(|a| !missing.contains(a)).collect();
        assert!(unlisted.is_empty(), "string abilities with no core rule and no entry in ACCOUNTED (port them, or list them with their reason and, if in scope, a row of GAPS): {unlisted:#?}");
        assert!(gone.is_empty(), "ACCOUNTED lists abilities the core has a rule for now (delete their lines, and their row of GAPS): {gone:#?}");
        let mut sorted = accounted.clone();
        sorted.sort();
        assert_eq!(sorted, accounted, "ACCOUNTED is sorted");
    }

    /// The rows of the loud-gap table are exactly the `Gap` entries: the same ability, the same event and relation, one row each.
    #[test]
    fn every_gap_entry_has_its_row_and_every_row_its_entry() {
        assert_eq!(GAPS.len(), GAP_IDS.len(), "GAPS and GAP_IDS are index-aligned");
        let r = crate::render::Render::new(Vec::new());
        for (index, id) in GAP_IDS.iter().enumerate() {
            let parts: Vec<&str> = id.split(':').collect();
            let (event, relation) = (parts[parts.len() - 2], parts[parts.len() - 1]);
            let row = &GAPS[index];
            assert_eq!(crate::kernel_sync::text::event_text(&r, row.event), event, "{id}: the row's event");
            assert_eq!(crate::kernel_sync::text::relation_text(row.relation), relation, "{id}: the row's relation");
            let kind = match row.kind {
                ti4_engine_core::choice::reaction::AbilityKind::Ability => "ability",
                ti4_engine_core::choice::reaction::AbilityKind::Leader => "leader",
                ti4_engine_core::choice::reaction::AbilityKind::Breakthrough => "breakthrough",
                ti4_engine_core::choice::reaction::AbilityKind::Technology => "technology",
                ti4_engine_core::choice::reaction::AbilityKind::Unit => "unit",
                ti4_engine_core::choice::reaction::AbilityKind::Promissory => "promissory",
                ti4_engine_core::choice::reaction::AbilityKind::Reaction => "reaction",
                ti4_engine_core::choice::reaction::AbilityKind::Other => "other",
            };
            assert_eq!(parts[0], kind, "{id}: the row's kind");
            let entries: Vec<usize> = ACCOUNTED.iter().filter_map(|(a, s)| matches!(s, Status::Gap(i) if *i == index).then_some(*a).map(|a| usize::from(a == *id))).collect();
            assert_eq!(entries, vec![1], "{id}: one ACCOUNTED entry names row {index}");
        }
        for (id, status) in ACCOUNTED {
            if let Status::Gap(index) = status {
                assert_eq!(GAP_IDS[*index], *id, "the entry of row {index}");
            }
        }
        // Every status says why.
        assert!(ACCOUNTED.iter().all(|(_, s)| match s {
            Status::Dropped(w) | Status::Carried(w) | Status::Lane(w) => !w.is_empty(),
            Status::Gap(i) => *i < GAPS.len(),
        }));
    }

    /// The live registry carries a row for every gap and shows it under the string's id with the owner's faction.
    #[test]
    fn a_native_game_registers_every_gap_for_every_seat_under_the_strings_id() {
        let mut table = crate::reactions_core::table();
        ti4_engine_core_factions::silent::gaps::register_live(&mut table);
        for (index, id) in GAP_IDS.iter().enumerate() {
            let spec = table.specs().iter().find(|s| s.slot == ti4_engine_core_factions::silent::gaps::slot(index)).expect("the row is registered");
            let r = crate::render::Render::new(Vec::new());
            let shown = crate::reactions_core::id_text(&r, spec, "hacan", "a", None);
            assert_eq!(shown, id.replacen(":*:", ":hacan:", 1), "row {index}");
        }
    }

    include!("ability_audit_g3_tests.rs");
}
