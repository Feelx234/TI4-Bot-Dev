//! Native egui front end for live and saved reviews.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, Color32, Sense};
use serde::{Deserialize, Serialize};
use ti4_content::ContentStore;
use ti4_model::content_types::ContentType;
use ti4_model::id::SystemId;

use crate::{
    AdvanceUnit, LiveReview, MAX_COMMAND_STEPS, ProfileTable, ReviewFrame, ReviewSession,
    SessionOutcome, SimulationConfig, default_sampling_temperature, export_html, load_session,
    save_session,
};

use crate::view::{
    BoardLayout, DecisionPath, PANEL_FILL, PANEL_TEXT, action_summary, board_view, content_label,
    decision_rows, draw_board, event_rows, item_section, planets_for_tile, player_color,
    seat_label, section, section_with_id, selected_system, stat_badge, step_view, unit_base,
};

const STEPS_PER_UI_FRAME: usize = 128;

/// The floor and ceiling on the adaptive autosave interval, in engine steps.
///
/// The floor keeps a short review saving often enough to be worth having. The ceiling stops a very
/// expensive save from pushing the next one so far out that a crash loses the run.
const AUTOSAVE_MIN_STEPS: usize = 1024;
const AUTOSAVE_MAX_STEPS: usize = 65_536;

/// The share of running time autosaving is allowed to take.
///
/// Ten means a save that took one second buys ten seconds of play before the next, so the cost
/// stays proportional to the run rather than to the run squared.
const AUTOSAVE_DUTY: u32 = 10;
const SETTINGS_PATH: &str = "out/reviews/reviewer-settings.json";
const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// Relationships, deals, signals and finished deals for a game played with structured diplomacy.
fn diplomacy_panel(ui: &mut egui::Ui, frame: &ReviewFrame) {
    let state = &frame.state;
    let diplomacy = &state.diplomacy;
    if !diplomacy.enabled {
        ui.weak("Structured diplomacy is off for this game.");
        return;
    }
    ui.label("How each row seat regards each column seat.");
    ui.small(crate::diplomacy::RELATIONSHIP_LEGEND);
    egui::Grid::new(format!("diplomacy-relationships-{}", frame.index))
        .striped(true)
        .show(ui, |ui| {
            ui.strong("regards →");
            for subject in &state.seating_order {
                seat_label(ui, subject, subject.to_string());
            }
            ui.end_row();
            for observer in &state.seating_order {
                seat_label(ui, observer, observer.to_string());
                for subject in &state.seating_order {
                    if observer == subject {
                        ui.weak("—");
                        continue;
                    }
                    let relationship = diplomacy.relationship(observer, subject);
                    let mut text = format!(
                        "{} · {}",
                        crate::diplomacy::stance(relationship),
                        crate::diplomacy::relationship_cell(relationship)
                    );
                    if ti4_engine::diplomacy::recent_attack(state, observer, subject) {
                        text.push_str(" ⚔");
                    }
                    if ti4_engine::diplomacy::recent_breach(state, observer, subject) {
                        text.push_str(" ✗");
                    }
                    ui.label(text);
                }
                ui.end_row();
            }
        });

    ui.strong(format!("Active deals · {}", diplomacy.active_deals.len()));
    if diplomacy.active_deals.is_empty() {
        ui.weak("None");
    }
    for deal in diplomacy.active_deals.values() {
        ui.group(|ui| {
            for (index, line) in crate::diplomacy::deal_lines(state, deal)
                .into_iter()
                .enumerate()
            {
                if index == 0 {
                    ui.strong(line);
                } else {
                    ui.label(line);
                }
            }
        });
    }

    ui.strong(format!(
        "Recent signals · {}",
        diplomacy.recent_signals.len()
    ));
    if diplomacy.recent_signals.is_empty() {
        ui.weak("None");
    }
    for signal in &diplomacy.recent_signals {
        ui.label(crate::diplomacy::signal_text(state, signal));
    }

    egui::CollapsingHeader::new(format!("Finished deals · {}", diplomacy.history.len()))
        .id_salt(format!("diplomacy-history-{}", frame.index))
        .show(ui, |ui| {
            if diplomacy.history.is_empty() {
                ui.weak("None yet");
            }
            for summary in diplomacy.history.iter().rev() {
                ui.group(|ui| {
                    for (index, line) in crate::diplomacy::summary_lines(state, summary)
                        .into_iter()
                        .enumerate()
                    {
                        if index == 0 {
                            ui.strong(line);
                        } else {
                            ui.label(line);
                        }
                    }
                });
            }
        });
    ui.small(format!(
        "Journal events so far: {}",
        diplomacy.journal.len()
    ));
}

#[derive(Clone, Debug)]
enum RunTarget {
    Count { unit: AdvanceUnit, remaining: usize },
    Round(u32),
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ReviewerSettings {
    checkpoint: String,
    map_pool: String,
    profile_table: ProfileTable,
    temperature: f64,
    last_review: Option<String>,
    diplomacy: bool,
}

impl Default for ReviewerSettings {
    fn default() -> Self {
        Self {
            checkpoint: String::new(),
            map_pool: String::new(),
            profile_table: ProfileTable::default(),
            temperature: default_sampling_temperature(),
            last_review: None,
            diplomacy: false,
        }
    }
}

fn load_settings() -> std::result::Result<ReviewerSettings, String> {
    let path = Path::new(SETTINGS_PATH);
    if !path.is_file() {
        return Ok(ReviewerSettings::default());
    }
    let metadata =
        fs::metadata(path).map_err(|error| format!("read settings metadata: {error}"))?;
    if metadata.len() > MAX_SETTINGS_BYTES {
        return Err(format!(
            "settings file is {} bytes, above the {MAX_SETTINGS_BYTES}-byte limit",
            metadata.len()
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read settings: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("parse settings: {error}"))
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TI4 learned-game reviewer")
            .with_inner_size([1500.0, 950.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TI4 learned-game reviewer",
        options,
        Box::new(|context| Ok(Box::new(ReviewApp::new(context)))),
    )
}

struct ReviewApp {
    checkpoint: String,
    map_pool: String,
    seed: String,
    rotation: usize,
    table: ProfileTable,
    temperature: f64,
    diplomacy: bool,
    run_count: String,
    run_unit: AdvanceUnit,
    live: Option<LiveReview>,
    replay: Option<ReviewSession>,
    viewed: usize,
    run_target: Option<RunTarget>,
    command_steps: usize,
    autosave: Option<PathBuf>,
    /// The step count at which the running command may autosave again.
    ///
    /// A fixed interval is what made a long review crawl and then appear to hang. `save_session`
    /// validates every frame, serialises every frame's whole `GameState` into one buffer and writes
    /// the lot, so one autosave costs O(frames); firing that on a fixed step interval makes a run
    /// O(frames^2). By two thousand frames the session is hundreds of megabytes and a single
    /// autosave blocks the UI thread for seconds -- low CPU the whole time, because it is
    /// serialisation and disk, which is exactly how it was reported.
    ///
    /// So the interval is set from what the last autosave actually cost: see
    /// [`ReviewApp::autosave_now`]. Autosaving stays a bounded share of the run however long it gets,
    /// and the file format is unchanged.
    next_autosave_step: usize,
    /// Whether the players (left) and decision (right) panels are open.
    show_players: bool,
    show_decision: bool,
    /// When the running command started, for the rate the budget above is spent against.
    command_started: Option<std::time::Instant>,
    last_review: Option<PathBuf>,
    status: String,
    selected_tile: Option<String>,
}

impl ReviewApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        context.egui_ctx.set_visuals(egui::Visuals::dark());
        let (settings, settings_error) = match load_settings() {
            Ok(settings) => (settings, None),
            Err(error) => (ReviewerSettings::default(), Some(error)),
        };
        Self {
            checkpoint: settings.checkpoint,
            map_pool: settings.map_pool,
            seed: "42".to_owned(),
            rotation: 0,
            table: settings.profile_table,
            temperature: settings.temperature,
            diplomacy: settings.diplomacy,
            run_count: "10".to_owned(),
            run_unit: AdvanceUnit::Step,
            live: None,
            replay: None,
            viewed: 0,
            run_target: None,
            command_steps: 0,
            next_autosave_step: AUTOSAVE_MIN_STEPS,
            show_players: true,
            show_decision: true,
            command_started: None,
            autosave: None,
            last_review: settings.last_review.map(PathBuf::from),
            status: settings_error.map_or_else(
                || "Restored the last checkpoint/profile and map-pool selections.".to_owned(),
                |error| format!("Settings were not restored: {error}"),
            ),
            selected_tile: None,
        }
    }

    fn session(&self) -> Option<&ReviewSession> {
        self.live
            .as_ref()
            .map(|live| &live.session)
            .or(self.replay.as_ref())
    }

    /// Lend the session to a `&mut self` method without copying it.
    ///
    /// The panels need `&mut self` for their own UI state while reading the session, which the
    /// borrow checker will not allow directly. Cloning it to get around that is what `ui` used to
    /// do, and it is ruinous here: a clone is a deep copy of every frame's whole `GameState`, so a
    /// 1,800-frame review copied hundreds of megabytes *per repaint* -- which is why the window got
    /// slower the longer a run went and why scrolling crawled.
    ///
    /// Moving it out and back costs two pointer writes. The session is restored on every path,
    /// including when `act` panics, because it is put back by the guard's `Drop`.
    fn with_session<R>(&mut self, act: impl FnOnce(&mut Self, &ReviewSession) -> R) -> Option<R> {
        /// Whichever field the session was taken from, so it goes back to the same one.
        enum Held {
            Live(Box<LiveReview>),
            Replay(Box<ReviewSession>),
        }

        // `LiveReview` moves whole rather than by its `session` field: `ReviewSession` is not
        // `Default`, so there is nothing to leave behind in its place. Once it is out, it is an
        // ordinary local -- borrowing it shared while `self` is borrowed mutably is disjoint, which
        // is what makes this safe as well as cheap.
        let held = match self.live.take() {
            Some(live) => Held::Live(Box::new(live)),
            None => Held::Replay(Box::new(self.replay.take()?)),
        };
        let result = {
            let session = match &held {
                Held::Live(live) => &live.session,
                Held::Replay(session) => session,
            };
            act(self, session)
        };
        match held {
            Held::Live(live) => self.live = Some(*live),
            Held::Replay(session) => self.replay = Some(*session),
        }
        Some(result)
    }

    fn latest_index(&self) -> usize {
        self.session()
            .map_or(0, |session| session.frames.len().saturating_sub(1))
    }

    fn settings(&self) -> ReviewerSettings {
        ReviewerSettings {
            checkpoint: self.checkpoint.trim().to_owned(),
            map_pool: self.map_pool.trim().to_owned(),
            profile_table: self.table,
            temperature: self.temperature,
            last_review: self
                .last_review
                .as_ref()
                .map(|path| path.display().to_string()),
            diplomacy: self.diplomacy,
        }
    }

    fn persist_settings(&mut self) {
        let bytes = match serde_json::to_vec_pretty(&self.settings()) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.status = format!("Save settings failed: {error}");
                return;
            }
        };
        if let Err(error) = super::replace_file(Path::new(SETTINGS_PATH), &bytes) {
            self.status = format!("Save settings failed: {error}");
        }
    }

    fn install_replay(&mut self, path: &Path, session: ReviewSession) {
        self.viewed = 0;
        self.live = None;
        self.replay = Some(session);
        self.run_target = None;
        self.autosave = None;
        self.last_review = Some(path.to_path_buf());
        self.status = format!("Opened {} in view-only mode", path.display());
        self.persist_settings();
    }

    fn previous_candidates(&self) -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Some(path) = &self.last_review {
            candidates.push(path.clone());
        }
        let mut discovered: Vec<(SystemTime, PathBuf)> = fs::read_dir("out/reviews")
            .into_iter()
            .flatten()
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| is_review(path))
            .filter_map(|path| {
                let modified = fs::metadata(&path).ok()?.modified().ok()?;
                Some((modified, path))
            })
            .collect();
        discovered.sort_by(|left, right| right.0.cmp(&left.0));
        candidates.extend(discovered.into_iter().map(|(_, path)| path));
        candidates.dedup();
        candidates
    }

    fn open_previous(&mut self) {
        let mut last_error = None;
        for path in self.previous_candidates() {
            match load_session(&path) {
                Ok(session) => {
                    self.install_replay(&path, session);
                    return;
                }
                Err(error) => last_error = Some(format!("{}: {error}", path.display())),
            }
        }
        self.status = last_error.map_or_else(
            || "No previous saved or autosaved game was found.".to_owned(),
            |error| format!("No valid previous game was found; last error: {error}"),
        );
    }

    fn load_start(&mut self) {
        let seed = match self.seed.trim().parse::<u64>() {
            Ok(seed) => seed,
            Err(error) => {
                self.status = format!("Invalid seed: {error}");
                return;
            }
        };
        let config = SimulationConfig {
            checkpoint: PathBuf::from(self.checkpoint.trim()),
            map_pool: PathBuf::from(self.map_pool.trim()),
            seed,
            rotation: self.rotation,
            table: self.table,
            temperature: self.temperature,
            diplomacy: self.diplomacy,
        };
        match LiveReview::start(&config) {
            Ok(live) => {
                let lineup = live.session.manifest.factions.join(" → ");
                self.autosave = Some(PathBuf::from("out/reviews").join(format!(
                    "autosave-{seed}-rotation{}-{}{}.ti4review.json.zst",
                    self.rotation,
                    match self.table {
                        ProfileTable::Learner => "learner",
                        ProfileTable::Accepted => "accepted",
                    },
                    if self.diplomacy { "-diplomacy" } else { "" }
                )));
                self.live = Some(live);
                self.replay = None;
                self.viewed = 0;
                self.run_target = None;
                self.status =
                    format!("Starting table loaded; no engine step has run. Seats 0–5: {lineup}");
                self.autosave_now();
                if let Some(path) = &self.autosave {
                    self.last_review = Some(path.clone());
                }
                self.persist_settings();
            }
            Err(error) => self.status = format!("Load failed: {error}"),
        }
    }

    /// Start a running command's autosave budget over.
    ///
    /// Each command is timed on its own. Carrying the previous one's rate across would let a run
    /// that was interrupted early set the interval for one that is not.
    fn begin_autosave_budget(&mut self) {
        self.command_started = Some(std::time::Instant::now());
        self.next_autosave_step = AUTOSAVE_MIN_STEPS;
    }

    fn autosave_now(&mut self) {
        let Some(path) = self.autosave.clone() else {
            return;
        };
        let Some(session) = self.session() else {
            return;
        };
        let started = std::time::Instant::now();
        if let Err(error) = save_session(&path, session) {
            self.status = format!("Autosave failed: {error}");
        }
        let cost = started.elapsed();

        // Buy back what the save cost, in steps, from the rate the run is actually managing. A
        // cheap save on a short session keeps the floor interval; an expensive one on a long
        // session earns a proportionally longer wait, which is what stops the quadratic blow-up.
        let elapsed = self
            .command_started
            .map_or(cost, |at| at.elapsed())
            .max(cost);
        let steps_per_second = if elapsed.as_secs_f64() > 0.0 {
            #[expect(
                clippy::cast_precision_loss,
                reason = "step counts are far below f64's exact-integer range"
            )]
            let steps = self.command_steps as f64;
            steps / elapsed.as_secs_f64()
        } else {
            0.0
        };
        let budget = cost.as_secs_f64() * f64::from(AUTOSAVE_DUTY) * steps_per_second;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped into usize range on the next line"
        )]
        let interval = (budget as usize).clamp(AUTOSAVE_MIN_STEPS, AUTOSAVE_MAX_STEPS);
        self.next_autosave_step = self.command_steps.saturating_add(interval);
    }

    fn save_as(&mut self) {
        let Some(session) = self.session() else {
            "Nothing to save.".clone_into(&mut self.status);
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Compressed TI4 review", &["zst"])
            .add_filter("Uncompressed TI4 review", &["json"])
            .set_file_name("game.ti4review.json.zst")
            .save_file()
        else {
            return;
        };
        match save_session(&path, session) {
            Ok(()) => {
                self.last_review = Some(path.clone());
                self.status = format!("Saved {}", path.display());
                self.persist_settings();
            }
            Err(error) => self.status = format!("Save failed: {error}"),
        }
    }

    fn open_review(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("TI4 review", &["zst", "json"])
            .pick_file()
        else {
            return;
        };
        match load_session(&path) {
            Ok(session) => self.install_replay(&path, session),
            Err(error) => self.status = format!("Open failed: {error}"),
        }
    }

    fn export(&mut self) {
        let Some(session) = self.session() else {
            "Nothing to export.".clone_into(&mut self.status);
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("HTML", &["html"])
            .set_file_name("game-review.html")
            .save_file()
        else {
            return;
        };
        match export_html(&path, session) {
            Ok(()) => self.status = format!("Exported {}", path.display()),
            Err(error) => self.status = format!("Export failed: {error}"),
        }
    }

    fn begin_count(&mut self, unit: AdvanceUnit, count: usize) {
        if self.live.is_none() {
            "A saved review is view-only; load a starting table to simulate."
                .clone_into(&mut self.status);
            return;
        }
        if count == 0 {
            "Run count is zero; no engine step was attempted.".clone_into(&mut self.status);
            return;
        }
        self.run_target = Some(RunTarget::Count {
            unit,
            remaining: count,
        });
        self.command_steps = 0;
        self.begin_autosave_budget();
        self.status = format!("Running {count} {unit:?}(s)…");
    }

    fn tick_run(&mut self, context: &egui::Context) {
        if self.run_target.is_none() {
            return;
        }
        for _ in 0..STEPS_PER_UI_FRAME {
            let Some(target) = self.run_target.clone() else {
                break;
            };
            let Some(live) = self.live.as_mut() else {
                self.run_target = None;
                break;
            };
            if live.is_terminal() {
                self.status = match &live.session.outcome {
                    SessionOutcome::Completed => "Game completed naturally.".to_owned(),
                    SessionOutcome::EngineFailed { error } => format!("Engine failed: {error}"),
                    other => format!("Simulation stopped: {other:?}"),
                };
                self.run_target = None;
                break;
            }
            let frame = live.step_once().clone();
            self.command_steps += 1;
            let done = match target {
                RunTarget::Count {
                    unit,
                    mut remaining,
                } => {
                    let crossed = match unit {
                        AdvanceUnit::Step => 1,
                        AdvanceUnit::Decision => frame.decisions.len(),
                        AdvanceUnit::Action => usize::from(frame.action_completed),
                    };
                    if crossed > 0 {
                        remaining = remaining.saturating_sub(crossed);
                    }
                    if remaining == 0 {
                        true
                    } else {
                        self.run_target = Some(RunTarget::Count { unit, remaining });
                        false
                    }
                }
                RunTarget::Round(round) => frame.round >= round,
                RunTarget::End => frame.finished,
            };
            if done {
                self.status = format!(
                    "Command complete at step {}, round {}, {:?}.",
                    frame.engine_step, frame.round, frame.phase
                );
                self.run_target = None;
            }
            if self.command_steps >= MAX_COMMAND_STEPS {
                live.session.outcome = SessionOutcome::SafetyLimit {
                    steps: self.command_steps,
                };
                self.status = format!(
                    "Command hit the {MAX_COMMAND_STEPS}-step safety limit; session is incomplete."
                );
                self.run_target = None;
            }
            self.viewed = self.latest_index();
            if self.run_target.is_none() {
                self.autosave_now();
                break;
            }
        }
        if self.run_target.is_some() {
            if self.command_steps >= self.next_autosave_step {
                self.autosave_now();
            }
            context.request_repaint();
        }
    }

    fn top_bar(&mut self, root: &mut egui::Ui) {
        egui::Panel::top("inputs").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Choose checkpoint…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON checkpoint", &["json"])
                        .pick_file()
                {
                    self.checkpoint = path.display().to_string();
                    self.persist_settings();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.checkpoint)
                        .desired_width(280.0)
                        .hint_text("checkpoint JSON"),
                );
                if ui.button("Choose map pool…").clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Map pool", &["json", "gz"])
                        .pick_file()
                {
                    self.map_pool = path.display().to_string();
                    self.persist_settings();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.map_pool)
                        .desired_width(280.0)
                        .hint_text("map pool JSON.GZ"),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Seed");
                ui.add(egui::TextEdit::singleline(&mut self.seed).desired_width(110.0));
                ui.label("Faction rotation");
                egui::ComboBox::from_id_salt("rotation")
                    .selected_text(self.rotation.to_string())
                    .show_ui(ui, |ui| {
                        for rotation in 0..6 {
                            ui.selectable_value(&mut self.rotation, rotation, rotation.to_string());
                        }
                    })
                    .response
                    .on_hover_text(
                        "The seed deterministically permutes faction order; rotation then cyclically shifts that same permutation across physical seats.",
                    );
                let profile_response = egui::ComboBox::from_id_salt("profile_table")
                    .selected_text(self.table.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.table, ProfileTable::Learner, "Learner");
                        ui.selectable_value(
                            &mut self.table,
                            ProfileTable::Accepted,
                            "Accepted champion",
                        );
                    });
                if profile_response.response.changed() {
                    self.persist_settings();
                }
                ui.label("Temperature");
                let temperature_response = ui
                    .add(
                        egui::DragValue::new(&mut self.temperature)
                            .range(0.01..=10.0)
                            .speed(0.05)
                            .max_decimals(2),
                    )
                    .on_hover_text(
                        "Lower values prefer the highest-scored move; higher values explore more. The setting applies when a new starting table is loaded.",
                    );
                if temperature_response.changed() {
                    self.persist_settings();
                }
                let diplomacy_response = ui
                    .checkbox(&mut self.diplomacy, "Structured diplomacy")
                    .on_hover_text(
                        "Seats may open contacts, offer and counter deals, send signals and keep or break promises; relationships between seats are tracked. Applies when a new starting table is loaded.",
                    );
                if diplomacy_response.changed() {
                    self.persist_settings();
                }
                if ui.button("Load starting table").clicked() {
                    self.load_start();
                }
                ui.separator();
                if ui.button("Previous game").clicked() {
                    self.open_previous();
                }
                if ui.button("Open review…").clicked() {
                    self.open_review();
                }
                if ui.button("Save As…").clicked() {
                    self.save_as();
                }
                if ui.button("Export HTML…").clicked() {
                    self.export();
                }
            });
            ui.label(&self.status);
        });
    }

    fn controls(&mut self, root: &mut egui::Ui) {
        egui::Panel::bottom("controls").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.toggle_value(&mut self.show_players, "◧ Players");
                ui.toggle_value(&mut self.show_decision, "◨ Decisions");
                ui.separator();
                let can_run = self.live.is_some() && self.run_target.is_none();
                if ui.add_enabled(can_run, egui::Button::new("Step")).clicked() {
                    self.begin_count(AdvanceUnit::Step, 1);
                }
                if ui
                    .add_enabled(can_run, egui::Button::new("Next decision"))
                    .clicked()
                {
                    self.begin_count(AdvanceUnit::Decision, 1);
                }
                if ui
                    .add_enabled(can_run, egui::Button::new("Next action (full turn)"))
                    .on_hover_text(
                        "Runs until the current active player hands off the turn; nested prompts, transactions, and Fleet Logistics stay inside it.",
                    )
                    .clicked()
                {
                    self.begin_count(AdvanceUnit::Action, 1);
                }
                ui.separator();
                ui.label("Run");
                ui.add(egui::TextEdit::singleline(&mut self.run_count).desired_width(70.0));
                egui::ComboBox::from_id_salt("run_unit")
                    .selected_text(format!("{:?}s", self.run_unit))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.run_unit, AdvanceUnit::Step, "Steps");
                        ui.selectable_value(&mut self.run_unit, AdvanceUnit::Decision, "Decisions");
                        ui.selectable_value(
                            &mut self.run_unit,
                            AdvanceUnit::Action,
                            "Actions (full turns)",
                        );
                    });
                if ui
                    .add_enabled(can_run, egui::Button::new("Run N"))
                    .clicked()
                {
                    match self.run_count.trim().parse::<usize>() {
                        Ok(count) if count <= crate::MAX_RUN_COUNT => {
                            self.begin_count(self.run_unit, count);
                        }
                        Ok(_) => {
                            self.status =
                                format!("Run count exceeds the {} limit.", crate::MAX_RUN_COUNT);
                        }
                        Err(error) => self.status = format!("Invalid run count: {error}"),
                    }
                }
                if ui
                    .add_enabled(can_run, egui::Button::new("End round"))
                    .clicked()
                    && let Some(round) = self
                        .live
                        .as_ref()
                        .map(|live| live.session.latest().round + 1)
                {
                    self.run_target = Some(RunTarget::Round(round));
                    self.command_steps = 0;
        self.begin_autosave_budget();
                    self.status = format!("Running to round {round}…");
                }
                if ui
                    .add_enabled(can_run, egui::Button::new("End game"))
                    .clicked()
                {
                    self.run_target = Some(RunTarget::End);
                    self.command_steps = 0;
        self.begin_autosave_budget();
                    "Running to natural completion…".clone_into(&mut self.status);
                }
                if ui
                    .add_enabled(self.run_target.is_some(), egui::Button::new("Stop"))
                    .clicked()
                {
                    self.run_target = None;
                    "Stopped at a clean engine-step boundary; session is incomplete."
                        .clone_into(&mut self.status);
                    self.autosave_now();
                }
            });
            if let Some(session) = self.session() {
                let frame_count = session.frames.len();
                let latest = frame_count.saturating_sub(1);
                let outcome = session.outcome.clone();
                ui.horizontal(|ui| {
                    if ui.button("Previous frame").clicked() {
                        self.viewed = self.viewed.saturating_sub(1);
                    }
                    ui.add(
                        egui::Slider::new(&mut self.viewed, 0..=latest)
                            .text("history frame")
                            .show_value(true),
                    );
                    if ui.button("Next frame").clicked() {
                        self.viewed = (self.viewed + 1).min(latest);
                    }
                    if ui.button("Latest").clicked() {
                        self.viewed = latest;
                    }
                    ui.label(format!("{frame_count} frames · {outcome:?}"));
                });
            }
        });
    }

    fn player_panel(
        root: &mut egui::Ui,
        open: &mut bool,
        session: &ReviewSession,
        frame: &ReviewFrame,
    ) {
        egui::Panel::left("players")
            .resizable(true)
            .default_size(340.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_FILL)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show_collapsible(root, open, |ui| {
                // A light sheet with black text: the dark theme stays on the board and the
                // decision panel, where colour carries the map.
                *ui.visuals_mut() = egui::Visuals::light();
                ui.visuals_mut().override_text_color = Some(PANEL_TEXT);
                ui.heading("Omniscient players");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let content = ContentStore::embedded();
                    section(ui, "Current policy profiles", |ui| {
                        let policy = &session.manifest.policy;
                        ui.group(|ui| {
                            ui.strong(if policy.format.is_empty() {
                                "Legacy review · profile details unavailable".to_owned()
                            } else {
                                format!(
                                    "{}{}",
                                    policy.format,
                                    policy.schema.map_or_else(String::new, |schema| format!(
                                        " · schema {schema}"
                                    ))
                                )
                            });
                            ui.small(format!(
                                "{} · {} · temperature {:.2}",
                                session.manifest.checkpoint_path,
                                if policy.format == "MLP inference bundle" {
                                    "shared actor with faction rows"
                                } else {
                                    session.manifest.profile_table.label()
                                },
                                session.manifest.temperature
                            ));
                            ui.small(format!(
                                "Seats 0–5: {}",
                                session.manifest.factions.join(" → ")
                            ));
                            if let Some(name) = &policy.name {
                                ui.label(name);
                            }
                            if let Some(source) = &policy.source {
                                ui.label(format!("Source: {source}"));
                            }
                            if let Some(commit) = &policy.git_commit {
                                ui.small(format!("Engine/training commit: {commit}"));
                            }
                            if let Some(update) = policy.update {
                                ui.small(format!("Training update: {update}"));
                            }
                            if let Some(dimensions) = &policy.dimensions {
                                ui.small(dimensions);
                            }
                            let mut runtime = Vec::new();
                            if let Some(abi) = policy.projection_abi {
                                runtime.push(format!("projection ABI {abi}"));
                            }
                            if let Some(version) = policy.oov_registry_version {
                                runtime.push(format!("OOV registry v{version}"));
                            }
                            if let Some(mode) = &policy.critic_mode {
                                runtime.push(format!("critic {mode}"));
                            }
                            if let Some(temperature) = policy.trained_temperature {
                                runtime.push(format!("trained temperature {temperature:.2}"));
                            }
                            if !runtime.is_empty() {
                                ui.small(runtime.join(" · "));
                            }
                            ui.separator();
                            ui.small(format!(
                                "Initial speaker: {} · map arrangement: {}",
                                session
                                    .manifest
                                    .initial_speaker
                                    .as_deref()
                                    .unwrap_or("legacy/unrecorded"),
                                session.manifest.map_arrangement_index.map_or_else(
                                    || "legacy/unrecorded".to_owned(),
                                    |value| value.to_string()
                                )
                            ));
                            if let Some(commit) = &session.manifest.engine_commit {
                                ui.small(format!(
                                    "Review engine: {commit}{}",
                                    if session.manifest.engine_dirty {
                                        " (dirty build)"
                                    } else {
                                        ""
                                    }
                                ));
                            }
                            if let Some(digest) = &session.manifest.content_sha256 {
                                ui.small(format!("Content: {digest}"));
                            }
                            if let Some(scope) = &session.manifest.source_scope {
                                ui.small(format!("Scope: {scope}"));
                            }
                        });
                        item_section(
                            ui,
                            "◫",
                            "Decision heads",
                            policy.heads.clone(),
                            Color32::LIGHT_BLUE,
                        );
                        item_section(
                            ui,
                            "♙",
                            "Available faction rows",
                            policy.factions.clone(),
                            Color32::LIGHT_BLUE,
                        );
                        if !policy.profiles.is_empty() {
                            item_section(
                                ui,
                                "ƒ",
                                "Loaded profiles",
                                policy.profiles.clone(),
                                Color32::LIGHT_BLUE,
                            );
                        }
                    });
                    section(ui, "Open public objectives", |ui| {
                        if frame.state.revealed_objectives.is_empty() {
                            ui.label("None revealed yet.");
                        } else {
                            for objective in &frame.state.revealed_objectives {
                                let record =
                                    content.get(ContentType::PublicObjectives, objective.as_str());
                                let name = record
                                    .as_ref()
                                    .and_then(|record| record.text("name"))
                                    .unwrap_or(objective.as_str());
                                let points = record
                                    .as_ref()
                                    .and_then(|record| record.int("points"))
                                    .unwrap_or(0);
                                let scored_by: Vec<String> = frame
                                    .state
                                    .scored_objectives
                                    .iter()
                                    .filter(|(_, scored)| scored.contains(objective))
                                    .map(|(player, _)| player.to_string())
                                    .collect();
                                ui.group(|ui| {
                                    ui.strong(format!("{name} · {points} VP"));
                                    ui.small(format!(
                                        "{} · scored by {}",
                                        objective,
                                        if scored_by.is_empty() {
                                            "nobody".to_owned()
                                        } else {
                                            scored_by.join(", ")
                                        }
                                    ));
                                    if let Some(text) =
                                        record.as_ref().and_then(|record| record.text("text"))
                                    {
                                        ui.label(text);
                                    }
                                });
                            }
                        }
                    });
                    section(ui, "Table state", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            stat_badge(ui, "♛", "Speaker", &frame.state.speaker);
                            stat_badge(
                                ui,
                                "◎",
                                "Custodians",
                                if frame.state.custodians_removed {
                                    "removed"
                                } else {
                                    "present"
                                },
                            );
                            stat_badge(
                                ui,
                                "▣",
                                "Action discard",
                                frame.state.discarded_action_cards.len(),
                            );
                        });
                        let initiative_order: Vec<String> = frame
                            .state
                            .initiative_order()
                            .into_iter()
                            .map(|player_id| {
                                let Some(player) = frame.state.player(&player_id) else {
                                    return player_id.to_string();
                                };
                                let cards = player
                                    .strategy_cards
                                    .iter()
                                    .map(|card| {
                                        let initiative = frame
                                            .state
                                            .card_initiative
                                            .get(card)
                                            .copied()
                                            .unwrap_or(99);
                                        format!("{card} ({initiative})")
                                    })
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!(
                                    "{} {}{}",
                                    player.id,
                                    player.faction,
                                    if cards.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" · {cards}")
                                    }
                                )
                            })
                            .collect();
                        item_section(
                            ui,
                            "➜",
                            "Initiative turn order",
                            initiative_order,
                            Color32::LIGHT_BLUE,
                        );
                        if frame.index > 0 {
                            let previous = &session.frames[frame.index - 1];
                            if previous.state.speaker != frame.state.speaker {
                                ui.strong(format!(
                                    "Speaker changed: {} → {} · events: {}",
                                    previous.state.speaker,
                                    frame.state.speaker,
                                    if frame.new_events.is_empty() {
                                        "unrecorded cause".to_owned()
                                    } else {
                                        frame.new_events.join(", ")
                                    }
                                ));
                            }
                        }
                        item_section(
                            ui,
                            "◆",
                            "Unclaimed strategy cards",
                            frame
                                .state
                                .unclaimed_strategy_cards
                                .iter()
                                .map(|card| {
                                    let goods = frame
                                        .state
                                        .strategy_card_goods
                                        .get(card)
                                        .copied()
                                        .unwrap_or_default();
                                    if goods > 0 {
                                        format!(
                                            "{} · {goods} TG",
                                            content_label(
                                                content,
                                                ContentType::StrategyCards,
                                                card
                                            )
                                        )
                                    } else {
                                        content_label(content, ContentType::StrategyCards, card)
                                    }
                                })
                                .collect(),
                            Color32::LIGHT_BLUE,
                        );
                        item_section(
                            ui,
                            "⚖",
                            "Laws in play",
                            frame
                                .state
                                .laws
                                .iter()
                                .map(|(law, outcome)| {
                                    format!(
                                        "{} · {outcome}",
                                        content_label(content, ContentType::Agendas, law)
                                    )
                                })
                                .collect(),
                            Color32::LIGHT_BLUE,
                        );
                        item_section(
                            ui,
                            "↯",
                            "Discarded action cards",
                            frame
                                .state
                                .discarded_action_cards
                                .iter()
                                .map(|card| content_label(content, ContentType::ActionCards, card))
                                .collect(),
                            Color32::LIGHT_BLUE,
                        );
                    });
                    section(ui, "Diplomacy", |ui| diplomacy_panel(ui, frame));
                    section(ui, "Player sheets", |ui| {
                        for player in &frame.state.players {
                            let color = player_color(&player.id);
                            egui::CollapsingHeader::new(format!(
                                "● {} · {} · {} VP",
                                player.id, player.faction, player.victory_points
                            ))
                            .default_open(true)
                            .show(ui, |ui| {
                                seat_label(
                                    ui,
                                    &player.id,
                                    format!(
                                        "{} · {}",
                                        player.faction,
                                        if player.passed { "PASSED" } else { "ACTIVE" }
                                    ),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    stat_badge(ui, "★", "VP", player.victory_points);
                                    stat_badge(ui, "◆", "TG", player.trade_goods);
                                    stat_badge(ui, "◇", "Com", player.commodities);
                                });
                                ui.horizontal_wrapped(|ui| {
                                    stat_badge(ui, "▲", "Tactic", player.tactic_tokens);
                                    stat_badge(ui, "⬟", "Fleet", player.fleet_tokens);
                                    stat_badge(ui, "●", "Strategy", player.strategic_tokens);
                                });

                                let strategy = player
                                    .strategy_cards
                                    .iter()
                                    .map(|card| {
                                        let label = content_label(
                                            content,
                                            ContentType::StrategyCards,
                                            card,
                                        );
                                        if player.exhausted_strategy_cards.contains(card) {
                                            format!("{label} · used")
                                        } else {
                                            label
                                        }
                                    })
                                    .collect();
                                item_section(ui, "◆", "Strategy cards", strategy, color);

                                let mut controlled_planets = Vec::new();
                                for tile in &session.board {
                                    let state = frame.state.board.get(&SystemId::new(&tile.system));
                                    for planet in planets_for_tile(session, frame, tile) {
                                        if state.and_then(|state| {
                                            state.planet_control.get(planet.id.as_str())
                                        }) != Some(&player.id)
                                        {
                                            continue;
                                        }
                                        let exhausted = frame
                                            .state
                                            .exhausted_planets
                                            .contains(planet.id.as_str());
                                        let attachments = frame
                                            .state
                                            .planet_attachments
                                            .get(planet.id.as_str())
                                            .map_or(0, Vec::len);
                                        controlled_planets.push(format!(
                                            "{} {}/{}{}{}",
                                            planet.label,
                                            planet.resources,
                                            planet.influence,
                                            if exhausted { " · exhausted" } else { "" },
                                            if attachments > 0 {
                                                format!(" · {attachments} attachment(s)")
                                            } else {
                                                String::new()
                                            }
                                        ));
                                    }
                                }
                                item_section(ui, "●", "Planets", controlled_planets, color);

                                let mut unit_counts: BTreeMap<String, usize> = BTreeMap::new();
                                for state in frame.state.board.values() {
                                    for unit in state.units.iter().chain(
                                        state.planet_units.values().flat_map(|units| units.iter()),
                                    ) {
                                        if unit.owner == player.id {
                                            *unit_counts
                                                .entry(unit_base(content, unit))
                                                .or_default() += 1;
                                        }
                                    }
                                }
                                item_section(
                                    ui,
                                    "⬡",
                                    "Units on board",
                                    unit_counts
                                        .into_iter()
                                        .map(|(kind, count)| format!("{kind} ×{count}"))
                                        .collect(),
                                    color,
                                );

                                item_section(
                                    ui,
                                    "⚙",
                                    "Technologies",
                                    player
                                        .technologies
                                        .iter()
                                        .map(|technology| {
                                            let label = content_label(
                                                content,
                                                ContentType::Technologies,
                                                technology,
                                            );
                                            if player.exhausted_technologies.contains(technology) {
                                                format!("{label} · exhausted")
                                            } else {
                                                label
                                            }
                                        })
                                        .collect(),
                                    color,
                                );
                                item_section(
                                    ui,
                                    "✓",
                                    "Scored objectives",
                                    frame
                                        .state
                                        .scored_objectives
                                        .get(&player.id)
                                        .into_iter()
                                        .flatten()
                                        .map(|objective| {
                                            content_label(
                                                content,
                                                ContentType::PublicObjectives,
                                                objective,
                                            )
                                        })
                                        .collect(),
                                    color,
                                );
                                item_section(
                                    ui,
                                    "?",
                                    "Secret objectives",
                                    player
                                        .secret_objectives
                                        .iter()
                                        .map(|objective| {
                                            content_label(
                                                content,
                                                ContentType::SecretObjectives,
                                                objective,
                                            )
                                        })
                                        .collect(),
                                    color,
                                );
                                item_section(
                                    ui,
                                    "▣",
                                    "Action cards",
                                    player
                                        .action_cards
                                        .iter()
                                        .map(|card| {
                                            content_label(content, ContentType::ActionCards, card)
                                        })
                                        .collect(),
                                    color,
                                );
                                item_section(
                                    ui,
                                    "✦",
                                    "Relics and fragments",
                                    player
                                        .relics
                                        .iter()
                                        .map(|relic| {
                                            let label =
                                                content_label(content, ContentType::Relics, relic);
                                            if player.exhausted_relics.contains(relic) {
                                                format!("{label} · exhausted")
                                            } else {
                                                label
                                            }
                                        })
                                        .chain(player.relic_fragments.iter().map(
                                            |(trait_name, count)| {
                                                format!("{trait_name} fragment ×{count}")
                                            },
                                        ))
                                        .collect(),
                                    color,
                                );
                                item_section(
                                    ui,
                                    "◈",
                                    "Exploration cards in play",
                                    player
                                        .exploration_cards
                                        .iter()
                                        .map(|card| {
                                            content_label(content, ContentType::Explores, card)
                                        })
                                        .collect(),
                                    color,
                                );
                                let mut promissory: Vec<String> = frame
                                    .state
                                    .promissory_notes
                                    .iter()
                                    .filter(|(_, holder)| *holder == &player.id)
                                    .map(|(note, _)| {
                                        let label = content_label(
                                            content,
                                            ContentType::PromissoryNotes,
                                            note,
                                        );
                                        if frame.state.promissory_faceup.contains(note) {
                                            format!("{label} · faceup")
                                        } else {
                                            label
                                        }
                                    })
                                    .collect();
                                promissory.extend(
                                    frame
                                        .state
                                        .support_holders
                                        .iter()
                                        .filter(|(_, holder)| *holder == &player.id)
                                        .map(|(owner, _)| {
                                            format!("Support for the Throne:{owner} · faceup")
                                        }),
                                );
                                promissory.sort();
                                promissory.dedup();
                                item_section(ui, "✉", "Promissory notes", promissory, color);
                                item_section(
                                    ui,
                                    "♟",
                                    "Leaders",
                                    player
                                        .leaders
                                        .iter()
                                        .map(|(leader, status)| {
                                            format!(
                                                "{} · {status:?}",
                                                content_label(
                                                    content,
                                                    ContentType::Leaders,
                                                    leader
                                                )
                                            )
                                        })
                                        .collect(),
                                    color,
                                );
                                item_section(ui, "⌁", "Plots", player.plots.clone(), color);
                                if let Some(breakthrough) = &player.breakthrough {
                                    item_section(
                                        ui,
                                        "⚡",
                                        "Breakthrough",
                                        vec![content_label(
                                            content,
                                            ContentType::Breakthroughs,
                                            breakthrough,
                                        )],
                                        color,
                                    );
                                }
                                ui.collapsing("Complete player JSON", |ui| {
                                    let text = serde_json::to_string_pretty(player)
                                        .unwrap_or_else(|error| error.to_string());
                                    ui.monospace(text);
                                });
                            });
                        }
                    });
                });
            });
    }

    fn decision_panel(
        &mut self,
        root: &mut egui::Ui,
        session: &ReviewSession,
        frame: &ReviewFrame,
    ) {
        egui::Panel::right("decision")
            .resizable(true)
            .default_size(410.0)
            .show_collapsible(root, &mut self.show_decision, |ui| {
                ui.heading("Step and policy detail");
                let step = step_view(frame);
                ui.label(format!(
                    "Step {} · decision {} · action {}",
                    step.engine_step, step.decision_count, step.action_count
                ));
                ui.label(format!(
                    "Round {} · {} · active {}",
                    step.round, step.phase, step.active
                ));
                ui.horizontal_wrapped(|ui| {
                    stat_badge(ui, "⬡", "Active system", &step.active_system);
                    stat_badge(ui, "⌛", "Pending", &step.pending);
                    stat_badge(ui, "⚔", "Combat round", step.combat_round);
                });
                if !step.agenda_lines.is_empty() {
                    ui.collapsing("Current timing / agenda state", |ui| {
                        for line in &step.agenda_lines {
                            ui.label(line);
                        }
                    });
                }
                if let Some(error) = &step.error {
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let action = action_summary(session, frame);
                    section_with_id(ui, action.title, "action-summary", |ui| {
                        if let Some(headline) = &action.headline {
                            ui.strong(headline);
                            if let Some(span) = &action.span {
                                ui.small(span);
                            }
                            for detail in &action.details {
                                ui.label(format!("• {detail}"));
                            }
                        } else {
                            ui.label("No action-phase turn has completed yet.");
                        }
                    });
                    section(ui, "Decisions", |ui| {
                        let rows = decision_rows(frame);
                        if rows.is_empty() {
                            ui.label("This engine step resolved no policy choice.");
                        }
                        for (decision_index, row) in rows.iter().enumerate() {
                            ui.separator();
                            ui.strong(format!(
                                "Decision {} · {} · {}",
                                row.sequence, row.player, row.faction
                            ));
                            // Who actually chose: the model, or a fleet plan carrying out an
                            // earlier decision. Without this a planned move reads as a decision
                            // the model made.
                            if let Some(note) = row.path.annotation() {
                                let colour = if row.path == DecisionPath::FleetPlan {
                                    Color32::LIGHT_GREEN
                                } else {
                                    Color32::LIGHT_YELLOW
                                };
                                ui.colored_label(colour, note);
                            }
                            ui.label(&row.prompt);
                            if let Some(context) = &row.context {
                                ui.collapsing("Typed decision context", |ui| {
                                    ui.monospace(context);
                                });
                            } else {
                                ui.small("Typed decision context unavailable (legacy or viewless choice)");
                            }
                            ui.label(&row.summary);
                            if let Some(rank) = &row.rank {
                                ui.small(format!(
                                    "Chosen rank {}/{} · p {} · best p {}{}",
                                    rank.position,
                                    rank.ranked,
                                    rank.probability,
                                    rank.best,
                                    if rank.below_greedy {
                                        " · sampled below the greedy choice"
                                    } else {
                                        ""
                                    }
                                ));
                            }
                            for option in &row.options {
                                egui::CollapsingHeader::new(&option.title)
                                    .default_open(option.selected)
                                    .show(ui, |ui| {
                                        ui.label(format!("id={} kind={}", option.id, option.kind));
                                        for line in &option.detail_lines {
                                            ui.strong(line);
                                        }
                                        if let Some(payload) = &option.payload {
                                            ui.collapsing("Structured payload", |ui| {
                                                ui.monospace(payload);
                                            });
                                        }
                                        if let Some(preview) = &option.preview {
                                            ui.collapsing("Consequence preview", |ui| {
                                                ui.monospace(preview);
                                            });
                                        } else {
                                            ui.small("Consequence preview unavailable");
                                        }
                                        egui::Grid::new(format!(
                                            "features-{}-{decision_index}-{}",
                                            frame.index, option.id
                                        ))
                                        .striped(true)
                                        .show(ui, |ui| {
                                            ui.strong("feature");
                                            ui.strong("value");
                                            ui.strong("weight");
                                            ui.strong("contribution");
                                            ui.end_row();
                                            for feature in &option.features {
                                                ui.label(&feature.name);
                                                ui.label(&feature.value);
                                                ui.label(&feature.weight);
                                                ui.label(&feature.contribution);
                                                ui.end_row();
                                            }
                                        });
                                    });
                            }
                        }
                    });
                    section(ui, "New engine events", |ui| {
                        let events = event_rows(frame);
                        if events.is_empty() && frame.new_events.is_empty() {
                            ui.label("—");
                        } else {
                            for event in &events {
                                egui::CollapsingHeader::new(&event.title).show(ui, |ui| {
                                    ui.monospace(&event.payload);
                                });
                            }
                            if !frame.new_events.is_empty() {
                                ui.collapsing("Legacy event-name trace", |ui| {
                                    for event in &frame.new_events {
                                        ui.monospace(event);
                                    }
                                });
                            }
                        }
                    });
                    if let Some(tile) = &self.selected_tile {
                        ui.separator();
                        let card = selected_system(session, frame, tile);
                        ui.strong(&card.title);
                        for line in &card.lines {
                            ui.label(line);
                        }
                        if card.planets.is_empty() {
                            if card.has_metadata {
                                ui.label("Planets: none");
                            }
                        } else {
                            ui.strong("Planets");
                            for planet in &card.planets {
                                ui.label(planet);
                            }
                        }
                        if let Some(dynamic) = &card.dynamic {
                            ui.strong("Dynamic board state");
                            ui.monospace(dynamic);
                        } else {
                            ui.label("Dynamic board state: empty");
                        }
                    }
                });
            });
    }

    fn board(&mut self, root: &mut egui::Ui, session: &ReviewSession, frame: &ReviewFrame) {
        egui::CentralPanel::default().show(root, |ui| {
            let content = ContentStore::embedded();
            ui.horizontal_wrapped(|ui| {
                ui.strong("Players:");
                for player in &frame.state.players {
                    ui.colored_label(
                        player_color(&player.id),
                        format!("● {} {}", player.id, player.faction),
                    );
                }
            });
            ui.small(
                "Thick outer edge = space control; thin inner edge = planet control (split when mixed). Wormholes: lettered rings; white outer rim = placed token; red slash = suppressed. IN/OUT portals connect the galaxy to the Fracture. Planet: resources/influence · C/H/I trait · B/G/R/Y specialty · ★ legendary · S station · × destroyed. Gray units are neutral; red slash = damaged; yellow ring = galvanized.",
            );
            let available = ui.available_size();
            let (response, painter) = ui.allocate_painter(available, Sense::click());
            let layout =
                BoardLayout::new(response.rect, available, frame.state.fracture_in_play);
            // What each tile *means* is `board_view`'s answer for the frame; where it goes is
            // `BoardLayout`; the strokes are `draw_board`. All three are shared, so the replayer
            // paints this board without restating a single number from it.
            let tiles = board_view(content, session, frame, self.selected_tile.as_deref());
            if let Some(system) = draw_board(&painter, &response, &layout, &tiles) {
                self.selected_tile = Some(system);
            }
        });
    }
}

impl eframe::App for ReviewApp {
    fn logic(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick_run(context);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_bar(root);
        self.controls(root);
        if self.session().is_none() {
            egui::CentralPanel::default().show(root, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.heading("Load a real learned-policy starting table to begin.");
                });
            });
            return;
        }
        // Borrowed, not cloned -- see `with_session`. The viewed frame is borrowed too: it carries
        // a whole `GameState`, so copying it once per repaint was the second-largest cost here.
        self.with_session(|app, session| {
            app.viewed = app.viewed.min(session.frames.len().saturating_sub(1));
            let frame = &session.frames[app.viewed];
            Self::player_panel(root, &mut app.show_players, session, frame);
            app.decision_panel(root, session, frame);
            app.board(root, session, frame);
        });
        if self.run_target.is_some() {
            root.ctx().request_repaint();
        }
    }
}

fn is_review(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with(".ti4review.json") || name.ends_with(".ti4review.json.zst")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_settings_preserve_input_selections_and_previous_game() {
        let settings = ReviewerSettings {
            checkpoint: "out/checkpoints/run-003/checkpoint-532156/slots.json".to_owned(),
            map_pool: "out/pools/save52_noadj_train.json".to_owned(),
            profile_table: ProfileTable::Accepted,
            temperature: 0.25,
            last_review: Some("out/reviews/autosave.ti4review.json".to_owned()),
            diplomacy: true,
        };
        let bytes = serde_json::to_vec(&settings).unwrap();
        let restored: ReviewerSettings = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.checkpoint, settings.checkpoint);
        assert_eq!(restored.map_pool, settings.map_pool);
        assert_eq!(restored.profile_table, settings.profile_table);
        assert!((restored.temperature - settings.temperature).abs() <= f64::EPSILON);
        assert_eq!(restored.last_review, settings.last_review);
    }

    #[test]
    fn previous_game_discovery_accepts_only_review_sessions() {
        assert!(is_review(Path::new("game.ti4review.json")));
        assert!(is_review(Path::new("game.ti4review.json.zst")));
        assert!(!is_review(Path::new("reviewer-settings.json")));
        assert!(!is_review(Path::new("game.html")));
    }
}
