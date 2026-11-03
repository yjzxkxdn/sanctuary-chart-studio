//! 应用主体：布局、走带、快捷键、文件操作、状态栏。

use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, Color32, CornerRadius, Key, Margin, RichText, Sense, Stroke, Ui};
use egui::{pos2, vec2};

use crate::audio::AudioEngine;
use crate::autochart::{self, ChartGenOptions, Snap, WaveGenOptions};
use crate::model::*;
use crate::theme::*;
use crate::ui_inspector;
use crate::ui_map::{self, MapState};
use crate::ui_timeline::{self, Focus, TimelineState};
use crate::undo::UndoStack;
use crate::widgets::*;

#[derive(Clone, Default)]
pub struct Clipboard {
    pub notes: Vec<Note>,
    pub waves: Vec<Wave>,
}

#[derive(Clone)]
pub struct Status {
    pub text: String,
    pub error: bool,
    pub until: Option<Instant>,
}

impl Status {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: false,
            until: Some(Instant::now() + std::time::Duration::from_secs(6)),
        }
    }
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            error: true,
            until: Some(Instant::now() + std::time::Duration::from_secs(12)),
        }
    }
}

pub struct Studio {
    pub design: Design,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub undo: UndoStack,
    pub audio: AudioEngine,

    pub focus: Focus,
    pub status: Option<Status>,

    // 走带
    pub clock: f64,
    pub playing: bool,
    pub metronome: bool,
    pub volume: f32,
    pub phase: f64,
    pub snap: Snap,

    pub tl: TimelineState,
    pub map: MapState,

    pub issues: Vec<Issue>,
    pub issues_stamp: u64,
    pub issues_computed: u64,
    pub show_issues: bool,
    pub show_shortcuts: bool,
    pub show_map_editor: bool,
    pub show_left: bool,
    pub show_right: bool,

    pub gen_chart: ChartGenOptions,
    pub gen_waves: WaveGenOptions,

    pub project_dir: PathBuf,
    pub last_frame: Instant,
    pub clipboard: Clipboard,
    pub sel_note: Option<u64>,
    pub sel_wave: Option<u64>,
    pub edit_stamp: u64,
    pub last_clock: f64,
    /// 播放倍率（0.25×—2×）
    pub rate: f64,
    /// 打点测速
    pub tap: crate::autochart::TapTempo,
    /// 录制中正在按住的音符：(id, 起始时间, 按下时刻)
    pub rec_note: Option<(u64, f64, Instant)>,
}
impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::install_fonts(&cc.egui_ctx);
        crate::theme::install_style(&cc.egui_ctx);

        let design = Design::new_design();
        let mut s = Self {
            design,
            path: None,
            dirty: false,
            undo: UndoStack::default(),
            audio: AudioEngine::new(),
            focus: Focus::Chart,
            status: Some(Status::info(
                "欢迎使用圣土关卡编排台 · 谱面与出怪在同一条时间轴上",
            )),
            clock: 0.0,
            playing: false,
            metronome: false,
            volume: 0.85,
            phase: 0.0,
            snap: Snap::Eighth,
            tl: TimelineState::default(),
            map: MapState::default(),
            issues: Vec::new(),
            issues_stamp: 1,
            issues_computed: 0,
            show_issues: false,
            show_shortcuts: false,
            show_map_editor: false,
            show_left: true,
            show_right: true,
            gen_chart: ChartGenOptions::default(),
            gen_waves: WaveGenOptions::default(),
            project_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            last_frame: Instant::now(),
            clipboard: Clipboard::default(),
            sel_note: None,
            sel_wave: None,
            edit_stamp: 0,
            last_clock: 0.0,
            rate: 1.0,
            tap: crate::autochart::TapTempo::default(),
            rec_note: None,
        };
        s.gen_chart.phase = 0.0;
        s.recompute_issues();
        // 首次打开时给一个可试听的骨架，避免面对空白时间轴
        if s.design.chart.is_empty() && s.design.waves.is_empty() {
            s.seed_demo();
        }

        // 无界面自检：设置 SCS_EXPORT=<路径> 时导出示例关卡后立刻退出，
        // 便于在无法截图的环境里验证「生成 → 校验 → 导出」完整链路。
        if let Ok(target) = std::env::var("SCS_EXPORT") {
            match s.design.export_json() {
                Ok(json) => match std::fs::write(&target, json) {
                    Ok(_) => println!("EXPORT_OK {target}"),
                    Err(e) => println!("EXPORT_FAIL {e}"),
                },
                Err(e) => println!("EXPORT_INVALID {e}"),
            }
            std::process::exit(0);
        }
        s
    }

    /// 生成一份可导出的示例内容（只有初始空关卡才会触发）
    fn seed_demo(&mut self) {
        let mut ok = self.gen_chart.clone();
        ok.phase = self.phase;
        ok.end = self.design.duration;
        self.design.chart = autochart::generate_notes(&self.design, None, &ok);
        let mut ow = self.gen_waves.clone();
        ow.phase = self.phase;
        self.design.waves = autochart::suggest_waves(&self.design, &ow);
        self.design.sort();
        self.undo.clear();
        self.dirty = false;
        self.recompute_issues();
        self.status = Some(Status::info(
            "已生成示例谱面与出怪 · 载入音频后可自动对拍重排",
        ));
    }

    // 编辑基础设施

    pub fn begin_edit(&mut self, label: &str) {
        self.undo.push(&self.design, label);
        self.dirty = true;
        self.edit_stamp += 1;
    }

    pub fn begin_edit_force(&mut self, label: &str) {
        self.undo.push_force(&self.design, label);
        self.dirty = true;
        self.edit_stamp += 1;
    }

    pub fn touch(&mut self) {
        self.dirty = true;
        self.edit_stamp += 1;
    }

    /// 任何结构性改动之后调用：重新校验并保持时间有序
    pub fn after_edit(&mut self) {
        self.design.sort();
        self.issues_stamp += 1;
        self.recompute_issues();
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.status = Some(Status::info(text));
    }

    pub fn toast_err(&mut self, text: impl Into<String>) {
        self.status = Some(Status::error(text));
    }

    pub fn recompute_issues(&mut self) {
        self.issues = self.design.all_issues();
        self.issues_computed = self.issues_stamp;
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }
    pub fn warn_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    pub fn seek(&mut self, t: f64) {
        let t = t.clamp(0.0, self.design.duration);
        self.clock = t;
        self.last_clock = t;
        self.audio.seek(t);
    }

    pub fn toggle_play(&mut self) {
        if self.audio.has_audio() {
            if self.playing {
                self.audio.pause();
                self.playing = false;
            } else {
                if self.clock >= self.design.duration - 0.01 {
                    self.seek(0.0);
                }
                self.audio.seek(self.clock);
                self.audio.play();
                self.playing = true;
            }
        } else {
            self.playing = !self.playing;
        }
        if self.playing {
            self.tl.follow = true;
        }
        self.last_clock = self.clock;
    }

    pub fn fit_timeline(&mut self) {
        self.tl.start = 0.0;
        self.tl.pps = TimelineState::pps_min(self.design.duration, 760.0 + ui_timeline::gutter());
        self.tl.fitted = true;
        self.tl.fit_duration = self.design.duration;
    }

    // 谱面操作

    pub fn add_note(&mut self, t: f64) {
        let t = t.clamp(0.0, self.design.duration);
        // 避免与已有音符冲突
        if let Some(prev) = self
            .design
            .chart
            .iter()
            .filter(|n| n.time <= t)
            .max_by(|a, b| {
                a.time
                    .partial_cmp(&b.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            if t - prev.tail() < MIN_NOTE_GAP {
                self.toast_err(format!("与 {:.3}s 的音符太近（至少 0.18s）", prev.time));
                return;
            }
        }
        self.begin_edit("添加音符");
        let mut n = Note::tap((t * 1000.0).round() / 1000.0);
        n.accent = self.design.beat_phase(n.time, self.phase) < 0.03;
        self.design.chart.push(n);
        self.after_edit();
        self.audio.blip();
    }

    /// 播放中实时打点
    pub fn record_tap(&mut self, t: f64) {
        self.add_note(t);
    }

    pub fn toggle_hold_for(&mut self, id: u64) {
        let beat = self.design.beat();
        let dur = self.design.duration;
        let Some(idx) = self.design.chart.iter().position(|n| n.id == id) else {
            return;
        };
        self.begin_edit("切换点按 / 长条");
        let n = &mut self.design.chart[idx];
        if n.kind == NoteKind::Hold {
            n.kind = NoteKind::Tap;
            n.end = None;
        } else {
            n.kind = NoteKind::Hold;
            n.end = Some((n.time + beat * 2.0).min(dur).max(n.time + MIN_HOLD));
        }
        self.after_edit();
    }

    pub fn delete_selected(&mut self) {
        if !self.tl.note_sel.is_empty() {
            let sel = self.tl.note_sel.clone();
            self.begin_edit("删除音符");
            self.design.chart.retain(|n| !sel.contains(&n.id));
            self.tl.note_sel.clear();
            self.touch();
            self.after_edit();
        }
        if !self.tl.wave_sel.is_empty() {
            let sel = self.tl.wave_sel.clone();
            self.begin_edit("删除出怪");
            self.design.waves.retain(|w| !sel.contains(&w.id));
            self.tl.wave_sel.clear();
            self.touch();
            self.after_edit();
        }
    }

    pub fn copy_selection(&mut self) {
        let notes: Vec<Note> = self
            .design
            .chart
            .iter()
            .filter(|n| self.tl.note_sel.contains(&n.id))
            .cloned()
            .collect();
        let waves: Vec<Wave> = self
            .design
            .waves
            .iter()
            .filter(|w| self.tl.wave_sel.contains(&w.id))
            .cloned()
            .collect();
        if notes.is_empty() && waves.is_empty() {
            self.toast_err("没有选中的对象");
            return;
        }
        let msg = format!("已复制 {} 音符 · {} 出怪", notes.len(), waves.len());
        self.clipboard = Clipboard { notes, waves };
        self.toast(msg);
    }

    pub fn paste(&mut self) {
        if self.clipboard.notes.is_empty() && self.clipboard.waves.is_empty() {
            self.toast_err("剪贴板为空");
            return;
        }
        let t0 = self
            .clipboard
            .notes
            .iter()
            .chain(std::iter::empty())
            .map(|n| n.time)
            .fold(f64::INFINITY, f64::min);
        let w0 = self
            .clipboard
            .waves
            .iter()
            .map(|w| w.time)
            .fold(f64::INFINITY, f64::min);
        let base = t0.min(w0);
        let base = if base.is_finite() { base } else { 0.0 };
        let target = self.clock;
        let delta = target - base;

        self.begin_edit("粘贴");
        self.tl.note_sel.clear();
        self.tl.wave_sel.clear();
        for n in self.clipboard.notes.clone() {
            let mut n = n;
            n.id = next_id();
            n.time = (n.time + delta).clamp(0.0, self.design.duration);
            if let Some(e) = n.end.as_mut() {
                *e = (*e + delta).clamp(n.time + MIN_HOLD, self.design.duration);
            }
            self.tl.note_sel.insert(n.id);
            self.design.chart.push(n);
        }
        for w in self.clipboard.waves.clone() {
            let mut w = w;
            w.id = next_id();
            w.time = (w.time + delta).clamp(0.0, self.design.duration);
            self.tl.wave_sel.insert(w.id);
            self.design.waves.push(w);
        }
        self.touch();
        self.after_edit();
        autochart::dedupe(&mut self.design.chart, MIN_NOTE_GAP);
        self.toast("已粘贴到播放头位置");
    }

    pub fn quantize_selection(&mut self) {
        let step = self.snap.step(self.design.bpm);
        let Some(step) = step else {
            self.toast_err("请先选择吸附精度");
            return;
        };
        let sel = self.tl.note_sel.clone();
        if sel.is_empty() {
            self.toast_err("没有选中的音符");
            return;
        }
        self.begin_edit("量化");
        let phase = self.phase;
        let dur = self.design.duration;
        for n in self.design.chart.iter_mut().filter(|n| sel.contains(&n.id)) {
            let k = ((n.time - phase) / step).round();
            let span = n.span();
            n.time = (phase + k * step).clamp(0.0, dur);
            if n.kind == NoteKind::Hold {
                let ek = ((n.time + span - phase) / step).round();
                n.end = Some((phase + ek * step).clamp(n.time + MIN_HOLD, dur));
            }
        }
        self.touch();
        self.after_edit();
    }

    pub fn nudge_selection(&mut self, dir: f64) {
        let step = self.snap.step(self.design.bpm).unwrap_or(0.1);
        let sel = self.tl.note_sel.clone();
        let wsel = self.tl.wave_sel.clone();
        if sel.is_empty() && wsel.is_empty() {
            return;
        }
        self.begin_edit("微调");
        let dur = self.design.duration;
        for n in self.design.chart.iter_mut().filter(|n| sel.contains(&n.id)) {
            n.time = (n.time + dir * step).clamp(0.0, dur);
            if let Some(e) = n.end.as_mut() {
                *e = (*e + dir * step).clamp(n.time + MIN_HOLD, dur);
            }
        }
        for w in self
            .design
            .waves
            .iter_mut()
            .filter(|w| wsel.contains(&w.id))
        {
            w.time = (w.time + dir * step).clamp(0.0, dur);
        }
        self.touch();
        self.after_edit();
    }

    // 批量编辑

    /// 把选中音符统一成点按 / 长条
    pub fn set_selected_note_kind(&mut self, kind: NoteKind) {
        let sel = self.tl.note_sel.clone();
        if sel.is_empty() {
            return;
        }
        self.begin_edit("批量改音符类型");
        let beat = self.design.beat();
        let dur = self.design.duration;
        for n in self.design.chart.iter_mut().filter(|n| sel.contains(&n.id)) {
            n.kind = kind;
            match kind {
                NoteKind::Tap => n.end = None,
                NoteKind::Hold => {
                    n.end = Some((n.time + beat * 2.0).min(dur).max(n.time + MIN_HOLD));
                }
            }
        }
        self.touch();
        self.after_edit();
    }

    pub fn set_selected_accent(&mut self, accent: bool) {
        let sel = self.tl.note_sel.clone();
        if sel.is_empty() {
            return;
        }
        self.begin_edit("批量重音");
        for n in self.design.chart.iter_mut().filter(|n| sel.contains(&n.id)) {
            n.accent = accent;
        }
        self.touch();
        self.after_edit();
    }

    pub fn align_selected_waves_to_beat(&mut self) {
        let sel = self.tl.wave_sel.clone();
        if sel.is_empty() {
            return;
        }
        self.begin_edit("出怪对齐拍点");
        let phase = self.phase;
        let bpm = self.design.bpm;
        for w in self.design.waves.iter_mut().filter(|w| sel.contains(&w.id)) {
            w.time = autochart::snap_time(w.time, Snap::Beat, bpm, phase);
        }
        self.touch();
        self.after_edit();
    }

    pub fn set_selected_wave_kind(&mut self, kind: WaveKind) {
        let sel = self.tl.wave_sel.clone();
        if sel.is_empty() {
            return;
        }
        self.begin_edit("批量改出怪类型");
        for w in self.design.waves.iter_mut().filter(|w| sel.contains(&w.id)) {
            w.kind = kind;
        }
        self.touch();
        self.after_edit();
    }

    pub fn set_selected_wave_route(&mut self, route: usize) {
        let sel = self.tl.wave_sel.clone();
        if sel.is_empty() || route >= self.design.paths.len() {
            return;
        }
        self.begin_edit("批量改出怪路线");
        for w in self.design.waves.iter_mut().filter(|w| sel.contains(&w.id)) {
            w.route = route;
        }
        self.touch();
        self.after_edit();
    }

    pub fn scale_selected_waves(&mut self, hp: f64, sp: f64) {
        let sel = self.tl.wave_sel.clone();
        if sel.is_empty() {
            return;
        }
        self.begin_edit("批量改出怪数值");
        for w in self.design.waves.iter_mut().filter(|w| sel.contains(&w.id)) {
            w.health = (w.health * hp).clamp(0.1, 20.0);
            w.speed = (w.speed * sp).clamp(0.1, 4.0);
        }
        self.touch();
        self.after_edit();
    }

    /// 自动铺一条折返路线
    pub fn auto_route(&mut self) {
        self.begin_edit("自动铺路");
        if self.design.paths.is_empty() {
            self.design.paths.push(Vec::new());
        }
        let active = self.map.active_route.min(self.design.paths.len() - 1);
        self.design.paths[active] = serpentine_path(active as i32);
        self.touch();
        self.after_edit();
        self.toast("已生成折返路线，可继续拖动修改");
    }

    pub fn add_wave(&mut self, t: f64, route: usize) {
        self.begin_edit("添加出怪");
        let mut w = Wave::new((t * 1000.0).round() / 1000.0, route, WaveKind::Normal);
        // 若这附近还没有特殊尘魄，第一次加怪就给特殊（保证能通关）
        if !self
            .design
            .waves
            .iter()
            .any(|x| x.kind == WaveKind::Special)
        {
            w.kind = WaveKind::Special;
        }
        w.id = next_id();
        self.design.waves.push(w);
        self.after_edit();
    }

    // 文件操作

    pub fn new_level(&mut self) {
        self.design = Design::new_design();
        self.path = None;
        self.undo.clear();
        self.clock = 0.0;
        self.playing = false;
        self.audio.pause();
        self.tl.note_sel.clear();
        self.tl.wave_sel.clear();
        self.recompute_issues();
        self.toast("已新建关卡草稿");
    }

    pub fn open_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("关卡 JSON", &["json"])
            .set_directory(&self.project_dir)
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match Design::import_json(&text) {
                Ok(d) => {
                    self.apply_design(d, Some(path));
                    self.toast("关卡已导入，可继续编排或直接导出");
                }
                Err(e) => self.toast_err(format!("导入失败：{e}")),
            },
            Err(e) => self.toast_err(format!("读取失败：{e}")),
        }
    }

    pub fn apply_design(&mut self, d: Design, path: Option<PathBuf>) {
        self.design = d;
        if let Some(p) = path {
            self.project_dir = p
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| self.project_dir.clone());
            self.path = Some(p);
        }
        self.undo.clear();
        self.clock = 0.0;
        self.playing = false;
        self.audio.pause();
        self.phase = 0.0;
        self.tl.note_sel.clear();
        self.tl.wave_sel.clear();
        self.tl.start = 0.0;
        self.recompute_issues();
        self.dirty = false;
        self.try_load_audio_auto();
    }

    fn try_load_audio_auto(&mut self) {
        let Some(p) = self.path.clone() else { return };
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        for ext in ["mp3", "ogg", "wav", "flac", "m4a"] {
            let candidate = p.with_file_name(format!("{stem}.{ext}"));
            if candidate.exists() {
                if self.audio.load_path(&candidate).is_ok() {
                    self.toast(format!(
                        "已自动载入音频 {}",
                        candidate.file_name().unwrap_or_default().to_string_lossy()
                    ));
                }
                return;
            }
        }
    }

    pub fn save(&mut self, save_as: bool) -> bool {
        let target = if save_as || self.path.is_none() {
            let default_name = format!("{}.json", sanitize(&self.design.name));
            rfd::FileDialog::new()
                .add_filter("关卡 JSON", &["json"])
                .set_directory(&self.project_dir)
                .set_file_name(default_name)
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(path) = target else { return false };
        match self.design.export_json() {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(_) => {
                    self.dirty = false;
                    self.path = Some(path.clone());
                    self.project_dir = path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| self.project_dir.clone());
                    self.toast(format!(
                        "已保存 {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                    true
                }
                Err(e) => {
                    self.toast_err(format!("写入失败：{e}"));
                    false
                }
            },
            Err(e) => {
                self.show_issues = true;
                self.toast_err(format!("无法保存：{e}"));
                false
            }
        }
    }

    pub fn export(&mut self) -> bool {
        let Ok(json) = self.design.export_json() else {
            self.show_issues = true;
            self.toast_err("存在错误，无法导出（见问题列表）");
            return false;
        };
        let default_name = format!("{}.json", sanitize(&self.design.name));
        let Some(path) = rfd::FileDialog::new()
            .add_filter("关卡 JSON", &["json"])
            .set_directory(&self.project_dir)
            .set_file_name(default_name)
            .save_file()
        else {
            return false;
        };
        match std::fs::write(&path, json) {
            Ok(_) => {
                self.project_dir = path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| self.project_dir.clone());
                self.toast(format!(
                    "已导出 {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                ));
                true
            }
            Err(e) => {
                self.toast_err(format!("写入失败：{e}"));
                false
            }
        }
    }

    pub fn load_audio_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("音频", &["mp3", "ogg", "wav", "flac", "m4a", "aac"])
            .set_directory(&self.project_dir)
            .pick_file()
        else {
            return;
        };
        self.load_audio_path(path);
    }

    pub fn load_audio_path(&mut self, path: std::path::PathBuf) {
        match self.audio.load_path(&path) {
            Ok(()) => {
                let name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.design.audio_name = name.clone();
                let dur = self.audio.duration();
                if (self.design.duration - dur).abs() > 1.0 && dur >= MIN_DURATION {
                    self.design.duration = dur.floor().min(MAX_DURATION);
                }
                // 载入后立刻自动分析 BPM 与节拍相位（之后都可手动修改）
                let detected = self.auto_detect_bpm(true);
                let phase = autochart::infer_phase(&self.audio.waveform, self.design.bpm);
                self.phase = phase;
                self.gen_chart.phase = phase;
                self.audio.set_phase(phase);
                self.design.sort();
                self.touch();
                self.after_edit();
                match detected {
                    Some((bpm, _)) => self.toast(format!(
                        "已载入 {name}（{dur:.1}s）· 自动分析 BPM {bpm:.2}、相位 {phase:.3}s，可手动修改"
                    )),
                    None => self.toast(format!("已载入 {name}（{dur:.1}s）· 未能自动识别 BPM，请手动输入或打点测速")),
                }
            }
            Err(e) => self.toast_err(e),
        }
    }

    /// 自动检测 BPM；`apply` 为真时直接写入关卡。
    pub fn auto_detect_bpm(&mut self, apply: bool) -> Option<(f64, f32)> {
        if !self.audio.has_audio() {
            return None;
        }
        let r = autochart::estimate_bpm(&self.audio.waveform)?;
        if apply {
            let bpm = (r.0 * 100.0).round() / 100.0;
            self.begin_edit("自动检测 BPM");
            self.design.bpm = bpm.clamp(MIN_BPM, MAX_BPM);
            // BPM 变了，相位也要重新对齐
            let phase = autochart::infer_phase(&self.audio.waveform, self.design.bpm);
            self.phase = phase;
            self.gen_chart.phase = phase;
            self.audio.set_tempo(self.design.bpm);
            self.audio.set_phase(phase);
            self.touch();
            self.after_edit();
            Some((self.design.bpm, r.1))
        } else {
            Some(r)
        }
    }

    /// 手动修改 BPM（重算相位，节拍器同步）
    pub fn set_bpm(&mut self, bpm: f64) {
        let bpm = bpm.clamp(MIN_BPM, MAX_BPM);
        self.design.bpm = bpm;
        self.audio.set_tempo(bpm);
        if self.audio.has_audio() {
            let phase = autochart::infer_phase(&self.audio.waveform, bpm);
            self.phase = phase;
            self.gen_chart.phase = phase;
            self.audio.set_phase(phase);
        }
        self.touch();
        self.after_edit();
    }

    /// 打点测速：记一次敲击并返回当前估计
    pub fn tap_bpm(&mut self) -> Option<f64> {
        let bpm = self.tap.tap();
        if let Some(b) = bpm {
            let b = (b * 10.0).round() / 10.0;
            self.design.bpm = b.clamp(MIN_BPM, MAX_BPM);
            self.audio.set_tempo(self.design.bpm);
        }
        bpm
    }

    pub fn apply_tapped_bpm(&mut self) {
        match self.tap.bpm() {
            Some(b) => {
                let b = (b * 10.0).round() / 10.0;
                self.begin_edit("打点测速");
                self.set_bpm(b);
                self.tap.reset();
                self.toast(format!("已应用打点测速结果 {b:.1} BPM"));
            }
            None => self.toast_err("至少敲击 3 次才能估算 BPM"),
        }
    }

    // 录制模式

    /// 录制：空格按下 → 在播放头处落一个音符
    pub fn rec_press(&mut self) {
        if self.rec_note.is_some() {
            return;
        }
        let step = self.snap.step(self.design.bpm);
        let t = self.design.snap_to(self.clock, step, self.phase);
        if let Some(prev) = self
            .design
            .chart
            .iter()
            .filter(|n| n.time <= t)
            .max_by(|a, b| {
                a.time
                    .partial_cmp(&b.time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            if t - prev.tail() < MIN_NOTE_GAP {
                return;
            }
        }
        self.begin_edit("录制音符");
        let mut n = Note::tap((t * 1000.0).round() / 1000.0);
        n.accent = self.design.beat_phase(n.time, self.phase) < 0.03;
        let id = n.id;
        self.design.chart.push(n);
        self.after_edit();
        self.audio.blip();
        self.rec_note = Some((id, t, Instant::now()));
    }

    /// 录制：空格松开 → 若按住超过阈值则把音符拉成长条
    pub fn rec_release(&mut self) {
        let Some((id, start, at)) = self.rec_note.take() else {
            return;
        };
        let held = at.elapsed().as_secs_f64();
        if held < 0.18 {
            return;
        }
        // 长条尾部：优先用播放头位置，未播放时给一个默认长度
        let beat = self.design.beat();
        let step = self.snap.step(self.design.bpm);
        let dur = self.design.duration;
        let mut end = self.design.snap_to(self.clock, step, self.phase);
        if end < start + beat * 0.5 {
            end = start + (beat * 2.0).min(dur).max(start + MIN_HOLD);
        }
        let end = end.clamp(start + MIN_HOLD, dur);
        self.begin_edit("拉长长条");
        if let Some(n) = self.design.chart.iter_mut().find(|n| n.id == id) {
            n.kind = NoteKind::Hold;
            n.end = Some((end * 1000.0).round() / 1000.0);
        }
        self.touch();
        self.after_edit();
    }

    // 自动生成

    pub fn run_auto_chart(&mut self) {
        self.begin_edit_force("自动生成谱面");
        let mut opt = self.gen_chart.clone();
        opt.phase = self.phase;
        opt.end = self.design.duration;
        let wave = if self.audio.has_audio() {
            Some(&self.audio.waveform)
        } else {
            None
        };
        let notes = autochart::generate_notes(&self.design, wave, &opt);
        let n = notes.len();
        self.design.chart = notes;
        self.tl.note_sel.clear();
        self.touch();
        self.after_edit();
        self.toast(format!("已生成 {n} 个音符，可直接试听或手动微调"));
    }

    pub fn run_auto_waves(&mut self) {
        self.begin_edit_force("自动编排出怪");
        let mut opt = self.gen_waves.clone();
        opt.phase = self.phase;
        self.design.waves = autochart::suggest_waves(&self.design, &opt);
        let n = self.design.waves.len();
        self.design.sort();
        self.tl.wave_sel.clear();
        self.touch();
        self.after_edit();
        self.toast(format!("已生成 {n} 个出怪事件（含特殊尘魄与首领）"));
    }

    pub fn infer_phase(&mut self) {
        if !self.audio.has_audio() {
            self.toast_err("请先载入音频再推断节拍相位");
            return;
        }
        let p = autochart::infer_phase(&self.audio.waveform, self.design.bpm);
        self.phase = p;
        self.gen_chart.phase = p;
        self.audio.set_phase(p);
        self.toast(format!("已推断节拍相位 {:.3}s（可在左侧微调）", p));
    }

    // 每帧

    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;

        let prev = self.last_clock;
        if self.playing {
            if self.audio.has_audio() {
                // 音频回调是按整块缓冲推进位置（每块 10—20ms），若每帧直接取它，
                // 得到的就是台阶式数值：一帧走 1 格、下一帧走 2 格。走带模式只动
                // 播放头看不出问题，定轴模式整条波形都在动，抖动会被放大成「蹦迪」。
                // 因此以「按帧累加」为主，再缓慢收敛到音频真实位置。
                let target = self.audio.position();
                let mut c = self.clock + dt * self.rate;
                let err = target - c;
                if err.abs() > 0.35 {
                    // 刚 seek / 严重漂移：直接对齐
                    c = target;
                } else {
                    c += err * 0.10;
                }
                // 绝不明显倒退（倒退看起来同样是抖动）
                if c < self.clock - 0.02 {
                    c = self.clock - 0.02;
                }
                self.clock = c.max(0.0);
            } else {
                self.clock += dt * self.rate;
            }
            if self.clock >= self.design.duration {
                self.clock = self.design.duration;
                self.playing = false;
                self.audio.pause();
            }
            ctx.request_repaint();
        }

        // 打击音
        if self.playing && self.tl.hit_sound {
            let crossed = self
                .design
                .chart
                .iter()
                .any(|n| n.time > prev - 1e-6 && n.time <= self.clock && n.time > 0.01);
            if crossed {
                self.audio.blip();
            }
        }
        self.last_clock = self.clock;

        self.audio.set_tempo(self.design.bpm);
        self.audio.set_phase(self.phase);
        self.audio.set_gain(self.volume);
        if (self.audio.rate() - self.rate).abs() > 1e-6 {
            self.audio.set_rate(self.rate);
        }

        if self.issues_computed != self.issues_stamp {
            self.recompute_issues();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let (
            space,
            undo,
            redo,
            save,
            export,
            del,
            open,
            esc,
            h_key,
            ctrl,
            shift,
            left,
            right,
            home,
            end,
            f_key,
        ) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Z) && i.modifiers.command,
                (i.key_pressed(Key::Y) && i.modifiers.command)
                    || (i.key_pressed(Key::Z) && i.modifiers.command && i.modifiers.shift),
                i.key_pressed(Key::S) && i.modifiers.command,
                i.key_pressed(Key::E) && i.modifiers.command,
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::O) && i.modifiers.command,
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::H) && !i.modifiers.command,
                i.modifiers.command,
                i.modifiers.shift,
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
                i.key_pressed(Key::F),
            )
        });
        // 录制模式：空格落音符（长按变长条），Enter 才是播放/暂停
        let (space_up, enter) =
            ctx.input(|i| (i.key_released(Key::Space), i.key_pressed(Key::Enter)));
        if self.tl.record {
            if space {
                self.rec_press();
            }
            if space_up {
                self.rec_release();
            }
            if enter {
                self.toggle_play();
            }
        } else {
            if space {
                self.toggle_play();
            }
            if enter {
                self.toggle_play();
            }
        }
        if undo {
            self.do_undo();
        }
        if redo {
            self.do_redo();
        }
        if save {
            self.save(false);
        }
        if export {
            self.export();
        }
        if open {
            self.open_file();
        }
        if esc {
            self.tl.note_sel.clear();
            self.tl.wave_sel.clear();
        }
        // 1/2/3/4：快速切换右侧检视面板
        if !ctrl {
            if ctx.input(|i| i.key_pressed(Key::Num1)) {
                self.focus = Focus::Chart;
            }
            if ctx.input(|i| i.key_pressed(Key::Num2)) {
                self.focus = Focus::Waves;
            }
            if ctx.input(|i| i.key_pressed(Key::Num3)) {
                self.focus = Focus::Balance;
            }
            if ctx.input(|i| i.key_pressed(Key::Num4)) {
                self.focus = Focus::Map;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::T)) && !ctrl {
            self.tap_bpm();
        }
        if f_key && !ctrl {
            // F：快速打点（录制模式下的键盘替代）
            let t = self
                .design
                .snap_to(self.clock, self.snap.step(self.design.bpm), self.phase);
            self.record_tap(t);
        }
        if ctrl && (ctx.input(|i| i.key_pressed(Key::C))) {
            self.copy_selection();
        }
        if ctrl && (ctx.input(|i| i.key_pressed(Key::V))) {
            self.paste();
        }
        if ctrl && (ctx.input(|i| i.key_pressed(Key::A))) {
            self.tl.note_sel = self.design.chart.iter().map(|n| n.id).collect();
            self.tl.wave_sel = self.design.waves.iter().map(|w| w.id).collect();
        }
        if del {
            self.delete_selected();
        }
        if h_key {
            let id = self
                .sel_note
                .or_else(|| self.tl.note_sel.iter().next().cloned());
            if let Some(id) = id {
                self.toggle_hold_for(id);
            }
        }
        if ctrl && (ctx.input(|i| i.key_pressed(Key::Q))) {
            self.quantize_selection();
        }
        if left || right {
            let step = self.snap.step(self.design.bpm).unwrap_or(0.1);
            let d = if left { -step } else { step };
            if shift {
                self.nudge_selection(if left { -1.0 } else { 1.0 });
            } else {
                self.seek(self.clock + d);
            }
        }
        if home {
            self.seek(0.0);
        }
        if end {
            self.seek(self.design.duration);
        }
    }

    pub fn do_undo(&mut self) {
        if let Some((label, prev)) = self.undo.undo(&self.design) {
            self.design = prev;
            self.touch();
            self.tl.note_sel.clear();
            self.tl.wave_sel.clear();
            self.recompute_issues();
            self.toast(format!("已撤销：{label}"));
        }
    }

    pub fn do_redo(&mut self) {
        if let Some((label, next)) = self.undo.redo(&self.design) {
            self.design = next;
            self.touch();
            self.tl.note_sel.clear();
            self.tl.wave_sel.clear();
            self.recompute_issues();
            self.toast(format!("已重做：{label}"));
        }
    }
}

pub fn sanitize(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || (c as u32) > 0x2E80 {
            out.push(c);
        }
    }
    let t = out.trim().to_string();
    if t.is_empty() {
        "level".to_string()
    } else {
        t
    }
}

impl eframe::App for Studio {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 每帧兜底：确保样式始终是深紫金深色（防止任何主题覆盖）
        crate::theme::ensure_dark(ctx);
        self.tick(ctx);
        self.handle_shortcuts(ctx);

        self.top_bar(ctx);
        self.status_bar(ctx);
        self.transport(ctx);
        self.left_panel(ctx);
        self.right_panel(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(BG_APP)
                    .inner_margin(Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| {
                // 概览条固定在底部，时间轴吃剩余高度
                egui::TopBottomPanel::bottom("overview_strip")
                    .exact_height(32.0)
                    .frame(egui::Frame::NONE.fill(BG_APP))
                    .show_inside(ui, |ui| {
                        ui_timeline::overview(self, ui);
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(BG_APP))
                    .show_inside(ui, |ui| {
                        ui_timeline::show(self, ui);
                    });
            });

        if self.show_issues {
            self.issues_window(ctx);
        }
        if self.show_shortcuts {
            self.shortcuts_window(ctx);
        }
        if self.show_map_editor {
            self.map_editor_window(ctx);
        }
    }
}

impl Studio {
    // 顶栏

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("topbar")
            .exact_height(52.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(14, 8))
                    .stroke(Stroke::new(1.0_f32, LINE_SOFT)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("✦").size(18.0).color(GOLD));
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new("圣土 · 关卡编排台")
                                .size(14.5)
                                .color(TEXT)
                                .strong(),
                        );
                        ui.label(
                            RichText::new("SANCTUARY · CHART STUDIO")
                                .size(8.5)
                                .color(TEXT_FAINT),
                        );
                    });
                    ui.add_space(12.0);

                    let mut name = self.design.name.clone();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut name)
                            .desired_width(200.0)
                            .hint_text("关卡名称")
                            .font(font_id(13.0)),
                    );
                    if resp.changed() {
                        self.begin_edit("重命名关卡");
                        self.design.name = name;
                        self.touch();
                    }
                    if resp.lost_focus() && self.design.name.trim().is_empty() {
                        self.design.name = "未命名的星途".to_string();
                    }
                    if self.dirty {
                        ui.label(RichText::new("●").size(10.0).color(WARN))
                            .on_hover_text("有未保存的修改");
                    }

                    right_aligned(ui, |ui| {
                        if ghost_button(ui, "问题").clicked() {
                            self.show_issues = !self.show_issues;
                        }
                        let (e, w) = (self.error_count(), self.warn_count());
                        if e + w > 0 {
                            let color = if e > 0 { ERR } else { WARN };
                            ui.label(
                                RichText::new(format!("{e} 错 · {w} 警"))
                                    .size(11.0)
                                    .color(color),
                            );
                        } else {
                            ui.label(RichText::new("校验通过").size(11.0).color(OK));
                        }
                        ui.add_space(4.0);
                        if ghost_button(ui, "新建").clicked() {
                            self.new_level();
                        }
                        if ghost_button(ui, "打开").clicked() {
                            self.open_file();
                        }
                        if ghost_button(ui, "保存").clicked() {
                            self.save(false);
                        }
                        if ghost_button(ui, "导出")
                            .on_hover_text("导出与游戏完全兼容的关卡 JSON")
                            .clicked()
                        {
                            self.export();
                        }
                        if primary_button(ui, "试玩 ↗")
                            .on_hover_text("导出后在游戏「关卡编纂室」里导入")
                            .clicked()
                        {
                            self.export();
                        }
                    });
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let mut expired = false;
        if let Some(s) = &self.status {
            if let Some(until) = s.until {
                if Instant::now() > until {
                    expired = true;
                }
            }
        }
        if expired {
            self.status = None;
        }

        egui::TopBottomPanel::bottom("statusbar")
            .exact_height(22.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(12, 1))
                    .stroke(Stroke::new(1.0_f32, LINE_SOFT)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(s) = &self.status {
                        let color = if s.error { ERR } else { TEAL };
                        ui.label(
                            RichText::new(if s.error { "✕" } else { "◆" })
                                .size(10.0)
                                .color(color),
                        );
                        ui.label(RichText::new(&s.text).size(11.0).color(color));
                    } else {
                        ui.label(RichText::new("就绪").size(10.5).color(TEXT_FAINT));
                    }
                    right_aligned(ui, |ui| {
                        let st = self.design.stats();
                        ui.label(
                            RichText::new(format!(
                                "{} 音符 · {:.2} NPS 均值 / {:.0} 峰值 · {} 出怪 · {} 路线",
                                st.notes, st.avg_nps, st.peak_nps, st.waves, st.roads
                            ))
                            .size(10.5)
                            .color(TEXT_FAINT),
                        );
                        ui.label(RichText::new("|").size(10.0).color(LINE));
                        ui.label(
                            RichText::new("空格 播放 · F 打点 · Ctrl+Z 撤销")
                                .size(10.5)
                                .color(TEXT_FAINT),
                        );
                    });
                });
            });
    }

    // 走带

    fn transport(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("transport")
            .exact_height(58.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_CARD)
                    .inner_margin(Margin::symmetric(14, 7))
                    .stroke(Stroke::new(1.0_f32, LINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let play_label = if self.playing { "❚❚" } else { "▶" };
                    let btn = egui::Button::new(
                        RichText::new(play_label)
                            .size(14.0)
                            .color(Color32::from_rgb(0x17, 0x10, 0x20)),
                    )
                    .fill(GOLD)
                    .min_size(vec2(34.0, 28.0))
                    .corner_radius(CornerRadius::same(14));
                    if ui.add(btn).on_hover_text("空格").clicked() {
                        self.toggle_play();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("⏮").size(12.0))
                                .min_size(vec2(26.0, 28.0)),
                        )
                        .on_hover_text("回到开头")
                        .clicked()
                    {
                        self.seek(0.0);
                    }
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new(fmt_time(self.clock))
                                .size(16.0)
                                .color(TEXT)
                                .monospace(),
                        );
                        let bar = self.design.bar_index(self.clock, self.phase);
                        ui.label(
                            RichText::new(format!(
                                "小节 {} · 拍 {} · {:.0} BPM",
                                bar.floor() as i64 + 1,
                                (bar.fract() * 4.0).floor() as i64 + 1,
                                self.design.bpm
                            ))
                            .size(9.5)
                            .color(TEXT_FAINT),
                        );
                    });
                    ui.add_space(8.0);
                    let width = (ui.available_width() - 430.0).max(140.0);
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(width, 22.0), Sense::click_and_drag());
                    self.draw_progress(ui, rect, &resp);
                    ui.add_space(8.0);
                    let mut met = self.metronome;
                    if crate::widgets::toggle(ui, "节拍器", met, OK).clicked() {
                        met = !met;
                        self.metronome = met;
                        self.audio.set_metronome(met);
                    }
                    // 变速：慢速对谱 / 原速检查
                    ui.label(RichText::new("速度").size(10.5).color(TEXT_DIM));
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (v, l) in [
                        (0.5_f64, "0.5×"),
                        (0.75, "0.75×"),
                        (1.0, "1×"),
                        (1.25, "1.25×"),
                    ] {
                        if pill(ui, l, (self.rate - v).abs() < 1e-6, TEAL).clicked() {
                            self.rate = v;
                            self.audio.set_rate(v);
                        }
                    }
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add(
                        egui::Slider::new(&mut self.volume, 0.0..=1.0)
                            .show_value(false)
                            .text(RichText::new("音量").size(10.5)),
                    );
                    if ghost_button(ui, "?").on_hover_text("快捷键").clicked() {
                        self.show_shortcuts = true;
                    }
                });
            });
    }

    fn draw_progress(&mut self, ui: &mut Ui, rect: egui::Rect, resp: &egui::Response) {
        let p = ui.painter();
        let inner = rect.shrink2(vec2(0.0, 5.0));
        p.rect_filled(inner, CornerRadius::same(3), BG_INPUT);
        let dur = self.design.duration.max(0.1);
        let w = inner.width();

        // 谱面热度
        let bins = (w as usize).clamp(24, 900);
        let hist = self.design.density_histogram(bins);
        let peak = hist.iter().cloned().fold(1.0f32, f32::max);
        let dx = w / bins as f32;
        for (i, v) in hist.iter().enumerate() {
            if *v <= 0.0 {
                continue;
            }
            let h = (v / peak) * inner.height();
            let x = inner.left() + i as f32 * dx;
            p.rect_filled(
                egui::Rect::from_min_max(
                    pos2(x, inner.bottom() - h),
                    pos2(x + dx.max(1.0), inner.bottom()),
                ),
                0.0,
                with_alpha(PURPLE, 0.35),
            );
        }
        // 出怪刻度
        for wv in &self.design.waves {
            let x = inner.left() + (wv.time / dur) as f32 * w;
            let c = wave_color(wv.kind);
            p.line_segment(
                [pos2(x, inner.top()), pos2(x, inner.bottom())],
                Stroke::new(1.0_f32, with_alpha(c, 0.55)),
            );
        }
        // 进度
        let frac = (self.clock / dur).clamp(0.0, 1.0) as f32;
        let x = inner.left() + w * frac;
        p.line_segment(
            [pos2(x, inner.top() - 3.0), pos2(x, inner.bottom() + 3.0)],
            Stroke::new(2.0_f32, GOLD),
        );
        p.circle_filled(pos2(x, inner.center().y), 4.0, GOLD);

        if resp.hovered() || resp.dragged() {
            if let Some(ptr) = resp.interact_pointer_pos() {
                let t = ((ptr.x - inner.left()) / w).clamp(0.0, 1.0) as f64 * dur;
                if resp.dragged() {
                    self.seek(t);
                }
                let bar = self.design.bar_index(t, self.phase);
                let text = format!("{:.3}s · 小节 {}", t, bar.floor() as i64 + 1);
                egui::show_tooltip_at_pointer(
                    ui.ctx(),
                    ui.layer_id(),
                    egui::Id::new("transport_tip"),
                    |ui| {
                        ui.label(RichText::new(text).size(11.0).color(TEXT));
                    },
                );
            }
        }
    }

    // 左右面板

    fn left_panel(&mut self, ctx: &egui::Context) {
        if !self.show_left {
            return;
        }
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(288.0)
            .width_range(250.0..=380.0)
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0_f32, LINE_SOFT)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui_inspector::left(self, ui);
                    });
            });
    }

    fn right_panel(&mut self, ctx: &egui::Context) {
        if !self.show_right {
            return;
        }
        // 右栏宽度自适应；只有窗口真的窄到放不下时才隐藏
        let w = ctx.screen_rect().width();
        if w < 900.0 {
            return;
        }
        let left_w = 288.0;
        let avail = (w - left_w).max(200.0);
        let width = (avail * 0.28).clamp(264.0, 360.0);
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .exact_width(width)
            .frame(
                egui::Frame::NONE
                    .fill(BG_PANEL)
                    .inner_margin(Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0_f32, LINE_SOFT)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui_inspector::right(self, ui);
                    });
            });
    }

    // 窗口

    fn map_editor_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_map_editor;
        egui::Window::new("地形编辑")
            .open(&mut open)
            .default_width(880.0)
            .default_height(640.0)
            .resizable(true)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| {
                ui_map::editor(self, ui);
            });
        self.show_map_editor = open;
    }

    fn issues_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_issues;
        egui::Window::new("校验与建议")
            .open(&mut open)
            .default_width(560.0)
            .default_pos(pos2(340.0, 110.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "「错误」会阻止导出；「警告 / 建议」不阻止导出，但影响手感与平衡。",
                    )
                    .size(11.0)
                    .color(TEXT_DIM),
                );
                ui.separator();
                let issues = self.issues.clone();
                let mut jump: Option<f64> = None;
                egui::ScrollArea::vertical()
                    .max_height(430.0)
                    .show(ui, |ui| {
                        if issues.is_empty() {
                            ui.label(RichText::new("没有发现问题").size(12.0).color(OK));
                        }
                        for issue in issues.iter() {
                            let color = match issue.severity {
                                Severity::Error => ERR,
                                Severity::Warning => WARN,
                                Severity::Tip => TEXT_FAINT,
                            };
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(issue.severity.label())
                                        .size(10.0)
                                        .color(color),
                                );
                                ui.label(
                                    RichText::new(format!("[{}]", issue.section.label()))
                                        .size(10.0)
                                        .color(TEXT_FAINT),
                                );
                                ui.label(RichText::new(&issue.message).size(11.5).color(color));
                                if let Some(t) = issue.time {
                                    if soft_button(ui, format!("跳转 {t:.2}s")).clicked() {
                                        jump = Some(t);
                                    }
                                }
                            });
                        }
                    });
                if let Some(t) = jump {
                    self.seek(t);
                }
            });
        self.show_issues = open;
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_shortcuts;
        egui::Window::new("快捷键")
            .open(&mut open)
            .default_width(400.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(BG_PANEL)
                    .stroke(Stroke::new(1.0_f32, LINE))
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                let rows = [
                    ("空格", "播放 / 暂停（录制模式下改为落音符，长按出长条）"),
                    ("Enter", "播放 / 暂停（录制模式下用）"),
                    ("T", "打点测速：跟着音乐连点"),
                    ("Shift + 拖拽", "框选（或用工具栏「框选」）"),
                    ("1 / 2 / 3 / 4", "切换右侧面板：音符 / 出怪 / 双压 / 地形"),
                    ("F", "打点录音（需先开启「打点录音」）"),
                    ("← →", "移动播放头；Shift + ← → 微调选中对象"),
                    ("Home / End", "跳到开头 / 结尾"),
                    ("Ctrl + Z / Y", "撤销 / 重做"),
                    ("Ctrl + S", "保存"),
                    ("Ctrl + E", "导出 JSON"),
                    ("Ctrl + O", "打开关卡"),
                    ("Ctrl + C / V", "复制 / 粘贴到播放头"),
                    ("Ctrl + A", "全选音符与出怪"),
                    ("Ctrl + Q", "量化选中音符"),
                    ("Delete", "删除选中的音符 / 出怪"),
                    ("H", "点按 ↔ 长条（双击音符也可以）"),
                    ("右键", "删除指针下的音符 / 出怪"),
                    ("滚轮 / Ctrl + 滚轮", "横向滚动 / 缩放时间轴"),
                    ("Esc", "取消选择"),
                ];
                for (k, v) in rows {
                    ui.horizontal(|ui| {
                        ui.allocate_ui(vec2(140.0, 20.0), |ui| {
                            ui.label(RichText::new(k).size(11.5).color(GOLD).monospace());
                        });
                        ui.label(RichText::new(v).size(11.5).color(TEXT_DIM));
                    });
                }
            });
        self.show_shortcuts = open;
    }
}

pub fn wave_color(kind: WaveKind) -> Color32 {
    match kind {
        WaveKind::Normal => Color32::from_rgb(0x9a, 0x8f, 0xb5),
        WaveKind::Special => GOLD,
        WaveKind::Armored => Color32::from_rgb(0x6f, 0xa8, 0xc9),
        WaveKind::Ranged => TEAL,
        WaveKind::Boss => Color32::from_rgb(0xe0, 0x6f, 0x8a),
        WaveKind::Thief => Color32::from_rgb(0xc9, 0x7f, 0xd0),
        WaveKind::Decoy => Color32::from_rgb(0x8a, 0xa8, 0x8a),
    }
}

/// 折返型路线，让塔台有更多攻击窗口
fn serpentine_path(route: i32) -> Vec<[i32; 2]> {
    let mut p: Vec<[i32; 2]> = Vec::new();
    match route {
        0 => {
            p.push([-1, 1]);
            for x in 0..=7 {
                p.push([x, 1]);
            }
            p.push([7, 2]);
            p.push([7, 3]);
            for x in (0..=6).rev() {
                p.push([x, 3]);
            }
            p.push([0, 4]);
            p.push([0, 5]);
            for x in 1..=8 {
                p.push([x, 5]);
            }
        }
        1 => {
            p.push([-1, 5]);
            for x in 0..=8 {
                p.push([x, 5]);
            }
            for y in (1..5).rev() {
                p.push([8, y]);
            }
            for x in (1..=7).rev() {
                p.push([x, 1]);
            }
        }
        _ => {
            p.push([-1, 3]);
            for y in (0..=3).rev() {
                p.push([0, y]);
            }
            for x in 1..=8 {
                p.push([x, 0]);
            }
            for y in 1..=6 {
                p.push([8, y]);
            }
            p.push([9, 6]);
        }
    }
    let mut out: Vec<[i32; 2]> = Vec::new();
    for c in p {
        if out.last() != Some(&c) {
            out.push(c);
        }
    }
    out
}
