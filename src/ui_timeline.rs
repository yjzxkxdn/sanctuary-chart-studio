//! 统一时间轴：波形 / 双压曲线 / 谱面 / 出怪 全部叠在同一条时间线上。
//!
//! 设计要点：
//! * 不切换页面也能同时看到「音乐」「谱面」「出怪」「难度」，随时判断手感和难度。
//! * 左侧固定轨道名，所有轨道共用同一个 x = f(t) 映射，播放头贯穿全高。
//! * 分音着色：正拍金色、拍点紫色、半拍青色，其余暗色，一眼看出节奏结构。
//! * 轨道高度可拖动分隔条调整；同一时刻堆叠的怪物会自动上下散开并标出数量。
//! * 支持框选批量编辑，以及「走带 / 定轴」两种播放滚动方式。

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui};
use egui::{pos2, vec2};

use crate::app::{wave_color, Studio};
use crate::autochart::Snap;
use crate::model::{fmt_time, NoteKind, WaveKind};
use crate::theme::*;
use crate::widgets::*;
const GUTTER: f32 = 58.0;
const RULER_H: f32 = 20.0;

/// 供外部计算时间轴可用宽度
pub fn gutter() -> f32 {
    GUTTER
}

// 轨道高度的下限（拖动时不会缩到看不见）
const MIN_WAVE: f32 = 22.0;
const MIN_CURVE: f32 = 18.0;
const MIN_ROUTE: f32 = 16.0;
const MIN_CHART: f32 = 34.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Chart,
    Waves,
    Balance,
    Map,
}

impl Focus {
    pub fn label(self) -> &'static str {
        match self {
            Focus::Chart => "打谱",
            Focus::Waves => "出怪",
            Focus::Balance => "双压平衡",
            Focus::Map => "地形",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Focus::Chart => "点空白放音符 · 拖动改时间 · 拖尾部改长条 · 空白处拖拽框选 · 右键删除",
            Focus::Waves => "泳道空白处点击加怪 · 拖动改时间 · 空白处拖拽框选 · 右键删除",
            Focus::Balance => "红色为在场怪物血量，金色为每秒判定数，紫色为「双压」重叠程度",
            Focus::Map => "在右侧直接预览地图；需要精修时点击「地形编辑」打开大画布",
        }
    }
}

/// 播放时的滚动方式
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollMode {
    /// 走带：背景不动，播放头向右移动
    Free,
    /// 定轴：播放头固定，背景向左滚动
    Fixed,
}

impl ScrollMode {
    pub fn label(self) -> &'static str {
        match self {
            ScrollMode::Free => "走带",
            ScrollMode::Fixed => "定轴",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            ScrollMode::Free => "背景不动，播放头移动",
            ScrollMode::Fixed => "播放头固定，背景向左滚动",
        }
    }
}

/// 各轨道高度（像素）。拖动分隔条即可修改。
#[derive(Clone, Copy, Debug)]
pub struct LaneH {
    pub wave: f32,
    pub curve: f32,
    pub route: f32,
    pub chart: f32,
}

impl Default for LaneH {
    fn default() -> Self {
        Self {
            wave: 96.0,
            curve: 84.0,
            route: 58.0,
            chart: 200.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragKind {
    NoteMove,
    NoteTail,
    WaveMove,
    Box,
}

#[derive(Clone, Debug)]
pub struct Drag {
    pub kind: DragKind,
    pub orig: Vec<(u64, f64, Option<f64>)>,
    pub anchor_t: f64,
    pub start_pos: egui::Pos2,
    pub moved: bool,
}

pub struct TimelineState {
    /// 每秒像素数
    pub pps: f32,
    /// 左端时间
    pub start: f64,
    /// 播放滚动方式
    pub scroll: ScrollMode,
    pub follow: bool,
    pub lane_h: LaneH,
    /// 正在拖动的分隔条（上、下两个轨道的标识）
    pub resizing: Option<(LaneId, LaneId)>,
    pub show_waveform: bool,
    pub show_curve: bool,
    pub show_chart: bool,
    pub show_waves: bool,
    pub show_grid: bool,
    pub color_by_beat: bool,
    /// 录制模式：空格落音符，长按变长条
    pub record: bool,
    /// 框选模式：在该模式下直接拖拽即框选
    pub box_mode: bool,
    pub hit_sound: bool,
    pub note_sel: BTreeSet<u64>,
    pub wave_sel: BTreeSet<u64>,
    pub drag: Option<Drag>,
    /// 框选时的当前指针位置
    pub box_cur: Option<egui::Pos2>,
    pub hover_note: Option<u64>,
    pub hover_wave: Option<u64>,
    pub hover_lane: Option<usize>,
    pub hover_time: Option<f64>,
    pub fitted: bool,
    pub fit_duration: f64,
    /// 上一帧绘图区的左边界（框选时把屏幕坐标换算回时间）
    pub band_plot_l: f32,
    /// 路线泳道 / 谱面轨道的纵向范围（框选时用来区分该选谁）
    pub lanes_routes_y: (f32, f32),
    pub lanes_chart_y: (f32, f32),
}

impl Default for TimelineState {
    fn default() -> Self {
        Self {
            pps: 150.0,
            start: 0.0,
            scroll: ScrollMode::Free,
            follow: true,
            lane_h: LaneH::default(),
            resizing: None,
            show_waveform: true,
            show_curve: true,
            show_chart: true,
            show_waves: true,
            show_grid: true,
            color_by_beat: true,
            record: false,
            box_mode: false,
            hit_sound: true,
            note_sel: BTreeSet::new(),
            wave_sel: BTreeSet::new(),
            drag: None,
            box_cur: None,
            hover_note: None,
            hover_wave: None,
            hover_lane: None,
            hover_time: None,
            fitted: false,
            fit_duration: 0.0,
            band_plot_l: 0.0,
            lanes_routes_y: (0.0, 0.0),
            lanes_chart_y: (0.0, 0.0),
        }
    }
}

impl TimelineState {
    pub fn pps_min(dur: f64, width: f32) -> f32 {
        ((width - GUTTER - 8.0) / dur.max(1.0) as f32).max(2.0) * 0.92
    }
    pub fn pps_max() -> f32 {
        1400.0
    }

    /// 「走带」模式下播放时保持播放头在视野中
    pub fn follow_clock(&mut self, clock: f64, dur: f64, width: f32) {
        if !self.follow {
            return;
        }
        let span = ((width - GUTTER - 8.0) / self.pps.max(1.0)) as f64;
        if clock < self.start + span * 0.08 || clock > self.start + span * 0.72 {
            self.start = (clock - span * 0.30).clamp(0.0, (dur - span * 0.5).max(0.0));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneId {
    Wave,
    Curve,
    Routes,
    Chart,
}

struct Lanes {
    plot_l: f32,
    plot_r: f32,
    ruler: Rect,
    wave: Option<Rect>,
    curve: Option<Rect>,
    chart: Option<Rect>,
    routes: Vec<(usize, Rect)>,
    all: Rect,
    /// 可拖动的分隔条：(y 坐标, 上方轨道, 下方轨道)
    handles: Vec<(f32, LaneId, LaneId)>,
}

/// 轨道高度为**绝对像素**，由谱面轨道吸收余量：拖动分隔条时鼠标移动多少就变多少。
fn lane_layout(app: &Studio, rect: Rect) -> (f32, f32, f32, f32) {
    let tl = &app.tl;
    let h = tl.lane_h;
    let routes_n = if tl.show_waves {
        app.design.paths.len().max(1) as f32
    } else {
        0.0
    };
    let avail = (rect.height() - RULER_H).max(60.0);

    let mut wave = if tl.show_waveform {
        h.wave.max(MIN_WAVE)
    } else {
        0.0
    };
    let mut curve = if tl.show_curve {
        h.curve.max(MIN_CURVE)
    } else {
        0.0
    };
    let mut route = if tl.show_waves {
        h.route.max(MIN_ROUTE)
    } else {
        0.0
    };
    let chart = if tl.show_chart {
        h.chart.max(MIN_CHART)
    } else {
        0.0
    };

    let fixed_sum = wave + curve + route * routes_n;

    if tl.show_chart {
        let remain = avail - fixed_sum;
        if remain >= MIN_CHART {
            // 余量先给谱面；超过软上限的部分转给出怪泳道（让堆叠的怪散得更开）
            let cap = (avail * 0.62).max(MIN_CHART);
            if remain > cap && routes_n > 0.0 {
                let extra = remain - cap;
                let room = (120.0 - route).max(0.0) * routes_n;
                let give = extra.min(room);
                let route_h = route + give / routes_n;
                let chart_h = avail - (wave + curve + route_h * routes_n);
                return (wave, curve, route_h, chart_h.max(MIN_CHART));
            }
            return (wave, curve, route, remain);
        }
        // 空间不足：按比例压缩上方轨道，给谱面留出最小高度
        let budget = (avail - MIN_CHART).max(MIN_WAVE + MIN_CURVE + MIN_ROUTE * routes_n);
        let k = (budget / fixed_sum.max(1.0)).clamp(0.0, 1.0);
        wave *= k;
        curve *= k;
        route *= k;
        let ch = (avail - (wave + curve + route * routes_n)).max(MIN_CHART);
        (wave, curve, route, ch)
    } else {
        // 没有谱面轨道时，让出怪轨吃掉余量
        let remain = avail - (wave + curve);
        if remain > 0.0 && routes_n > 0.0 {
            route = (remain / routes_n).max(MIN_ROUTE);
        }
        (wave, curve, route, chart)
    }
}

fn lane_rects(app: &Studio, rect: Rect) -> Lanes {
    let tl = &app.tl;
    let plot_l = rect.left() + GUTTER;
    let plot_r = (rect.right() - 6.0).max(plot_l + 40.0);
    let (wave_h, curve_h, route_h, chart_h) = lane_layout(app, rect);
    let mut y = rect.top();
    let mut handles: Vec<(f32, LaneId, LaneId)> = Vec::new();

    macro_rules! mk {
        ($h:expr) => {{
            let r = Rect::from_min_max(pos2(plot_l, y), pos2(plot_r, y + $h));
            y += $h;
            r
        }};
    }

    let ruler = mk!(RULER_H);
    let wave = if tl.show_waveform {
        Some(mk!(wave_h.max(MIN_WAVE)))
    } else {
        None
    };
    let curve = if tl.show_curve {
        if wave.is_some() {
            handles.push((y, LaneId::Wave, LaneId::Curve));
        }
        Some(mk!(curve_h.max(MIN_CURVE)))
    } else {
        None
    };
    let mut routes = Vec::new();
    if tl.show_waves {
        if curve.is_some() || wave.is_some() {
            handles.push((
                y,
                if curve.is_some() {
                    LaneId::Curve
                } else {
                    LaneId::Wave
                },
                LaneId::Routes,
            ));
        }
        for i in 0..app.design.paths.len().max(1) {
            routes.push((i, mk!(route_h.max(MIN_ROUTE))));
        }
    }
    let chart = if tl.show_chart {
        if !routes.is_empty() {
            handles.push((y, LaneId::Routes, LaneId::Chart));
        }
        Some(mk!(chart_h.max(MIN_CHART)))
    } else {
        None
    };

    Lanes {
        plot_l,
        plot_r,
        ruler,
        wave,
        curve,
        chart,
        routes,
        all: Rect::from_min_max(rect.min, pos2(plot_r, y.min(rect.bottom()))),
        handles,
    }
}

// 入口

pub fn show(app: &mut Studio, ui: &mut Ui) {
    toolbar(app, ui);
    ui.add_space(4.0);

    let avail = ui.available_size();

    // 首次布局 / 时长变化后自动整曲适配
    if !app.tl.fitted || app.tl.fit_duration != app.design.duration {
        let pps = TimelineState::pps_min(app.design.duration, avail.x);
        app.tl.pps = pps;
        app.tl.start = 0.0;
        app.tl.fitted = true;
        app.tl.fit_duration = app.design.duration;
    }

    let want_h = avail.y.max(160.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(avail.x, want_h), Sense::click_and_drag());
    let lanes = lane_rects(app, rect);
    app.tl.band_plot_l = lanes.plot_l;
    // 记录各轨道纵向范围，框选时按轨道区分对象
    app.tl.lanes_routes_y = match (lanes.routes.first(), lanes.routes.last()) {
        (Some((_, a)), Some((_, b))) => (a.top(), b.bottom()),
        _ => (0.0, 0.0),
    };
    app.tl.lanes_chart_y = lanes.chart.map_or((0.0, 0.0), |r| (r.top(), r.bottom()));

    // 分隔条拖动优先处理
    if lanes_drag(app, ui, &lanes, &resp) {
        // 拖动分隔条时不响应其他交互
        paint_all(app, ui, rect, &lanes, &resp);
        return;
    }

    handle_view(app, ui, &lanes, &resp, rect);
    paint_all(app, ui, rect, &lanes, &resp);
    let st = app.tl.start;
    let pp = app.tl.pps;
    hover_feedback(app, ui, &lanes, &resp, &|t| x_of(&lanes, st, pp, t));
    interaction(app, ui, &lanes, &resp);
}

fn x_of(lanes: &Lanes, start: f64, pps: f32, t: f64) -> f32 {
    lanes.plot_l + ((t - start) * pps as f64) as f32
}

fn paint_all(app: &Studio, ui: &mut Ui, rect: Rect, lanes: &Lanes, resp: &egui::Response) {
    let start = app.tl.start;
    let pps = app.tl.pps;
    let xf = |t: f64| x_of(lanes, start, pps, t);
    let t_of = |x: f32| start + ((x - lanes.plot_l) / pps.max(1.0)) as f64;

    let painter = ui.painter_at(rect.expand2(vec2(0.0, 2.0)));
    painter.rect_filled(rect, CornerRadius::same(6), BG_INPUT);

    // 轨道底色
    for r in [Some(lanes.ruler), lanes.wave, lanes.curve, lanes.chart]
        .into_iter()
        .flatten()
    {
        painter.rect_filled(r, 0.0, BG_PANEL);
    }
    for (i, r) in &lanes.routes {
        let tint = if *i % 2 == 0 { BG_PANEL } else { BG_CARD };
        painter.rect_filled(*r, 0.0, tint);
    }

    label_gutter(app, &painter, lanes);
    draw_grid(app, &painter, lanes, &xf);
    if let Some(r) = lanes.wave {
        draw_waveform(app, &painter, r);
    }
    if let Some(r) = lanes.curve {
        draw_curve(app, &painter, r, &xf, &t_of);
    }
    for (i, r) in &lanes.routes {
        draw_route_lane(app, &painter, *r, *i, &xf);
    }
    if let Some(r) = lanes.chart {
        draw_notes(app, &painter, r, &xf);
    }

    // 分隔条（可拖动）
    for (y, _, _) in &lanes.handles {
        let hovered = resp
            .hover_pos()
            .map(|p| (p.y - y).abs() < 4.0 && p.x > lanes.plot_l - GUTTER)
            .unwrap_or(false);
        let active = app.tl.resizing.is_some();
        let color = if hovered || active {
            with_alpha(GOLD, 0.55)
        } else {
            with_alpha(LINE, 0.9)
        };
        painter.line_segment(
            [
                pos2(lanes.plot_l - GUTTER + 4.0, *y),
                pos2(lanes.plot_r, *y),
            ],
            Stroke::new(1.0_f32, color),
        );
        if hovered {
            for k in -1..=1 {
                let x = lanes.plot_l - GUTTER + 10.0 + k as f32 * 3.0;
                painter.circle_filled(pos2(x, *y), 1.0, with_alpha(GOLD, 0.8));
            }
        }
    }

    // 播放头
    let px = xf(app.clock);
    if px >= lanes.plot_l - 1.0 && px <= lanes.plot_r + 1.0 {
        painter.line_segment(
            [pos2(px, lanes.ruler.top()), pos2(px, lanes.all.bottom())],
            Stroke::new(1.6_f32, if app.tl.record { ERR } else { PLAYHEAD }),
        );
        painter.circle_filled(
            pos2(px, lanes.ruler.top() + 1.0),
            4.0,
            if app.tl.record { ERR } else { PLAYHEAD },
        );
    }

    // 定轴模式的锚线
    if app.tl.scroll == ScrollMode::Fixed {
        let anchor = lanes.plot_l + (lanes.plot_r - lanes.plot_l) * 0.30;
        painter.line_segment(
            [
                pos2(anchor, lanes.all.top()),
                pos2(anchor, lanes.all.bottom()),
            ],
            Stroke::new(1.0_f32, with_alpha(TEAL, 0.28)),
        );
    }

    // 框选矩形
    if let Some(d) = &app.tl.drag {
        if d.kind == DragKind::Box {
            if let Some(p) = app.tl.box_cur {
                let r = Rect::from_two_pos(d.start_pos, p);
                painter.rect_filled(r, 0.0, with_alpha(PURPLE, 0.12));
                painter.rect_stroke(
                    r,
                    0.0,
                    Stroke::new(1.0_f32, with_alpha(PURPLE, 0.75)),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    // 录制指示
    if app.tl.record {
        let badge = Rect::from_min_size(
            pos2(lanes.plot_r - 52.0, lanes.ruler.top() + 2.0),
            vec2(46.0, 14.0),
        );
        painter.rect_filled(badge, CornerRadius::same(3), with_alpha(ERR, 0.85));
        painter.text(
            badge.center(),
            egui::Align2::CENTER_CENTER,
            "● REC",
            font_id(9.0),
            Color32::from_rgb(0x1a, 0x08, 0x08),
        );
    }
}

// 顶部工具条

fn toolbar(app: &mut Studio, ui: &mut Ui) {
    egui::Frame::NONE
        .fill(BG_CARD)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(9, 6))
        .stroke(Stroke::new(1.0_f32, LINE_SOFT))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for f in [Focus::Chart, Focus::Waves, Focus::Balance, Focus::Map] {
                    if pill(ui, f.label(), app.focus == f, GOLD).clicked() {
                        app.focus = f;
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("吸附").size(10.0).color(TEXT_FAINT));
                let mut snap = app.snap;
                egui::ComboBox::from_id_salt("tl_snap")
                    .selected_text(RichText::new(snap.label()).size(10.5))
                    .width(56.0)
                    .show_ui(ui, |ui| {
                        for s in Snap::ALL {
                            if ui.selectable_label(snap == s, s.label()).clicked() {
                                snap = s;
                            }
                        }
                    });
                app.snap = snap;

                ui.add_space(4.0);
                if soft_button(ui, "−").clicked() {
                    app.tl.pps = (app.tl.pps / 1.35).clamp(2.0, TimelineState::pps_max());
                }
                ui.label(
                    RichText::new(format!("{:.0}%", app.tl.pps / 150.0 * 100.0))
                        .size(9.5)
                        .color(TEXT_FAINT)
                        .monospace(),
                );
                if soft_button(ui, "＋").clicked() {
                    app.tl.pps = (app.tl.pps * 1.35).clamp(2.0, TimelineState::pps_max());
                }
                if soft_button(ui, "整曲").clicked() {
                    app.fit_timeline();
                }

                // 轨道高度
                ui.menu_button(RichText::new("轨道").size(10.5), |ui| {
                    ui.set_min_width(210.0);
                    ui.label(
                        RichText::new("也可以直接拖动轨道之间的分隔条")
                            .size(9.5)
                            .color(TEXT_FAINT),
                    );
                    let h = &mut app.tl.lane_h;
                    ui.add(egui::Slider::new(&mut h.wave, MIN_WAVE..=300.0).text("波形"));
                    ui.add(egui::Slider::new(&mut h.curve, MIN_CURVE..=300.0).text("难度"));
                    ui.add(
                        egui::Slider::new(&mut h.route, MIN_ROUTE..=120.0).text("出怪（每条路线）"),
                    );
                    ui.label(
                        RichText::new("谱面轨道自动填充剩余高度")
                            .size(9.0)
                            .color(TEXT_FAINT),
                    );
                    if soft_button(ui, "恢复默认").clicked() {
                        *h = LaneH::default();
                    }
                });

                ui.add_space(3.0);
                if toggle(ui, "谱面", app.tl.show_chart, GOLD).clicked() {
                    app.tl.show_chart = !app.tl.show_chart;
                }
                if toggle(ui, "出怪", app.tl.show_waves, PURPLE).clicked() {
                    app.tl.show_waves = !app.tl.show_waves;
                }
                if toggle(ui, "难度", app.tl.show_curve, ERR).clicked() {
                    app.tl.show_curve = !app.tl.show_curve;
                }
                if toggle(ui, "波形", app.tl.show_waveform, TEAL).clicked() {
                    app.tl.show_waveform = !app.tl.show_waveform;
                }
                if toggle(
                    ui,
                    "分音着色",
                    app.tl.color_by_beat,
                    Color32::from_rgb(0xd0, 0x8a, 0xe8),
                )
                .clicked()
                {
                    app.tl.color_by_beat = !app.tl.color_by_beat;
                }
            });

            ui.add_space(3.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                // 录制：大号红色按钮，一眼可见
                let rec_label = if app.tl.record {
                    "■ 结束录制"
                } else {
                    "● 录制打谱"
                };
                let (rec_fill, rec_border, rec_text) = if app.tl.record {
                    (with_alpha(ERR, 0.78), ERR, Color32::WHITE)
                } else {
                    (with_alpha(ERR, 0.14), with_alpha(ERR, 0.7), ERR)
                };
                if chip_button(ui, rec_label, rec_fill, rec_border, rec_text)
                    .on_hover_text(
                        "录制模式下：空格 = 在播放头处落音符，长按 = 长条；Enter = 播放/暂停",
                    )
                    .clicked()
                {
                    app.tl.record = !app.tl.record;
                    app.toast(if app.tl.record {
                        "录制已开启：空格落音符（长按出长条），Enter 播放 / 暂停"
                    } else {
                        "录制已关闭：空格恢复为播放 / 暂停"
                    });
                }
                if app.tl.record {
                    ui.label(
                        RichText::new("空格落音符 · 长按出长条 · Enter 播放/暂停")
                            .size(9.5)
                            .color(ERR),
                    );
                }
                // 框选
                if pill(ui, "框选", app.tl.box_mode, PURPLE)
                    .on_hover_text("开启后拖拽即为框选；也可以临时按住 Shift 拖拽")
                    .clicked()
                {
                    app.tl.box_mode = !app.tl.box_mode;
                }
                if pill(ui, "全选", false, GOLD).clicked() {
                    app.tl.note_sel = app.design.chart.iter().map(|n| n.id).collect();
                    app.tl.wave_sel = app.design.waves.iter().map(|w| w.id).collect();
                }
                if !app.tl.note_sel.is_empty() || !app.tl.wave_sel.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "已选 {} 音符 · {} 出怪",
                            app.tl.note_sel.len(),
                            app.tl.wave_sel.len()
                        ))
                        .size(9.5)
                        .color(GOLD),
                    );
                    if soft_button(ui, "清空选择").clicked() {
                        app.tl.note_sel.clear();
                        app.tl.wave_sel.clear();
                    }
                }

                ui.add_space(4.0);
                if toggle(ui, "打击音", app.tl.hit_sound, OK).clicked() {
                    app.tl.hit_sound = !app.tl.hit_sound;
                }
                ui.label(RichText::new("|").size(9.5).color(LINE));
                ui.label(RichText::new("播放").size(9.5).color(TEXT_FAINT));
                for m in [ScrollMode::Free, ScrollMode::Fixed] {
                    if pill(ui, m.label(), app.tl.scroll == m, TEAL)
                        .on_hover_text(m.hint())
                        .clicked()
                    {
                        app.tl.scroll = m;
                        if m == ScrollMode::Fixed {
                            app.tl.follow = true;
                        }
                    }
                }
                if app.tl.scroll == ScrollMode::Free
                    && toggle(ui, "跟随", app.tl.follow, PURPLE).clicked()
                {
                    app.tl.follow = !app.tl.follow;
                }
                ui.label(RichText::new("|").size(9.5).color(LINE));
                ui.label(RichText::new(app.focus.hint()).size(9.5).color(TEXT_FAINT));
            });
        });
}

// 轨道高度拖动

fn lanes_drag(app: &mut Studio, ui: &mut Ui, lanes: &Lanes, resp: &egui::Response) -> bool {
    let primary = egui::PointerButton::Primary;
    // 悬停到分隔条时提示可拖动；按下（6px 内）即抓住
    if let Some(p) = resp.hover_pos() {
        if app.tl.resizing.is_none() {
            if let Some(&(_, a, b)) = lanes
                .handles
                .iter()
                .find(|(y, _, _)| (p.y - y).abs() < 6.0 && p.x > lanes.plot_l - GUTTER)
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                if resp.drag_started_by(primary) {
                    app.tl.resizing = Some((a, b));
                    return true;
                }
            }
        }
    }
    let Some((above, below)) = app.tl.resizing else {
        // 补救：按下瞬间指针移动太快会错过命中窗口——拖拽已开始但
        // 起点仍贴近分隔条（<8px 位移）时，把这次手势转成调整高度。
        if resp.dragged_by(primary) {
            if let (Some(p), Some(o)) = (
                resp.interact_pointer_pos(),
                ui.ctx().input(|i| i.pointer.press_origin()),
            ) {
                if (p - o).length() < 8.0 {
                    if let Some(&(_, a, b)) = lanes
                        .handles
                        .iter()
                        .find(|(y, _, _)| (o.y - y).abs() < 6.0 && o.x > lanes.plot_l - GUTTER)
                    {
                        app.tl.drag = None;
                        app.tl.box_cur = None;
                        app.tl.resizing = Some((a, b));
                        return true;
                    }
                }
            }
        }
        return false;
    };
    if resp.dragged_by(primary) {
        let dy = ui.ctx().input(|i| i.pointer.delta().y);
        if dy.abs() > 0.0 {
            apply_resize(app, above, below, dy);
        }
    }
    if resp.drag_stopped() || !ui.ctx().input(|i| i.pointer.any_down()) {
        app.tl.resizing = None;
    }
    true
}

/// 拖动时**鼠标移动多少像素，轨道就变多少像素**；受双方最小高度约束，绝不超调。
fn apply_resize(app: &mut Studio, above: LaneId, below: LaneId, dy: f32) {
    let routes = app.design.paths.len().max(1) as f32;
    let h = &mut app.tl.lane_h;
    // 把「出怪轨」当作整体（条数 × 单条高度）参与运算，拖一条边就是整体让出
    match (above, below) {
        (LaneId::Wave, LaneId::Curve) => {
            let d = clamp_pair(h.wave, MIN_WAVE, h.curve, MIN_CURVE, dy);
            h.wave += d;
            h.curve -= d;
        }
        (LaneId::Curve, LaneId::Routes) => {
            let routes_total = h.route * routes;
            let d = clamp_pair(h.curve, MIN_CURVE, routes_total, MIN_ROUTE * routes, dy);
            h.curve += d;
            h.route = (routes_total - d) / routes;
        }
        (LaneId::Wave, LaneId::Routes) => {
            let routes_total = h.route * routes;
            let d = clamp_pair(h.wave, MIN_WAVE, routes_total, MIN_ROUTE * routes, dy);
            h.wave += d;
            h.route = (routes_total - d) / routes;
        }
        (LaneId::Routes, LaneId::Chart) => {
            // 谱面轨道吸收余量，改出怪高度即可（谱面会自动跟随）
            let routes_total = h.route * routes;
            let d = clamp_pair(routes_total, MIN_ROUTE * routes, f32::MAX, 0.0, dy);
            h.route = (routes_total + d) / routes;
        }
        _ => {}
    }
}

/// 让 a 增加 d、b 减少 d，并把 d 限制在两者都不越界，保证 1:1 跟随鼠标。
fn clamp_pair(a: f32, a_min: f32, b: f32, b_min: f32, d: f32) -> f32 {
    let hi = (b - b_min).max(0.0); // 最多能让 b 缩小多少
    let lo = -(a - a_min).max(0.0); // 最多能让 a 缩小多少
    if hi < lo {
        0.0
    } else {
        d.clamp(lo, hi)
    }
}

// 视图操作

fn handle_view(app: &mut Studio, ui: &mut Ui, lanes: &Lanes, resp: &egui::Response, rect: Rect) {
    let dur = app.design.duration;
    let width = rect.width();
    let pmin = TimelineState::pps_min(dur, width);
    let (scroll, zoom, ctrl) = ui
        .ctx()
        .input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers.command));
    if resp.hovered() {
        if ctrl || zoom != 1.0 {
            let ptr = ui
                .ctx()
                .input(|i| i.pointer.hover_pos())
                .unwrap_or(rect.center());
            let t_anchor = app.tl.start + ((ptr.x - lanes.plot_l) / app.tl.pps.max(1.0)) as f64;
            let factor = if zoom != 1.0 {
                zoom
            } else {
                (1.0 + scroll.y * 0.0025).clamp(0.5, 2.0)
            };
            app.tl.pps = (app.tl.pps * factor).clamp(pmin, TimelineState::pps_max());
            app.tl.start =
                (t_anchor - ((ptr.x - lanes.plot_l) / app.tl.pps.max(1.0)) as f64).max(0.0);
        } else if scroll.y.abs() > 0.2 || scroll.x.abs() > 0.2 {
            let d = if scroll.x.abs() > scroll.y.abs() {
                scroll.x
            } else {
                scroll.y
            };
            app.tl.start = (app.tl.start - d as f64 / app.tl.pps.max(1.0) as f64).max(0.0);
        }
    }
    let span = ((lanes.plot_r - lanes.plot_l) / app.tl.pps.max(1.0)) as f64;
    let max_start = (dur - span * 0.35).max(0.0);
    app.tl.pps = app.tl.pps.clamp(pmin, TimelineState::pps_max());
    app.tl.start = app.tl.start.clamp(0.0, max_start);

    if app.playing {
        match app.tl.scroll {
            ScrollMode::Free => app.tl.follow_clock(app.clock, dur, width),
            ScrollMode::Fixed => {
                // 播放头固定在 30% 处，背景左移
                let s = app.clock - span * 0.30;
                app.tl.start = s.clamp(0.0, (dur - span * 0.5).max(0.0));
            }
        }
    }
}

// 绘制

fn label_gutter(app: &Studio, painter: &egui::Painter, lanes: &Lanes) {
    let all = lanes.all;
    let g = Rect::from_min_max(all.min, pos2(all.left() + GUTTER - 6.0, all.bottom()));
    painter.rect_filled(g, 0.0, BG_CARD);
    painter.line_segment(
        [pos2(g.right(), g.top()), pos2(g.right(), g.bottom())],
        Stroke::new(1.0_f32, LINE_SOFT),
    );

    let put = |r: Rect, text: &str, color: Color32, sub: Option<&str>| {
        painter.text(
            pos2(g.left() + 8.0, r.center().y - 1.0),
            egui::Align2::LEFT_CENTER,
            text,
            font_id(10.5),
            color,
        );
        if let Some(s) = sub {
            painter.text(
                pos2(g.left() + 8.0, r.center().y + 9.0),
                egui::Align2::LEFT_CENTER,
                s,
                font_id(8.5),
                TEXT_FAINT,
            );
        }
    };
    put(lanes.ruler, "小节", TEXT_FAINT, None);
    if let Some(r) = lanes.wave {
        let name = if app.audio.has_audio() {
            "波形"
        } else {
            "无音频"
        };
        put(r, name, TEXT_DIM, Some("能量"));
    }
    if let Some(r) = lanes.curve {
        put(r, "难度", PURPLE, Some("双压"));
    }
    for (i, r) in &lanes.routes {
        let c = route_color(*i);
        painter.circle_filled(pos2(g.left() + 5.0, r.center().y), 3.0, c);
        put(*r, &format!("路线 {}", i + 1), TEXT_DIM, None);
    }
    if let Some(r) = lanes.chart {
        put(
            r,
            "谱面",
            GOLD,
            Some(&format!("{} 音符", app.design.chart.len())),
        );
    }
}

fn draw_grid(app: &Studio, painter: &egui::Painter, lanes: &Lanes, x_of: &impl Fn(f64) -> f32) {
    if !app.tl.show_grid {
        return;
    }
    let beat = app.design.beat();
    let bar = app.design.bar();
    let span = (lanes.plot_r - lanes.plot_l) / app.tl.pps.max(1.0);
    let t0 = app.tl.start;
    let t1 = t0 + span as f64;
    let top = lanes.ruler.bottom();
    let bottom = lanes.all.bottom();

    let first_bar = ((t0 - app.phase) / bar).floor() as i64 - 1;
    let bar_px = (bar * app.tl.pps as f64) as f32;
    // 放大后每个小节都标号；缩小时每 4 小节标一次
    let label_every = if bar_px > 46.0 { 1 } else { 4 };
    let mut i = first_bar;
    loop {
        let t = app.phase + i as f64 * bar;
        i += 1;
        if t > t1 {
            break;
        }
        if t < t0 - bar {
            continue;
        }
        let x = x_of(t);
        // 隔小节铺一层淡淡的紫色底，节拍结构一眼可辨
        if i % 2 == 0 {
            let xa = x.max(lanes.plot_l);
            let xb = (x + bar_px).min(lanes.plot_r);
            if xb > xa {
                painter.rect_filled(
                    Rect::from_min_max(pos2(xa, top), pos2(xb, bottom)),
                    0.0,
                    with_alpha(PURPLE_DEEP, 0.07),
                );
            }
        }
        painter.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            Stroke::new(1.2_f32, Color32::from_rgb(0x4d, 0x41, 0x66)),
        );
        if label_every == 1 || i % 4 == 1 || i == first_bar + 1 {
            painter.text(
                pos2(x + 3.0, lanes.ruler.center().y),
                egui::Align2::LEFT_CENTER,
                format!("{}", i.max(0) + 1),
                font_id(9.5),
                TEXT_FAINT,
            );
        }
    }
    if (bar * app.tl.pps as f64) > 26.0 {
        let first_beat = ((t0 - app.phase) / beat).floor() as i64 - 1;
        let mut j = first_beat;
        loop {
            let t = app.phase + j as f64 * beat;
            j += 1;
            if t > t1 {
                break;
            }
            if t < t0 {
                continue;
            }
            if j % 4 == 0 {
                continue;
            }
            let x = x_of(t);
            painter.line_segment(
                [pos2(x, top), pos2(x, bottom)],
                Stroke::new(1.0_f32, with_alpha(LINE, 0.95)),
            );
        }
    }
}

fn draw_waveform(app: &Studio, painter: &egui::Painter, rect: Rect) {
    if !app.audio.has_audio() || app.audio.waveform.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "尚未载入音频 — 载入后这里会显示波形与能量曲线",
            font_id(11.0),
            TEXT_FAINT,
        );
        return;
    }
    let wf = &app.audio.waveform;
    let n = wf.min.len();
    if n == 0 {
        return;
    }
    let pps = app.tl.pps.max(1.0) as f64;
    let t0 = app.tl.start;
    let t1 = t0 + rect.width() as f64 / pps;
    let mid = rect.center().y;
    let half = rect.height() * 0.42;
    let bucket = wf.bucket.max(1e-6);

    // 关键：按**时间轴**遍历（锚定在音频桶的整数倍上），而不是按屏幕列遍历。
    // 按屏幕列遍历时，列边界相对音频格的位置每帧都在漂移，缩放小时一列跨
    // 好几格，取到的格子逐帧变化 —— 线条就会在相邻峰值之间反复横跳。
    // 时间轴锚定后，同一段音频永远落在同一个格上，滚动只是整体平移。
    let px_per_bucket = bucket * pps;
    let mut per = if px_per_bucket >= 1.5 {
        1usize
    } else {
        (1.5 / px_per_bucket).ceil() as usize
    };
    let visible_buckets = ((t1 - t0) / bucket).max(1.0) as usize;
    per = per.max(visible_buckets.div_ceil(4000)).max(1);

    let first = ((t0 / bucket).floor().max(0.0) as usize) / per * per;
    let last = (((t1 / bucket).ceil() as usize) + per).min(n);
    let bar_w = ((per as f64 * bucket * pps) as f32).max(1.0);
    let mut g = first;
    while g < last {
        let e = (g + per).min(n);
        let mut lo = 0.0f32;
        let mut hi = 0.0f32;
        for i in g..e {
            lo = lo.min(wf.min[i]);
            hi = hi.max(wf.max[i]);
        }
        let t = g as f64 * bucket;
        let x = rect.left() + ((t - t0) * pps) as f32;
        let a = mid - hi * half;
        let b = mid - lo * half;
        painter.rect_filled(
            Rect::from_min_max(pos2(x, a), pos2(x + bar_w, b.max(a + 0.8))),
            0.0,
            with_alpha(TEAL, 0.42),
        );
        g += per;
    }

    // ---------------- 能量线：同样按时间轴遍历 ----------------
    let hop = 0.02;
    let env = wf.energy_envelope(hop);
    if env.len() > 1 {
        let px_per_hop = hop * pps;
        let mut eper = if px_per_hop >= 2.5 {
            1usize
        } else {
            (2.5 / px_per_hop).ceil() as usize
        };
        let vis_hops = ((t1 - t0) / hop).max(1.0) as usize;
        eper = eper.max(vis_hops.div_ceil(2400)).max(1);
        let ef = ((t0 / hop).floor().max(0.0) as usize) / eper * eper;
        let el = (((t1 / hop).ceil() as usize) + eper).min(env.len());
        let mut pts: Vec<egui::Pos2> = Vec::new();
        let mut k = ef;
        while k < el {
            let e = (k + eper).min(env.len());
            // 一组内取最大值：既能保住峰值，又不会逐帧抖动
            let v = env[k..e].iter().cloned().fold(0.0f32, f32::max);
            let t = (k as f64 + eper as f64 * 0.5) * hop;
            let x = rect.left() + ((t - t0) * pps) as f32;
            pts.push(pos2(x, rect.bottom() - 2.0 - v * (rect.height() - 8.0)));
            k += eper;
        }
        if pts.len() > 1 {
            painter.add(egui::Shape::line(
                pts,
                Stroke::new(
                    1.3_f32,
                    with_alpha(Color32::from_rgb(0xf4, 0xec, 0xd6), 0.82),
                ),
            ));
        }
    }

    painter.text(
        pos2(rect.left() + 4.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        &app.audio.source_name,
        font_id(9.5),
        TEXT_FAINT,
    );
}

/// 难度曲线：**跟随当前缩放与滚动**，与其它轨道共用同一套 x = f(t)。
fn draw_curve(
    app: &Studio,
    painter: &egui::Painter,
    rect: Rect,
    x_of: &impl Fn(f64) -> f32,
    t_of: &impl Fn(f32) -> f64,
) {
    // 固定分辨率采样，避免每帧重建巨量 bin
    let bins = ((app.design.duration / 0.05).ceil() as usize).clamp(200, 6000);
    let c = app.design.curves(bins);
    if c.bin <= 0.0 {
        return;
    }
    let t0 = t_of(rect.left()).max(0.0);
    let t1 = t_of(rect.right()).max(0.0);
    let i0 = ((t0 / c.bin).floor() as usize).min(bins - 1);
    let i1 = ((t1 / c.bin).ceil() as usize).min(bins);

    // 可见范围峰值（跟随缩放，读数是当前窗口的）
    let mut vis_n = 0.0f32;
    let mut vis_t = 0.0f32;
    for k in i0..i1 {
        vis_n = vis_n.max(c.nps[k]);
        vis_t = vis_t.max(c.threat[k]);
    }
    // 三条曲线**各自**按当前窗口的峰值归一化：
    // 后期怪物血量堆积不会再把整条 y 轴撑高、压扁其余数据。
    let peak_n = vis_n.max(0.5);
    // 首领血量是普通怪的 19 倍，直接线性归一化会把其余曲线压平，这里开根号
    let peak_t = vis_t.max(1.0).sqrt();
    let h = rect.height() - 6.0;

    // 按 bin 画柱状
    let mut i = i0;
    while i < i1 {
        let t_a = i as f64 * c.bin;
        let t_b = t_a + c.bin;
        let xa = x_of(t_a);
        let xb = x_of(t_b).max(xa + 0.7);
        if xb < rect.left() - 2.0 || xa > rect.right() + 2.0 {
            i += 1;
            continue;
        }
        let th = (c.threat[i].max(0.0).sqrt() / peak_t).clamp(0.0, 1.0);
        if th > 0.005 {
            let hh = th * h;
            painter.rect_filled(
                Rect::from_min_max(pos2(xa, rect.bottom() - hh), pos2(xb, rect.bottom())),
                0.0,
                with_alpha(Color32::from_rgb(0xd0, 0x6a, 0x5a), 0.26),
            );
        }
        let np = (c.nps[i] / peak_n).clamp(0.0, 1.0);
        // 冲突度同样用窗口峰值现算，保证和柱高一致
        let cp = (np * th).clamp(0.0, 1.0);
        if cp > 0.02 {
            let hh = cp * h;
            painter.rect_filled(
                Rect::from_min_max(pos2(xa, rect.bottom() - hh), pos2(xb, rect.bottom())),
                0.0,
                with_alpha(Color32::from_rgb(0xc5, 0xa3, 0xee), 0.16 + cp * 0.22),
            );
        }
        let y = rect.bottom() - np * h;
        painter.line_segment(
            [pos2(xa, y), pos2(xb, y)],
            Stroke::new(1.4_f32, with_alpha(GOLD, 0.85)),
        );
        i += 1;
    }

    // 双压热点：在可见范围内找窗口归一化冲突度的局部峰值
    let mut hot: Vec<(usize, f32)> = Vec::new();
    {
        let mut k = i0.max(1);
        while k < i1.saturating_sub(1) {
            let np = (c.nps[k] / peak_n).clamp(0.0, 1.0);
            let th = (c.threat[k].max(0.0).sqrt() / peak_t).clamp(0.0, 1.0);
            let v = np * th;
            if v > 0.5 {
                let vp = {
                    let kp = k.saturating_sub(1);
                    (c.nps[kp] / peak_n).clamp(0.0, 1.0)
                        * (c.threat[kp].max(0.0).sqrt() / peak_t).clamp(0.0, 1.0)
                };
                let vn = {
                    let kn = (k + 1).min(bins - 1);
                    (c.nps[kn] / peak_n).clamp(0.0, 1.0)
                        * (c.threat[kn].max(0.0).sqrt() / peak_t).clamp(0.0, 1.0)
                };
                if v >= vp && v >= vn {
                    hot.push((k, v));
                }
            }
            k += 1;
        }
    }
    hot.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (k, v) in hot.into_iter().take(6) {
        let x = x_of(k as f64 * c.bin);
        if x >= rect.left() && x <= rect.right() {
            painter.line_segment(
                [pos2(x, rect.top() + 2.0), pos2(x, rect.bottom() - 2.0)],
                Stroke::new(1.0_f32, with_alpha(ERR, 0.35 + v * 0.5)),
            );
        }
    }

    // 可见范围峰值已在开头算好并用于归一化
    painter.text(
        pos2(rect.left() + 4.0, rect.bottom() - 2.0),
        egui::Align2::LEFT_BOTTOM,
        format!("窗口峰值 {:.1} 判定/s · 血量 {:.0}", vis_n, vis_t),
        font_id(8.5),
        with_alpha(TEXT_FAINT, 0.85),
    );
}

fn route_color(i: usize) -> Color32 {
    [GOLD, PURPLE, TEAL][i % 3]
}

/// 一个怪物标记在屏幕上的位置（同一时刻的多个怪会上下散开）
#[derive(Clone, Copy)]
struct Mark {
    idx: usize,
    x: f32,
    y: f32,
    r: f32,
}

/// 计算某条路线在可见范围内的所有怪物标记；同一时刻堆叠的会纵向散开。
fn layout_marks(
    app: &Studio,
    rect: Rect,
    route: usize,
    x_of: &impl Fn(f64) -> f32,
) -> (Vec<Mark>, Vec<(f32, usize)>) {
    let mut items: Vec<(f32, usize)> = Vec::new();
    for (i, w) in app.design.waves.iter().enumerate() {
        if w.route != route {
            continue;
        }
        let x = x_of(w.time);
        if x < rect.left() - 14.0 || x > rect.right() + 14.0 {
            continue;
        }
        items.push((x, i));
    }
    items.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // 聚簇：相邻像素 < 13，且整簇宽度 < 44，否则一个密集段会塌成一列
    let mut clusters: Vec<(usize, usize)> = Vec::new();
    let mut s = 0usize;
    for i in 1..items.len() {
        if items[i].0 - items[i - 1].0 > 13.0 || items[i].0 - items[s].0 > 44.0 {
            clusters.push((s, i));
            s = i;
        }
    }
    if !items.is_empty() {
        clusters.push((s, items.len()));
    }

    let mut marks = Vec::with_capacity(items.len());
    let mut counts = Vec::with_capacity(clusters.len());
    let usable = (rect.height() - 8.0).max(8.0);
    for (a, b) in clusters {
        let n = b - a;
        let step = (usable / n as f32).clamp(6.0, 15.0);
        let cx = items[a..b].iter().map(|(x, _)| *x).sum::<f32>() / n as f32;
        counts.push((cx, n));
        for (k, (x, idx)) in items[a..b].iter().enumerate() {
            let off = (k as f32 - (n as f32 - 1.0) * 0.5) * step;
            let r = if n > 2 {
                4.0
            } else if n == 2 {
                4.8
            } else {
                5.5
            };
            marks.push(Mark {
                idx: *idx,
                x: *x,
                y: rect.center().y + off,
                r,
            });
        }
    }
    (marks, counts)
}

fn draw_route_lane(
    app: &Studio,
    painter: &egui::Painter,
    rect: Rect,
    route: usize,
    x_of: &impl Fn(f64) -> f32,
) {
    let base = route_color(route);
    let (marks, counts) = layout_marks(app, rect, route, x_of);
    for m in &marks {
        let w = &app.design.waves[m.idx];
        let c = wave_color(w.kind);
        let sel = app.tl.wave_sel.contains(&w.id);
        let cy = m.y;
        if w.kind.is_boss() {
            let d = m.r * 1.5;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(m.x, cy - d),
                    pos2(m.x + d, cy),
                    pos2(m.x, cy + d),
                    pos2(m.x - d, cy),
                ],
                with_alpha(c, 0.95),
                Stroke::new(
                    1.0_f32,
                    if sel {
                        Color32::WHITE
                    } else {
                        with_alpha(base, 0.8)
                    },
                ),
            ));
        } else if w.kind == WaveKind::Special {
            painter.circle_filled(pos2(m.x, cy), m.r, c);
            painter.circle_stroke(
                pos2(m.x, cy),
                m.r + 2.5,
                Stroke::new(1.0_f32, with_alpha(c, 0.55)),
            );
            if sel {
                painter.circle_stroke(
                    pos2(m.x, cy),
                    m.r + 4.5,
                    Stroke::new(1.5_f32, Color32::WHITE),
                );
            }
        } else {
            let r = Rect::from_center_size(pos2(m.x, cy), vec2(m.r * 2.0, m.r * 2.0));
            painter.rect_filled(r, CornerRadius::same(2), with_alpha(c, 0.9));
            if sel {
                painter.rect_stroke(
                    r.expand(1.5),
                    CornerRadius::same(3),
                    Stroke::new(1.5_f32, Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    // 堆叠数量徽标
    for (x, n) in counts {
        if n < 2 {
            continue;
        }
        let txt = format!("×{n}");
        let galley = painter.layout_no_wrap(txt, font_id(8.5), TEXT);
        let w = galley.size().x + 5.0;
        let r = Rect::from_center_size(pos2(x, rect.top() + 7.0), vec2(w, 11.0));
        painter.rect_filled(r, CornerRadius::same(2), with_alpha(BG_APP, 0.85));
        painter.rect_stroke(
            r,
            CornerRadius::same(2),
            Stroke::new(1.0_f32, with_alpha(GOLD, 0.5)),
            egui::StrokeKind::Inside,
        );
        painter.galley(
            pos2(
                r.center().x - galley.size().x * 0.5,
                r.center().y - galley.size().y * 0.5,
            ),
            galley,
            GOLD,
        );
    }
}

fn note_color(app: &Studio, n: &crate::model::Note) -> Color32 {
    if !app.tl.color_by_beat {
        return if n.kind == NoteKind::Hold {
            PURPLE
        } else {
            GOLD
        };
    }
    let beat = app.design.beat();
    let pos = (n.time - app.phase) / beat.max(1e-6);
    let fr = pos - pos.floor();
    let in_bar = pos.rem_euclid(4.0);
    let bar_fr = in_bar - in_bar.floor();
    if fr < 0.03 || fr > 0.97 {
        if bar_fr < 0.03 || bar_fr > 0.97 {
            if n.kind == NoteKind::Hold {
                Color32::from_rgb(0xf2, 0xd6, 0x9a)
            } else {
                GOLD
            }
        } else if n.kind == NoteKind::Hold {
            PURPLE
        } else {
            Color32::from_rgb(0xb9, 0x9b, 0xe6)
        }
    } else if (fr - 0.5).abs() < 0.06 {
        TEAL
    } else if (fr - 0.333).abs() < 0.05 || (fr - 0.666).abs() < 0.05 {
        Color32::from_rgb(0x9a, 0xa8, 0xd0)
    } else {
        TEXT_FAINT
    }
}

fn draw_notes(app: &Studio, painter: &egui::Painter, rect: Rect, x_of: &impl Fn(f64) -> f32) {
    let sel = &app.tl.note_sel;
    let hover = app.tl.hover_note;
    let cy = rect.center().y;
    let hgt = (rect.height() - 10.0).max(12.0);
    let spacing = (app.design.beat() / 4.0 * app.tl.pps as f64) as f32;
    let nw = (spacing * 0.62).clamp(2.0, 13.0);
    let tap_h = (hgt * 0.44).clamp(8.0, 22.0);
    let hold_h = (hgt * 0.55).clamp(9.0, 30.0);
    for n in &app.design.chart {
        let x = x_of(n.time);
        if x < rect.left() - 20.0 {
            continue;
        }
        if x > rect.right() + 20.0 {
            break;
        }
        let c = note_color(app, n);
        let is_sel = sel.contains(&n.id);
        let is_hover = hover == Some(n.id);
        if n.kind == NoteKind::Hold {
            let xe = x_of(n.tail());
            let r = Rect::from_min_max(
                pos2(x, cy - hold_h * 0.5),
                pos2(xe.max(x + 2.0), cy + hold_h * 0.5),
            );
            painter.rect_filled(r, CornerRadius::same(2), with_alpha(c, 0.28));
            painter.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.0_f32, with_alpha(c, if is_sel { 1.0 } else { 0.75 })),
                egui::StrokeKind::Inside,
            );
            painter.rect_filled(
                Rect::from_center_size(pos2(x, cy), vec2(nw.max(2.5), hold_h)),
                CornerRadius::same(2),
                c,
            );
            if is_sel || is_hover {
                painter.rect_stroke(
                    r.expand(1.5),
                    CornerRadius::same(3),
                    Stroke::new(
                        1.5_f32,
                        if is_sel {
                            Color32::WHITE
                        } else {
                            with_alpha(Color32::WHITE, 0.45)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
        } else {
            let h = if n.accent { tap_h } else { tap_h * 0.72 };
            let r = Rect::from_center_size(pos2(x, cy), vec2(nw, h));
            painter.rect_filled(r, CornerRadius::same(1), c);
            if n.accent && nw > 4.0 {
                painter.rect_filled(
                    Rect::from_min_size(r.min, vec2(r.width(), 1.8)),
                    0.0,
                    with_alpha(Color32::WHITE, 0.8),
                );
            }
            if is_sel || is_hover {
                painter.rect_stroke(
                    r.expand(2.0),
                    CornerRadius::same(2),
                    Stroke::new(
                        1.5_f32,
                        if is_sel {
                            Color32::WHITE
                        } else {
                            with_alpha(Color32::WHITE, 0.45)
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }
    if let Some(id) = hover {
        if let Some(n) = app.design.chart.iter().find(|n| n.id == id) {
            let x = x_of(n.time);
            painter.line_segment(
                [pos2(x, rect.top()), pos2(x, rect.bottom())],
                Stroke::new(1.0_f32, with_alpha(Color32::WHITE, 0.22)),
            );
        }
    }
}

// 命中测试

fn note_at(app: &Studio, t: f64, tolerance: f64) -> Option<u64> {
    let mut best: Option<(f64, u64)> = None;
    for n in &app.design.chart {
        let d = if n.kind == NoteKind::Hold {
            if t >= n.time - tolerance && t <= n.tail() + tolerance {
                (t - n.time).abs().min((t - n.tail()).abs())
            } else {
                continue;
            }
        } else {
            let d = (t - n.time).abs();
            if d > tolerance {
                continue;
            }
            d
        };
        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, n.id));
        }
    }
    best.map(|(_, id)| id)
}

/// 在已经散开的标记里找最近的
fn wave_at_mark(marks: &[Mark], app: &Studio, pos: egui::Pos2) -> Option<u64> {
    let mut best: Option<(f32, u64)> = None;
    for m in marks {
        let d = pos2(m.x, m.y).distance(pos);
        if d <= m.r + 4.0 && best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, app.design.waves[m.idx].id));
        }
    }
    best.map(|(_, id)| id)
}

// 悬停反馈

fn hover_feedback(
    app: &mut Studio,
    ui: &mut Ui,
    lanes: &Lanes,
    resp: &egui::Response,
    x_of: &impl Fn(f64) -> f32,
) {
    let Some(p) = resp.hover_pos() else {
        app.tl.hover_note = None;
        app.tl.hover_wave = None;
        app.tl.hover_lane = None;
        app.tl.hover_time = None;
        return;
    };
    let t = app.tl.start + ((p.x - lanes.plot_l) / app.tl.pps.max(1.0)) as f64;
    app.tl.hover_time = Some(t);
    app.tl.hover_note = None;
    app.tl.hover_wave = None;
    app.tl.hover_lane = None;
    let tol = (8.0 / app.tl.pps.max(1.0) as f64).max(0.02);

    if let Some(r) = lanes.chart {
        if r.contains(p) {
            app.tl.hover_lane = Some(0);
            app.tl.hover_note = note_at(app, t, tol);
            ui.ctx().set_cursor_icon(if app.tl.hover_note.is_some() {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::PointingHand
            });
        }
    }
    for (route, r) in &lanes.routes {
        if r.contains(p) {
            app.tl.hover_lane = Some(*route + 1);
            let (marks, _) = layout_marks(app, *r, *route, x_of);
            app.tl.hover_wave = wave_at_mark(&marks, app, p);
            ui.ctx().set_cursor_icon(if app.tl.hover_wave.is_some() {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::PointingHand
            });
        }
    }
    if lanes.ruler.contains(p) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let tip = if let Some(id) = app.tl.hover_note {
        app.design.chart.iter().find(|n| n.id == id).map(|n| {
            let bar = app.design.bar_index(n.time, app.phase);
            format!(
                "{}\n{} · {:.3}s\n小节 {:.0}.{:.0}",
                n.kind.label(),
                fmt_time(n.time),
                n.time,
                bar.floor() + 1.0,
                (bar.fract() * 4.0).floor() + 1.0
            )
        })
    } else if let Some(id) = app.tl.hover_wave {
        app.design.waves.iter().find(|w| w.id == id).map(|w| {
            let st = app.design.wave_stats(w);
            let same = app
                .design
                .waves
                .iter()
                .filter(|x| x.route == w.route && (x.time - w.time).abs() < 0.02)
                .count();
            let mut s = format!(
                "{} · 路线 {}\n{:.2}s · 血量 {:.0} · 移速 {:.2} 格/s\n单程 {:.1}s · 漏过扣 {:.0}",
                w.kind.label(),
                w.route + 1,
                w.time,
                st.hp,
                st.speed,
                st.travel,
                st.damage
            );
            if same > 1 {
                s.push_str(&format!("\n同一时刻共 {same} 只"));
            }
            s
        })
    } else {
        None
    };
    if let Some(text) = tip {
        egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("tl_tip"), |ui| {
            ui.label(RichText::new(text).size(11.0).color(TEXT));
        });
    }
}

// 交互

fn interaction(app: &mut Studio, ui: &mut Ui, lanes: &Lanes, resp: &egui::Response) {
    let start = app.tl.start;
    let pps = app.tl.pps;
    let x_of = |t: f64| x_of(lanes, start, pps, t);
    let snap_step = app.snap.step(app.design.bpm);
    let phase = app.phase;
    let primary = egui::PointerButton::Primary;

    let Some(p) = resp.hover_pos() else {
        if resp.drag_stopped() {
            commit_drag(app);
        }
        return;
    };
    let raw_t = start + ((p.x - lanes.plot_l) / pps.max(1.0)) as f64;
    let t = app.design.snap_to(raw_t, snap_step, phase);
    let tol = (8.0 / pps.max(1.0) as f64).max(0.02);
    let shift = ui.ctx().input(|i| i.modifiers.shift);

    // 开始拖动
    if resp.drag_started_by(primary) {
        if lanes.ruler.contains(p) {
            app.seek(t);
        } else if app.tl.box_mode || shift {
            // 框选
            app.tl.drag = Some(Drag {
                kind: DragKind::Box,
                orig: Vec::new(),
                anchor_t: raw_t,
                start_pos: p,
                moved: false,
            });
            app.tl.box_cur = Some(p);
        } else if let Some(r) = lanes.chart {
            if r.contains(p) {
                if let Some(id) = note_at(app, raw_t, tol) {
                    start_note_drag(app, ui, id, raw_t, p);
                } else {
                    // 空白处直接拖拽 = 框选（原样松手不动的单击仍是放置音符）
                    app.tl.drag = Some(Drag {
                        kind: DragKind::Box,
                        orig: Vec::new(),
                        anchor_t: raw_t,
                        start_pos: p,
                        moved: false,
                    });
                    app.tl.box_cur = Some(p);
                }
            }
        }
        if app.tl.drag.is_none() {
            for (route, r) in &lanes.routes {
                if r.contains(p) {
                    let (marks, _) = layout_marks(app, *r, *route, &x_of);
                    if let Some(id) = wave_at_mark(&marks, app, p) {
                        if !app.tl.wave_sel.contains(&id) {
                            if !shift {
                                app.tl.wave_sel.clear();
                            }
                            app.tl.wave_sel.insert(id);
                        }
                        let ids: Vec<u64> = app.tl.wave_sel.iter().cloned().collect();
                        let orig = app
                            .design
                            .waves
                            .iter()
                            .filter(|w| ids.contains(&w.id))
                            .map(|w| (w.id, w.time, None))
                            .collect();
                        app.begin_edit("移动出怪");
                        app.tl.drag = Some(Drag {
                            kind: DragKind::WaveMove,
                            orig,
                            anchor_t: raw_t,
                            start_pos: p,
                            moved: false,
                        });
                    } else {
                        // 泳道空白处直接拖拽 = 框选
                        app.tl.drag = Some(Drag {
                            kind: DragKind::Box,
                            orig: Vec::new(),
                            anchor_t: raw_t,
                            start_pos: p,
                            moved: false,
                        });
                        app.tl.box_cur = Some(p);
                    }
                }
            }
        }
    }

    // 拖动中
    if resp.dragged_by(primary) {
        let Some(d) = app.tl.drag.clone() else {
            if lanes.ruler.contains(p) {
                app.seek(t);
            }
            return;
        };
        let moved = (p - d.start_pos).length() > 2.0;
        match d.kind {
            DragKind::Box => {
                app.tl.box_cur = Some(p);
            }
            DragKind::NoteMove => {
                let delta = app.design.snap_to(raw_t, snap_step, phase) - d.anchor_t;
                let dur = app.design.duration;
                for (id, otime, oend) in d.orig.clone() {
                    if let Some(n) = app.design.chart.iter_mut().find(|n| n.id == id) {
                        let nt = (otime + delta).clamp(0.0, dur);
                        let shift_n = nt - n.time;
                        n.time = nt;
                        if let (Some(e), Some(oe)) = (n.end.as_mut(), oend) {
                            *e = (oe + shift_n).clamp(0.0, dur);
                        }
                    }
                }
                app.touch();
            }
            DragKind::NoteTail => {
                let nt = app.design.snap_to(raw_t, snap_step, phase);
                for (id, otime, _) in d.orig.clone() {
                    if let Some(n) = app.design.chart.iter_mut().find(|n| n.id == id) {
                        n.end = Some(nt.clamp(otime + crate::model::MIN_HOLD, app.design.duration));
                    }
                }
                app.touch();
            }
            DragKind::WaveMove => {
                let delta = app.design.snap_to(raw_t, snap_step, phase) - d.anchor_t;
                let dur = app.design.duration;
                for (id, otime, _) in d.orig.clone() {
                    if let Some(w) = app.design.waves.iter_mut().find(|w| w.id == id) {
                        w.time = (otime + delta).clamp(0.0, dur);
                    }
                }
                app.touch();
            }
        }
        if moved {
            if let Some(dd) = app.tl.drag.as_mut() {
                dd.moved = true;
            }
        }
        return;
    }

    // 结束拖动
    if resp.drag_stopped() {
        commit_drag(app);
    }

    // 单击
    if resp.clicked_by(primary) {
        if lanes.ruler.contains(p) {
            app.seek(t);
            return;
        }
        if let Some(r) = lanes.chart {
            if r.contains(p) {
                if let Some(id) = note_at(app, raw_t, tol) {
                    if !shift {
                        app.tl.note_sel.clear();
                    }
                    app.tl.note_sel.insert(id);
                    app.sel_note = Some(id);
                    app.focus = Focus::Chart;
                } else {
                    app.add_note(t);
                    app.tl.note_sel.clear();
                    if let Some(n) = app.design.chart.iter().min_by(|a, b| {
                        (a.time - t)
                            .abs()
                            .partial_cmp(&(b.time - t).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        if (n.time - t).abs() < 1e-6 {
                            app.tl.note_sel.insert(n.id);
                            app.sel_note = Some(n.id);
                        }
                    }
                }
                return;
            }
        }
        for (route, r) in &lanes.routes {
            if r.contains(p) {
                let (marks, _) = layout_marks(app, *r, *route, &x_of);
                if let Some(id) = wave_at_mark(&marks, app, p) {
                    if !shift {
                        app.tl.wave_sel.clear();
                    }
                    app.tl.wave_sel.insert(id);
                    app.sel_wave = Some(id);
                    app.focus = Focus::Waves;
                } else {
                    app.add_wave(t, *route);
                    if let Some(w) = app.design.waves.iter().min_by(|a, b| {
                        (a.time - t)
                            .abs()
                            .partial_cmp(&(b.time - t).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }) {
                        if (w.time - t).abs() < 1e-6 && w.route == *route {
                            app.tl.wave_sel.clear();
                            app.tl.wave_sel.insert(w.id);
                            app.sel_wave = Some(w.id);
                        }
                    }
                }
                return;
            }
        }
        app.tl.note_sel.clear();
        app.tl.wave_sel.clear();
    }

    // 右键删除
    if resp.clicked_by(egui::PointerButton::Secondary) {
        if let Some(r) = lanes.chart {
            if r.contains(p) {
                if let Some(id) = note_at(app, raw_t, tol) {
                    app.begin_edit("删除音符");
                    app.design.chart.retain(|n| n.id != id);
                    app.tl.note_sel.remove(&id);
                    app.touch();
                    app.after_edit();
                    return;
                }
            }
        }
        for (route, r) in &lanes.routes {
            if r.contains(p) {
                let (marks, _) = layout_marks(app, *r, *route, &x_of);
                if let Some(id) = wave_at_mark(&marks, app, p) {
                    app.begin_edit("删除出怪");
                    app.design.waves.retain(|w| w.id != id);
                    app.tl.wave_sel.remove(&id);
                    app.touch();
                    app.after_edit();
                    return;
                }
            }
        }
    }

    // 双击：音符切换点按 / 长条
    if resp.double_clicked_by(primary) {
        if let Some(r) = lanes.chart {
            if r.contains(p) {
                if let Some(id) = note_at(app, raw_t, tol) {
                    app.toggle_hold_for(id);
                }
            }
        }
    }
}

fn start_note_drag(app: &mut Studio, ui: &mut Ui, id: u64, raw_t: f64, p: egui::Pos2) {
    let Some(n) = app.design.chart.iter().find(|n| n.id == id).cloned() else {
        return;
    };
    let on_tail = n.kind == NoteKind::Hold && (raw_t - n.tail()).abs() < (raw_t - n.time).abs();
    if !app.tl.note_sel.contains(&id) {
        if !ui.ctx().input(|i| i.modifiers.shift) {
            app.tl.note_sel.clear();
        }
        app.tl.note_sel.insert(id);
    }
    let ids: Vec<u64> = app.tl.note_sel.iter().cloned().collect();
    let orig = app
        .design
        .chart
        .iter()
        .filter(|n| ids.contains(&n.id))
        .map(|n| (n.id, n.time, n.end))
        .collect();
    app.begin_edit(if on_tail {
        "调整长条"
    } else {
        "移动音符"
    });
    app.tl.drag = Some(Drag {
        kind: if on_tail {
            DragKind::NoteTail
        } else {
            DragKind::NoteMove
        },
        orig,
        anchor_t: raw_t,
        start_pos: p,
        moved: false,
    });
}

fn commit_drag(app: &mut Studio) {
    let Some(d) = app.tl.drag.take() else { return };
    if d.kind == DragKind::Box {
        let cur = app.tl.box_cur.unwrap_or(d.start_pos);
        app.tl.box_cur = None;
        if !d.moved {
            return;
        }
        let r = Rect::from_two_pos(d.start_pos, cur);
        apply_box_select(app, r);
        return;
    }
    app.tl.box_cur = None;
    app.design.sort();
    app.after_edit();
}

/// 框选：音符按时间区间、出怪按标记位置判定。
fn apply_box_select(app: &mut Studio, band: Rect) {
    let (t0, t1) = time_range_of_band(app, band);
    // 只选「框真正扫过」的轨道：在谱面区拖框不会顺带选中出怪，反之亦然
    let (ry0, ry1) = app.tl.lanes_routes_y;
    let (cy0, cy1) = app.tl.lanes_chart_y;
    let vtouch = |a0: f32, a1: f32, b0: f32, b1: f32| a1.min(b1) - a0.max(b0) > 1.0;
    let pick_notes = vtouch(band.top(), band.bottom(), cy0, cy1);
    let pick_waves = vtouch(band.top(), band.bottom(), ry0, ry1);
    let mut n_sel = 0usize;
    for n in app.design.chart.iter().filter(|_| pick_notes) {
        let head_in = n.time >= t0 && n.time <= t1;
        let inside = match n.kind {
            NoteKind::Hold => {
                let end = n.tail();
                head_in && end <= t1 || (end >= t0 && end <= t1) || (n.time <= t0 && end >= t1)
            }
            NoteKind::Tap => head_in,
        };
        if inside {
            app.tl.note_sel.insert(n.id);
            n_sel += 1;
        }
    }
    let mut w_sel = 0usize;
    for w in app.design.waves.iter().filter(|_| pick_waves) {
        if w.time >= t0 && w.time <= t1 {
            app.tl.wave_sel.insert(w.id);
            w_sel += 1;
        }
    }
    if n_sel + w_sel > 0 {
        app.toast(format!(
            "框选：{n_sel} 音符 · {w_sel} 出怪（可批量量化 / 微调 / 改属性）"
        ));
    }
}

/// 把屏幕矩形换算成时间区间（用与绘制相同的映射，保证所见即所选）
fn time_range_of_band(app: &Studio, band: Rect) -> (f64, f64) {
    let plot_l = app.tl.band_plot_l;
    let pps = app.tl.pps.max(1.0) as f64;
    let a = app.tl.start + ((band.left() - plot_l) as f64) / pps;
    let b = app.tl.start + ((band.right() - plot_l) as f64) / pps;
    (a.min(b).max(0.0), a.max(b).max(0.0))
}

// 概览条

pub fn overview(app: &mut Studio, ui: &mut Ui) {
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), ui.available_height().max(22.0)),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(4), BG_INPUT);
    let dur = app.design.duration.max(0.1);
    let w = rect.width();
    let x_of = |t: f64| rect.left() + (t / dur).clamp(0.0, 1.0) as f32 * w;

    let bins = (w as usize).clamp(32, 1600);
    let hist = app.design.density_histogram(bins);
    let peak = hist.iter().cloned().fold(1.0f32, f32::max);
    let dx = w / bins as f32;
    for (i, v) in hist.iter().enumerate() {
        if *v <= 0.0 {
            continue;
        }
        let h = (v / peak) * (rect.height() - 10.0);
        let x = rect.left() + i as f32 * dx;
        painter.rect_filled(
            Rect::from_min_max(
                pos2(x, rect.bottom() - h - 2.0),
                pos2(x + dx.max(1.0), rect.bottom() - 2.0),
            ),
            0.0,
            with_alpha(GOLD, 0.55),
        );
    }
    for wave in &app.design.waves {
        let x = x_of(wave.time);
        let c = wave_color(wave.kind);
        painter.line_segment(
            [pos2(x, rect.top() + 2.0), pos2(x, rect.bottom() - 2.0)],
            Stroke::new(1.0_f32, with_alpha(c, 0.5)),
        );
    }
    let span = ((ui.available_width() - GUTTER) / app.tl.pps.max(1.0)) as f64;
    let a = x_of(app.tl.start);
    let b = x_of((app.tl.start + span).min(dur));
    painter.rect_filled(
        Rect::from_min_max(pos2(a, rect.top()), pos2(b, rect.bottom())),
        2.0,
        with_alpha(PURPLE, 0.14),
    );
    painter.rect_stroke(
        Rect::from_min_max(pos2(a, rect.top()), pos2(b, rect.bottom())),
        2.0,
        Stroke::new(1.0_f32, with_alpha(PURPLE, 0.6)),
        egui::StrokeKind::Inside,
    );
    let px = x_of(app.clock);
    painter.line_segment(
        [pos2(px, rect.top()), pos2(px, rect.bottom())],
        Stroke::new(1.6_f32, PLAYHEAD),
    );

    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let t = ((p.x - rect.left()) / w).clamp(0.0, 1.0) as f64 * dur;
            app.seek(t);
            let span = ((ui.available_width() - GUTTER) / app.tl.pps.max(1.0)) as f64;
            app.tl.start = (t - span * 0.5).clamp(0.0, (dur - span * 0.3).max(0.0));
        }
    }
}
