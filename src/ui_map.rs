//! 「地形编辑」窗口：10×7 战场网格、路线绘制、障碍、实时行军预览。

use eframe::egui::{self, Color32, CornerRadius, Margin, Rect, RichText, Sense, Stroke, Ui};
use egui::{pos2, vec2};

use crate::app::Studio;
use crate::model::{COLS, ROWS};
use crate::theme::*;
use crate::widgets::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MapTool {
    Route,
    Block,
    Erase,
    Inspect,
}

impl MapTool {
    pub fn label(self) -> &'static str {
        match self {
            MapTool::Route => "画路线",
            MapTool::Block => "障碍",
            MapTool::Erase => "橡皮",
            MapTool::Inspect => "查看",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            MapTool::Route => "从左侧入口条按住拖动；点回上一格可回退",
            MapTool::Block => "点击格子切换是否不可建造（不能压在路线上）",
            MapTool::Erase => "点击移除该格的路线 / 障碍",
            MapTool::Inspect => "只查看，不修改",
        }
    }
}

pub struct MapState {
    pub tool: MapTool,
    pub active_route: usize,
    pub show_flow: bool,
    pub show_coords: bool,
    pub show_labels: bool,
    pub show_enemies: bool,
    pub drag_last: Option<(i32, i32)>,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            tool: MapTool::Route,
            active_route: 0,
            show_flow: true,
            show_coords: true,
            show_labels: true,
            show_enemies: true,
            drag_last: None,
        }
    }
}

enum Action {
    Append(i32, i32),
    TrimTo(usize),
    Erase(i32, i32),
    ToggleBlock(i32, i32),
    SetRoute(usize),
    AddRoute,
    RemoveRoute,
    ClearRoute,
    AutoRoute,
}

/// 窗口主体
pub fn editor(app: &mut Studio, ui: &mut Ui) {
    let mut actions: Vec<Action> = Vec::new();
    egui::SidePanel::right("map_side")
        .resizable(false)
        .exact_width(220.0)
        .frame(
            egui::Frame::NONE
                .fill(BG_CARD)
                .inner_margin(Margin::symmetric(10, 8))
                .corner_radius(CornerRadius::same(6)),
        )
        .show_inside(ui, |ui| {
            inspector(app, ui, &mut actions);
        });
    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(BG_INPUT)
                .inner_margin(Margin::same(8)),
        )
        .show_inside(ui, |ui| {
            toolbar(app, ui, &mut actions);
            ui.add_space(4.0);
            canvas(app, ui, &mut actions);
        });
    for a in actions {
        apply(app, a);
    }
}

fn toolbar(app: &mut Studio, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("路线").size(11.0).color(TEXT_DIM));
        for i in 0..app.design.paths.len() {
            let label = format!("{} · {} 格", i + 1, app.design.paths[i].len());
            if pill(ui, &label, app.map.active_route == i, GOLD).clicked() {
                actions.push(Action::SetRoute(i));
            }
        }
        if app.design.paths.len() < 3 && accent_button(ui, "+ 路线").clicked() {
            actions.push(Action::AddRoute);
        }
        ui.separator();
        for t in [
            MapTool::Route,
            MapTool::Block,
            MapTool::Erase,
            MapTool::Inspect,
        ] {
            if pill(ui, t.label(), app.map.tool == t, PURPLE).clicked() {
                app.map.tool = t;
            }
        }
        right_aligned(ui, |ui| {
            if ghost_button(ui, "自动铺路").clicked() {
                actions.push(Action::AutoRoute);
            }
            if ghost_button(ui, "清空").clicked() {
                actions.push(Action::ClearRoute);
            }
            if app.design.paths.len() > 1 && ghost_button(ui, "删除").clicked() {
                actions.push(Action::RemoveRoute);
            }
        });
    });
    ui.label(
        RichText::new(app.map.tool.hint())
            .size(10.0)
            .color(TEXT_FAINT),
    );
}

fn canvas(app: &mut Studio, ui: &mut Ui, actions: &mut Vec<Action>) {
    let avail = ui.available_size();
    let entry_w = 30.0;
    let label_h = if app.map.show_coords { 16.0 } else { 0.0 };
    let footer = 94.0;
    let cell = ((avail.x - entry_w - 12.0) / COLS as f32)
        .min((avail.y - label_h - footer) / ROWS as f32)
        .clamp(24.0, 110.0);
    let grid = vec2(cell * COLS as f32 + entry_w, cell * ROWS as f32 + label_h);
    let (rect, resp) = ui.allocate_exact_size(
        vec2(avail.x.max(grid.x + 8.0), (grid.y + footer).max(avail.y)),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let pal = app.design.palette();

    let origin = pos2(rect.left() + entry_w + 4.0, rect.top() + label_h + 2.0);
    let cell_rect = |x: i32, y: i32| {
        Rect::from_min_size(
            pos2(origin.x + x as f32 * cell, origin.y + y as f32 * cell),
            vec2(cell - 2.0, cell - 2.0),
        )
    };
    let entry_rect = Rect::from_min_max(
        pos2(rect.left() + 4.0, origin.y),
        pos2(origin.x - 6.0, origin.y + ROWS as f32 * cell - 2.0),
    );

    painter.rect_filled(rect, CornerRadius::same(8), hex(pal.bg[0]));
    painter.rect_filled(entry_rect, CornerRadius::same(4), hex(pal.bg[1]));
    painter.text(
        pos2(entry_rect.center().x, entry_rect.top() - 2.0),
        egui::Align2::CENTER_BOTTOM,
        "入口",
        font_id(9.0),
        TEXT_FAINT,
    );

    for y in 0..ROWS {
        for x in 0..COLS {
            let r = cell_rect(x, y);
            painter.rect_filled(r, CornerRadius::same(3), hex(pal.tile).gamma_multiply(0.62));
            painter.rect_stroke(
                r,
                CornerRadius::same(3),
                Stroke::new(1.0_f32, with_alpha(hex(pal.bg[0]), 0.6)),
                egui::StrokeKind::Inside,
            );
        }
    }
    for b in &app.design.blocked {
        let r = cell_rect(b[0], b[1]);
        painter.rect_filled(
            r.shrink(1.0),
            CornerRadius::same(3),
            with_alpha(BG_APP, 0.62),
        );
        hatch(&painter, r.shrink(4.0), with_alpha(TEXT_FAINT, 0.35));
        painter.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "✕",
            font_id((cell * 0.3).clamp(8.0, 14.0)),
            with_alpha(TEXT_FAINT, 0.75),
        );
    }

    let route_colors = [GOLD, PURPLE, TEAL];
    for (ri, path) in app.design.paths.iter().enumerate() {
        let color = route_colors[ri % route_colors.len()];
        let a = if ri == app.map.active_route { 1.0 } else { 0.5 };
        if path.len() < 2 {
            continue;
        }
        let center = |c: &[i32; 2]| -> egui::Pos2 {
            if c[0] < 0 {
                pos2(
                    entry_rect.center().x,
                    origin.y + (c[1] as f32 + 0.5) * cell - 1.0,
                )
            } else {
                pos2(
                    origin.x + (c[0] as f32 + 0.5) * cell - 1.0,
                    origin.y + (c[1] as f32 + 0.5) * cell - 1.0,
                )
            }
        };
        for w in path.windows(2) {
            painter.line_segment(
                [center(&w[0]), center(&w[1])],
                Stroke::new((cell * 0.30).clamp(7.0, 24.0), with_alpha(color, 0.22 * a)),
            );
        }
    }
    for (ri, path) in app.design.paths.iter().enumerate() {
        let color = route_colors[ri % route_colors.len()];
        let a = if ri == app.map.active_route {
            1.0
        } else {
            0.55
        };
        for (i, c) in path.iter().enumerate() {
            let r = if c[0] < 0 {
                Rect::from_center_size(
                    pos2(
                        entry_rect.center().x,
                        origin.y + (c[1] as f32 + 0.5) * cell - 1.0,
                    ),
                    vec2(entry_rect.width() - 5.0, cell - 5.0),
                )
            } else {
                cell_rect(c[0], c[1])
            };
            painter.rect_filled(
                r,
                CornerRadius::same(3),
                with_alpha(hex(pal.road), 0.94 * a),
            );
            painter.rect_stroke(
                r,
                CornerRadius::same(3),
                Stroke::new(1.0_f32, with_alpha(color, 0.6 * a)),
                egui::StrokeKind::Inside,
            );
            if app.map.show_labels && cell > 30.0 {
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", i + 1),
                    font_id((cell * 0.28).clamp(8.0, 14.0)),
                    with_alpha(Color32::WHITE, 0.8 * a),
                );
            }
            if app.map.show_flow && i + 1 < path.len() && cell > 36.0 {
                let c1 = path[i];
                let c2 = path[i + 1];
                if c1[0] >= 0 && c2[0] >= 0 {
                    let p1 = cell_rect(c1[0], c1[1]).center();
                    let p2 = cell_rect(c2[0], c2[1]).center();
                    let dir = (p2 - p1).normalized();
                    let mid = p1 + (p2 - p1) * 0.5;
                    let n = vec2(-dir.y, dir.x) * (cell * 0.09);
                    let tip = mid + dir * (cell * 0.13);
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            tip,
                            mid - dir * (cell * 0.08) + n,
                            mid - dir * (cell * 0.08) - n,
                        ],
                        with_alpha(color, 0.8 * a),
                        Stroke::NONE,
                    ));
                }
            }
        }
        if let Some(last) = path.last() {
            if last[0] >= 0 {
                let r = cell_rect(last[0], last[1]);
                painter.circle_filled(r.center(), cell * 0.24, with_alpha(GOLD, 0.92 * a));
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "✿",
                    font_id((cell * 0.26).clamp(8.0, 13.0)),
                    Color32::from_rgb(0x2a, 0x1c, 0x12),
                );
            }
        }
    }

    // 当前位置的怪物
    if app.map.show_enemies {
        for e in app.design.enemies_alive_at(app.clock) {
            let c = crate::app::wave_color(e.kind);
            // 连续映射：x<0 时按比例外推到入口外侧，怪物平滑走进来
            let p = pos2(
                origin.x + (e.x + 0.5) * cell - 1.0,
                origin.y + (e.y + 0.5) * cell - 1.0,
            );
            painter.circle_filled(p, (cell * 0.22).clamp(4.0, 12.0), c);
            painter.circle_stroke(
                p,
                (cell * 0.22).clamp(4.0, 12.0),
                Stroke::new(1.0_f32, with_alpha(Color32::WHITE, 0.55)),
            );
        }
    }

    // 悬停
    let hovered = resp.hover_pos().and_then(|p| {
        if entry_rect.expand2(vec2(4.0, 0.0)).contains(p) {
            let y = ((p.y - origin.y) / cell).floor() as i32;
            return if (0..ROWS).contains(&y) {
                Some((-1, y))
            } else {
                None
            };
        }
        let x = ((p.x - origin.x) / cell).floor() as i32;
        let y = ((p.y - origin.y) / cell).floor() as i32;
        if (0..COLS).contains(&x) && (0..ROWS).contains(&y) {
            Some((x, y))
        } else {
            None
        }
    });
    if let Some((x, y)) = hovered {
        let r = if x < 0 {
            Rect::from_center_size(
                pos2(
                    entry_rect.center().x,
                    origin.y + (y as f32 + 0.5) * cell - 1.0,
                ),
                vec2(entry_rect.width() - 5.0, cell - 5.0),
            )
        } else {
            cell_rect(x, y)
        };
        let color = match app.map.tool {
            MapTool::Route => GOLD,
            MapTool::Block => WARN,
            MapTool::Erase => ERR,
            MapTool::Inspect => TEAL,
        };
        painter.rect_stroke(
            r,
            CornerRadius::same(3),
            Stroke::new(2.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }

    if app.map.show_coords {
        for x in 0..COLS {
            painter.text(
                pos2(origin.x + (x as f32 + 0.5) * cell - 1.0, origin.y - 3.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{x}"),
                font_id(8.5),
                TEXT_FAINT,
            );
        }
        for y in 0..ROWS {
            painter.text(
                pos2(rect.left() + 12.0, origin.y + (y as f32 + 0.5) * cell - 1.0),
                egui::Align2::CENTER_CENTER,
                format!("{y}"),
                font_id(8.5),
                TEXT_FAINT,
            );
        }
    }

    // footer
    let info_top = origin.y + ROWS as f32 * cell + 8.0;
    let active = app
        .map
        .active_route
        .min(app.design.paths.len().saturating_sub(1));
    let path = app.design.paths.get(active);
    let len = path.map(|p| p.len()).unwrap_or(0);
    let travel = (len.max(2) - 1) as f64 / app.design.speed.max(0.01);
    painter.text(
        pos2(rect.left() + 6.0, info_top),
        egui::Align2::LEFT_TOP,
        format!(
            "路线 {} · {len} 格 · 单程约 {travel:.1}s · 障碍 {} 格",
            active + 1,
            app.design.blocked.len()
        ),
        font_id(11.0),
        TEXT_DIM,
    );
    if let Some(p) = path {
        let text = p
            .iter()
            .map(|c| format!("{},{}", c[0], c[1]))
            .collect::<Vec<_>>()
            .join(" → ");
        painter.text(
            pos2(rect.left() + 6.0, info_top + 17.0),
            egui::Align2::LEFT_TOP,
            truncate(&text, 120),
            font_id(9.5),
            TEXT_FAINT,
        );
    }
    let line3 = match hovered {
        Some((x, y)) => {
            let on_road = app.design.paths.iter().position(|p| p.contains(&[x, y]));
            let blocked = app.design.is_blocked(x, y);
            if let Some(r) = on_road {
                format!("({x},{y}) 位于路线 {} 上", r + 1)
            } else if blocked {
                format!("({x},{y}) 不可建造")
            } else {
                format!("({x},{y}) 可建造空地")
            }
        }
        None => "从入口条开始绘制，最后一格即守卫目标".to_string(),
    };
    painter.text(
        pos2(rect.left() + 6.0, info_top + 34.0),
        egui::Align2::LEFT_TOP,
        line3,
        font_id(9.5),
        with_alpha(GOLD, 0.85),
    );

    handle(app, &resp, hovered, actions);
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n).collect::<String>() + " …"
    }
}

fn hatch(painter: &egui::Painter, rect: Rect, color: Color32) {
    let step = 7.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        let a = pos2(
            x.max(rect.left()),
            if x < rect.left() {
                rect.top() + (rect.left() - x)
            } else {
                rect.top()
            },
        );
        let b = pos2(
            (x + rect.height()).min(rect.right()),
            if x + rect.height() > rect.right() {
                rect.bottom() - (x + rect.height() - rect.right())
            } else {
                rect.bottom()
            },
        );
        painter.line_segment([a, b], Stroke::new(1.0_f32, color));
        x += step;
    }
}

fn can_append(path: &[[i32; 2]], cell: (i32, i32)) -> bool {
    match path.last() {
        None => true,
        Some(last) => (last[0] - cell.0).abs() + (last[1] - cell.1).abs() == 1,
    }
}

fn handle(
    app: &mut Studio,
    resp: &egui::Response,
    hover: Option<(i32, i32)>,
    actions: &mut Vec<Action>,
) {
    let dragging = resp.dragged_by(egui::PointerButton::Primary);
    let started = resp.drag_started_by(egui::PointerButton::Primary)
        || resp.clicked_by(egui::PointerButton::Primary);
    let secondary = resp.clicked_by(egui::PointerButton::Secondary);
    if resp.drag_stopped() || resp.clicked() {
        app.map.drag_last = None;
    }
    if !(started || dragging || secondary) {
        return;
    }
    let Some(cell) = hover else { return };
    if secondary || app.map.tool == MapTool::Erase {
        if app.map.drag_last != Some(cell) {
            app.map.drag_last = Some(cell);
            actions.push(Action::Erase(cell.0, cell.1));
        }
        return;
    }
    match app.map.tool {
        MapTool::Block => {
            if cell.0 >= 0 && app.map.drag_last != Some(cell) {
                app.map.drag_last = Some(cell);
                actions.push(Action::ToggleBlock(cell.0, cell.1));
            }
        }
        MapTool::Inspect => {}
        MapTool::Route | MapTool::Erase => {
            let active = app
                .map
                .active_route
                .min(app.design.paths.len().saturating_sub(1));
            let Some(path) = app.design.paths.get(active) else {
                return;
            };
            if app.map.drag_last == Some(cell) {
                return;
            }
            app.map.drag_last = Some(cell);
            if path.len() >= 2 && path[path.len() - 2] == [cell.0, cell.1] {
                actions.push(Action::TrimTo(path.len() - 1));
                return;
            }
            if path.last() == Some(&[cell.0, cell.1]) {
                return;
            }
            if can_append(path, cell) {
                actions.push(Action::Append(cell.0, cell.1));
            }
        }
    }
}

fn apply(app: &mut Studio, action: Action) {
    let active = app
        .map
        .active_route
        .min(app.design.paths.len().saturating_sub(1));
    match action {
        Action::Append(x, y) => {
            if app.design.paths.is_empty() {
                return;
            }
            if app.design.paths[active].len() >= 100 {
                app.toast_err("单条路线最多 100 格");
                return;
            }
            app.begin_edit("绘制路线");
            app.design.paths[active].push([x, y]);
            app.issues_stamp += 1;
        }
        Action::TrimTo(n) => {
            if app.design.paths.is_empty() {
                return;
            }
            app.begin_edit("回退路线");
            app.design.paths[active].truncate(n);
            app.issues_stamp += 1;
        }
        Action::Erase(x, y) => {
            let on_road = app.design.paths.iter().any(|p| p.contains(&[x, y]));
            let on_block = app.design.blocked.contains(&[x, y]);
            if !on_road && !on_block {
                return;
            }
            app.begin_edit("擦除");
            for p in app.design.paths.iter_mut() {
                if let Some(pos) = p.iter().position(|c| *c == [x, y]) {
                    p.remove(pos);
                }
            }
            app.design.blocked.retain(|c| *c != [x, y]);
            app.touch();
            app.after_edit();
        }
        Action::ToggleBlock(x, y) => {
            if app.design.is_road(x, y) {
                app.toast_err("路线上不能放置障碍");
                return;
            }
            app.begin_edit("障碍");
            if let Some(pos) = app.design.blocked.iter().position(|c| *c == [x, y]) {
                app.design.blocked.remove(pos);
            } else if app.design.blocked.len() < 70 {
                app.design.blocked.push([x, y]);
            } else {
                app.toast_err("不可建造地块最多 70 个");
            }
            app.touch();
            app.after_edit();
        }
        Action::SetRoute(i) => app.map.active_route = i,
        Action::AddRoute => {
            app.begin_edit("新增路线");
            app.design.paths.push(Vec::new());
            app.map.active_route = app.design.paths.len() - 1;
            app.touch();
            app.after_edit();
        }
        Action::RemoveRoute => {
            if app.design.paths.len() <= 1 {
                return;
            }
            app.begin_edit("删除路线");
            app.design.paths.remove(active);
            for w in app.design.waves.iter_mut() {
                if w.route == active {
                    w.route = 0;
                } else if w.route > active {
                    w.route -= 1;
                }
            }
            app.map.active_route = app.map.active_route.min(app.design.paths.len() - 1);
            app.touch();
            app.after_edit();
        }
        Action::ClearRoute => {
            if app.design.paths.is_empty() {
                return;
            }
            app.begin_edit("清空路线");
            app.design.paths[active].clear();
            app.touch();
            app.after_edit();
        }
        Action::AutoRoute => {
            app.auto_route();
        }
    }
}

fn inspector(app: &mut Studio, ui: &mut Ui, actions: &mut Vec<Action>) {
    ui.label(RichText::new("路线概览").size(11.5).color(GOLD).strong());
    ui.add_space(3.0);
    for i in 0..app.design.paths.len() {
        let len = app.design.paths[i].len();
        let travel = (len.max(2) - 1) as f64 / app.design.speed.max(0.01);
        let active = app.map.active_route == i;
        egui::Frame::NONE
            .fill(if active {
                BG_RAISED
            } else {
                Color32::TRANSPARENT
            })
            .corner_radius(CornerRadius::same(4))
            .inner_margin(Margin::symmetric(6, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(5.0, 16.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 2.0, [GOLD, PURPLE, TEAL][i % 3]);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new(format!("路线 {}", i + 1))
                                .size(11.0)
                                .color(TEXT),
                        );
                        ui.label(
                            RichText::new(format!("{len} 格 · {travel:.1}s"))
                                .size(9.0)
                                .color(TEXT_FAINT),
                        );
                    });
                    right_aligned(ui, |ui| {
                        if soft_button(ui, "选").clicked() {
                            actions.push(Action::SetRoute(i));
                        }
                    });
                });
            });
    }

    ui.add_space(8.0);
    section(ui, "显示");
    if toggle(ui, "行进方向", app.map.show_flow, TEAL).clicked() {
        app.map.show_flow = !app.map.show_flow;
    }
    if toggle(ui, "坐标", app.map.show_coords, PURPLE).clicked() {
        app.map.show_coords = !app.map.show_coords;
    }
    if toggle(ui, "格序号", app.map.show_labels, GOLD).clicked() {
        app.map.show_labels = !app.map.show_labels;
    }
    if toggle(ui, "当前怪物位置", app.map.show_enemies, ERR).clicked() {
        app.map.show_enemies = !app.map.show_enemies;
    }

    ui.add_space(8.0);
    section(ui, "规则");
    ui.label(
        RichText::new("· 起点通常 x = -1（入口条）\n· 最后一格即守卫目标\n· 每条 3—100 格、最多 3 条\n· 障碍不能压在路线上，最多 70 个")
            .size(9.5)
            .color(TEXT_FAINT),
    );
}
