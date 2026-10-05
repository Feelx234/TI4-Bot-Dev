# Reproduction Investigation Report
**Session:** 2026-10-01 Human Playthrough Findings  
**Status:** IN PROGRESS  
**Last Updated:** 2026-10-05

> 📌 **Linked from:** [TODO_SESSION_2026-10-01_HUMAN_PLAYTHROUGH.md](./TODO_SESSION_2026-10-01_HUMAN_PLAYTHROUGH.md)

---

## Investigation Summary

This document contains reproducible steps for each finding from the human playthrough session. Each issue is investigated via code review and/or automated reproduction attempts.

---

## Critical Issues (Blocking)

### C1: Bot Crash with Diplomacy Card
**Status:** ✅ INVESTIGATED  
**Severity:** 🔴 CRITICAL

**Critical Crash Points Identified:** ✅

**PRIMARY SUSPECT: Promise Evaluation Panics**
- **Location:** `strategy_cards.rs` lines 571 and 1174
- **Issue:** Two `expect()` calls assume promise evaluation never fails
  - Line 571: Trade primary when leader used for someone else
  - Line 1174: Trade primary when commodities replenished
- **Crash Condition:** If `evaluate_event()` returns error, program panics with unhelpful message
- **Root Cause:** Diplomacy state corruption or deal/promise inconsistency

**SECONDARY SUSPECT: Deal Revision Access**
- **Location:** `diplomacy/promises.rs` lines 320-327
- **Issue:** `expect("deal revision")` panics if deal has no revisions
- **Risk:** Low in normal operation, possible with corrupted state

**LOWER PRIORITY: Signal Index Access**
- **Location:** `diplomacy/signals.rs` line 190
- **Issue:** Direct array indexing without bounds checking
- **Risk:** Very low - indices validated immediately

**Why Crash Happens:**
1. No state validation before executing Diplomacy card abilities
2. Error handling uses panics instead of proper error propagation
3. Deal state assumes consistency without runtime verification
4. Missing transaction error handling during promise settlement

**Reproduction Steps:**
- Play Diplomacy strategy card during game
- If diplomacy state is corrupted (failed promise, invalid deal, etc.)
- `expect()` calls will panic with generic error message
- Bot crashes, player stuck waiting

**Fix Priority:**
1. Replace `expect()` with proper error handling returning `IllegalChoice`
2. Add diplomacy state validation before Diplomacy card execution
3. Change deal revision access: `.expect()` → `.ok_or()?`
4. Add tracing/logging to capture error context before panic

---

### C2: Initial Map Generation Broken
**Status:** ✅ INVESTIGATED  
**Severity:** 🔴 CRITICAL

**Architecture is Sound:**
- Proper randomization system: seeded RNG with domain separation
- Map filler selection uses `map_filler()` which shuffles systems based on seed
- HTTP API generates random seeds with `rand::random::<u64>()`

**Real Problem: Seed Reuse**
The appearance of "pre-fabricated maps" is likely caused by **seeds being reused** across games, not hard-coded layouts.

**Pre-Fabricated Maps Exist But Unused:**
- `/crates/ti4-content/content/map_templates.json` contains 9,000+ lines
- Never called during normal game creation
- Only for testing/reference

**Generation Flow:**
1. HTTP handler generates random seed (or uses provided seed)
2. Game creation calls `seating::map_filler()` with seed
3. Filler selection shuffles neutral systems based on seed
4. Board construction places shuffled systems in three rings

**Critical Files:**
- **Entry:** `/crates/ti4-server/src/map.rs` - `create_game_with_map()`
- **Randomization:** `/crates/ti4-engine/src/seating.rs` - `map_filler()` and `neutral_systems()`
- **HTTP handler:** `/crates/ti4-server/src/http/games.rs` line 149 (seed generation)
- **Session management:** `/crates/ti4-server/src/session/registry.rs` (seed passing)

**Root Cause Candidates (Priority):**
1. **Seed not randomized** - `rand::random()` not being called in production
2. **Seed caching** - same seed value reused across games
3. **RNG failure** - shuffle operation not working properly
4. **Test fixtures leaked** - hard-coded seeds in dev code being used in production

**Reproduction Steps:**
- Create multiple games in sequence
- Check map layout in each game
- Verify if they're identical or very similar
- Check seed values being passed to `map_filler()`

**Investigation Needed:**
- [ ] Verify `rand::random()` is actually being called
- [ ] Check for seed caching/reuse logic
- [ ] Trace seed from HTTP handler through to map generation
- [ ] Look for env vars or config overriding randomization

---

### C3: React Warning - Duplicate Keys in Action List
**Status:** ✅ INVESTIGATED  
**Severity:** 🔴 CRITICAL (Logic)

**Root Cause Found:** ✅

**Location:** `/web/src/components/EventLog.tsx`
- **Key Generation:** Line 114
- **Action Node Creation:** Lines 115-125
- **Rendering:** Line 326
- **Reset Logic:** Line 93

**The Problem:**
Action node key: `${phaseNode.id}:action:${entry.action_id}`

When phase ID is already `round:2:action` (action = phase name), adding `action_295` creates:
```
round:2:action:action:action_295  ← DUPLICATE KEY!
```

**How Duplicate Occurs:**

1. Phase ID = `round:2:action` (phase name is "action")
2. Decision creates action node, pushed to phase children
3. Non-decision event (marker/transition) resets `lastAction = undefined` (line 93)
4. Another decision in SAME PHASE: condition `lastAction?.id !== id` is TRUE
5. **Second action node created with same ID**
6. React finds two elements with identical key during render

**Exact Condition (Line 115):**
```typescript
if (lastAction?.id !== id || lastParent !== phaseNode) {
  // Creates action node - can trigger twice for same action!
}
```

**Reproduction Steps:**
- Round 2 gameplay
- Multiple decisions in Action phase with same action_id (e.g., `action_295`)
- Marker or non-decision event occurs between decisions (resets `lastAction`)
- Next decision causes duplicate node creation
- React warning appears in console

**Fix Approaches:**
1. Track which action nodes were already created to prevent duplicates
2. Use `phaseNode.id + action_id` as unique identifier instead
3. Don't reset `lastAction` when entering same phase

---

## High Priority Issues

### H1: Hacan Trade System Broken
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Initial Findings:**
- Hacan trade implemented in `/web/src/services/advisorService.ts`
- Trade card logic in strategy cards
- Related components: `TradeDeskModal`, transaction handling

**Game Reference:**
- URL: `f49965ba37a5a31f310befa69744f1e0`

**Possible Issues:**
- Trade good exchange calculation error
- UI not showing trade options correctly
- Payment validation rejecting valid trades
- Promissory note handling in trades

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Execute Hacan trade in controlled game state
- [ ] Log trade exchange values
- [ ] Verify UI correctly shows trade options
- [ ] Check payment calculation

---

### H2: Invasions Display - No Show to Other Players When No Fight
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Issue Description:**
- Invasions without combat (0 defenders, instant victory) should not be shown to other players
- Current: All invasions displayed, cluttering UI
- Expected: Only show invasions with combat

**Affected Components:**
- `/web/src/components/InvasionOverlay.tsx`
- `/web/src/components/InvasionLandingTray.tsx`
- Game shell decision rendering

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Start invasion with 0 defending units
- [ ] Verify invasion overlay is shown
- [ ] Check if non-actors see the invasion
- [ ] Confirm non-fighting invasions should be hidden

---

### H3: Payment & Build Flow - Multiple Steps Required
**Status:** ✅ INVESTIGATED  
**Severity:** 🟠 HIGH

**Root Cause Identified:** ✅

The engine uses a **mandatory loop** that asks for each payment individually:
- **Location:** `/crates/ti4-engine/src/production.rs` lines 639-672
- **Function:** `while paid < owed` loop asks for one payment at a time
- This matches TI4 rules (planets exhausted one-at-a-time) but creates UX friction

**Why Planets Are Prioritized First:**
- `payment_options()` function adds ALL spendable planets BEFORE trade goods
- When auto-selection happens (single option), planets are always chosen first
- **Location:** `/crates/ti4-engine/src/production.rs` lines 370-456

**UI Batching Incomplete:**
- PaymentDrawer.tsx supports `onSubmitBatch` via BasketPlan (lines 174-219)
- Backend `/basket` endpoint appears not fully implemented
- Falls back to sequential pipeline processing

**Example Reproduction:**
Player needs 3 resources, has: 3 planets (1 resource each) + 5 trade goods
- Step 1: Auto-exhausts first planet
- Step 2: "Pay 2 more resources" decision
- Step 3: Exhausts second planet or trade good
- **Result:** 2-3 separate decision prompts for one payment action

**Quick Fixes Available:**
1. Reorder `payment_options()` to show trade goods alongside planets
2. Add "Planets First / Goods First" toggle in PaymentDrawer
3. Improve messaging explaining multi-step process

**Medium-Term Fixes:**
1. Implement backend `/basket` endpoint for batch submission
2. Add visual progress: "Step 1 of 3"
3. Show all payment options upfront

**Files Affected:**
- Engine: `/crates/ti4-engine/src/production.rs`
- UI: `/web/src/components/PaymentDrawer.tsx`
- Protocol: `/web/src/protocol/client.ts`

---

### H4: Simultaneous Action Pipeline Missing
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Issue:** Technology, Warfare, and Strategy Cards should allow pre-selection before actual turn  
**Current:** Sequential decision making  
**Expected:** "Decide now, execute in order when it's your turn"

**Affected Cards/Actions:**
- Technology secondary
- Warfare (command token recall)
- Most Strategy Cards

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Play technology strategy card
- [ ] Verify you must decide immediately
- [ ] Check if simultaneous pre-selection is offered
- [ ] Trace decision workflow

---

### H5: Action Card Triggers Not Displayed
**Status:** ✅ INVESTIGATED  
**Severity:** 🟠 HIGH

**Root Cause:** ✅ Found

Trigger conditions exist in backend but aren't transmitted to frontend.

**How It Should Work:**
1. Rust engine has window text (trigger conditions) in content store
2. `reaction_card_options()` generates choice options but **doesn't add description**
3. Frontend has infrastructure to display but **doesn't receive data**
4. UI only shows card name, not trigger condition

**Missing Data Flow:**
```
Backend: Window text exists ❌ (not extracted)
  ↓
ChoiceOption: Missing description field ❌ (not populated)
  ↓
Wire: ChoiceOptionDto already supports description ✅
  ↓
Frontend: Has infrastructure but no data ❌
```

**Key Files & Changes Needed:**

1. **Rust Engine:** `/crates/ti4-engine/src/choice.rs` (line 48)
   - Add: `pub description: Option<String>`

2. **Reaction Generation:** `/crates/ti4-engine/src/reactions.rs` (lines 812-836)
   - Extract window text from content store
   - Populate `option.description` with trigger condition

3. **Frontend Display:** `/web/src/components/ReactionStatusBar.tsx` (line 178)
   - Change from: `Play {opt.label}`
   - To show: Card name + trigger condition

**Reproduction Steps:**
1. Play game to trigger reaction (e.g., Decoy Operation after system activation)
2. Check browser console for missing description in ChoiceOptionDto
3. Verify window text exists in content manifest
4. Trace presentation layer for action cards

**Data Available:**
- Window text already in content: `content.get(ContentType::ActionCards, alias).and_then(|record| record.text("window"))`
- Just needs to be extracted and transmitted

---

### H6: Single-Choice Decisions Should Auto-Resolve
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Issue:** When only one valid choice exists, system should auto-select instead of prompting

**Examples to Find:**
- Strategy card selection (last card in draft)
- No resources available for payment
- Only one system reachable for activation
- Only one fleet able to move
- Last technology available to research

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Identify all decision points that can have single option
- [ ] Verify current behavior shows decision UI
- [ ] Create test cases for each scenario
- [ ] Implement auto-resolve logic

---

### H7: Undo Non-Permanent Decisions During Turn
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Issue:** Players should be able to undo non-permanent decisions during their turn

**What's Non-Permanent:**
- Strategic action selection (before action taken)
- Unit placement (before committed)
- Payment selections (before confirmed)
- Dice rolls (before results applied)

**What's Permanent:**
- Attack declarations (once locked)
- Finished phases
- Confirmed trades
- Spent resources/tokens

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Make a strategic action choice
- [ ] Check if undo is available
- [ ] Verify state rollback works
- [ ] Test randomness isn't re-rolled

---

### H8: Exchange/Trade UI Unclear
**Status:** INVESTIGATING  
**Severity:** 🟠 HIGH

**Issues:**
- Card descriptions not shown when offered in trade
- Exchange UI cluttered
- Trade simultaneous decisions not possible

**Affected Components:**
- TradeDeskModal
- Trade choice rendering
- Card description display

**Reproduction Steps:** PENDING SUBAGENT INVESTIGATION
- [ ] Initiate trade desk opening
- [ ] Verify card descriptions are shown
- [ ] Check UI layout for clarity
- [ ] Test simultaneous trade decisions

---

## Medium Priority Issues

[Medium priority items will be investigated in Phase 2 of subagent queue]

---

## Subagent Investigation Queue

### Phase 1: Critical Path (Current)
- [ ] **C1 Investigation:** Diplomacy crash analysis
- [ ] **C2 Investigation:** Map generation source
- [ ] **C3 Investigation:** Duplicate key finder
- [ ] **H1 Investigation:** Hacan trade flow
- [ ] **H2 Investigation:** Invasion display filtering

### Phase 2: High Priority
- [ ] **H3 Investigation:** Payment flow analysis
- [ ] **H4 Investigation:** Strategy card decision pipeline
- [ ] **H5 Investigation:** Action card trigger display
- [ ] **H6 Investigation:** Single-choice scenarios catalog
- [ ] **H7 Investigation:** Undo capability audit
- [ ] **H8 Investigation:** Trade UI components

### Phase 3: Medium Priority
- [ ] Combat results display
- [ ] Token hover information
- [ ] Planet resource visibility in invasion
- [ ] And more...

---

## Notes

- Investigation follows code-first approach: search codebase, understand logic, create reproduction cases
- Each issue assigned to subagent for focused investigation
- Results compiled here with actionable reproduction steps
- Use reproduction steps to identify root causes for fixing

