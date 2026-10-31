// Lane G3: the registry of the rules of `silent::g3` against the string's, owner by owner and bucket by bucket (the body of
// `the_native_registry_equals_the_string_one_for_the_ported_abilities`, for the faction pairs of this lane; kept apart so the lanes that add pairs to
// that test do not meet here).

#[test]
fn the_registry_of_the_g3_rules_equals_the_string_one() {
    use crate::core_sync::{with_core, Footprint, Sources};
    use ti4_engine_core::state::Slice;
    for factions in g3_pairs() {
        let mut gs = crate::fixtures::seated_game(&factions, ti4_model::content_types::POK);
        gs.promissory_notes.clear();
        gs.promissory_faceup.clear();
        let resolver = crate::fixtures::armed_resolver(&gs);
        let view = resolver.kernel_view();
        let want: Vec<(String, String, String, bool, bool, bool, bool)> = view
            .abilities
            .iter()
            .filter(|a| slot_of(&a.id).is_some())
            .map(|a| (a.id.clone(), a.owner.clone(), a.event.clone(), a.when, a.optional, a.repeatable, a.has_condition))
            .collect();
        // The copies of Ssruu are in the comparison (a pair rule: owned by the borrower, the lender in the id).
        if factions == [("a", "yin"), ("b", "yssaril")] {
            assert!(want.iter().any(|x| x.0 == "leader:yin:a:yssarilagent:yinagent:SHIP_DESTROYED:after" && x.1 == "b"), "{want:?}");
        }
        if factions == [("a", "yssaril"), ("b", "letnev")] {
            // The string registers the borrowed round agents before any seat's slots, so the copy is offered first even when its borrower is
            // the earlier seat (the order of a window pick).
            let at = |id: &str| want.iter().position(|x| x.0 == id).unwrap_or_else(|| panic!("{id} in {want:?}"));
            assert!(at("leader:ssruu-copy:a:letnevagent:b:COMBAT_ROUND_STARTED:after") < at("reaction:yssaril:COMBAT_ROUND_STARTED:after"), "{want:?}");
        }
        if factions == [("a", "yssaril"), ("b", "sol")] {
            assert!(want.iter().any(|x| x.0 == "leader:ssruu-copy:a:solagent:b:GROUND_COMBAT_ROUND_STARTED:after" && x.1 == "a"), "{want:?}");
        }
        let (mut cx, r) = crate::dual_window::tables(&gs).unwrap();
        let before = gs.clone();
        let mut fol = Follow { table: table(), resolver: Resolver::new(&[Seat(0), Seat(1)], Some(Seat(0))), sources: ti4_model::content_types::POK };
        let mut got = Vec::new();
        let fp = Footprint::reads(&[Slice::Meta, Slice::Seats, Slice::Players, Slice::Board, Slice::Cards]);
        with_core(&mut gs, &mut cx, Sources::default(), fp, |core, _| {
            let order: Vec<Seat> = (0..r.seats.len()).filter_map(|i| u8::try_from(i).ok().map(Seat)).collect();
            fol.arm(&r, &before, core);
            let defs = fol.table.registry(core, &order, &mut |spec, who, lender| {
                ability_id(&r, &id_text(&r, spec, &crate::promissory::faction_name(&before, &ti4_model::id::PlayerId::new(r.seat_text(who))), r.seat_text(who), lender.map(|l| (crate::promissory::faction_name(&before, &ti4_model::id::PlayerId::new(r.seat_text(l))), r.seat_text(l).to_owned())).as_ref()))
            });
            for d in defs {
                got.push((
                    crate::kernel_sync::text::ability_text(&r, d.id),
                    r.seat_text(d.owner).to_owned(),
                    event_text(&r, d.event),
                    d.relation == ti4_engine_core::choice::reaction::Relation::When,
                    d.optional,
                    d.repeatable,
                    d.has_condition,
                ));
            }
        })
        .unwrap();
        // The string registers per seat in seat order; the order that matters is each owner's.
        for owner in ["a", "b"] {
            // Between windows the order is not observable (a bucket is one window): compare bucket by bucket,
            // each bucket in registration order (the sort is stable).
            let mut w: Vec<_> = want.iter().filter(|x| x.1 == owner).cloned().collect();
            let mut g: Vec<_> = got.iter().filter(|x| x.1 == owner).cloned().collect();
            w.sort_by(|a, b| (a.2.as_str(), a.3).cmp(&(b.2.as_str(), b.3)));
            g.sort_by(|a, b| (a.2.as_str(), a.3).cmp(&(b.2.as_str(), b.3)));
            let missing: Vec<_> = w.iter().filter(|x| !g.contains(x)).collect();
            let extra: Vec<_> = g.iter().filter(|x| !w.contains(x)).collect();
            assert!(missing.is_empty() && extra.is_empty(), "{factions:?} owner {owner}: the string registers {missing:?} and the core {extra:?}");
            if let Some(at) = g.iter().zip(&w).position(|(a, b)| a != b) {
                panic!("{factions:?} owner {owner}: the order of the registry differs at {at}: the core has {:?}, the string {:?}", &g[at.saturating_sub(1)..=(at + 1).min(g.len() - 1)], &w[at.saturating_sub(1)..=(at + 1).min(w.len() - 1)]);
            }
        }
    }
}

/// The pairs: every lobby faction seated first and second against a faction that shares none of its modules (the strings register the faction modules for every seat,
/// so each owner sees its own faction's rules and every module's rules for notes and technologies in one registry).
fn g3_pairs() -> Vec<[(&'static str, &'static str); 2]> {
    const LOBBY: [&str; 24] = ["arborec", "argent", "cabal", "empyrean", "ghost", "hacan", "jolnar", "l1z1x", "letnev", "mahact", "mentak", "muaat", "naalu", "naaz", "nekro", "nomad", "saar", "sardakk", "sol", "titans", "winnu", "xxcha", "yin", "yssaril"];
    let mut pairs = Vec::new();
    for faction in LOBBY {
        let other = if faction == "sol" { "hacan" } else { "sol" };
        pairs.push([("a", faction), ("b", other)]);
        pairs.push([("a", other), ("b", faction)]);
    }
    pairs
}
