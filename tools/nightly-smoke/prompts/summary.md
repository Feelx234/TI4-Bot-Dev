You are reviewing the results of last night's automated UI smoke sweep of a Twilight Imperium 4
web client (React/TypeScript frontend in `web/`, Rust engine in `crates/ti4-engine`, server in
`crates/ti4-server`). Haiku proctors ran random-click playthroughs (random seeds) until round
{{STOP_ROUND}} and appended one entry per run to the night report.

Night report: {{REPORT}}
Run directories (raw evidence: trace/, run.log, digest.md, server-data/): {{NIGHT_DIR}}
Repository: {{REPO}}

Your job (read-only — do NOT modify, create or delete any file, do NOT commit, do NOT implement fixes):
1. Read the night report. Verify the important findings against the raw evidence in the run
   directories (failure.txt, run.log, trace.jsonl, digest.md) — proctors can be wrong.
2. De-duplicate findings across runs and rank them: blockers (game cannot continue), engine/
   server errors, JavaScript errors, server rejections of offered controls, rules
   inconsistencies, harness/test-infrastructure problems.
3. For each real issue, locate the likely root cause in the code (read the relevant source) and
   **suggest** a fix: files, functions, what to change, and a test that would catch it. Say
   how confident you are. If a fix is not necessary (noise, expected behaviour), say so.
4. Briefly summarise game coverage: how many runs, rounds reached, notable events (action cards
   played, Mecatol Rex taken, technologies, agendas), and decision subtypes never reached.

Output only the morning summary in markdown, starting with a 3–5 line TL;DR, then
"## Findings" (ranked), "## Suggested fixes", "## Coverage", "## Harness / sweep issues".
