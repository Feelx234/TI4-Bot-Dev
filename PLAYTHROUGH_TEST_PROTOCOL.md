# Random Playthrough Bug Catch Protocol

**Objective:** Run 10 random 6-player games to catch bugs in M-item implementations

**Setup:**
1. Start game server: `npm run dev` (web directory)
2. Open browser: http://localhost:3000
3. Create 6 AI players (random names)
4. Use seed-based random map generation

**For Each Playthrough:**

| # | Map Seed | Players | Status | Bug Report |
|---|----------|---------|--------|------------|
| 1 | Random | 6 AI | 🔄 | - |
| 2 | Random | 6 AI | ⏳ | - |
| 3 | Random | 6 AI | ⏳ | - |
| 4 | Random | 6 AI | ⏳ | - |
| 5 | Random | 6 AI | ⏳ | - |
| 6 | Random | 6 AI | ⏳ | - |
| 7 | Random | 6 AI | ⏳ | - |
| 8 | Random | 6 AI | ⏳ | - |
| 9 | Random | 6 AI | ⏳ | - |
| 10 | Random | 6 AI | ⏳ | - |

**Watch For:**
- ✅ M11: Strategy cards grayed out properly
- ✅ M12: VP breakdown displays correctly
- ✅ M13: Agenda phase card display
- ✅ M14: Action card triggers visible
- ✅ M15: Turn sound notifications
- ✅ M16: Player list sorting
- ✅ M19: Fleet supply in production
- ✅ M20: Invasion pre-selection working
- ✅ M22: Strategy cards loading correctly
- ❌ C1: Diplomacy card crashes
- ❌ C2: Map generation issues
- ❌ C3: Duplicate key warnings
- ❌ H1-H8: High priority issues

**Bug Tracking Format:**
```
## Playthrough #X - [Status]
- **Seed:** XXXX
- **Duration:** HH:MM
- **Final Score:** P1: X, P2: X, ...
- **Bugs Found:**
  1. [Component] - [Issue] - [Reproduction Steps]
  2. ...
```

**Report Location:** `/root/TI4-Bot-Dev/PLAYTHROUGH_RESULTS_2026-10-05.md`
