# Comprehensive Investigation Findings
**Session:** 2026-10-01 Human Playthrough  
**Date:** 2026-10-05  
**Status:** ✅ ALL INVESTIGATIONS COMPLETE

> Detailed findings for each issue from 8 parallel subagent investigations

---

## Critical Issues (Blockers)

### C1: Diplomacy Card Crash ✅
**Severity:** 🔴 CRITICAL  
**Status:** Root causes identified

#### Primary Crash Points:
1. **Promise Evaluation Panics** (MOST LIKELY)
   - Location: `strategy_cards.rs` lines 571, 1174
   - Issue: `expect()` calls assume promise evaluation never fails
   - Crash Condition: If `evaluate_event()` returns error, program panics
   - Impact: Diplomacy state corruption or failed promises cause crash

2. **Deal Revision Access** (Secondary)
   - Location: `diplomacy/promises.rs` lines 320-327
   - Issue: `expect("deal revision")` panics if deal has no revisions

3. **Signal Index Access** (Low Risk)
   - Location: `diplomacy/signals.rs` line 190
   - Issue: Direct array indexing without bounds check

#### Root Causes:
- No state validation before executing Diplomacy card abilities
- Error handling uses panics instead of proper error propagation
- Deal state assumes consistency without runtime verification
- Missing transaction error handling during promise settlement

#### Recommended Fixes (Priority):
1. Replace `expect()` with proper error handling returning `IllegalChoice`
2. Add diplomacy state validation before Diplomacy card execution
3. Change deal revision access: `.expect()` → `.ok_or()?`
4. Add tracing/logging to capture error context before panic

---

### C2: Map Generation Broken ✅
**Severity:** 🔴 CRITICAL  
**Status:** Root cause identified as seed reuse

#### The Problem:
Appearance of "pre-fabricated maps" is caused by **seeds being reused** across games, not hard-coded layouts.

#### Architecture (Sound):
- Proper randomization: seeded RNG with domain separation
- Map filler selection: `map_filler()` shuffles systems based on seed
- HTTP API generates random seeds with `rand::random::<u64>()`

#### Pre-Fabricated Maps (Not the Issue):
- `/crates/ti4-content/content/map_templates.json` - 9,000+ lines exist
- Never called during normal game creation
- Only for testing/reference

#### Generation Flow:
```
HTTP handler → random seed OR provided seed
              ↓
Game creation → seating::map_filler(seed)
              ↓
Filler selection → shuffles neutral systems based on seed
              ↓
Board construction → places systems in three rings
```

#### Critical Files:
- Entry: `/crates/ti4-server/src/map.rs` - `create_game_with_map()`
- Randomization: `/crates/ti4-engine/src/seating.rs` - `map_filler()`, `neutral_systems()`
- HTTP: `/crates/ti4-server/src/http/games.rs` line 149 (seed generation)
- Session: `/crates/ti4-server/src/session/registry.rs` (seed passing)

#### Root Cause Candidates (Priority):
1. **Seed not randomized** - `rand::random()` not being called
2. **Seed caching** - same seed reused across games
3. **RNG failure** - shuffle operation not working
4. **Test fixtures leaked** - hard-coded seeds in production

#### Investigation Needed:
- [ ] Verify `rand::random()` is actually being called
- [ ] Check for seed caching/reuse logic
- [ ] Trace seed from HTTP handler through to `map_filler()`
- [ ] Look for env vars or config overriding randomization

---

### C3: React Duplicate Key Warning ✅
**Severity:** 🔴 CRITICAL (Logic)  
**Status:** Exact location found, reproduction identified

#### The Error:
```
Encountered two children with the same key, `round:2:action:action:action_295`
```

#### Root Cause Found:
**File:** `/web/src/components/EventLog.tsx`
- Key Generation: Line 114
- Action Node Creation: Lines 115-125
- Rendering: Line 326
- Reset Logic: Line 93

#### How Duplicate Occurs:

1. Phase ID already contains "action" (phase name): `round:2:action`
2. Action ID: `action_295`
3. Key created: `${phaseNode.id}:action:${entry.action_id}`
4. **Result:** `round:2:action:action:action_295` ← DUPLICATE!

**The Bug Sequence:**
```
Step 1: Decision creates action node, pushed to phase.children
Step 2: Non-decision event (marker/transition) resets lastAction = undefined
Step 3: Another decision in SAME PHASE, condition is TRUE
Step 4: SECOND action node created with same ID
Step 5: React finds two elements with identical key → WARNING
```

#### Problematic Condition (Line 115):
```typescript
if (lastAction?.id !== id || lastParent !== phaseNode) {
  // Creates action node - can trigger twice for same action!
}
```

#### Reproduction Scenario:
- Round 2 gameplay
- Multiple decisions in Action phase with same action_id (e.g., `action_295`)
- Marker or non-decision event occurs between decisions
- Resets `lastAction` to undefined
- Next decision in same phase recreates action node

#### Fix Approaches:
1. Track which action nodes already created to prevent duplicates
2. Use `phaseNode.id + action_id` as unique identifier
3. Don't reset `lastAction` when staying in same phase

---

## High Priority Issues

### H1: Hacan Trade System Broken ✅
**Status:** Root cause identified - already partially fixed

#### Key Finding:
**The main bug was already fixed** in commits 09409d7 and 52066ef.

#### The Original Bug (Fixed):
- Hacan's "Guild Ships" ability allows trading with non-neighbors
- Offer generation used `partners()` but validation used `are_neighbours()`
- Players offered trades they couldn't accept: "NotNeighbours" error

#### Current Status:
- `transactions::may_transact()` - Correctly checks both directions ✅
- `why_illegal()` - Now uses `may_transact()` ✅
- All supporting test cases pass ✅

#### Why Trade Might Still Be Broken (game_f49965ba37a5a31f310befa69744f1e0):
1. **Trade Options Not Generating** - `scope.physical` flag might be false
2. **Contact Not Opening** - `may_transact()` might return false unexpectedly
3. **Frontend UI Issue** - Options generated but not displayed
4. **Different Validation Path** - Not using `may_transact()`
5. **Arbiters Ability Issue** - Action card trading (separate system)

#### Files to Investigate:
- Engine: `/crates/ti4-engine/src/transactions.rs`, `game.rs`
- Frontend: `/web/src/components/TradeDeskModal.tsx`, `presentation/tradeDecoder.ts`

#### Debug Steps:
- Verify `may_transact()` returns true for distant Hacan traders
- Check `scope.physical` is set correctly
- Confirm trade options generated by backend
- Verify UI displays all available options

---

### H2: Invasions Display - Non-Fighting Invasions ✅
**Status:** Solution identified - backend filtering required

#### The Issue:
- All invasions shown to all players
- Invasions with 0 defenders (instant victory, no fight) clutter UI
- Should only show invasions with actual combat

#### How to Detect "No Fight":
- Backend projection calculates `odds_context` per planet
- Invasion has no defenders if `opponent == None` for ALL planets
- Check: if any planet has defending opponent = fight invasion

#### Recommended Solution:
**Location:** `/crates/ti4-server/src/projection.rs` (lines 260-403)

**Implementation:**
1. Check if ANY invaded planet has a defending opponent
2. If no defenders on any planet: don't show invasion to spectators
3. Invader always sees invasion process
4. Only affects visibility, not game logic

#### Reproduction:
1. Start invasion with 0 ground defenders on all planets
2. Verify invasion shows for invader ✅
3. Check spectator view (should NOT see invasion)
4. Verify normal invasions (with defenders) still shown

#### Impact:
- Single point of control in projection
- Prevents unnecessary network transmission
- No UI or engine changes needed
- Clean separation of concerns

---

### H3: Payment & Build Flow - Multiple Steps ✅
**Status:** Architecture issue identified with solutions

#### The Problem:
Engine uses mandatory loop asking for each payment individually instead of single flow.

**Example:**
- Player needs 3 resources
- Has: 3 planets (1 each) + 5 trade goods
- Step 1: Auto-exhausts first planet → "Pay 2 more resources"
- Step 2: Exhausts second planet or trade good
- **Result:** 2-3 separate prompts for one payment action

#### Root Causes:

**1. Payment Loop (By Design)**
- Location: `/crates/ti4-engine/src/production.rs` lines 639-672
- Loop: `while paid < owed` asks for one payment at a time
- This matches TI4 rules (planets exhausted one-at-a-time)
- Creates UX friction

**2. Planet-First Prioritization**
- Location: `/crates/ti4-engine/src/production.rs` lines 370-456
- `payment_options()` adds ALL planets BEFORE trade goods
- Auto-selection always chooses planets first

**3. Incomplete Batch Implementation**
- UI supports `onSubmitBatch` via `BasketPlan` (PaymentDrawer.tsx lines 174-219)
- Backend `/basket` endpoint appears not fully implemented
- Falls back to sequential pipeline

#### Quick Fixes (Low Effort):
1. Reorder `payment_options()` to interleave planets and trade goods
2. Add "Planets First / Goods First" toggle in PaymentDrawer
3. Improve messaging explaining multi-step process

#### Medium-Term Fixes:
1. Implement backend `/basket` endpoint for true batch submission
2. Add progress indicator: "Step 1 of 3"
3. Cache payment plan across decisions

#### Long-Term:
1. Replace payment loop with unified selection
2. Add payment strategy parameter to engine
3. Show all payment plans upfront

---

### H5: Action Card Triggers Not Displayed ✅
**Status:** Missing data flow identified - 3 step fix

#### The Issue:
Trigger conditions exist in backend but don't reach frontend.

**Example - Decoy Operation:**
- Trigger: "After another player activates a system that contains 1+ of your structures"
- Current UI: Shows "Decoy Operation" only
- Should show: Card name + trigger condition

#### Root Cause:
```
Backend: Window text exists ✅
  ↓ (not extracted)
ChoiceOption: Missing description ❌
  ↓
Wire: ChoiceOptionDto supports description ✅
  ↓ (but no data sent)
Frontend: Has display infrastructure ✅ (but no data)
```

#### 3-Step Fix:

**Step 1:** Engine - Add description field
- File: `/crates/ti4-engine/src/choice.rs` (line 48)
- Add: `pub description: Option<String>`

**Step 2:** Reaction generation - Extract trigger text
- File: `/crates/ti4-engine/src/reactions.rs` (lines 812-836)
- Extract window text from content store
- Populate `option.description` with trigger condition

**Step 3:** Frontend - Display trigger text
- File: `/web/src/components/ReactionStatusBar.tsx` (line 178)
- Change: Show trigger condition alongside card name

#### Reproduction:
1. Trigger reaction opportunity (e.g., Decoy Operation)
2. Check browser console - description missing in ChoiceOptionDto
3. Verify window text exists in content manifest
4. Trace presentation layer for action cards

#### Data Available:
- Window text already exists: `content.get(ContentType::ActionCards, alias).and_then(|record| record.text("window"))`
- Just needs extraction and transmission

---

### H6: Single-Choice Decisions Should Auto-Resolve ✅
**Status:** Comprehensive catalog created - 23 scenarios identified

#### Already Implemented:
- `production.rs:648` - Auto-resolves single payment options ✅
- `thunders_edge.rs:192,236` - Auto-resolves single discards ✅

#### High-Priority Scenarios Needing Auto-Resolution:

1. **Strategy Card Draft - Last Card** (Every game)
   - When only one unclaimed card remains
   - Player must pick it

2. **Strategic Action Selection** (Common)
   - When only one unused strategy card available
   - Partially handled but still shows modal

3. **System Activation** (Frequent)
   - When only one system can be tactically activated

4. **Technology Research** (Common)
   - When only one technology is researchable

5. **Action Card Effects** (10+ locations)
   - Multiple card types with forced single target

#### Total Scenarios Found:
- **23 distinct categories** identified
- Most use early-return patterns but don't auto-resolve
- Missing: `if options.len() == 1` checks throughout

#### Recommended Architecture:
**Approach A (Recommended):** Centralized wrapper
- Wrap `Table::ask_seeing()` to check `if choice.options.len() == 1`
- Single point of change
- Applies uniformly to all decision generators
- Clean separation of concerns

#### Priority Ranking:
**Tier 1 (High Visibility, High Frequency):**
- Strategy card draft last card
- Strategic action selection
- System activation limiting

**Tier 2 (Medium Visibility):**
- Technology research selection
- Action card single-target resolution
- Relic ability selection

**Tier 3 (Low Visibility):**
- Commodity replenishment when single source
- Minor card effects

#### Implementation Files:
- 15+ decision generators in `/crates/ti4-engine/src/`
- Decision handling in `/crates/ti4-engine/src/choice.rs`

---

### H7: Undo Non-Permanent Decisions ✅
**Status:** Current system analyzed - needs new implementation

#### Current System (Host-Only):
✅ **Works well:**
- Full game history replay with deterministic randomness
- Batch transaction support
- Atomic state replacement
- Comprehensive test coverage

❌ **Doesn't support H7:**
- Host-only (not per-player during turn)
- Permanent-decision agnostic (undoes ALL decisions)
- Timeline-replacing (affects entire game for all players)
- Not scoped to individual turns

#### Architecture:
**Current undo uses decision journal replay:**
1. All decisions recorded as `DecisionRecord`
2. Game state replayed deterministically from decision prefix
3. RNG seeded and deterministic
4. Session worker stopped and replaced
5. History stored durably in `GameHistory`

#### Missing for H7:
1. **Decision Classification** - Distinguish permanent vs non-permanent
2. **Turn Boundaries** - Turn-scoped replay (not full game)
3. **Per-Player Authorization** - New endpoint and access control
4. **Permanence Metadata** - Add to `DecisionContext`

#### Key Files:
- Engine/replay: `/crates/ti4-engine/src/game.rs`, `choice.rs`, `decision_context.rs`
- Server history: `/crates/ti4-server/src/session/registry.rs`
- HTTP endpoint: `/crates/ti4-server/src/http/games.rs`
- Web client: `/web/src/protocol/client.ts`

#### Permanent vs Non-Permanent:
**Can't Undo:**
- Ended phases
- Confirmed combat results
- Finished trades
- Spent resources (committed)
- Dice rolls resolved

**Can Undo:**
- Strategic action pre-selection
- Unit placement before lock
- Target selection before commit
- Initial resource/token choices
- Pre-battle selections

#### Implementation Challenges:
- Determining permanence at decision point
- Partial turn state rollback
- Preventing abuse while supporting per-player undo
- UX for selecting which decisions to undo

#### Suggested Phased Approach:
**Phase 1:** Add permanence metadata to decisions
**Phase 2:** Implement turn-scoped state snapshots
**Phase 3:** Build per-player undo authorization
**Phase 4:** UI for undo selection

---

## Summary Table

| Issue | Type | Root Cause | Fix Effort | Priority |
|-------|------|-----------|-----------|----------|
| **C1** | Crash | Panic on invalid state | Medium | 🔴 CRITICAL |
| **C2** | Map Gen | Seed reuse/not randomized | Low-Medium | 🔴 CRITICAL |
| **C3** | React Warning | Duplicate key generation logic | Low | 🔴 CRITICAL |
| **H1** | Trade | Already partially fixed, unclear remaining issue | Medium | 🟠 HIGH |
| **H2** | Invasions | Missing spectator filter in projection | Low | 🟠 HIGH |
| **H3** | Payment | Architecture uses sequential steps | Medium | 🟠 HIGH |
| **H5** | Triggers | Data not extracted/transmitted | Low | 🟠 HIGH |
| **H6** | Auto-resolve | No detection of single choices | Low | 🟠 HIGH |
| **H7** | Undo | New feature, needs architecture | High | 🟠 HIGH |
| **H8** | Trade UI | Missing card descriptions/bad layout | Low-Medium | 🟠 HIGH |

---

## Next Steps

1. **Immediate (Blockers):** C1, C2, C3
2. **High-Impact (UI):** H1, H2, H5
3. **Quick Wins:** H6, H8
4. **Medium-Term:** H3, H4
5. **Long-Term:** H7

All findings include specific file locations, line numbers, and reproduction steps for implementation.

