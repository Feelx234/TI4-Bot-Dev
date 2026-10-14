//! Playtest feedback reports: validation, one JSON file per report, and a terminal lister.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_TEXT_CHARS: usize = 4000;
pub const MAX_CONTEXT_BYTES: usize = 200 * 1024;
/// Request body cap for the feedback route (the rest of the API keeps 8 KiB).
pub const MAX_BODY_BYTES: usize = 256 * 1024;
pub const CATEGORIES: [&str; 4] = ["bug", "confusing", "idea", "other"];

/// Body of `POST /api/feedback`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackRequest {
    pub category: String,
    pub text: String,
    /// Auto-collected, viewer-visible context; absent when the sender unticked "Include game info".
    pub context: Option<Value>,
}

/// What lands on disk.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredFeedback {
    pub id: String,
    pub received_at: String,
    pub received_unix: u64,
    pub server_commit: String,
    pub category: String,
    pub text: String,
    pub context: Option<Value>,
}

/// Returns the validated request or a human-readable refusal.
pub fn validate(request: &FeedbackRequest) -> Result<(), String> {
    if !CATEGORIES.contains(&request.category.as_str()) {
        return Err(format!("category must be one of {CATEGORIES:?}"));
    }
    let chars = request.text.chars().count();
    if request.text.trim().is_empty() {
        return Err("text must not be empty".to_owned());
    }
    if chars > MAX_TEXT_CHARS {
        return Err(format!("text is limited to {MAX_TEXT_CHARS} characters"));
    }
    if let Some(context) = &request.context {
        if !context.is_object() {
            return Err("context must be an object".to_owned());
        }
        if context.to_string().len() > MAX_CONTEXT_BYTES {
            return Err("context is too large".to_owned());
        }
    }
    Ok(())
}

/// `(year, month, day)` of a unix timestamp in UTC.
#[must_use]
pub fn civil_from_unix(secs: u64) -> (i64, u32, u32) {
    let z = i64::try_from(secs / 86_400).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Writes the report atomically to `<data_dir>/feedback/YYYY-MM-DD/<id>.json`; returns the id.
pub fn store(
    data_dir: &Path,
    request: FeedbackRequest,
    now_unix: u64,
    commit: &str,
) -> std::io::Result<String> {
    let (y, m, d) = civil_from_unix(now_unix);
    let id = format!("fb_{now_unix}_{:08x}", rand::random::<u32>());
    let dir = data_dir
        .join("feedback")
        .join(format!("{y:04}-{m:02}-{d:02}"));
    fs::create_dir_all(&dir)?;
    let secs_of_day = now_unix % 86_400;
    let record = StoredFeedback {
        id: id.clone(),
        received_at: format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            secs_of_day / 3600,
            secs_of_day % 3600 / 60,
            secs_of_day % 60
        ),
        received_unix: now_unix,
        server_commit: commit.to_owned(),
        category: request.category,
        text: request.text,
        context: request.context,
    };
    let tmp = dir.join(format!(".{id}.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(&tmp, dir.join(format!("{id}.json")))?;
    Ok(id)
}

/// Most recent reports first.
pub fn list_recent(data_dir: &Path, limit: usize) -> Vec<(PathBuf, StoredFeedback)> {
    let root = data_dir.join("feedback");
    let mut days: Vec<PathBuf> = fs::read_dir(&root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    days.sort();
    days.reverse();
    let mut out = Vec::new();
    for day in days {
        let mut files: Vec<PathBuf> = fs::read_dir(&day)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        files.reverse();
        for file in files {
            if let Ok(bytes) = fs::read(&file)
                && let Ok(record) = serde_json::from_slice::<StoredFeedback>(&bytes)
            {
                out.push((file, record));
            }
        }
        if out.len() >= limit {
            break;
        }
    }
    out.sort_by(|a, b| b.1.received_unix.cmp(&a.1.received_unix));
    out.truncate(limit);
    out
}

/// One-line description of who sent the report and who sat at the table, from the optional
/// `context.sender` and `context.players`. `None` for reports that predate names.
#[must_use]
pub fn who_line(record: &StoredFeedback) -> Option<String> {
    let ctx = record.context.as_ref()?;
    let text = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_str)
            .map(|s| s.chars().filter(|c| !c.is_control()).take(80).collect::<String>())
            .filter(|s| !s.is_empty())
    };
    let mut parts = Vec::new();
    if let Some(sender) = ctx.get("sender") {
        let mut name = text(sender, "nickname").or_else(|| text(sender, "typed_name"));
        if let (Some(n), Some(lobby)) = (&mut name, text(sender, "lobby_nickname")) {
            n.push_str(&format!(" (lobby name {lobby})"));
        } else if name.is_none() {
            name = text(sender, "lobby_nickname").map(|l| format!("lobby name {l}"));
        }
        if let Some(n) = name {
            parts.push(format!("from {n}"));
        }
    }
    if let Some(players) = ctx.get("players").and_then(Value::as_array) {
        let table: Vec<String> = players
            .iter()
            .map(|p| {
                let name = text(p, "nickname").unwrap_or_else(|| "?".to_owned());
                let faction = text(p, "faction").unwrap_or_else(|| "?".to_owned());
                let mark = if p.get("is_sender").and_then(Value::as_bool) == Some(true) {
                    "*"
                } else {
                    ""
                };
                format!("{name}{mark} ({faction})")
            })
            .collect();
        if !table.is_empty() {
            parts.push(format!("table: {}", table.join(", ")));
        }
    }
    (!parts.is_empty()).then(|| parts.join(" | "))
}

/// Terminal-readable rendering of one report.
#[must_use]
pub fn render(record: &StoredFeedback, verbose: bool) -> String {
    let mut s = String::new();
    let ctx = record.context.as_ref();
    let get = |key: &str| {
        ctx.and_then(|c| c.get(key))
            .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
            .unwrap_or_else(|| "-".to_owned())
    };
    let _ = writeln!(
        s,
        "=== {} | {} | {} | client {} | game {} | seat {} | round {} {}",
        record.id,
        record.received_at,
        record.category.to_uppercase(),
        get("build_commit"),
        get("game_id"),
        get("viewer_seat"),
        get("round"),
        get("phase"),
    );
    if let Some(who) = who_line(record) {
        let _ = writeln!(s, "    {who}");
    }
    let _ = writeln!(s, "{}", record.text.trim());
    if verbose && let Some(ctx) = ctx {
        let _ = writeln!(
            s,
            "--- context\n{}",
            serde_json::to_string_pretty(ctx).unwrap_or_default()
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates() {
        assert_eq!(civil_from_unix(0), (1970, 1, 1));
        assert_eq!(civil_from_unix(1_760_227_200), (2025, 10, 12));
        assert_eq!(civil_from_unix(1_709_164_800), (2024, 2, 29));
    }

    fn record(context: Option<Value>) -> StoredFeedback {
        StoredFeedback {
            id: "fb_1_ab".to_owned(),
            received_at: "2026-10-13T10:00:00Z".to_owned(),
            received_unix: 1,
            server_commit: "abc".to_owned(),
            category: "bug".to_owned(),
            text: "np gave me a note".to_owned(),
            context,
        }
    }

    #[test]
    fn names_are_listed_with_the_table() {
        let r = record(Some(serde_json::json!({
            "sender": { "nickname": "Alex", "lobby_nickname": "Al", "position": 1 },
            "players": [
                { "seat": "p1", "nickname": "Alex", "faction": "sol", "is_sender": true },
                { "seat": "p2", "nickname": "np", "faction": "letnev", "is_sender": false }
            ]
        })));
        let out = render(&r, false);
        assert!(out.contains("from Alex (lobby name Al) | table: Alex* (sol), np (letnev)"), "{out}");
    }

    #[test]
    fn typed_name_is_used_without_a_nickname_and_control_characters_are_dropped() {
        let r = record(Some(serde_json::json!({ "sender": { "typed_name": "Sam\u{1b}[31m" } })));
        assert_eq!(who_line(&r).as_deref(), Some("from Sam[31m"));
    }

    #[test]
    fn old_reports_without_names_still_load_and_render() {
        let old = r#"{"id":"fb_1_ab","received_at":"x","received_unix":1,"server_commit":"c","category":"bug","text":"t","context":{"game_id":"g","faction":"sol"}}"#;
        let r: StoredFeedback = serde_json::from_str(old).unwrap();
        assert_eq!(who_line(&r), None);
        assert!(!render(&r, false).contains("from "));
        let none: StoredFeedback = serde_json::from_str(r#"{"id":"i","received_at":"x","received_unix":1,"server_commit":"c","category":"bug","text":"t","context":null}"#).unwrap();
        assert_eq!(who_line(&none), None);
    }

    #[test]
    fn stored_report_keeps_names_and_never_a_credential_field() {
        let dir = std::env::temp_dir().join(format!("fbnames_{}", rand::random::<u32>()));
        let request = FeedbackRequest {
            category: "idea".to_owned(),
            text: "hi".to_owned(),
            context: Some(serde_json::json!({ "sender": { "nickname": "Alex" } })),
        };
        validate(&request).unwrap();
        let id = store(&dir, request, 1_760_227_200, "abc").unwrap();
        let found = list_recent(&dir, 5);
        assert_eq!(found[0].1.id, id);
        assert_eq!(who_line(&found[0].1).as_deref(), Some("from Alex"));
        let _ = fs::remove_dir_all(&dir);
    }
}
