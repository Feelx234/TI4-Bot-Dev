// Lane G3 (`include!`d into `ability_audit::tests`): the events the rules of `silent::g3` listen to, each with who announces it natively in this base -- or the honest "nobody", so a rule that
// waits on such an event is tested against the core and the string's own tests under `TI4_AREAS=all=dual` only, never in lockstep (part c of `docs/BAIL_MAP.md`: listeners without an emitter).

/// `(event, the native emitter, or `None` when nothing in this base announces it)`.
const G3_EMITTERS: &[(&str, Option<&str>)] = &[
    ("COMMAND_TOKEN_PLACED", Some("the tactical activation (`rules::tactical::activate`)")),
    ("GROUND_COMBAT_ENDED", Some("the invasion window")),
    ("GROUND_COMBAT_STARTED", Some("the invasion window")),
    ("PILLAGE_USED", Some("`silent::pillage`, through `g3::suffi_an::after_pillage` (only where a Ssruu copy could answer, like the string)")),
    ("PLANET_CONTROL_GAINED", Some("the invasion window (`rules::invasion::control`)")),
    ("SPACE_COMBAT_ROUND_ENDED", Some("the combat flow (`rules::combat::flow`)")),
    ("SPACE_COMBAT_STARTED", Some("the combat window")),
    ("SPACE_COMBAT_WON", Some("the aftermath (`driver::windows::aftermath`)")),
    ("STATUS_PHASE_ENDED", None),
    ("STRATEGIC_ACTION_BEGAN", None),
    ("STRATEGY_PHASE_BEGAN", Some("the phase arm")),
    ("SYSTEM_ACTIVATED", Some("the tactical activation (`rules::tactical::activate`)")),
    ("TECHNOLOGY_GAINED", Some("the turn prelude's reconcile (a technology nobody announced)")),
    ("TURN_BEGAN", Some("the turn prelude")),
    ("UNITS_COMMITTED", Some("the invasion window")),
];

/// Every rule of `silent::g3` (slots `42_000..42_100`) listens to an event listed above; a new rule on another event fails this until the list says who announces it.
#[test]
fn every_event_a_g3_rule_listens_to_has_a_noted_emitter_or_is_noted_to_have_none() {
    let table = crate::reactions_core::table();
    let r = crate::render::Render::new(Vec::new());
    let mut seen = std::collections::BTreeSet::new();
    for spec in table.specs().iter().filter(|s| (42_000..42_100).contains(&s.slot.0)) {
        let event = crate::kernel_sync::text::event_text(&r, spec.event);
        assert!(G3_EMITTERS.iter().any(|(name, _)| *name == event), "slot {}: the event {event} has no entry in G3_EMITTERS", spec.slot.0);
        seen.insert(event);
    }
    assert!(seen.len() >= 12, "the rules listen to {} events: {seen:?}", seen.len());
    // The list has no stale entry either.
    for (name, _) in G3_EMITTERS {
        assert!(seen.contains(*name), "G3_EMITTERS names {name} but no rule of the lane listens to it");
    }
}
