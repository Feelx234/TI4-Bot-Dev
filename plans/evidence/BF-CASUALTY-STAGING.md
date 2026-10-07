# BF-CASUALTY-STAGING

P01-CASUALTY-PROVENANCE compile-coherent staging specification, P1, no external effects. Dependencies: existing committed timing checkpoint and printed casualty producer contract; current migration is implemented in shared WIP and model83 tests pass. Source norm: Brother Milor destruction during combat; cause and combat timing are independent facts, legacy decode conservative.

One explicit atomic-size integration exception spans seven source files because the public tuple/API change cannot compile in isolation: model/state.rs plus exact engine writers/readers in combat.rs, action_cards.rs, reactions.rs and three faction adapters muaat.rs/sardakk.rs/yin.rs. This exception combines only the model tuple schema, exact consumer adaptations and source/combat-fact preservation with narrow compatibility fixtures. It does not combine the entire inherited BF wave. Root selects scoped hunks; whole-file staging of mixed source is forbidden. No commit/acceptance is claimed by this spec.

Tuple contract: pending_destructions old3 -> new5(system,owner,unit,cause,during_space_combat); transitional4 preserves cause and only historical space_combat sentinel yields true. last_sustain old4 -> new5 adds timingboolean, conservative legacyfalse. New destruction APIs carry cause and independent timing; Muaat Nova Seed remains outside combat, Yin flagship inherits timing while identifying its own unit ability. Consumers DirectHit/Courageous/Milor must not confuse effect source with combat timing.

Staging roles:
- model/state.rs: exact fields/customdecoders/current+legacy fixtures, exclude unrelated initiative format.
- combat.rs: exact destruction API/emitter/drain and sustain tuple adapters plus narrow provenancefixtures. Planetary Maximum participation/removal/menu, ground hits and copied round dice are separate packages.
- action_cards.rs: exact sustain readers and DirectHit/Courageous tuple/provenance writers/fixtures. Refit/Rider/Naaz placement separate.
- reactions.rs: pending drain/payload handoff only. Borrowed registration separate.
- muaat.rs: destruction cause adapter and arity5fixture only; preserve other faction WIP.
- sardakk.rs: both Exotrireme II destruction adapters, cause technology:exo2 and combat=true; no broader faction changes.
- yin.rs: destruction context adapter and booleanfilter consumer/provenancefixtures only. Breakthrough/Alliance/copy coverage separate.

Game is not a tuple compile dependency. Its strict staged-drain Result/transaction hunk is a separate eighth-file error-atomicity package if accepted; preserve all income/selector/loop experiments and all other Game continuations. Invasion has no direct schema references; its errors/round producers are separate.

Definition of done: exact staged-hunk inspection, independent tierC source review, legacy/current modeltests and affectedengine gate, focused commit excluding all unrelated hunks. No isolated committed-tree build is implied by shared-tree results. Index currently empty; no staging performed.


Independent follow-up corrected the initial six-file map: HEAD Sardakk has two destroy_units callers that must adapt to the new cause/context API. Minimum provenance slice is therefore seven files; leaving Sardakk out would not be compile/behavior coherent. The new ship_destroyed_payload, destroyed_during_combat, destruction API wrapper and emitter are part of provenance; exact Maximum remember_sustain_target/planetary_sustain_location/remover and strict Game drain remain separately staged. supply::staging_enabled already exists in HEAD. No staged blob or commit created from either preliminary map.
