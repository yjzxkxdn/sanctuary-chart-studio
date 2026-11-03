//! 视觉主题：深紫金 · 专业编排台配色、字体装载与控件样式、
use eframe::egui;
use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke,
    TextStyle, Visuals,
};
use std::sync::Arc;

// 调色板

pub const BG_APP: Color32 = Color32::from_rgb(0x0b, 0x08, 0x10);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x14, 0x10, 0x1c);
pub const BG_CARD: Color32 = Color32::from_rgb(0x1b, 0x16, 0x26);
pub const BG_RAISED: Color32 = Color32::from_rgb(0x24, 0x1d, 0x31);
pub const BG_INPUT: Color32 = Color32::from_rgb(0x11, 0x0d, 0x19);
pub const BG_HOVER: Color32 = Color32::from_rgb(0x2e, 0x25, 0x40);
pub const LINE: Color32 = Color32::from_rgb(0x33, 0x2a, 0x44);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x25, 0x1e, 0x33);

pub const TEXT: Color32 = Color32::from_rgb(0xee, 0xe9, 0xf6);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xb2, 0xaa, 0xcc);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x8c, 0x83, 0xa4);

pub const GOLD: Color32 = Color32::from_rgb(0xd8, 0xbc, 0x86);
pub const PURPLE: Color32 = Color32::from_rgb(0xc5, 0xa3, 0xee);
pub const PURPLE_DEEP: Color32 = Color32::from_rgb(0x6c, 0x4f, 0x9c);
pub const TEAL: Color32 = Color32::from_rgb(0x7f, 0xc9, 0xc1);
pub const OK: Color32 = Color32::from_rgb(0x7f, 0xb9, 0x8a);
pub const WARN: Color32 = Color32::from_rgb(0xe0, 0xb1, 0x5e);
pub const ERR: Color32 = Color32::from_rgb(0xe0, 0x7a, 0x7a);
pub const PLAYHEAD: Color32 = Color32::from_rgb(0xf0, 0x9d, 0x5b);

/// 统一的浅紫色按钮配色（填色 / 描边 / 文字）
pub struct BtnStyle {
    pub fill: Color32,
    pub border: Color32,
    pub text: Color32,
}
pub const LILAC: BtnStyle = BtnStyle {
    fill: Color32::from_rgba_premultiplied(0x0a, 0x09, 0x0e, 26), // 淡浅紫底（预乘）
    border: Color32::from_rgb(0xa8, 0x8c, 0xd8),                  // 浅紫描边
    text: Color32::from_rgb(0xd6, 0xc9, 0xf2),                    // 亮浅紫文字
};

// 字体

pub fn font_id(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

const FONT_CANDIDATES: &[(&str, u32)] = &[
    (r"C:\Windows\Fonts\msyh.ttc", 0),
    (r"C:\Windows\Fonts\Deng.ttf", 0),
    (r"C:\Windows\Fonts\simhei.ttf", 0),
    (r"C:\Windows\Fonts\msjh.ttc", 0),
    (r"C:\Windows\Fonts\simsun.ttc", 0),
    (r"/System/Library/Fonts/PingFang.ttc", 0),
    (r"/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    (
        r"/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        0,
    ),
];

fn load_ui_fonts() -> Vec<(String, FontData)> {
    let mut out = Vec::new();
    let make = |bytes: Vec<u8>, index: u32| {
        let mut fd = FontData::from_owned(bytes);
        fd.index = index;
        fd
    };
    // 程序同目录 assets/font.*（可自行替换成任意中文字体）
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["assets/font.ttf", "assets/font.otf", "assets/font.ttc"] {
                let p = dir.join(name);
                if let Ok(bytes) = std::fs::read(&p) {
                    out.push(("asset_font".to_string(), make(bytes, 0)));
                    break;
                }
            }
        }
    }
    // 系统字体
    for (i, (path, index)) in FONT_CANDIDATES.iter().enumerate() {
        if let Ok(bytes) = std::fs::read(path) {
            out.push((format!("sys_font_{i}"), make(bytes, *index)));
        }
    }
    out
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let loaded = load_ui_fonts();
    let names: Vec<String> = loaded.iter().map(|(n, _)| n.clone()).collect();
    for (name, data) in loaded {
        fonts.font_data.insert(name, Arc::new(data));
    }
    let prop = fonts.families.entry(FontFamily::Proportional).or_default();
    for (i, n) in names.iter().enumerate() {
        prop.insert(i, n.clone());
    }
    let mono_family = fonts.families.entry(FontFamily::Monospace).or_default();
    for n in names.iter() {
        mono_family.push(n.clone());
    }
    ctx.set_fonts(fonts);
}

// 样式

pub fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals = Visuals::dark();
    let v = &mut style.visuals;
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG_APP;
    v.window_fill = BG_PANEL;
    v.extreme_bg_color = BG_INPUT;
    v.faint_bg_color = BG_CARD;
    v.window_stroke = Stroke::new(1.0_f32, LINE);
    v.selection.bg_fill = PURPLE_DEEP.gamma_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0_f32, PURPLE);
    v.hyperlink_color = GOLD;
    v.widgets.noninteractive.bg_fill = BG_CARD;
    v.widgets.noninteractive.weak_bg_fill = BG_CARD;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE_SOFT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_DIM);
    v.widgets.noninteractive.corner_radius = CornerRadius::same(4);
    v.widgets.inactive.bg_fill = BG_RAISED;
    v.widgets.inactive.weak_bg_fill = BG_RAISED;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.inactive.corner_radius = CornerRadius::same(4);
    v.widgets.hovered.bg_fill = BG_HOVER;
    v.widgets.hovered.weak_bg_fill = BG_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, PURPLE_DEEP);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.hovered.corner_radius = CornerRadius::same(4);
    v.widgets.active.bg_fill = PURPLE_DEEP;
    v.widgets.active.weak_bg_fill = PURPLE_DEEP;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, PURPLE);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.active.corner_radius = CornerRadius::same(4);
    v.widgets.open.bg_fill = BG_HOVER;
    v.widgets.open.corner_radius = CornerRadius::same(4);

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(0.0, 24.0);
    style.spacing.slider_width = 150.0;
    style.spacing.combo_width = 120.0;
    style.spacing.indent = 16.0;
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.menu_margin = Margin::same(6);

    style.text_styles = [
        (TextStyle::Small, font_id(11.0)),
        (TextStyle::Body, font_id(13.0)),
        (TextStyle::Button, font_id(13.0)),
        (TextStyle::Heading, font_id(17.0)),
        (TextStyle::Monospace, mono(12.5)),
    ]
    .into();

    ctx.set_style(style);
}

/// 每帧兜底：一旦发现 egui 被切回亮色（例如系统主题覆盖），立刻恢复深色。
/// 只在必要时 set_style，避免每帧重置动画状态。
pub fn ensure_dark(ctx: &egui::Context) {
    let dark = ctx.style().visuals.dark_mode;
    if !dark {
        install_style(ctx);
    }
}

// 绘制小工具

/// 把 #rrggbb 转成 egui 颜色
pub fn hex(s: &str) -> Color32 {
    let t = s.trim_start_matches('#');
    if t.len() >= 6 {
        let r = u8::from_str_radix(&t[0..2], 16).unwrap_or(0x80);
        let g = u8::from_str_radix(&t[2..4], 16).unwrap_or(0x80);
        let b = u8::from_str_radix(&t[4..6], 16).unwrap_or(0x80);
        Color32::from_rgb(r, g, b)
    } else {
        Color32::GRAY
    }
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).clamp(0.0, 255.0) as u8)
}
