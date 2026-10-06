# Yin breakthrough Alliance-dispatch census

Status: read-only source census, 2026-10-04. No engine behavior is changed by this note.

## Scope and finding

The DEFAULT content set is the full source set, including PoK and Thunder's Edge. Filtering `crates/ti4-content/content/leaders.json` for `type == commander` and source `pok` or `thunders_edge` yields 32 commander records: 26 PoK records and 6 Thunder's Edge records. The printed text below is the corpus `abilityWindow` followed by `abilityText` (and the corpus unlock condition is included for dispatch context).

There are live commander routes for the 18 currently represented faction modules plus generic routes in `leaders.rs`. That does not cover every source-scoped record: `redcreusscommander` has no live handler, seven PoK factions have no module (`cabal`, `empyrean`, `keleres`, `mahact`, `nekro`, `nomad`, `titans`), and none of the six TE factions has a module. Thus the census finds 18 of 32 with a corresponding existing route (some partial; see below) and 14 without a route. A Yin BT random pool must not filter out the latter factions: granting the ability is a public acquired-ability fact, and content availability is independent of present handler coverage.

## Source-scoped commanders

| Faction | Commander ID | Source | Unlock condition | Printed ability text |
|---|---|---|---|---|
| Ghost / Red Creuss | `redcreusscommander` | PoK | Resolve a combat with another player | At the end of a combat between any players: Gain 1 commodity or convert 1 of your commodities to a trade good. |
| Arborec | `arboreccommander` | PoK | Have 12 ground forces on planets you control. | After another player activates a system that contains 1 or more of your units that have PRODUCTION: You may produce 1 unit in that system. |
| Argent Flight | `argentcommander` | PoK | Have 6 units that have ANTI-FIGHTER BARRAGE, SPACE CANNON, or BOMBARDMENT on the game board. | When 1 or more of your units make a roll for a unit ability: You may choose 1 of those units to roll 1 additional die. |
| Vuil'raith Cabal | `cabalcommander` | PoK | Have units in 3 gravity rifts. | When you produce fighter or infantry units: Up to 2 of those units do not count against your PRODUCTION limit. |
| Ghosts of Creuss | `ghostcommander` | PoK | Have units in 3 systems that contain alpha or beta wormholes. | After your ships move: For each ship that has a capacity value and moved through 1 or more wormholes, you may place 1 fighter from your reinforcements with that ship if you have unused capacity in the active system. |
| Empyrean | `empyreancommander` | PoK | Be neighbors with all other players. | After another player moves ships into a system that contains 1 of your command tokens: You may return that token to your reinforcements. |
| Emirates of Hacan | `hacancommander` | PoK | Have 10 trade goods. | When you cast votes: You may spend any number of trade goods: cast 2 additional votes for each trade good spent. |
| Universities of Jol-Nar | `jolnarcommander` | PoK | Own 8 technologies. | After you roll dice for a unit ability: You may reroll any of those dice. |
| Keleres | `kelerescommander` | PoK | Spend 1 trade good after you play an action card that has a component action | After you perform a component action: You may perform an additional action. |
| L1Z1X Mindnet | `l1z1xcommander` | PoK | Have 4 dreadnoughts on the game board. | At any time: Units that have PLANETARY SHIELD do not prevent you from using BOMBARDMENT. |
| Barony of Letnev | `letnevcommander` | PoK | Have 5 non-fighter ships in 1 system. | After 1 of your units uses SUSTAIN DAMAGE: You may gain 1 trade good. |
| Mahact Gene-Sorcerers | `mahactcommander` | PoK | Have 2 other factions' command tokens in your fleet pool. | During your tactical actions: you can activate systems that contain your command tokens. If you do, return both command tokens to your reinforcements and end your turn. |
| Mentak Coalition | `mentakcommander` | PoK | Have 4 cruisers on the game board. | After you win a space combat: You may force your opponent to give you 1 promissory note from their hand. |
| Embers of Muaat | `muaatcommander` | PoK | Produce a war sun. | After you spend a token from your strategy pool: You may gain 1 trade good. |
| Naalu Collective | `naalucommander` | PoK | Have ground forces in or adjacent to the Mecatol Rex system. | At any time: You may look at your neighbors' hands of promissory notes and the top and bottom card of the agenda deck. |
| Naaz-Rokha Alliance | `naazcommander` | PoK | Have mechs in 3 systems. | After you gain control of a planet that was controlled by another player: You may explore that planet. |
| Nekro Virus | `nekrocommander` | PoK | Own 3 technologies. A "Valefar Assimilator" technology counts only if its X or Y token is on a technology. | After you gain a technology: You may draw 1 action card. |
| Nomad | `nomadcommander` | PoK | Have 1 scored secret objective. | When you produce: You can produce your flagship without spending resources. |
| Saar | `saarcommander` | PoK | Have 3 space docks on the game board. | When you produce fighters or infantry: You may place each of those units at any of your space docks that are not blockaded. |
| Sardakk N'orr | `sardakkcommander` | PoK | Control 5 planets in non-home systems. | During the "Commit Ground Forces" step: You can commit (move) up to 1 ground force from each planet in the active system and each planet in adjacent systems that do not contain 1 of your command tokens. |
| Federation of Sol | `solcommander` | PoK | Control planets that have a combined total of at least 12 resources. | At the start of a ground combat on a planet you control: You may place 1 infantry from your reinforcements on that planet. |
| Titans of Ul | `titanscommander` | PoK | Have 5 structures on the game board. | When 1 or more of your units use PRODUCTION: You may gain 1 trade good. |
| Winnu | `winnucommander` | PoK | Control Mecatol Rex or enter into a combat in the Mecatol Rex system. | During combat: Apply +2 to the result of each of your unit's combat rolls in the Mecatol Rex system, your home system, and each system that contains a legendary planet. |
| Xxcha Kingdom | `xxchacommander` | PoK | Control planets that have a combined total of at least 12 influence. | When you vote: Each planet you exhaust to cast votes provides 1 additional vote. Game effects cannot prevent you from voting on an agenda. |
| Yin Brotherhood | `yincommander` | PoK | Use one of your faction abilities. | At any time: This card satisfies a green technology prerequisite. When you research a tech owned by another player, you may return 1 of your infantry to reinforcements to ignore its prerequisites. |
| Yssaril Tribes | `yssarilcommander` | PoK | Have 7 action cards. | After another player activates a system that contains your units: You may look at that player's action cards, promissory notes, or secret objectives. |
| The Firmament | `bastioncommander` | Thunder's Edge | There are 3 galvanized units on the game board | At any time Your action cards cannot be canceled by "Sabotage" action cards. The Nekro Virus cannot place assimilator tokens on your components. |
| Deepwrought Scholarate | `deepwroughtcommander` | Thunder's Edge | Have an ocean card in play. | When another player spends resources to research a technology That player may reduce the cost by 1; if they do, gain 1 commodity or convert 1 of your commodities to a trade good. |
| Crimson Rebellion | `crimsoncommander` | Thunder's Edge | Place a breach token in a system that contains another player's unit. | At the end of a combat between any players: Gain 1 commodity or convert 1 of your commodities to a trade good. |
| Ral Nel Consortium | `ralnelcommander` | Thunder's Edge | Be the last person to pass during the Action Phase | When you declare a retreat Immediately retreat up to 2 of your ships from the active system to an adjacent system that does not contain another player's ships. Place a command token from your reinforcements into that system. |
| Firmament | `firmamentcommander` | Thunder's Edge | Have a plot card in play | At any time You can treat planets in systems that contain your ships as if they were controlled by you for the purpose of scoring secret objectives. |
| Obsidian | `obsidiancommander` | Thunder's Edge | Have units in The Fracture. | At any time Apply +1 to the result of each of your units' combat rolls in The Fracture. |

## Existing routes and limitations

The 18 represented faction modules are `arborec`, `argent`, `ghost`, `hacan`, `jolnar`, `l1z1x`, `letnev`, `mentak`, `muaat`, `naalu`, `naaz`, `saar`, `sardakk`, `sol`, `winnu`, `xxcha`, `yin`, and `yssaril`. `leaders.rs` is the generic dispatcher/read point for leader unlock status and central effects such as L1Z1X planetary-shield bypass, Letnev sustain gain, Sol infantry, and Xxcha voting. Module handlers and those generic effect reads are not a universal Alliance dispatcher: many branches first test the player's faction id or the faction leader's own status.

Supported/partial route audit:

| Commander(s) | Current route | Census status |
|---|---|---|
| `arboreccommander` | `factions/arborec.rs`, activation/system production timing | Implemented route; identity/status guard must be made ability-source aware. |
| `argentcommander` | `factions/argent.rs`, unit-ability-roll extra die | Implemented route; identity/status guard must be made ability-source aware. |
| `ghostcommander` | `factions/ghost.rs`, ship-move wormhole fighter timing | Implemented route; identity/status guard must be made ability-source aware. |
| `hacancommander` | `leaders.rs::vote_bonus`, consumed by vote casting | Partial: current code grants a flat +3; printed ability is an optional spend for +2 per TG. |
| `jolnarcommander` | `combat.rs` and `invasion.rs` unit-ability reroll support | Implemented effect route; ensure Alliance-aware source gating. |
| `l1z1xcommander` | `leaders.rs::ignores_planetary_shield`, used by invasion bombardment legality | Implemented generic route; ensure Alliance-aware source gating. |
| `letnevcommander` | `leaders.rs::pays_on_sustain`, used by combat sustain resolution | Implemented generic route; ensure Alliance-aware source gating. |
| `mentakcommander` | `factions/mentak.rs`, after winning space combat | Implemented route; identity/status guard must be made ability-source aware. |
| `muaatcommander` | `factions/muaat.rs`, strategy-pool token spend | Partial: route is wired only to some token-spending producers; census has known missing spend sites. |
| `naalucommander` | `factions/naalu.rs` / card observation path | Implemented observation/visibility route; ensure Alliance-aware source gating and preserve privacy scope. |
| `naazcommander` | `factions/naaz.rs`, planet-control gain/exploration timing | Implemented route; identity/status guard must be made ability-source aware. |
| `saarcommander` | `factions/saar.rs` production placement | Unlock exists, but the printed arbitrary unblocked-dock placement ability lacks a complete live handler. |
| `sardakkcommander` | `factions/sardakk.rs` / ground commit resolver | Implemented route; identity/status guard must be made ability-source aware. |
| `solcommander` | `leaders.rs` ground-combat start | Implemented generic route; ensure Alliance-aware source gating. |
| `winnucommander` | `factions/winnu.rs`, combat result modifier | Implemented route; identity/status guard must be made ability-source aware. |
| `xxchacommander` | `leaders.rs` / `vote.rs`, extra vote and voting protection | Implemented generic route; ensure Alliance-aware source gating. |
| `yincommander` | `technology.rs` and `factions/mod.rs` strategy hooks | Implemented prerequisite/payment route; identity/status guard must be made ability-source aware. |
| `yssarilcommander` | `factions/yssaril.rs`, activation hand reveal | Implemented route; identity/status guard must be made ability-source aware. |
| `redcreusscommander` | No matching handler found in source search | Unsupported. |

No live routes were found for `cabalcommander`, `empyreancommander`, `kelerescommander`, `mahactcommander`, `nekrocommander`, `nomadcommander`, or `titanscommander`; all six Thunder's Edge commander IDs are likewise unsupported. This is a bounded census, not a claim that the corpus card objects are absent.

## Alliance support and dispatch design

`promissory.rs::commander_ability_from` is the only Alliance-specific commander lookup found. `rg` found no call sites. Its contract is the ordinary promissory-note rule: the note must be faceup, owner must map to a seated faction, and that faction's commander must be unlocked. Existing `available_notes` logic also withholds Alliance until unlock. These are appropriate for trading/playing ordinary Alliance, but cannot represent Yin's direct grant of an unused faction's ability. A source scan found no second generic Alliance possession check at effect call sites; current faction hooks instead commonly compare faction id and/or `LeaderStatus::Unlocked`.

Recommended shared predicate: `has_commander_ability(state, player, commander_id)`. It should return true when (a) the player's own matching commander is unlocked, (b) the player has a public durable direct-grant mark for that commander, or (c) an ordinary faceup Alliance held by the player names a seated faction whose matching commander is unlocked. The ordinary Alliance branch must keep the current 69.3 owner-seat/unlock constraints. The direct grant branch must not require a faction owner/seat or an unlocked record, because the chosen faction may be unused; it is an acquired ability for the Yin player, not a transfer of the absent faction's leader state.

Store direct grants in public game state, for example `commander_ability:<recipient-player-id>:<commander-id>` under `faction_marks`. The mark must be public to all seats (do not use the `private:<seat>:` prefix), stable across save/restore, and keyed by both recipient and exact commander id. Persist the once-per-game/once-per-BT selection separately if the breakthrough permits only one acquisition. The random candidate list should enumerate all DEFAULT source-scoped commander records (excluding only invalid duplicates or already granted candidates as rules require); do not filter on module presence, `LeaderStatus`, or seated faction membership.

To dispatch correctly, replace each effect route's local faction/status gate with the shared predicate, including generic reads in `leaders.rs`. Keep status checks in unlock-condition evaluation and ordinary Alliance availability; they answer different questions. Test direct grants from an unseated faction, normal Alliance from an eligible seated owner, unlocked own commander, no grant, un-unlocked ordinary Alliance, and a persisted public grant after a state round trip.

## Missing-ability implementation clusters and estimate

The unsupported records are not one generic callback. They cover at least these behavior clusters: economy choices (`redcreuss`, `crimson`), production modifiers/placement (`cabal`, `nomad`, `titans`, plus incomplete `saar`), token/map/action control (`empyrean`, `mahact`, `ralnel`), action-card timing (`keleres`, `nekro`, `bastion`), technology cost/payment (`deepwrought`), combat modifiers and cancellation/assimilation protections (`obsidian`, `bastion`), and scoring interpretation (`firmament`). Each handler needs its own precise timing and legal-choice semantics; broadening the random pool without those handlers silently grants inert abilities.

Smallest safe split: (1) foundation: public grant marks, deterministic full-content candidate sampling, persisted acquisition state, shared predicate, and direct-grant tests; (2) reuse pass: thread the predicate through the 18 extant module/generic routes and test ordinary Alliance compatibility; (3) unsupported handlers in timing clusters, each with real resolver/production/action tests; (4) enable Yin's live BT choice only when every pool candidate has a live effect handler, or explicitly define a rules-faithful way to handle unimplemented selections. Do not ship a pool over all 32 with only the first two steps: that produces selectable but nonfunctional abilities.

Estimated scope: the foundation and reuse pass touch shared leader/promissory state plus roughly 18 route modules; unsupported dispatch adds at least 14 handlers (one is a second Ghost-faction commander) across production, combat, movement, action, technology, scoring, token, and privacy paths. The exact implementation effort is multi-package and should be split by behavior cluster after the common predicate lands. No source changes or tests were run for this census.

## Root correction: candidate selection follows faction sheets

The 32 rows above count raw commander records, not random faction candidates. `redcreusscommander` is not referenced by the Ghost faction sheet; Ghost references `ghostcommander`. Derive candidates from source-scoped `ti4_content::factions::catalogue` and each sheet's `leaders()`, validate the commander kind, canonicalize variant faction identity where the rules require, and exclude seated factions. Do not sample unreferenced historical commander rows. Keleres comes from codex sources included in DEFAULT, despite the earlier prose describing only PoK and TE. Last Bastion's canonical faction alias is `bastion`, with faction name `Last Bastion`; use faction records rather than the raw leader's faction label. The existing-handler census also overstates Sol's live delivery: grep finds only its unlock/registry/manual dispatcher, without a ground-combat timing listener. Saar's production placement consumer exists in the recent completion package and must be inspected before labeling it absent. The candidate-support gate requires actual event/call-site execution, not registry presence.

## Operator scope correction, October 4

The operator explicitly states Crimson and interactions with Crimson are outside this faction package. The coordinator had expanded Yin breakthrough dependencies into all canonical unused-faction commander implementations; that expansion is not accepted package scope. Crimson implementation was interrupted before any Crimson code/evidence edit was reported. Suspend further external-commander packages and reconcile their scope before resuming them. Preserve already-existing uncommitted Nomad/Cabal/Titans work without claiming it is required or accepted here; do not delete/reset shared edits.

Completion work returns to the twelve named BF factions, their own assets and shared hooks. The printed Yin breakthrough's acquisition of unsupported faction commanders remains an explicit dependency limitation; do not falsify coverage or silently restrict its random pool to make a ledger green. Record this limit during scope-ledger reconciliation instead of implementing excluded factions.

BF-SSRUU-MUAAT-ACTION stays in scope because both Yssaril and Muaat are among the twelve BF factions. It is assigned to the native Pi model exactly as an implementation agent: disjoint edit scope, inline actual-dispatch fixtures, root-run Cargo, independent review before acceptance. Package specification: BF_LOCAL_MODEL_SSRUU_MUAAT_WORK_PACKAGE.md. Root's production-entry atomicity verification remains next shared fix; no broad compatibility expansion or simulator widening before dependencies are reconciled.
## Operator decision, October 5

Asked by the Claude coordinator: Yin's breakthrough grants the commander of a random unused faction, most of which have no handler. Operator answer: **implement the commanders**. Scope is every unused-faction commander in Yin's canonical pool except Crimson (still excluded by the October 4 correction). `yinbt` stays unclaimed until every pool candidate has a live, tested handler; the pool is not narrowed. Split by behavior cluster per the "Smallest safe split" above.
