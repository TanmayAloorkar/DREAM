//! DREAM desktop launcher: checks the setup, runs the pipeline and reports one job at a time,
//! and opens the results. Design: "bench protocol" (numbered steps that tick themselves off).

mod job;
mod project;
mod theme;

use eframe::egui::{
    self, Align, Align2, Button, Color32, CornerRadius, Frame, Id, Layout, Margin, RichText, Sense, Stroke, Ui, vec2,
};
use job::Jobs;
use project::{ReportGroup, State, Status};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use theme::*;

const CONTENT_WIDTH: f32 = 880.0;
const SECTION_GAP: f32 = 22.0;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Setup,
    Run,
    Results,
}

enum Action {
    Start { name: String, script: String, env: Vec<(&'static str, String)> },
    Stop,
    ChooseRoot,
    PickSamplesheet,
    Open(PathBuf),
    Refresh,
}

struct App {
    root: Option<PathBuf>,
    root_error: Option<String>,
    tab: Tab,
    status: Option<Status>,
    reports: Vec<ReportGroup>,
    jobs: Jobs,
    memory: Option<(u64, u64)>,
    checked: Instant,
    log_open: bool,
    confirm_quit: bool,
    theme: egui::ThemePreference,
    // pipeline form
    samplesheet: String,
    strandedness: Option<&'static str>,
    outdir: String,
    resume: bool,
    // reports form
    treatment: String,
    control: String,
    moa: String,
    tcga_project: String,
    label_check: bool,
    gates: bool,
    selected: [bool; 4],
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // DREAM_GUI_THEME overrides the saved choice (used for screenshots)
        let theme = theme::parse(&std::env::var("DREAM_GUI_THEME").ok().or_else(|| project::setting("theme")).unwrap_or_default());
        theme::install(&cc.egui_ctx, theme);
        let tab = match std::env::var("DREAM_GUI_TAB").as_deref() {
            Ok("run") => Tab::Run,
            Ok("results") => Tab::Results,
            _ => Tab::Setup,
        };
        let mut app = App {
            root: project::find(),
            root_error: None,
            tab,
            status: None,
            reports: Vec::new(),
            jobs: Jobs::default(),
            memory: project::memory(),
            checked: Instant::now(),
            log_open: false,
            confirm_quit: false,
            theme,
            samplesheet: "samplesheets/GSE197555_A549.csv".into(),
            strandedness: None,
            outdir: "results/GSE197555_A549".into(),
            resume: true,
            treatment: "trametinib".into(),
            control: "control".into(),
            moa: "MEK inhibitor".into(),
            tcga_project: "TCGA-LUAD".into(),
            label_check: true,
            gates: true,
            selected: [true; 4],
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        if let Some(root) = &self.root {
            self.status = Some(Status::read(root, &self.tcga_project));
            self.reports = project::reports(root);
        }
        self.memory = project::memory();
        self.checked = Instant::now();
    }

    fn abs(&self, p: &str) -> PathBuf {
        let p = Path::new(p);
        match &self.root {
            Some(root) if p.is_relative() => root.join(p),
            _ => p.to_path_buf(),
        }
    }

    fn busy_reason(&self) -> Option<String> {
        self.jobs.busy().map(|name| format!("Busy: {name} is running. DREAM runs one job at a time."))
    }

    fn act(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Start { name, script, env } => {
                if let Some(root) = self.root.clone() {
                    let env: Vec<(&str, String)> = env.iter().map(|(k, v)| (*k, v.clone())).collect();
                    self.jobs.start(ctx, &name, &script, &root, &env);
                    self.log_open = true;
                }
            }
            Action::Stop => self.jobs.stop(),
            Action::ChooseRoot => {
                if let Some(dir) = rfd::FileDialog::new().set_title("Choose the DREAM folder").pick_folder() {
                    if project::is_dream(&dir) {
                        project::remember(&dir);
                        self.root = Some(dir);
                        self.root_error = None;
                        self.refresh();
                    } else {
                        self.root_error = Some(format!("{} has no pipeline/main.nf. Pick the folder you cloned.", project::tilde(&dir)));
                    }
                }
            }
            Action::PickSamplesheet => {
                let mut dialog = rfd::FileDialog::new().set_title("Choose a samplesheet").add_filter("CSV", &["csv"]);
                if let Some(root) = &self.root {
                    dialog = dialog.set_directory(root.join("samplesheets"));
                }
                if let Some(file) = dialog.pick_file() {
                    let shown = match &self.root {
                        Some(root) => file.strip_prefix(root).unwrap_or(&file).to_path_buf(),
                        None => file.clone(),
                    };
                    let stem = file.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    self.samplesheet = shown.to_string_lossy().into();
                    self.outdir = format!("results/{stem}");
                    self.label_check = stem == "GSE197555_A549";
                }
            }
            Action::Open(path) => {
                let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            }
            Action::Refresh => self.refresh(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.jobs.poll() {
            self.refresh();
        }
        if self.checked.elapsed() > Duration::from_secs(3) {
            self.refresh();
        }
        // keeps the RAM bar and elapsed time moving
        ctx.request_repaint_after(Duration::from_secs(1));

        if ctx.input(|i| i.viewport().close_requested()) && self.jobs.busy().is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_quit = true;
        }

        let mut actions = Vec::new();
        self.top_bar(ui);
        self.log_drawer(ui, &mut actions);
        egui::CentralPanel::default().frame(Frame::new().fill(pal(ui.visuals()).bg)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                // centred column; Margin is i8, so side space is added as plain space instead
                let width = (ui.available_width() - 48.0).min(CONTENT_WIDTH);
                let side = (ui.available_width() - width) / 2.0;
                ui.add_space(28.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(side);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        match self.tab {
                            Tab::Setup => self.setup_tab(ui, &mut actions),
                            Tab::Run => self.run_tab(ui, &mut actions),
                            Tab::Results => self.results_tab(ui, &mut actions),
                        }
                    });
                });
                ui.add_space(32.0);
            });
        });
        self.quit_dialog(&ctx, &mut actions);
        #[cfg(feature = "screenshot")]
        dev_screenshot(&ctx);
        for action in actions {
            self.act(&ctx, action);
        }
    }
}

#[cfg(feature = "screenshot")]
fn dev_screenshot(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    static FRAMES: AtomicU32 = AtomicU32::new(0);
    static SAVED: AtomicBool = AtomicBool::new(false);
    let Ok(path) = std::env::var("DREAM_GUI_SHOT") else { return };
    if SAVED.load(Ordering::SeqCst) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        return;
    }
    ctx.request_repaint();
    // a few frames in, so fonts, layout and animations have settled
    if FRAMES.fetch_add(1, Ordering::SeqCst) == 20 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |image| {
            let [w, h] = image.size;
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            ppm.extend(image.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b()]));
            let _ = std::fs::write(&path, ppm);
            SAVED.store(true, Ordering::SeqCst);
        })));
    }
}

// ---------- shared pieces ----------

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn primary(ui: &mut Ui, text: &str, blocked: Option<&str>) -> bool {
    let enabled = blocked.is_none();
    let button = Button::new(RichText::new(text).font(semibold(14.0)).color(Color32::WHITE))
        .fill(pal(ui.visuals()).accent_fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(RADIUS))
        .min_size(vec2(0.0, 34.0));
    let response = ui.add_enabled(enabled, button);
    match blocked {
        Some(why) => response.on_disabled_hover_text(why).clicked(),
        None => response.clicked(),
    }
}

fn secondary(ui: &mut Ui, text: &str, blocked: Option<&str>) -> bool {
    let response = ui.add_enabled(blocked.is_none(), Button::new(text));
    match blocked {
        Some(why) => response.on_disabled_hover_text(why).clicked(),
        None => response.clicked(),
    }
}

fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let p = pal(ui.visuals());
    Frame::new()
        .fill(p.pane)
        .stroke(Stroke::new(1.0, p.hairline))
        .corner_radius(CornerRadius::same(RADIUS))
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
}

fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).weak()
}

fn mono_muted(text: impl Into<String>) -> RichText {
    RichText::new(text).font(mono(12.0)).weak()
}

/// A numbered protocol step: number circle in the gutter, a connector line to the next step,
/// and content to the right. The circle fills when the step is done.
fn step(ui: &mut Ui, n: u8, title: &str, subtitle: &str, done: bool, last: bool, add: impl FnOnce(&mut Ui)) {
    let p = pal(ui.visuals());
    let t = ui.ctx().animate_bool_with_time(Id::unique(("step-done", n)), done, 0.3);
    let left = ui.cursor().left();
    let top = ui.cursor().top();
    let rect = ui
        .horizontal_top(|ui| {
            ui.add_space(44.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(title).font(semibold(16.0)).strong());
                if !subtitle.is_empty() {
                    ui.label(muted(subtitle).size(13.0));
                }
                ui.add_space(4.0);
                add(ui);
            });
        })
        .response
        .rect;
    let painter = ui.painter();
    let c = egui::pos2(left + 14.0, top + 11.0);
    if !last {
        let (a, b) = (egui::pos2(c.x, c.y + 17.0), egui::pos2(c.x, rect.bottom() + SECTION_GAP - 6.0));
        painter.line_segment([a, b], Stroke::new(2.0, p.hairline));
        painter.line_segment([a, egui::pos2(c.x, a.y + (b.y - a.y) * t)], Stroke::new(2.0, p.accent));
    }
    painter.circle(c, 13.0, mix(p.pane, p.accent_fill, t), Stroke::new(1.0, mix(p.border, p.accent_fill, t)));
    painter.text(c, Align2::CENTER_CENTER, n.to_string(), semibold(13.0), mix(p.muted, Color32::WHITE, t));
    ui.add_space(SECTION_GAP);
}

fn glyph(ui: &mut Ui, state: State) {
    let pal = pal(ui.visuals());
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
    let p = ui.painter();
    let c = rect.center();
    match state {
        State::Ok => {
            let s = Stroke::new(1.8, pal.ink);
            p.line_segment([c + vec2(-5.0, 0.5), c + vec2(-1.5, 4.0)], s);
            p.line_segment([c + vec2(-1.5, 4.0), c + vec2(5.0, -4.0)], s);
        }
        State::Missing => {
            p.line_segment([c + vec2(-4.5, 0.0), c + vec2(4.5, 0.0)], Stroke::new(1.8, pal.danger));
        }
        State::Partial => {
            p.circle_stroke(c, 5.0, Stroke::new(1.6, pal.muted));
            p.line_segment([c + vec2(0.0, -5.0), c + vec2(0.0, 5.0)], Stroke::new(1.6, pal.muted));
        }
    }
}

fn check_rows(ui: &mut Ui, checks: &[&project::Check]) {
    card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().interact_size.y = 24.0;
        for (i, c) in checks.iter().enumerate() {
            if i > 0 {
                ui.add(egui::Separator::default().spacing(10.0));
            }
            ui.horizontal(|ui| {
                glyph(ui, c.state);
                ui.label(c.label);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(mono_muted(&c.detail));
                });
            });
        }
    });
}

fn ago(t: SystemTime) -> String {
    let s = SystemTime::now().duration_since(t).unwrap_or_default().as_secs();
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}

// ---------- top bar and log ----------

impl App {
    fn top_bar(&mut self, ui: &mut Ui) {
        let p = pal(ui.visuals());
        let response = egui::Panel::top("top")
            .exact_size(56.0)
            .frame(Frame::new().fill(p.pane).inner_margin(Margin::symmetric(24, 0)))
            .show(ui, |ui| {
                let bottom = ui.max_rect().bottom();
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("DREAM").font(semibold(17.0)).strong());
                    ui.label(muted("RNA-seq to drug repurposing").size(12.5));
                    ui.add_space(28.0);
                    let mut active = None;
                    for (i, (tab, name)) in [(Tab::Setup, "Setup"), (Tab::Run, "Run"), (Tab::Results, "Results")].into_iter().enumerate() {
                        let text = format!("{}  {name}", i + 1);
                        let size = ui.painter().layout_no_wrap(text.clone(), semibold(14.0), p.ink).size();
                        let (rect, r) = ui.allocate_exact_size(size, Sense::click());
                        let r = r.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let selected = self.tab == tab;
                        let color = if selected || r.hovered() { p.ink } else { p.muted };
                        ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, text, semibold(14.0), color);
                        if r.clicked() {
                            self.tab = tab;
                        }
                        if selected {
                            active = Some(rect);
                        }
                        ui.add_space(14.0);
                    }
                    if let Some(rect) = active {
                        let ctx = ui.ctx();
                        let x = ctx.animate_value_with_time(Id::unique("tab-x"), rect.left(), 0.15);
                        let w = ctx.animate_value_with_time(Id::unique("tab-w"), rect.width(), 0.15);
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(egui::pos2(x, bottom - 2.0), vec2(w, 2.0)),
                            0.0,
                            p.accent,
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = format!("Theme: {}", theme::preference_name(self.theme));
                        if ui.link(RichText::new(label).size(13.0)).on_hover_text("Auto follows the system setting").clicked() {
                            self.theme = theme::next(self.theme);
                            ui.ctx().set_theme(self.theme);
                            project::save_setting("theme", &format!("{:?}", self.theme).to_lowercase());
                        }
                        ui.add_space(20.0);
                        self.memory_meter(ui);
                        ui.add_space(20.0);
                        self.job_badge(ui);
                    });
                });
            })
            .response;
        let r = response.rect;
        ui.painter().hline(r.x_range(), r.bottom(), Stroke::new(1.0, p.hairline));
    }

    fn memory_meter(&self, ui: &mut Ui) {
        let Some((free, total)) = self.memory else { return };
        let used = total.saturating_sub(free);
        let share = used as f32 / total as f32;
        let pal = pal(ui.visuals());
        let (rect, _) = ui.allocate_exact_size(vec2(84.0, 6.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(3), pal.pressed);
        let fill = egui::Rect::from_min_size(rect.min, vec2(rect.width() * share, rect.height()));
        p.rect_filled(fill, CornerRadius::same(3), if share > 0.85 { pal.danger } else { pal.accent });
        let gib = |b: u64| b as f64 / (1u64 << 30) as f64;
        ui.label(RichText::new(format!("RAM {:.1} / {:.1} GB", gib(used), gib(total))).font(mono(12.0)).weak())
            .on_hover_text("Memory in use on this machine. Heavy steps need about 11 GB.");
    }

    fn job_badge(&self, ui: &mut Ui) {
        let p = pal(ui.visuals());
        if let Some(run) = &self.jobs.running {
            ui.label(RichText::new(format!("{} · {}", run.name, job::elapsed(run.started.elapsed()))).font(semibold(13.0)).color(p.accent));
            ui.add(egui::Spinner::new().size(14.0).color(p.accent));
        } else if let Some(last) = &self.jobs.last {
            let (text, color) = match (last.code, last.stopped) {
                (_, true) => (format!("{} stopped", last.name), p.muted),
                (Some(0), _) => (format!("{} finished · {}", last.name, job::elapsed(last.elapsed)), p.muted),
                _ => (format!("{} failed", last.name), p.danger),
            };
            ui.label(RichText::new(text).size(13.0).color(color));
        } else {
            ui.label(muted("Idle").size(13.0));
        }
    }

    fn log_drawer(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let p = pal(ui.visuals());
        let frame = Frame::new().fill(p.pane).inner_margin(Margin::symmetric(24, 8));
        let panel = if self.log_open {
            egui::Panel::bottom("log-open").resizable(true).default_size(220.0).min_size(120.0)
        } else {
            egui::Panel::bottom("log-closed").exact_size(40.0)
        };
        let response = panel
            .frame(frame)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Log").font(semibold(13.0)).strong());
                    if let Some(line) = self.jobs.log.lock().unwrap().back() {
                        ui.add(egui::Label::new(mono_muted(line.clone())).truncate());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.link(if self.log_open { "Hide" } else { "Show" }).clicked() {
                            self.log_open = !self.log_open;
                        }
                        if self.log_open && ui.link("Clear").clicked() {
                            self.jobs.clear();
                        }
                        if self.jobs.busy().is_some() && secondary(ui, "Stop job", None) {
                            actions.push(Action::Stop);
                        }
                    });
                });
                if self.log_open {
                    ui.add_space(4.0);
                    let log = self.jobs.log.lock().unwrap();
                    let row = ui.text_style_height(&egui::TextStyle::Monospace);
                    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(ui, row, log.len(), |ui, range| {
                        for line in log.range(range) {
                            ui.add(egui::Label::new(RichText::new(line).font(mono(12.0))).extend());
                        }
                    });
                }
            })
            .response;
        let r = response.rect;
        ui.painter().hline(r.x_range(), r.top(), Stroke::new(1.0, p.hairline));
    }

    fn quit_dialog(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if !self.confirm_quit {
            return;
        }
        let name = self.jobs.busy().unwrap_or("A job").to_string();
        egui::Modal::new(Id::unique("quit")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(format!("{name} is still running")).font(semibold(16.0)).strong());
            ui.label(muted("Quitting stops it. Nextflow can resume finished tasks next time with -resume."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if secondary(ui, "Keep running", None) {
                    self.confirm_quit = false;
                }
                if primary(ui, "Stop job and quit", None) {
                    actions.push(Action::Stop);
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

// ---------- tabs ----------

impl App {
    fn setup_tab(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(root) = self.root.clone() else {
            ui.add_space(40.0);
            ui.label(RichText::new("Choose your DREAM folder").font(semibold(22.0)).strong());
            ui.label(muted("The folder you cloned from GitHub. It holds pipeline/, analysis/ and samplesheets/."));
            ui.add_space(12.0);
            if primary(ui, "Choose folder…", None) {
                actions.push(Action::ChooseRoot);
            }
            if let Some(err) = &self.root_error {
                ui.label(RichText::new(err).color(ui.visuals().error_fg_color));
            }
            return;
        };
        let busy = self.busy_reason();
        let busy = busy.as_deref();

        ui.horizontal(|ui| {
            ui.label(muted("Project"));
            ui.label(RichText::new(project::tilde(&root)).font(mono(13.0)).strong());
            if ui.link("Change").clicked() {
                actions.push(Action::ChooseRoot);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.link("Check again").clicked() {
                    actions.push(Action::Refresh);
                }
            });
        });
        if let Some(err) = &self.root_error {
            ui.label(RichText::new(err).color(ui.visuals().error_fg_color));
        }
        ui.add_space(18.0);

        let Some(st) = &self.status else { return };
        let envs_done = st.envs.iter().all(|c| c.state == State::Ok);
        step(ui, 1, "Software", "Nextflow builds the pipeline's own tool environments on its first run.", envs_done, false, |ui| {
            check_rows(ui, &st.envs.iter().collect::<Vec<_>>());
            if envs_done {
                return;
            }
            ui.horizontal_wrapped(|ui| {
                if st.conda.is_none() {
                    ui.hyperlink_to("Install Miniforge", "https://github.com/conda-forge/miniforge");
                }
                for (label, file, name) in [("dream-qc environment", "envs/qc.yml", "dream-qc"), ("dream-r environment", "envs/r.yml", "dream-r")] {
                    if st.conda.is_some() && !st.ok(label) && secondary(ui, &format!("Create {name}"), busy) {
                        actions.push(Action::Start {
                            name: format!("Create {name}"),
                            script: format!("{} env create -y -f {file}", st.conda_bin("mamba")),
                            env: vec![],
                        });
                    }
                }
                if !st.ok("Nextflow") {
                    ui.hyperlink_to("Install Nextflow", "https://www.nextflow.io/docs/latest/install.html");
                }
                if !st.ok("Java") {
                    ui.hyperlink_to("Install Java 17 or later", "https://adoptium.net/");
                }
            });
        });

        let inputs_done = st.inputs.iter().all(|c| c.state == State::Ok);
        step(ui, 2, "Reference and inputs", "Genome index, annotation, H460 check samples and TCGA data, about 12 GB.", inputs_done, false, |ui| {
            check_rows(ui, &st.inputs.iter().collect::<Vec<_>>());
            if !inputs_done {
                let blocked = busy.map(str::to_string).or_else(|| (!st.ok("dream-qc environment")).then(|| "Create dream-qc first".into()));
                if secondary(ui, "Download inputs", blocked.as_deref()) {
                    actions.push(Action::Start {
                        name: "Download inputs".into(),
                        script: format!("{} run -n dream-qc --no-capture-output bash scripts/get_inputs.sh", st.conda_bin("conda")),
                        env: vec![("TCGA_PROJECT", self.tcga_project.clone())],
                    });
                }
            }
        });

        step(ui, 3, "LINCS 2020", "Reference drug signatures for report 04. One 6.3 GB file, checked against its sha256.", st.lincs.state == State::Ok, true, |ui| {
            check_rows(ui, &[&st.lincs]);
            if st.lincs.state != State::Ok && secondary(ui, "Download LINCS 2020", busy) {
                actions.push(Action::Start { name: "Download LINCS".into(), script: "bash scripts/download_lincs.sh".into(), env: vec![] });
            }
        });
    }

    fn run_tab(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(root) = self.root.clone() else {
            ui.label(muted("Choose the DREAM folder in Setup first."));
            return;
        };
        let Some(st) = &self.status else { return };
        let busy = self.busy_reason();
        let counts = self.abs(&self.outdir).join("featurecounts/counts.txt");

        let sheet_ok = self.abs(&self.samplesheet).is_file();
        let pipeline_blocked = busy.clone().or_else(|| {
            [
                (!st.ok("Nextflow") || !st.ok("Java"), "Install Nextflow and Java (Setup step 1)"),
                (!st.ok("HISAT2 index, GRCh38") || !st.ok("Ensembl 116 annotation"), "Download the reference (Setup step 2)"),
                (!sheet_ok, "Samplesheet not found"),
                (self.strandedness.is_none(), "Choose the strandedness"),
            ]
            .into_iter()
            .find(|(bad, _)| *bad)
            .map(|(_, why)| why.to_string())
        });

        step(ui, 4, "Pipeline", "Reads to gene counts: fastp, HISAT2, featureCounts, with FastQC, Qualimap and MultiQC.", counts.is_file(), false, |ui| {
            card(ui, |ui| {
                egui::Grid::new("pipeline").num_columns(2).spacing(vec2(16.0, 10.0)).show(ui, |ui| {
                    ui.label(muted("Samplesheet"));
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.samplesheet).font(egui::TextStyle::Monospace).desired_width(420.0));
                        if secondary(ui, "Browse…", None) {
                            actions.push(Action::PickSamplesheet);
                        }
                    });
                    ui.end_row();

                    ui.label(muted("Strandedness"));
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("strand")
                            .selected_text(self.strandedness.unwrap_or("Choose…"))
                            .width(160.0)
                            .show_ui(ui, |ui| {
                                for s in ["unstranded", "forward", "reverse"] {
                                    ui.selectable_value(&mut self.strandedness, Some(s), s);
                                }
                            });
                        ui.label(muted("GSE197555 is unstranded. A wrong choice loses about half the reads.").size(12.5));
                    });
                    ui.end_row();

                    ui.label(muted("Output folder"));
                    ui.add(egui::TextEdit::singleline(&mut self.outdir).font(egui::TextStyle::Monospace).desired_width(420.0));
                    ui.end_row();

                    ui.label(muted("Resume"));
                    ui.checkbox(&mut self.resume, "Reuse tasks that already finished (-resume)");
                    ui.end_row();
                });
            });
            ui.add_space(6.0);
            let command = project::pipeline_command(&project::PipelineArgs {
                samplesheet: &self.samplesheet,
                strandedness: self.strandedness.unwrap_or("…"),
                outdir: &self.outdir,
                resume: self.resume,
            });
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(mono_muted(&command)).wrap()).on_hover_text("The exact command this button runs");
            });
            ui.horizontal(|ui| {
                if let Some(why) = &pipeline_blocked {
                    ui.label(muted(why).size(13.0));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary(ui, "Run pipeline", pipeline_blocked.as_deref()) {
                        actions.push(Action::Start { name: "Pipeline".into(), script: command.clone(), env: vec![] });
                    }
                });
            });
        });

        let deseq2_ready = self.abs(&self.outdir).join("deseq2/deseq2_results.tsv").is_file() || self.selected[0];
        let tcga_ready = ["star_tpm", "survival", "clinical", "somaticmutation_wxs"]
            .iter()
            .all(|k| project::tcga_file(&root, &self.tcga_project, k).is_file());
        let h460_ready = root.join("results/GSE197555_H460_subset/featurecounts/counts.txt").is_file();
        let any = self.selected.iter().any(|s| *s);
        let reports_blocked = busy.or_else(|| {
            [
                (!any, "Select at least one report"),
                (!st.ok("dream-r environment"), "Create the dream-r environment (Setup step 1)"),
                (self.selected[0] && !counts.is_file(), "Report 01 needs gene counts: run the pipeline first"),
                (self.selected[0] && self.label_check && !h460_ready, "The GEO label check needs the H460 pipeline run"),
                (!deseq2_ready, "Reports 02-04 need report 01's results"),
                (self.selected[2] && !tcga_ready, "Report 03 needs the TCGA data (Setup step 2)"),
                (self.selected[3] && !st.ok("LINCS 2020 signatures"), "Report 04 needs LINCS 2020 (Setup step 3)"),
            ]
            .into_iter()
            .find(|(bad, _)| *bad)
            .map(|(_, why)| why.to_string())
        });
        let rendered = project::REPORTS.iter().all(|r| root.join(format!("analysis/{r}.html")).is_file());

        step(ui, 5, "Reports", "Quarto reports 01-04, rendered in order. Each stops if its built-in check fails.", rendered, true, |ui| {
            card(ui, |ui| {
                egui::Grid::new("reports").num_columns(2).spacing(vec2(16.0, 10.0)).show(ui, |ui| {
                    for [(l1, v1), (l2, v2)] in [
                        [("Treatment", &mut self.treatment), ("Control", &mut self.control)],
                        [("Expected mimic", &mut self.moa), ("TCGA project", &mut self.tcga_project)],
                    ] {
                        ui.label(muted(l1));
                        ui.horizontal(|ui| {
                            ui.add(egui::TextEdit::singleline(v1).desired_width(200.0));
                            ui.add_space(16.0);
                            ui.add_sized([110.0, 20.0], egui::Label::new(muted(l2)));
                            ui.add(egui::TextEdit::singleline(v2).desired_width(200.0));
                        });
                        ui.end_row();
                    }
                    ui.label(muted("Checks"));
                    ui.vertical(|ui| {
                        ui.checkbox(&mut self.gates, "Stop when a built-in check fails (DUSP6, MAPK, MEK mimics)");
                        ui.checkbox(&mut self.label_check, "GEO label check against H460 (GSE197555 only)");
                    });
                    ui.end_row();
                    ui.label(muted("Render"));
                    ui.horizontal_wrapped(|ui| {
                        for (i, name) in ["01 Expression", "02 Mechanism", "03 Patients", "04 Repurposing"].iter().enumerate() {
                            ui.checkbox(&mut self.selected[i], *name);
                        }
                    });
                    ui.end_row();
                });
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if let Some(why) = &reports_blocked {
                    ui.label(muted(why).size(13.0));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary(ui, "Render reports", reports_blocked.as_deref()) {
                        let script = project::report_command(
                            &project::ReportArgs {
                                root: &root,
                                samplesheet: &self.samplesheet,
                                outdir: &self.outdir,
                                treatment: &self.treatment,
                                control: &self.control,
                                moa: &self.moa,
                                tcga_project: &self.tcga_project,
                                label_check: self.label_check,
                                gates: self.gates,
                                selected: self.selected,
                            },
                            &st.conda_bin("conda"),
                        );
                        actions.push(Action::Start { name: "Reports".into(), script, env: vec![] });
                    }
                });
            });
        });
    }

    fn results_tab(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(root) = self.root.clone() else {
            ui.label(muted("Choose the DREAM folder in Setup first."));
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Results").font(semibold(20.0)).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary(ui, "Open results folder", None) {
                    actions.push(Action::Open(root.join("results")));
                }
                if ui.link("Refresh").clicked() {
                    actions.push(Action::Refresh);
                }
            });
        });
        ui.add_space(14.0);
        for group in &self.reports {
            ui.horizontal(|ui| {
                ui.label(RichText::new(group.name).font(semibold(15.0)).strong());
                ui.label(mono_muted(group.reports.len().to_string()));
            });
            if group.reports.is_empty() {
                ui.label(muted(if group.name == "Latest renders" { "Nothing yet. Render reports in the Run tab." } else { "Nothing here yet." }).size(13.0));
            } else {
                card(ui, |ui| {
                    for (i, r) in group.reports.iter().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(r.title.as_str());
                                ui.label(mono_muted(r.path.strip_prefix(&root).unwrap_or(&r.path).display().to_string()));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if secondary(ui, "Open", None) {
                                    actions.push(Action::Open(r.path.clone()));
                                }
                                if let Some(m) = r.modified {
                                    ui.label(muted(ago(m)).size(12.5));
                                }
                            });
                        });
                    }
                });
            }
            ui.add_space(18.0);
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DREAM")
            .with_app_id("dream-gui")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native("DREAM", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
