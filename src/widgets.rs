//! 通用控件：卡片、分区标题、胶囊、内嵌数值框、色板、芯片、统计格。

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Key, Layout, Margin, Response, RichText, Sense,
    Stroke, TextEdit, Ui,
};
use egui::{pos2, vec2};

use crate::theme::*;

// 卡片与分区

/// 带标题的小卡片。返回内容区结果。
pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let mut out = None;
    egui::Frame::NONE
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0_f32, LINE_SOFT))
        .corner_radius(CornerRadius::same(7))
        .inner_margin(Margin {
            left: 11,
            right: 11,
            top: 9,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            header(ui, title);
            out = Some(add(ui));
        });
    ui.add_space(7.0);
    out.unwrap()
}

/// 卡片标题：金色小方块 + 标题 + 右侧细线
pub fn header(ui: &mut Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(vec2(3.0, 11.0), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::same(2), GOLD);
        ui.label(RichText::new(title).size(11.0).color(TEXT).strong());
        let rest = ui.available_rect_before_wrap();
        if rest.width() > 12.0 {
            let y = rest.center().y.round();
            ui.painter().line_segment(
                [pos2(rest.left() + 2.0, y), pos2(rest.right(), y)],
                Stroke::new(1.0_f32, LINE_SOFT),
            );
        }
        ui.allocate_space(vec2(rest.width().max(0.0), 1.0));
    });
    ui.add_space(7.0);
}

/// 旧接口保留
pub fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    header(ui, title);
}

/// 小号说明文字
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(10.0).color(TEXT_FAINT));
}

// 数值框

#[derive(Clone, Copy)]
pub struct NumCfg {
    pub speed: f64,
    pub min: f64,
    pub max: f64,
    pub decimals: usize,
    pub suffix: &'static str,
    pub width: f32,
    pub accent: Color32,
}

impl Default for NumCfg {
    fn default() -> Self {
        Self {
            speed: 0.5,
            min: 0.0,
            max: 1e9,
            decimals: 1,
            suffix: "",
            width: 78.0,
            accent: GOLD,
        }
    }
}

impl NumCfg {
    pub fn new(
        min: f64,
        max: f64,
        speed: f64,
        decimals: usize,
        suffix: &'static str,
        width: f32,
    ) -> Self {
        Self {
            speed,
            min,
            max,
            decimals,
            suffix,
            width,
            accent: GOLD,
        }
    }

    pub fn with_accent(mut self, c: Color32) -> Self {
        self.accent = c;
        self
    }
}

/// 自绘数值框：横向拖动改变数值，双击输入。`cfg.width <= 0` 时填满可用宽度。
pub fn num_field(ui: &mut Ui, id: egui::Id, value: &mut f64, cfg: NumCfg) -> bool {
    let h = CTRL_H;
    let w = if cfg.width <= 0.0 {
        ui.available_width().max(40.0)
    } else {
        cfg.width
    };
    let cfg = NumCfg { width: w, ..cfg };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let edit_key = id.with("editbuf");
    let mut buf: Option<String> = ui.data(|d| d.get_temp(edit_key));
    let mut changed = false;
    let mut start_edit = false;

    if resp.double_clicked() && buf.is_none() {
        let s = format!("{:.*}", cfg.decimals, *value);
        ui.data_mut(|d| d.insert_temp(edit_key, s.clone()));
        buf = Some(s);
        start_edit = true;
    }
    if buf.is_none() && resp.dragged() {
        let (dx, shift, alt) =
            ui.input(|i| (i.pointer.delta().x, i.modifiers.shift, i.modifiers.alt));
        if dx != 0.0 {
            let mult = if shift {
                0.12
            } else if alt {
                8.0
            } else {
                1.0
            };
            let step_factor = if cfg.speed < 0.02 { 0.002 } else { cfg.speed };
            *value = (*value + dx as f64 * step_factor * mult).clamp(cfg.min, cfg.max);
            changed = true;
        }
    }
    if buf.is_none() && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }

    let hovered = resp.hovered();
    let painter = ui.painter();
    let bg = if hovered { BG_HOVER } else { BG_INPUT };
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(
            1.0_f32,
            if hovered {
                cfg.accent.gamma_multiply(0.55)
            } else {
                LINE
            },
        ),
        egui::StrokeKind::Inside,
    );

    if let Some(mut b) = buf {
        let te = TextEdit::singleline(&mut b)
            .font(mono(12.0))
            .horizontal_align(Align::RIGHT)
            .vertical_align(Align::Center)
            .margin(Margin::symmetric(6, 2))
            .desired_width(rect.width() - 2.0);
        let tr = ui.put(rect.shrink(1.0), te);
        if start_edit {
            tr.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if enter || tr.lost_focus() {
            if let Ok(v) = b.trim().replace(',', "").parse::<f64>() {
                *value = v.clamp(cfg.min, cfg.max);
                changed = true;
            }
            ui.data_mut(|d| d.remove_temp::<String>(edit_key));
        } else {
            ui.data_mut(|d| d.insert_temp(edit_key, b));
        }
    } else {
        let text = format!("{:.*}", cfg.decimals, *value);
        let font = mono(12.0);
        let suf_font = FontId::proportional(9.5);
        let painter = ui.painter();
        let mut right = rect.right() - 7.0;
        if !cfg.suffix.is_empty() {
            let g = painter.layout_no_wrap(cfg.suffix.to_string(), suf_font, TEXT_FAINT);
            let w = g.size().x;
            painter.galley(
                pos2(right - w, rect.center().y - g.size().y * 0.5),
                g,
                TEXT_FAINT,
            );
            right -= w + 3.0;
        }
        let g = painter.layout_no_wrap(text, font, TEXT);
        let w = g.size().x;
        painter.galley(pos2(right - w, rect.center().y - g.size().y * 0.5), g, TEXT);
    }
    changed
}

/// 「标签在上、数值框在下」的紧凑字段
pub fn labeled_num(ui: &mut Ui, label: &str, id: egui::Id, value: &mut f64, cfg: NumCfg) -> bool {
    let mut changed = false;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.label(RichText::new(label).size(9.5).color(TEXT_DIM));
        changed = num_field(ui, id, value, cfg);
    });
    changed
}

// 按钮与胶囊

pub fn pill(ui: &mut Ui, text: &str, active: bool, accent: Color32) -> Response {
    let font = font_id(11.5);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), TEXT);
    let pad = vec2(9.0, 4.0);
    let size = vec2(galley.size().x + pad.x * 2.0, CTRL_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let (fill, stroke, color) = if active {
        (
            with_alpha(accent, 0.20),
            Stroke::new(1.0_f32, accent.gamma_multiply(0.75)),
            Color32::WHITE,
        )
    } else if hovered {
        (BG_HOVER, Stroke::new(1.0_f32, LINE), TEXT)
    } else {
        (
            Color32::TRANSPARENT,
            Stroke::new(1.0_f32, Color32::TRANSPARENT),
            TEXT_DIM,
        )
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(4), fill);
    if stroke.width > 0.0 && (active || hovered) {
        p.rect_stroke(
            rect,
            CornerRadius::same(4),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        color,
    );
    resp
}

/// 所有按钮 / 数值框的统一高度。同一行里的控件高度一致才不会参差不齐。
pub const CTRL_H: f32 = 24.0;

/// 通用自绘按钮：高度固定为 CTRL_H，文字垂直居中，样式不随系统主题漂移。
pub fn chip_button(
    ui: &mut Ui,
    text: &str,
    fill: Color32,
    border: Color32,
    label: Color32,
) -> Response {
    let font = font_id(11.5);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), label);
    let pad_x = 10.0;
    let size = vec2(galley.size().x + pad_x * 2.0, CTRL_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let (fill, border, label) = if resp.hovered() {
        (
            with_alpha(border, 0.34),
            Stroke::new(1.0_f32, border),
            Color32::WHITE,
        )
    } else {
        (fill, Stroke::new(1.0_f32, border), label)
    };
    p.rect_filled(rect, CornerRadius::same(5), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(5),
        border,
        egui::StrokeKind::Inside,
    );
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        label,
    );
    resp
}

pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    chip_button(ui, text, GOLD, GOLD, Color32::from_rgb(0x1a, 0x12, 0x24))
}

/// 浅紫色实心按钮（用于「载入音频」这类强调入口）
pub fn accent_button(ui: &mut Ui, text: &str) -> Response {
    chip_button(
        ui,
        text,
        with_alpha(LILAC.border, 0.30),
        LILAC.border,
        Color32::WHITE,
    )
}

/// 浅紫色描边按钮：替换白色 / 深紫底的原生按钮，样式写死不随主题漂移。
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    chip_button(ui, text, LILAC.fill, LILAC.border, LILAC.text)
}

/// 紧凑版浅紫按钮（替换 ui.small_button，同样写死样式、统一高度）
pub fn soft_button(ui: &mut Ui, text: impl Into<String>) -> Response {
    let t = text.into();
    let font = font_id(10.5);
    let galley = ui
        .painter()
        .layout_no_wrap(t.clone(), font.clone(), LILAC.text);
    let pad_x = 7.0;
    let size = vec2(galley.size().x + pad_x * 2.0, CTRL_H - 2.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    let (fill, border, tcol) = if resp.hovered() {
        (
            with_alpha(LILAC.border, 0.30),
            Stroke::new(1.0_f32, with_alpha(LILAC.border, 0.9)),
            Color32::WHITE,
        )
    } else {
        (LILAC.fill, Stroke::new(1.0_f32, LILAC.border), LILAC.text)
    };
    p.rect_filled(rect, CornerRadius::same(4), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        border,
        egui::StrokeKind::Inside,
    );
    p.text(rect.center(), egui::Align2::CENTER_CENTER, t, font, tcol);
    resp
}

/// 带颜色的开关按钮：开 = 彩色底 + 白字，关 = 描边 + 灰字。
/// 取代复选框，开关状态一眼可辨。左侧是「轨道滑块」式开关头，不遮文字。
pub fn toggle(ui: &mut Ui, text: &str, on: bool, accent: Color32) -> Response {
    let font = font_id(11.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), TEXT);
    let dot_d = 7.0; // 开关头直径
    let pad_x = 6.0;
    let gap = 5.0; // 开关头与文字的间距
    let size = vec2(pad_x * 2.0 + dot_d + gap + galley.size().x, CTRL_H - 2.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    let p = ui.painter();
    let (fill, stroke, color, knob_fill, knob_border) = if on {
        (
            with_alpha(accent, 0.22),
            Stroke::new(1.0_f32, accent.gamma_multiply(0.8)),
            Color32::WHITE,
            accent,
            Stroke::new(1.0_f32, with_alpha(Color32::WHITE, 0.65)),
        )
    } else if hovered {
        (
            BG_HOVER,
            Stroke::new(1.0_f32, LINE),
            TEXT,
            BG_INPUT,
            Stroke::new(1.0_f32, TEXT_FAINT),
        )
    } else {
        (
            with_alpha(BG_RAISED, 0.55),
            Stroke::new(1.0_f32, LINE),
            TEXT_DIM,
            BG_INPUT,
            Stroke::new(1.0_f32, LINE),
        )
    };
    p.rect_filled(rect, CornerRadius::same(10), fill);
    p.rect_stroke(
        rect,
        CornerRadius::same(10),
        stroke,
        egui::StrokeKind::Inside,
    );
    // 开关头：实心圆 + 描边（更像开关，比裸圆点醒目且不会压住文字）
    let c = pos2(rect.left() + pad_x + dot_d * 0.5, rect.center().y);
    p.circle_filled(c, dot_d * 0.5, knob_fill);
    p.circle_stroke(c, dot_d * 0.5, knob_border);
    if on {
        // 亮起时的光晕
        p.circle_stroke(
            c,
            dot_d * 0.5 + 1.8,
            Stroke::new(1.2_f32, with_alpha(accent, 0.35)),
        );
    }
    // 文字在开关头右侧，从精确位置开始画，永不重叠
    p.text(
        pos2(rect.left() + pad_x + dot_d + gap, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        font,
        color,
    );
    resp
}

// 色板 / 统计 / 芯片

/// 一排配色圆点，返回被点中的索引。选中圈与圆心严格同心。
pub fn palette_dots(ui: &mut Ui, current: &str, accent: Color32) -> Option<usize> {
    let mut hit = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 7.0;
        for (i, pal) in crate::model::PALETTES.iter().enumerate() {
            let selected = current == pal.id;
            let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
            let p = ui.painter();
            let center = rect.center();
            // 所有圆严格共用一个圆心：外圈 = 内点半径 + 2.75px 空隙
            p.circle_filled(center, 8.0, hex(pal.tile));
            p.circle_filled(center, 4.5, hex(pal.accent));
            if selected {
                p.circle_stroke(center, 10.75, Stroke::new(1.5_f32, accent));
            } else if resp.hovered() {
                p.circle_stroke(center, 10.0, Stroke::new(1.0_f32, LINE));
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let resp = resp.on_hover_text(pal.name);
            if resp.clicked() {
                hit = Some(i);
            }
        }
    });
    hit
}

/// 统计小格：大数字 + 标签
pub fn stat_tile(ui: &mut Ui, value: &str, label: &str, color: Color32) {
    let w = ui.available_width();
    egui::Frame::NONE
        .fill(BG_INPUT)
        .stroke(Stroke::new(1.0_f32, LINE_SOFT))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 5,
            bottom: 6,
        })
        .show(ui, |ui| {
            ui.set_width(w - 16.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(value)
                    .size(16.0)
                    .color(color)
                    .strong()
                    .monospace(),
            );
            ui.label(RichText::new(label).size(9.0).color(TEXT_FAINT));
        });
}

/// 问题芯片
pub fn issue_chip(ui: &mut Ui, text: &str, color: Color32, icon: &str) -> bool {
    let resp = ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new(icon).size(10.0).color(color));
        ui.label(RichText::new(text).size(10.0).color(color));
    });
    resp.response.interact(Sense::click()).clicked()
}

pub fn right_aligned<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(Layout::right_to_left(Align::Center), add)
        .inner
}
