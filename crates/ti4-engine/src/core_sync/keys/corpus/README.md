# Key corpus (package X1)

`marks.tsv` (`json(key) TAB json(value)`) and `other.tsv` (`kind TAB json(text)`) hold every text
the debug dump saw:

* the 200-game golden set (`tools/rulebot/selfplay.sh bench`, which contains the 20 games of
  `tools/engine_profile/profile-20.json`, run again on its own),
* every test of `ti4-model ti4-content ti4-engine ti4-host ti4-rule-bot ti4-selfplay ti4-engine-core ti4-engine-core-bridge`
  (engine tests and the fixtures they load: `faction_marks` goes through `Deserialize` too).

`keydump.patch` is the instrumentation (apply to the commit this folder was made on, run with
`TI4_KEYDUMP=<file>`, `sort -u` the file): a `Marks` wrapper that logs every insert and
deserialised mark, `note_vp` for ledger reasons, the destruction announcers for causes, and a snapshot
of `laws`, `agenda_votes`, `agenda_choices`, `agenda_veto_replacement`, `exploration_decks`,
`expedition_slices`, `wormhole_tokens`, `ion_storm`, `galactic_event` at every `Game::step`. It is not
part of the build.

The three families whose values are JSON (`private:#staged:event:*`, `combat:sustain_target`,
`yssaril:ssruu-round-agent:*`) are thinned to 40 rows each (the dump had thousands).

`unmapped.txt` lists the marks the codec does not type; every one is a test invention (a `test:` key,
a system or planet id a test makes up, a card name that is not a content id). Regenerate with
`X1_WRITE_UNMAPPED=1 cargo test -p ti4-engine --lib core_sync::keys::tests::every_observed_mark`,
and read the diff: a new line is either a new test fixture or a codec gap.
