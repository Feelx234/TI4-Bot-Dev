# Resource and influence icons

Audit item M1, decision: one icon for resources and one for influence, used everywhere in `web/src`.
Everything below was found by grepping `web/src` for `Res`, `Inf`, `resource(s)`, `influence`, `NR/MI`
and the payment unit letters, and then reading each hit. Line numbers are on the branch
`ui-resource-influence-icons-2026-10-07`.

## The component (`web/src/components/PlanetValueIcons.tsx` + `.css`)

| Export | Use |
|---|---|
| `PlanetValue` | icon + number, `kind="resources" \| "influence"`, `size="inline" \| "bar" \| "tooltip"`, `state="ready" \| "spent" \| "muted"` (zero defaults to muted), `total` (+ `alwaysTotal`) for ready/total, `sign` for `+3`, `label` to override the text equivalent |
| `ValueUnit` | just the icon, where a number is already shown next to it (`Paid 3 / 5 [icon]`) |
| `PlanetValuePair` | resources then influence (a planet's printed values) |
| `ValueText` | text from our own presentation models (`Cost: 3 resources`, `Lodor (3R/1I)`): every `N resources`, `N influence` and `NR/MI` becomes icon + number, the rest stays text |
| `PlanetValueGlyph` | the same glyph as SVG `<svg x y>` for the map (cards, payment marks, planet badge, map preview) |
| `ResourceIcon`, `InfluenceIcon` | the bare decorative icons (overlay toolbar button) |
| `valueLabel`, `pairLabel`, `valueKind` | the text equivalent: `1 resource`, `0 resources`, `6 influence`, `2 of 5 resources`; `Resources`, `Influence`, `R`, `I` to a kind |

Accessible text: every `PlanetValue` is `role="img"` with `aria-label` and `title` set to the full
word (singular for 1, plural otherwise; "influence" has no plural). The visible number is
`aria-hidden`, so a screen reader says "3 resources", not "3 3". SVG glyph groups get `role="img"`,
`aria-label` and a `<title>`.

Sizes: icon is `max(12px, 1.05em)` inline, 16 px in `bar`, 12 px in `tooltip`; gap 0.25em; the two
paths are cropped to their own square so both fill the same box. Colours are tokens
(`--value-resource`, `--value-influence`, `--value-spent`) with a `[data-theme="light"]` override;
the app has no light theme yet, so only the dark values are in use. On a selected button
(`aria-pressed="true"`, `.button--primary`) the icon follows the text colour so it cannot vanish into the fill.
Spent/zero: `muted`/`spent` dim the number and the icon (0.55 opacity).

## Inventory: converted

| file:line | was | now |
|---|---|---|
| `board/EconomyOverlay.tsx:34` (`EconomyCard`) | two coloured cards with bare numbers `3` / `2/5` | glyph + number, `aria-label` "2 resources ready of 3 total" |
| `board/EconomyOverlay.tsx:108` (`EconomyLine`) | `Ready: 3 Res / 1 Inf`, `Total:`, `Exhausted:` | `Ready:` + two `PlanetValue` (tooltip size), group `aria-label` |
| `MapOverlayToolbar.tsx:134` | button `Res / Inf` with the money-bag emoji | the two icons, `aria-label` "Resources and influence overlay" |
| `board/StandardOverlay.tsx:167` | payment mark `4R`, `✓ 4I` | glyph + number, `aria-label` "Staged: 4 resources", `<title>` |
| `board/StandardOverlay.tsx:209` | planet badge `3/1` | two small glyphs + numbers, `aria-label` "3 resources, 1 influence" |
| `board/StandardOverlay.tsx` (button label) | "Exhaust planet X for 4 resources" | same sentence, from `valueLabel` (singular fixed) |
| `board/BoardTooltip.tsx:97` | `(3 Res / 1 Inf)` | `PlanetValuePair` (tooltip size) |
| `SystemInspector.tsx:157` | `3 Res / 1 Inf` | `PlanetValuePair` (tooltip size) |
| `SystemFactsView.tsx:28` | `Lodor (3R/1I)` as one string | `PlanetValuePair` inline |
| `MapPreviewBoard.tsx:103` | bare `2/1` per planet, plus `<title>` "2/1" | one glyph row per planet; `<title>` uses `pairLabel` |
| `PlanetSelectionBar.tsx:78,319` | `(3R/1I)` after the planet in the action line, ` 3R/1I` on chips | `ValueText` (icons); 0 values dimmed |
| `LegendaryParts.tsx:34` | `(3R/1I)` | `ValueText` |
| `PredictOutcomeParts.tsx:36` | `Lodor (3R/1I)` | `ValueText` |
| `PaymentBar.tsx:111` | badge `Pay 5 resources` | `Pay` + `PlanetValue` (bar size) |
| `PaymentBar.tsx:128` | `Paid 3 / 5 resources` | `Paid 3 / 5` + `ValueUnit` |
| `PaymentBar.tsx:149` | `Short by 5 resources: ...` (from `paymentDraft.ts`) | `ValueText` |
| `PaymentDrawer.tsx:184,193` | title `Pay 4 Resources`, progress `1 / 4 Resources already paid` (`DecisionHeader` now takes nodes) | `PlanetValue` / `ValueUnit` |
| `PaymentDrawer.tsx:222,230,240` | `4 Resources` totals, `1 Resources` (sic), `+3 Resources` | `PlanetValue` (singular fixed, `sign="+"`, zero muted) |
| `PaymentDrawer.tsx:293` | planet badge `+4 Res`, `+3 Inf` (`currency.slice(0, 3)`) | `PlanetValue sign="+"` |
| `PaymentDrawer.tsx:312` | `1 TG = 1 Res` | `PlanetValue` |
| `CommandTokenPanel.tsx:186` | `3 influence each, up to 3` | `PlanetValue` + text |
| `CommandTokenPanel.tsx:215` | `Influence available 9 · spent 6` | `ValueUnit` + `available 9 · spent 6` |
| `CommandTokenPanel.tsx:273` | `jord · 2 influence · ready` | `PlanetValue` |
| `CommandTokenPanel.tsx:281` | `Trade goods (1 influence each, 2 held)` | `PlanetValue` |
| `CommandTokenPanel.tsx:311,315` | account line (all influence) and `Short by 2 influence: ...` | trailing `ValueUnit`; `ValueText` |
| `AgendaBallotModal.tsx:333` | planet card `3 v` | `PlanetValue kind="influence"`, `aria-label` "3 votes (influence)" (a planet's influence is its votes) |
| `ProductionBuilderDrawer.tsx:204,211` | `Resources:` / `3 / 5 Resources (2 Left)` | `ValueUnit budget` / `3 / 5 [icon] (2 Left)` |
| `TechnologyModal.tsx:591` | `Available Resources: 6` | `Available:` + `PlanetValue` (bar), `aria-label` "6 resources available" |
| `TechnologyModal.tsx:609,620` | `Cost: 6 Resources` | `Cost:` + `PlanetValue` |
| `TechnologyModal.tsx:654,657` | button `Confirm Research (2 Techs - 6 Resources)` | `ValueText` |
| `TurnActionBar.tsx:377` (chips from `presentation/turnBar.ts:175`) | `Cost: 4 resources` | `ValueText` |
| `UnitAbilityParts.tsx:20` (lines from `presentation/unitAbilityOptions.ts:75`) | `Cost: 3 resources` | `ValueText` |
| `StrategySecondaryPanel.tsx:57` (labels from `presentation/strategySecondary.ts`) | `Spend 1 strategy token + 4 resources to research`, `Spend 3 influence for a command token` | `ValueText` |
| `PlayerSheet.tsx:393,419` | hand-styled icon + ready/total spans | `PlanetValue` with `total`, `alwaysTotal`, muted at 0 ready; same `title` |
| `InvasionLandingTray.tsx:367` | icon + `R 2`, `I 2` (letters redundant next to the icon) | `PlanetValue` (number only; singular label) |

## Inventory: kept as text, on purpose

| where | why |
|---|---|
| Engine prompts and option labels: `choice.prompt`, `option.label` (`pay 5 resources`, `exhaust jord for 4 resources`, `spend 3 influence`) rendered by the generic option lists, `PendingChoiceModal`, `AutoResolveToast`, corner toasts | free text generated by the engine; parsing it would be guessing. The payment surfaces show the structured values (icons) next to it and `PaymentBar` hides a prompt that only repeats "pay" |
| Event log text (`EventLog.tsx`) | server-generated sentences; the renderer has no resource/influence wording of its own |
| Card, ability, objective and agenda rules text (`CardDetails`, `OfferCardPanel`, `ObjectivesModal`, content catalog, `cardDatabase.ts`) e.g. "Spend 8 influence" | content strings; inside a sentence an icon would hurt readability and the text is not ours to rewrite |
| `AgendaBallotModal.tsx:197,302` "Spend influence to vote", "...exhaust for influence votes:" | sentence/title where the word explains the action; the planet cards below carry the icon |
| `TechnologyModal.tsx:449,593` "Available resources and tech skips are shown below", "(4 planet + 2 TG)" | instruction sentence; the parenthetical has no resource word |
| `PaymentDrawer.tsx` "via influence" (`planet.sourceKind`) | names which engine pool pays; it is a word, not a value |
| `presentation/*.ts` source strings (`paymentDraft.ts`, `commandTokens.ts`, `strategySecondary.ts`, `turnBar.ts`, `unitAbilityOptions.ts`, `planetSelection.ts`, `legendaryMenu.ts`, `predictOutcome.ts`) | they still build the plain strings (tests, aria text, other consumers); the components turn the value words into icons with `ValueText` |
| `dev/*GalleryCases.ts`, `DecisionGallery.tsx` | synthetic engine-like fixtures and notes for developers |
| `PlayerSheet.tsx:351` `TG: 2 \| Comm: 0` | trade goods and commodities, not resources or influence |
| Trade desk (`TradeDeskModal.tsx`, `tradeDecoder.ts`) | only trade goods ("for 3 TG") and commodities appear there; no resource or influence value exists on that surface |
| `CommandTokenPanel.tsx` "Pays with lodor (3)" | the engine-style note has numbers but no resource/influence word |
| `playerStats.ts`, `mapOverlays.ts`, `boardPresentation.ts`, `choiceModel.ts`, `protocol/*`, `technologyData.ts` ("R" tech colour) | data and logic, no rendered words |

## Not visible from this branch

Surfaces that exist only on other unmerged branches (for example inline production-builder stats,
the Leadership map payment) could not be searched or converted here. They should use `PlanetValue`
/ `ValueText` when those branches are merged; the grep that finds them:
`grep -rnE "Res /|\bInf\b|[0-9]R/|resources|influence" web/src --include=*.tsx`.

## Tests and screenshots

- Unit: `PlanetValueIcons.test.tsx` (labels, plural/zero, ready/total, signs, `ValueText`); the
  component tests that matched old text now assert the number and the `aria-label`.
- E2E specs that matched `Resources` text (`decision_workflows_mocked`, `production_batches`,
  `technology_research`) now look for the `img` with the full word; the overlay capture in
  `C-system-selection` uses the button test id.
- Screenshots: `cd web && npm run screenshots -- AA` (folder `AA-resource-influence-icons`).
