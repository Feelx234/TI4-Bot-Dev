# Investigation Notes - Session 2026-10-01

## Duplicate Key Warning Investigation

### Error
```
Encountered two children with the same key, `round:2:action:action:action_295`. 
Keys should be unique so that components maintain their identity across updates. 
Non-unique keys may cause children to be duplicated and/or omitted
```

### Likely Causes
1. Action list rendering where the same action ID appears twice in a list
2. Nested action rendering (action within action) causing key duplication
3. Key generation logic that doesn't account for all contexts (round, action type, etc.)

### Search Strategy
1. Search for all `.map()` calls in component files that render actions
2. Look for key patterns using `round:`, `action:`, nonces, or IDs
3. Check EventLog, DecisionList, ActionQueue, ReactionStatusBar components
4. Look for async state updates that might cause stale key references

### Files to Check
- `/root/TI4-Bot-Dev/web/src/components/ReactionStatusBar.tsx`
- `/root/TI4-Bot-Dev/web/src/components/EventLog.tsx`
- `/root/TI4-Bot-Dev/web/src/components/DecisionList.tsx` (if exists)
- `/root/TI4-Bot-Dev/web/src/dev/DecisionGallery.tsx`
- Any component that renders `action_295` or similar action lists

### Reproduction Steps
1. Play through multiple rounds
2. Check browser console for React warnings
3. Look for duplicate actions in the UI or event log
4. Check if actions at round 2 are affected specifically

---

## Game Investigation Status

### Diplomacy Crash (C1)
- **URL:** https://ti4.beckermann-zibert.com/games/game_64a6e64f5568b4974af228fec0c10592
- **Status:** INVESTIGATION IN PROGRESS (Subagent)
- **Expected Output:** Reproducible steps and error logs

### Hacan Trade Bug (H1)
- **URL:** https://ti4.beckermann-zibert.com/games/game_f49965ba37a5a31f310befa69744f1e0
- **Status:** INVESTIGATION IN PROGRESS (Subagent)
- **Expected Output:** Exact trade sequence that fails

---

## Code Search Patterns for Fixes

### Auto-Resolve Single Choice Cases
Search for:
- Strategy card selection (last card)
- Payment validation (no resources)
- System activation (only one reachable)
- Decision options generation

Files likely involved:
- `src/decisions/` or `src/engine/`
- Strategy card components
- Payment UI components
- System activation logic

### Action Card Triggers
Missing trigger text display:
- Search for action card rendering
- Find where card descriptions/abilities are shown
- Add trigger condition display

### Hacan Trade System
- Look for trade logic in Hacan-specific code
- Check trade card choice generation
- Verify trade goods exchange calculation

