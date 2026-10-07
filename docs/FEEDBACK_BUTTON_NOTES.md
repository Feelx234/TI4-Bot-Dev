# In-game feedback button: notes (not started)

Idea: a "Give feedback" button in the web UI. The player writes feedback, and it is sent
together with the game history. Collected 2026-10-06 for later; nothing is built yet.

## Constraint

Not everyone on the dev team has access to the server, so storing feedback only on the server
is not enough. It has to land somewhere the whole team can see.

## What the server already has

- Every game is persisted under `data/games/<game_id>/` (`decisions.jsonl`, `events.jsonl`,
  `history.json`, `init.json`, `lobby.json`). The history therefore does not need to be uploaded
  by the UI: the endpoint can attach it from disk.
- Caveat seen in the nightly runs: the persisted copy can lag behind the live game, so the
  endpoint should flush or snapshot the session first.

## Receiving options

| Option | Setup | Notes |
|---|---|---|
| Discord channel webhook | create a webhook URL, put it in a server env var | simplest; no bot; file attachments up to about 25 MB, enough for a game history; whole team sees it |
| Telegram group bot | create a bot, bot token in an env var, add it to a group | about as easy; files up to 50 MB |
| GitHub issue | fine-grained token on the server | durable and linkable to fixes; the API cannot attach files to issues, so the history would go in a secret gist linked from the issue |
| Email | SMTP credentials or a mail service | most setup (deliverability, spam handling) for the same result |
| File on the server | none | keep as a fallback so a failed webhook never loses feedback; not shared with the team on its own |

Slack incoming webhooks cannot attach files, so Slack is a poor fit.

## Leaning

A chat webhook (Discord or Telegram, depending on what the team uses) that posts the feedback
text plus the history as an attached file, with the plain file on the server as a fallback.

## Open questions

- Which chat does the team use, Discord or Telegram? (Decides the first implementation.)
- Rate limiting per game to stop spam; the webhook secret lives in the server environment.
- What to include besides the text: game id, player seat, game version, browser/user agent,
  and the current screen state or a screenshot?
- Privacy: whether private cards in the history may be sent, since the history contains hidden
  information (hands, secret objectives).
- Should the feedback also be visible to the player afterwards (a confirmation with a reference id)?

## Rough shape if built

1. `POST /api/games/{game_id}/feedback` with `{ text, ... }` and the player session header;
   validates the seat, applies the rate limit, snapshots the game history.
2. Writes `feedback-<time>.json` next to the game's files (fallback, always done).
3. Posts to the configured webhook with the history attached; failures are logged, never
   shown as lost feedback to the player.
4. A button and dialog in the web UI (`web/src/components`), with a Vitest test and a server
   test for the endpoint.
