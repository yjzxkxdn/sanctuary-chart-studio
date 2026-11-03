//! 左右检视面板：
//! * 左：关卡元数据、音乐与节拍、自动生成、统计
//! * 右：随焦点切换 —— 音符属性 / 出怪属性（含实时战斗推演）/ 难度平衡 / 地图预览

use eframe::egui::{self, Color32, CornerRadius, Rect, RichText, Sense, Stroke, Ui};
use egui::{pos2, vec2};

use crate::app::{wave_color, Studio};
use crate::autochart::{Density, WaveIntensity};
use crate::model::*;
use crate::theme::*;
use crate::ui_timeline::Focus;
use crate::widgets::*;

// 左栏

pub fn left(app: &mut Studio, ui: &mut Ui) {
    level_section(app, ui);
    music_section(app, ui);
    generator_section(app, ui);
    stats_section(app, ui);
    issues_inline(app, ui);
    ui.add_space(10.0);
}

fn tile_row(ui: &mut Ui, items: &[(&str, &str, Color32)]) {
    let total = ui.available_width();
    let gap = 6.0;
    let n = items.len().max(1) as f32;
    let w = ((total - gap * (n - 1.0)) / n).max(40.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (v, l, c) in items {
            ui.allocate_ui_with_layout(
                vec2(w, 40.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(w);
                    stat_tile(ui, v, l, *c);
                },
            );
        }
    });
}

fn travel_val(app: &Studio) -> f64 {
    app.design
        .paths
        .first()
        .map(|p| (p.len().max(2) - 1) as f64 / app.design.speed.max(0.01))
        .unwrap_or(0.0)
}

/// 一行两个「标签在上、数值框在下」的字段，列宽自适应，永不溢出。
fn num_pair(
    ui: &mut Ui,
    a: (&'static str, egui::Id, &mut f64, NumCfg),
    b: (&'static str, egui::Id, &mut f64, NumCfg),
) -> (bool, bool) {
    let gap = 8.0;
    let col_w = ((ui.available_width() - gap) / 2.0).max(56.0);
    let mut ca = false;
    let mut cb = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(
            vec2(col_w, 38.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(col_w);
                ca = labeled_num(
                    ui,
                    a.0,
                    a.1,
                    a.2,
                    NumCfg {
                        width: col_w,
                        ..a.3
                    },
                );
            },
        );
        ui.allocate_ui_with_layout(
            vec2(col_w, 38.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(col_w);
                cb = labeled_num(
                    ui,
                    b.0,
                    b.1,
                    b.2,
                    NumCfg {
                        width: col_w,
                        ..b.3
                    },
                );
            },
        );
    });
    (ca, cb)
}

fn level_section(app: &mut Studio, ui: &mut Ui) {
    let mut changed = false;
    card(ui, "关卡", |ui| {
        // 名称
        let mut name = app.design.name.clone();
        let resp = ui.add(
            egui::TextEdit::singleline(&mut name)
                .hint_text("关卡名称")
                .font(font_id(13.0))
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(7, 5)),
        );
        if resp.changed() {
            app.begin_edit("重命名关卡");
            app.design.name = name;
            app.touch();
        }
        if resp.lost_focus() && app.design.name.trim().is_empty() {
            app.design.name = "未命名的星途".to_string();
        }

        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("配色").size(9.5).color(TEXT_DIM));
            if let Some(i) = palette_dots(ui, &app.design.palette, GOLD) {
                app.begin_edit("切换配色");
                app.design.palette = PALETTES[i].id.to_string();
                app.touch();
            }
        });
        // 配色名称单独一行。注意：竖排容器里不能用 right_to_left 布局，
        // 它会占掉剩余全部高度，把下方所有控件推走。
        ui.label(
            RichText::new(palette_of(&app.design.palette).name)
                .size(9.5)
                .color(TEXT_DIM),
        );

        ui.add_space(7.0);
        let mut dur = app.design.duration;
        let mut hp = app.design.goal_hp as f64;
        let (c1, c2) = num_pair(
            ui,
            (
                "时长（秒）",
                egui::Id::new("num_dur"),
                &mut dur,
                NumCfg::new(MIN_DURATION, MAX_DURATION, 0.5, 1, "s", 0.0),
            ),
            (
                "目标血量",
                egui::Id::new("num_goal"),
                &mut hp,
                NumCfg::new(1.0, 999.0, 0.5, 0, "", 0.0),
            ),
        );
        if c1 {
            app.design.duration = dur;
            changed = true;
        }
        if c2 {
            app.design.goal_hp = hp.round().clamp(1.0, 999.0) as u32;
            changed = true;
        }

        ui.add_space(7.0);
        let mut sp = app.design.speed;
        let mut eh = app.design.enemy_health as f64;
        let (c1, c2) = num_pair(
            ui,
            (
                "尘魄移速",
                egui::Id::new("num_speed"),
                &mut sp,
                NumCfg::new(0.1, 3.0, 0.01, 2, "格/s", 0.0).with_accent(TEAL),
            ),
            (
                "尘魄血量",
                egui::Id::new("num_ehp"),
                &mut eh,
                NumCfg::new(10.0, 9999.0, 1.0, 0, "", 0.0),
            ),
        );
        if c1 {
            app.design.speed = sp;
            changed = true;
        }
        if c2 {
            app.design.enemy_health = eh.round().clamp(10.0, 9999.0) as u32;
            changed = true;
        }

        ui.add_space(7.0);
        let mut tv = travel_val(app);
        let col_w = ((ui.available_width() - 8.0) / 2.0).max(56.0);
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2(col_w, 38.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(col_w);
                    labeled_num(
                        ui,
                        "单程（自动）",
                        egui::Id::new("ro_travel"),
                        &mut tv,
                        NumCfg::new(0.0, 1e9, 0.0, 1, "s", col_w).with_accent(TEXT_FAINT),
                    );
                },
            );
        });
    });
    if changed {
        app.touch();
        app.after_edit();
    }
}

fn music_section(app: &mut Studio, ui: &mut Ui) {
    card(ui, "音乐与节拍", |ui| {
        let has_audio = app.audio.has_audio();
        // 音频来源（只用本地文件）：文件名单独一行，按钮靠右、不再压住小字
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (r, _) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
            ui.painter()
                .circle_filled(r.center(), 12.0, with_alpha(GOLD, 0.14));
            ui.painter().circle_stroke(
                r.center(),
                12.0,
                Stroke::new(1.0_f32, with_alpha(GOLD, 0.35)),
            );
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "♪",
                font_id(12.0),
                GOLD,
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let name = if has_audio {
                    truncate(&app.audio.source_name, 22)
                } else {
                    "未选择音频".to_string()
                };
                ui.label(RichText::new(name).size(12.5).color(if has_audio {
                    TEXT
                } else {
                    TEXT_DIM
                }));
            });
        });
        ui.add_space(3.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let sub = if has_audio {
                format!("本地音频 · {:.2}s", app.audio.duration())
            } else {
                "载入后自动分析 BPM".to_string()
            };
            // truncate：宽度不够时省略号，绝不压到按钮
            ui.add(egui::Label::new(RichText::new(sub).size(10.0).color(TEXT_DIM)).truncate());
            if accent_button(
                ui,
                if has_audio {
                    "更换音频"
                } else {
                    "载入音频"
                },
            )
            .clicked()
            {
                app.load_audio_dialog();
            }
        });
        if let Some(err) = app.audio.error() {
            ui.label(RichText::new(err).size(9.5).color(WARN));
        }

        // BPM：标签独占一行，下面一行是 输入框 + 自动检测 + 打点测速（等高、对齐）
        ui.add_space(10.0);
        ui.label(
            RichText::new("BPM（可直接改，或点右侧按钮）")
                .size(10.0)
                .color(TEXT_DIM),
        );
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut bpm = app.design.bpm;
            let changed = num_field(
                ui,
                egui::Id::new("num_bpm"),
                &mut bpm,
                NumCfg::new(MIN_BPM, MAX_BPM, 0.2, 2, "", 74.0),
            );
            if changed && (bpm - app.design.bpm).abs() > 1e-9 {
                app.set_bpm(bpm);
            }
            if ghost_button(ui, "自动检测")
                .on_hover_text("用音头自相关分析整首曲子的速度")
                .clicked()
            {
                if !app.audio.has_audio() {
                    app.toast_err("请先载入音频");
                } else {
                    match app.auto_detect_bpm(true) {
                        Some((b, c)) => {
                            app.toast(format!("自动检测：{b:.2} BPM（置信度 {:.0}%）", c * 100.0))
                        }
                        None => app.toast_err("未能识别 BPM，请手动输入或打点测速"),
                    }
                }
            }
            // 打点测速：开着 live 读数，与其它按钮同行等高
            let taps = app.tap.count();
            let tap_label = if taps == 0 {
                "打点测速".to_string()
            } else {
                format!("打点 {taps}")
            };
            let tap_resp = chip_button(
                ui,
                &tap_label,
                with_alpha(TEAL, 0.16),
                with_alpha(TEAL, 0.55),
                TEAL,
            )
            .on_hover_text("跟着音乐连点这个按钮（或按 T）至少 3 次，点完按「应用」");
            if tap_resp.clicked() {
                app.tap_bpm();
            }
            if let Some(b) = app.tap.bpm() {
                ui.label(
                    RichText::new(format!("≈{b:.1}"))
                        .size(11.0)
                        .color(TEAL)
                        .monospace(),
                );
                if ghost_button(ui, "应用").clicked() {
                    app.apply_tapped_bpm();
                }
                if ghost_button(ui, "×").on_hover_text("清空打点").clicked() {
                    app.tap.reset();
                }
            }
        });
        // 打点提示单独一行，不再挤在按钮后面
        if app.tap.count() > 0 && app.tap.bpm().is_none() {
            ui.label(
                RichText::new(format!(
                    "已打点 {} 次，再点 {} 次即可测速",
                    app.tap.count(),
                    3 - app.tap.count()
                ))
                .size(9.5)
                .color(TEAL),
            );
        }

        // 相位
        ui.add_space(10.0);
        ui.label(
            RichText::new("节拍相位（第一拍的位置）")
                .size(10.0)
                .color(TEXT_DIM),
        );
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let mut ph = app.phase;
            if num_field(
                ui,
                egui::Id::new("num_phase"),
                &mut ph,
                NumCfg::new(0.0, 4.0, 0.002, 3, "s", 92.0).with_accent(PURPLE),
            ) {
                app.phase = ph;
                app.gen_chart.phase = ph;
                app.audio.set_phase(ph);
            }
            if ghost_button(ui, "自动推断")
                .on_hover_text("用音头能量对齐节拍网格")
                .clicked()
            {
                app.infer_phase();
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 7.0;
            ui.label(
                RichText::new(format!("小节 {:.3}s", app.design.bar()))
                    .size(10.0)
                    .color(TEXT_DIM),
            );
            ui.label(RichText::new("·").size(10.0).color(TEXT_FAINT));
            ui.label(
                RichText::new(format!("拍 {:.3}s", app.design.beat()))
                    .size(10.0)
                    .color(TEXT_DIM),
            );
            ui.label(RichText::new("·").size(10.0).color(TEXT_FAINT));
            ui.label(
                RichText::new(format!(
                    "{} 小节",
                    (app.design.duration / app.design.bar()).ceil() as i64
                ))
                .size(10.0)
                .color(TEXT_DIM),
            );
        });
    });
}

fn generator_section(app: &mut Studio, ui: &mut Ui) {
    card(ui, "一键生成", |ui| {
        let w = ui.available_width();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let half = (w - 6.0) * 0.5;
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("生成谱面")
                            .size(11.5)
                            .color(Color32::from_rgb(0x1a, 0x12, 0x24)),
                    )
                    .fill(GOLD)
                    .stroke(Stroke::new(1.0_f32, GOLD))
                    .corner_radius(CornerRadius::same(5))
                    .min_size(vec2(half, CTRL_H)),
                )
                .on_hover_text("按音头能量 / BPM 网格铺满整首曲子")
                .clicked()
            {
                app.run_auto_chart();
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("生成出怪").size(11.5).color(PURPLE))
                        .fill(with_alpha(PURPLE, 0.16))
                        .stroke(Stroke::new(1.0_f32, with_alpha(PURPLE, 0.4)))
                        .corner_radius(CornerRadius::same(5))
                        .min_size(vec2(half, CTRL_H)),
                )
                .on_hover_text("生成递进出怪曲线，自动含特殊尘魄与首领")
                .clicked()
            {
                app.run_auto_waves();
            }
        });

        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("谱面密度").size(9.5).color(TEXT_DIM));
            right_aligned(ui, |ui| {
                for v in [Density::Sparse, Density::Normal, Density::Dense] {
                    if pill(ui, v.label(), app.gen_chart.density == v, PURPLE).clicked() {
                        app.gen_chart.density = v;
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("出怪强度").size(9.5).color(TEXT_DIM));
            right_aligned(ui, |ui| {
                for v in [
                    WaveIntensity::Relaxed,
                    WaveIntensity::Standard,
                    WaveIntensity::Intense,
                ] {
                    if pill(ui, v.label(), app.gen_waves.intensity == v, GOLD).clicked() {
                        app.gen_waves.intensity = v;
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new("载入音频后按音头能量生成；未载入则按 BPM 网格")
                .size(9.0)
                .color(TEXT_FAINT),
        );
    });
}

fn stats_section(app: &mut Studio, ui: &mut Ui) {
    let st = app.design.stats();
    let c = app.design.curves(240);
    card(ui, "统计", |ui| {
        tile_row(
            ui,
            &[
                (&format!("{}", st.notes), "音符", PURPLE),
                (&format!("{}", st.holds), "长条", GOLD),
            ],
        );
        ui.add_space(6.0);
        tile_row(
            ui,
            &[
                (
                    &format!("{:.2}", st.avg_nps),
                    "平均 NPS",
                    if st.avg_nps > 3.2 { WARN } else { TEAL },
                ),
                (
                    &format!("{:.0}", st.peak_nps),
                    "峰值 NPS",
                    if st.peak_nps > 8.0 { ERR } else { TEAL },
                ),
            ],
        );
        ui.add_space(6.0);
        tile_row(
            ui,
            &[
                (&format!("{}", st.waves), "出怪事件", TEXT),
                (
                    &format!("{:.0}%", c.peak_conflict * 100.0),
                    "双压峰值",
                    if c.peak_conflict > 0.7 { ERR } else { OK },
                ),
            ],
        );

        ui.add_space(9.0);
        ui.label(RichText::new("出怪构成").size(9.5).color(TEXT_DIM));
        ui.add_space(3.0);
        let total = st.waves.max(1) as f32;
        let mut any = false;
        for (i, k) in WaveKind::ALL.iter().enumerate() {
            let n = st.wave_kinds[i];
            if n == 0 {
                continue;
            }
            any = true;
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(6.0, 6.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.0, wave_color(*k));
                ui.label(RichText::new(k.short()).size(9.5).color(TEXT_DIM));
                let avail = ui.available_width() - 36.0;
                let (bar, _) = ui.allocate_exact_size(vec2(avail.max(20.0), 4.0), Sense::hover());
                ui.painter().rect_filled(bar, 2.0, BG_INPUT);
                let mut f = bar;
                f.set_width(bar.width() * (n as f32 / total));
                ui.painter().rect_filled(f, 2.0, wave_color(*k));
                ui.label(RichText::new(format!("{n}")).size(9.5).color(TEXT));
            });
        }
        if !any {
            hint(ui, "尚未编排出怪");
        }
    });
}

fn issues_inline(app: &mut Studio, ui: &mut Ui) {
    let mut items: Vec<(Severity, String, Option<f64>)> = app
        .issues
        .iter()
        .map(|i| (i.severity, i.message.clone(), i.time))
        .collect();
    items.sort_by_key(|(s, _, _)| *s);
    let errors = items
        .iter()
        .filter(|(s, _, _)| *s == Severity::Error)
        .count();
    let warns = items
        .iter()
        .filter(|(s, _, _)| *s == Severity::Warning)
        .count();
    card(ui, "检查", |ui| {
        if items.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("✓").size(11.0).color(OK));
                ui.label(RichText::new("未发现问题，可直接导出").size(10.5).color(OK));
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{errors}"))
                    .size(13.0)
                    .color(if errors > 0 { ERR } else { TEXT_FAINT })
                    .strong(),
            );
            ui.label(RichText::new("错误").size(9.0).color(TEXT_FAINT));
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{warns}"))
                    .size(13.0)
                    .color(if warns > 0 { WARN } else { TEXT_FAINT })
                    .strong(),
            );
            ui.label(RichText::new("警告").size(9.0).color(TEXT_FAINT));
            right_aligned(ui, |ui| {
                if ghost_button(ui, "全部").clicked() {
                    app.show_issues = true;
                }
            });
        });
        ui.add_space(4.0);
        let mut jump = None;
        egui::ScrollArea::vertical()
            .max_height(132.0)
            .id_salt("inline_issues")
            .show(ui, |ui| {
                for (sev, msg, t) in items.iter().take(24) {
                    let (color, icon) = match sev {
                        Severity::Error => (ERR, "✕"),
                        Severity::Warning => (WARN, "!"),
                        Severity::Tip => (TEXT_FAINT, "·"),
                    };
                    if issue_chip(ui, msg, color, icon) {
                        if let Some(t) = t {
                            jump = Some(*t);
                        }
                    }
                }
            });
        if let Some(t) = jump {
            app.seek(t);
        }
    });
}

// 右栏

pub fn right(app: &mut Studio, ui: &mut Ui) {
    ui.horizontal(|ui| {
        for f in [Focus::Chart, Focus::Waves, Focus::Balance, Focus::Map] {
            if pill(ui, f.label(), app.focus == f, GOLD).clicked() {
                app.focus = f;
            }
        }
    });
    ui.add_space(4.0);
    match app.focus {
        Focus::Chart => note_panel(app, ui),
        Focus::Waves => wave_panel(app, ui),
        Focus::Balance => balance_panel(app, ui),
        Focus::Map => map_panel(app, ui),
    }
}

fn note_panel(app: &mut Studio, ui: &mut Ui) {
    section(ui, "音符属性");
    let id = app
        .sel_note
        .or_else(|| app.tl.note_sel.iter().next().cloned());
    let Some(id) = id else {
        hint(
            ui,
            "在时间轴的「谱面」轨道点一下即可放置音符；点已有音符查看属性",
        );
        multi_note_tools(app, ui);
        return;
    };
    let Some(idx) = app.design.chart.iter().position(|n| n.id == id) else {
        hint(ui, "该音符已不存在");
        return;
    };
    let beat = app.design.beat();
    let dur = app.design.duration;
    let phase = app.phase;
    let mut changed = false;

    {
        // 先算好派生量，避免与可变借用冲突
        let (cur_t, cur_kind, cur_end, cur_accent) = {
            let n = &app.design.chart[idx];
            (n.time, n.kind, n.end, n.accent)
        };
        let bar = app.design.bar_index(cur_t, phase);
        let fr = app.design.beat_phase(cur_t, phase);
        let n = &mut app.design.chart[idx];
        ui.label(
            RichText::new(format!(
                "{} · 第 {} 小节第 {} 拍",
                cur_kind.label(),
                bar.floor() as i64 + 1,
                (bar.fract() * 4.0).floor() as i64 + 1
            ))
            .size(10.5)
            .color(TEXT_FAINT),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("时间").size(11.0).color(TEXT_DIM));
            let mut t = n.time;
            if ui
                .add(
                    egui::DragValue::new(&mut t)
                        .speed(0.005)
                        .range(0.0..=dur)
                        .fixed_decimals(3)
                        .suffix(" s"),
                )
                .changed()
            {
                n.time = t;
                changed = true;
            }
            if soft_button(ui, "从播放头")
                .on_hover_text("把时间设为当前播放头")
                .clicked()
            {
                n.time = app.clock.clamp(0.0, dur);
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("类型").size(11.0).color(TEXT_DIM));
            let mut is_hold = n.kind == NoteKind::Hold;
            if toggle(ui, "长条", is_hold, GOLD).clicked() {
                is_hold = !is_hold;
                if is_hold {
                    n.kind = NoteKind::Hold;
                    n.end = Some((n.time + beat * 2.0).clamp(n.time + MIN_HOLD, dur));
                } else {
                    n.kind = NoteKind::Tap;
                    n.end = None;
                }
                changed = true;
            }
            if n.kind == NoteKind::Hold {
                let mut end = n.end.unwrap_or(cur_end.unwrap_or(n.time + beat));
                if ui
                    .add(
                        egui::DragValue::new(&mut end)
                            .speed(0.005)
                            .range(n.time + MIN_HOLD..=dur)
                            .fixed_decimals(3)
                            .suffix(" s"),
                    )
                    .changed()
                {
                    n.end = Some(end);
                    changed = true;
                }
                let mut beats = (end - n.time) / beat;
                if ui
                    .add(
                        egui::DragValue::new(&mut beats)
                            .speed(0.05)
                            .range(0.25..=32.0)
                            .suffix(" 拍"),
                    )
                    .changed()
                {
                    n.end = Some((n.time + beats * beat).clamp(n.time + MIN_HOLD, dur));
                    changed = true;
                }
            }
        });
        if toggle(ui, "重音（正拍强调）", n.accent, PURPLE).clicked() {
            n.accent = !n.accent;
            changed = true;
        }
        let _ = cur_accent;
        ui.label(
            RichText::new(format!(
                "拍内位置 {:.0}%  ·  偏离网格 {:.0} ms",
                fr * 100.0,
                ((fr - fr.round()) * beat * 1000.0).abs()
            ))
            .size(9.5)
            .color(if (fr - fr.round()).abs() * beat > 0.05 {
                WARN
            } else {
                TEXT_FAINT
            }),
        );
    }
    if changed {
        app.begin_edit("编辑音符");
        app.touch();
        app.after_edit();
    }

    ui.horizontal(|ui| {
        if ghost_button(ui, "量化到网格").clicked() {
            let step = app.snap.step(app.design.bpm);
            if let Some(step) = step {
                app.begin_edit("量化音符");
                let ph = app.phase;
                let n = &mut app.design.chart[idx];
                let span = n.span();
                let k = ((n.time - ph) / step).round();
                n.time = (ph + k * step).clamp(0.0, dur);
                if n.kind == NoteKind::Hold {
                    n.end = Some((n.time + span).clamp(n.time + MIN_HOLD, dur));
                }
                app.touch();
                app.after_edit();
            }
        }
        if ghost_button(ui, "切换点按/长条").clicked() {
            app.toggle_hold_for(id);
        }
        if ghost_button(ui, "删除").clicked() {
            app.tl.note_sel.insert(id);
            app.delete_selected();
        }
    });

    if accent_button(ui, "从此处试听 ♫").clicked() {
        let t = app.design.chart[idx].time;
        let was = app.playing;
        app.seek((t - 1.2).max(0.0));
        if !was {
            app.toggle_play();
        }
    }

    multi_note_tools(app, ui);
}

fn multi_note_tools(app: &mut Studio, ui: &mut Ui) {
    section(ui, "批量编辑");
    ui.label(
        RichText::new(format!(
            "已选 {} 音符 · {} 出怪",
            app.tl.note_sel.len(),
            app.tl.wave_sel.len()
        ))
        .size(10.5)
        .color(
            if app.tl.note_sel.is_empty() && app.tl.wave_sel.is_empty() {
                TEXT_FAINT
            } else {
                GOLD
            },
        ),
    );
    hint(
        ui,
        "在时间轴空白处直接拖拽即可框选音符与出怪（或按住 Shift）；选中后用下方按钮批量处理",
    );
    ui.horizontal_wrapped(|ui| {
        if ghost_button(ui, "全选").clicked() {
            app.tl.note_sel = app.design.chart.iter().map(|n| n.id).collect();
            app.tl.wave_sel = app.design.waves.iter().map(|w| w.id).collect();
        }
        if ghost_button(ui, "量化")
            .on_hover_text("按当前吸附精度对齐到网格")
            .clicked()
        {
            app.quantize_selection();
        }
        if ghost_button(ui, "复制").clicked() {
            app.copy_selection();
        }
        if ghost_button(ui, "粘贴")
            .on_hover_text("粘贴到播放头")
            .clicked()
        {
            app.paste();
        }
        if ghost_button(ui, "删除").clicked() {
            app.delete_selected();
        }
    });
    // 批量微调
    let step = app.snap.step(app.design.bpm).unwrap_or(0.1);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("整体移动").size(10.0).color(TEXT_DIM));
        if soft_button(ui, "−1 格").clicked() {
            app.nudge_selection(-1.0);
        }
        if soft_button(ui, "+1 格").clicked() {
            app.nudge_selection(1.0);
        }
        ui.label(
            RichText::new(format!("（{:.0} ms）", step * 1000.0))
                .size(9.0)
                .color(TEXT_FAINT)
                .monospace(),
        );
    });
    // 音符批量属性
    if !app.tl.note_sel.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("音符").size(10.0).color(TEXT_DIM));
            if soft_button(ui, "全变点按").clicked() {
                app.set_selected_note_kind(NoteKind::Tap);
            }
            if soft_button(ui, "全长条").clicked() {
                app.set_selected_note_kind(NoteKind::Hold);
            }
            if soft_button(ui, "加重音").clicked() {
                app.set_selected_accent(true);
            }
            if soft_button(ui, "去重音").clicked() {
                app.set_selected_accent(false);
            }
        });
    }
    // 出怪批量属性
    if !app.tl.wave_sel.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("出怪").size(10.0).color(TEXT_DIM));
            if soft_button(ui, "对齐到拍").clicked() {
                app.align_selected_waves_to_beat();
            }
            ui.label(RichText::new("类型").size(10.0).color(TEXT_DIM));
            for k in [
                WaveKind::Normal,
                WaveKind::Armored,
                WaveKind::Special,
                WaveKind::Boss,
            ] {
                if soft_button(ui, k.short())
                    .on_hover_text(k.label())
                    .clicked()
                {
                    app.set_selected_wave_kind(k);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("路线").size(10.0).color(TEXT_DIM));
            for i in 0..app.design.paths.len() {
                if soft_button(ui, format!("{}", i + 1)).clicked() {
                    app.set_selected_wave_route(i);
                }
            }
            ui.label(RichText::new("血/速").size(10.0).color(TEXT_DIM));
            if soft_button(ui, "×1.0").clicked() {
                app.scale_selected_waves(1.0, 1.0);
            }
            if soft_button(ui, "血×1.5").clicked() {
                app.scale_selected_waves(1.5, 1.0);
            }
            if soft_button(ui, "速×0.8").clicked() {
                app.scale_selected_waves(1.0, 0.8);
            }
        });
    }
}

fn wave_panel(app: &mut Studio, ui: &mut Ui) {
    section(ui, "出怪属性");
    let id = app
        .sel_wave
        .or_else(|| app.tl.wave_sel.iter().next().cloned());
    let Some(id) = id else {
        hint(ui, "在时间轴的路线泳道上点空白处加怪；点已有怪物查看属性");
        wave_gen_tools(app, ui);
        return;
    };
    let Some(idx) = app.design.waves.iter().position(|w| w.id == id) else {
        hint(ui, "该事件已不存在");
        return;
    };
    let dur = app.design.duration;
    let routes = app.design.paths.len().max(1);
    let mut changed = false;
    {
        let w = &mut app.design.waves[idx];
        ui.horizontal(|ui| {
            ui.label(RichText::new("时间").size(11.0).color(TEXT_DIM));
            let mut t = w.time;
            if ui
                .add(
                    egui::DragValue::new(&mut t)
                        .speed(0.01)
                        .range(0.0..=dur)
                        .fixed_decimals(2)
                        .suffix(" s"),
                )
                .changed()
            {
                w.time = t;
                changed = true;
            }
            ui.label(RichText::new("路线").size(11.0).color(TEXT_DIM));
            let mut r = w.route as i32 + 1;
            if ui
                .add(
                    egui::DragValue::new(&mut r)
                        .speed(0.1)
                        .range(1..=routes as i32),
                )
                .changed()
            {
                w.route = (r.max(1) - 1) as usize;
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("类型").size(11.0).color(TEXT_DIM));
            egui::ComboBox::from_id_salt("wave_kind")
                .selected_text(RichText::new(w.kind.label()).size(11.0))
                .width(ui.available_width() - 6.0)
                .show_ui(ui, |ui| {
                    for k in WaveKind::ALL {
                        if ui.selectable_label(w.kind == k, k.label()).clicked() {
                            w.kind = k;
                            changed = true;
                        }
                    }
                });
        });
        ui.label(RichText::new(w.kind.hint()).size(9.5).color(TEXT_FAINT));
        ui.horizontal(|ui| {
            ui.label(RichText::new("血量倍率").size(11.0).color(TEXT_DIM));
            let mut h = w.health;
            if ui
                .add(
                    egui::DragValue::new(&mut h)
                        .speed(0.05)
                        .range(0.1..=20.0)
                        .fixed_decimals(2),
                )
                .changed()
            {
                w.health = h;
                changed = true;
            }
            ui.label(RichText::new("速度倍率").size(11.0).color(TEXT_DIM));
            let mut s = w.speed;
            if ui
                .add(
                    egui::DragValue::new(&mut s)
                        .speed(0.01)
                        .range(0.1..=4.0)
                        .fixed_decimals(2),
                )
                .changed()
            {
                w.speed = s;
                changed = true;
            }
        });
    }
    if changed {
        app.begin_edit("编辑出怪");
        app.touch();
        app.after_edit();
    }

    // 实时战斗推演
    let w = app.design.waves[idx].clone();
    let st = app.design.wave_stats(&w);
    section(ui, "战斗推演");
    ui.label(
        RichText::new(format!(
            "生命 {:.0}  ·  移速 {:.2} 格/秒  ·  单程 {:.1}s",
            st.hp, st.speed, st.travel
        ))
        .size(11.0)
        .color(TEXT),
    );
    let arrive = w.time + st.travel;
    ui.label(
        RichText::new(format!(
            "出现 {:.2}s → 抵达目标 {:.2}s{}",
            w.time,
            arrive,
            if arrive > app.design.duration {
                "（超出关卡时长！）"
            } else {
                ""
            }
        ))
        .size(10.0)
        .color(if arrive > app.design.duration {
            ERR
        } else {
            TEXT_FAINT
        }),
    );
    // 血量递进系数：游戏会按出现时刻把血量抬到 1.0—4.5 倍
    ui.label(
        RichText::new(format!(
            "血量递进 ×{:.2}（越靠后越硬）· 同时在场约 {:.0} 只",
            st.ramp,
            app.design
                .waves
                .iter()
                .filter(|x| {
                    let s = app.design.wave_stats(x);
                    x.time <= app.clock && app.clock <= x.time + s.travel
                })
                .count()
        ))
        .size(9.5)
        .color(TEXT_FAINT),
    );
    // 漏怪总和
    let mut leak = 0.0;
    for x in &app.design.waves {
        let s = app.design.wave_stats(x);
        if x.time + s.travel <= app.design.duration {
            leak += s.damage;
        }
    }
    ui.label(
        RichText::new(format!(
            "若全部漏过：合计扣 {:.0} 点 · 目标血量 {}（{} 只可漏）",
            leak,
            app.design.goal_hp,
            (app.design.goal_hp as f64 / leak.max(1.0)).floor()
        ))
        .size(10.0)
        .color(if leak > app.design.goal_hp as f64 {
            WARN
        } else {
            TEXT_FAINT
        }),
    );
    // 与音符的对齐
    let nearest = app
        .design
        .chart
        .iter()
        .map(|n| (n.time - w.time).abs())
        .fold(f64::INFINITY, f64::min);
    if nearest.is_finite() {
        ui.label(
            RichText::new(format!(
                "最近的音符 {:.3}s {}",
                nearest,
                if nearest < 0.12 {
                    "· 与节拍同步"
                } else {
                    ""
                }
            ))
            .size(10.0)
            .color(if nearest < 0.12 { OK } else { TEXT_FAINT }),
        );
    }

    ui.horizontal(|ui| {
        if ghost_button(ui, "对齐到拍").clicked() {
            app.begin_edit("对齐出怪");
            let t = app.design.snap_to(
                w.time,
                crate::autochart::Snap::Beat.step(app.design.bpm),
                app.phase,
            );
            app.design.waves[idx].time = t;
            app.touch();
            app.after_edit();
        }
        if ghost_button(ui, "跳到该时刻").clicked() {
            let t = w.time;
            app.seek(t);
        }
        if ghost_button(ui, "删除").clicked() {
            app.tl.wave_sel.insert(id);
            app.delete_selected();
        }
    });

    wave_gen_tools(app, ui);
    multi_note_tools(app, ui);
}

fn wave_gen_tools(app: &mut Studio, ui: &mut Ui) {
    section(ui, "编排");
    ui.horizontal(|ui| {
        if primary_button(ui, "自动编排").clicked() {
            app.run_auto_waves();
        }
        let mut it = app.gen_waves.intensity;
        for (v, l) in [
            (WaveIntensity::Relaxed, "舒缓"),
            (WaveIntensity::Standard, "标准"),
            (WaveIntensity::Intense, "紧张"),
        ] {
            if pill(ui, l, it == v, GOLD).clicked() {
                it = v;
            }
        }
        app.gen_waves.intensity = it;
    });
    ui.horizontal(|ui| {
        if toggle(ui, "对齐拍点", app.gen_waves.align_to_beat, TEAL).clicked() {
            app.gen_waves.align_to_beat = !app.gen_waves.align_to_beat;
        }
        ui.label(RichText::new("特殊每").size(10.5).color(TEXT_DIM));
        let mut n = app.gen_waves.special_every as i32;
        if ui
            .add(egui::DragValue::new(&mut n).speed(0.2).range(1..=30))
            .changed()
        {
            app.gen_waves.special_every = n.clamp(1, 30) as usize;
        }
        ui.label(RichText::new("只").size(10.5).color(TEXT_DIM));
    });
    ui.label(
        RichText::new(format!(
            "当前 {} 个事件 · 平均间隔 {:.2}s",
            app.design.waves.len(),
            avg_gap(app)
        ))
        .size(9.5)
        .color(TEXT_FAINT),
    );
    enemy_codex(app, ui);
}

/// 怪物图鉴：点一下就把真实数值套用到当前选中的出怪事件。
fn enemy_codex(app: &mut Studio, ui: &mut Ui) {
    section(ui, "怪物图鉴");
    hint(
        ui,
        "点击任意条目，把它的真实血量 / 移速倍率套用到选中的出怪事件",
    );
    let target = app
        .sel_wave
        .or_else(|| app.tl.wave_sel.iter().next().cloned());
    let mut apply: Option<&'static crate::model::EnemyType> = None;
    egui::ScrollArea::vertical()
        .max_height(230.0)
        .id_salt("codex")
        .show(ui, |ui| {
            for e in crate::model::ENEMY_TYPES.iter() {
                let resp = egui::Frame::NONE
                    .fill(BG_INPUT)
                    .stroke(Stroke::new(1.0_f32, LINE_SOFT))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 5,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(e.name).size(11.0).color(TEXT).strong());
                            right_aligned(ui, |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "血 ×{:.1} · 速 ×{:.2}",
                                        e.health, e.speed
                                    ))
                                    .size(9.0)
                                    .color(TEXT_FAINT)
                                    .monospace(),
                                );
                            });
                        });
                        ui.label(
                            RichText::new(format!("漏 -{:.0} · 射程 {:.1} 格", e.leak, e.range))
                                .size(8.5)
                                .color(TEXT_FAINT)
                                .monospace(),
                        );
                        ui.label(RichText::new(e.desc).size(9.0).color(TEXT_FAINT));
                    });
                let clickable = resp.response.interact(Sense::click());
                if clickable.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if clickable.clicked() {
                    apply = Some(e);
                }
                ui.add_space(3.0);
            }
        });
    if let Some(e) = apply {
        match target.and_then(|id| app.design.waves.iter().position(|w| w.id == id)) {
            Some(idx) => {
                app.begin_edit("套用怪物数值");
                crate::model::apply_enemy_template(&mut app.design.waves[idx], e);
                app.touch();
                app.after_edit();
                app.toast(format!("已套用「{}」({}) 的血量与移速", e.name, e.id));
            }
            None => app.toast_err("请先在时间轴上选中一个出怪事件"),
        }
    }
}

fn avg_gap(app: &Studio) -> f64 {
    if app.design.waves.len() < 2 {
        return 0.0;
    }
    let mut v: Vec<f64> = app.design.waves.iter().map(|w| w.time).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f64 = v.windows(2).map(|w| w[1] - w[0]).sum();
    sum / (v.len() - 1) as f64
}

fn balance_panel(app: &mut Studio, ui: &mut Ui) {
    section(ui, "双压平衡");
    hint(
        ui,
        "「双压」= 同一时刻既需要密集打谱、又有一大波怪物在场。这是决定关卡手感的核心指标。",
    );
    let c = app.design.curves(600);
    ui.add_space(4.0);
    egui::Grid::new("balance_grid")
        .num_columns(2)
        .spacing(vec2(8.0, 3.0))
        .show(ui, |ui| {
            ui.label(RichText::new("峰值 NPS").size(11.0).color(TEXT_DIM));
            ui.label(
                RichText::new(format!("{:.1}", c.peak_nps))
                    .size(11.0)
                    .color(if c.peak_nps > 8.0 { ERR } else { TEXT }),
            );
            ui.end_row();
            ui.label(RichText::new("峰值在场血量").size(11.0).color(TEXT_DIM));
            ui.label(
                RichText::new(format!("{:.0}", c.peak_threat))
                    .size(11.0)
                    .color(if c.peak_threat > 500.0 { WARN } else { TEXT }),
            );
            ui.end_row();
            ui.label(RichText::new("双压峰值").size(11.0).color(TEXT_DIM));
            ui.label(
                RichText::new(format!("{:.0}%", c.peak_conflict * 100.0))
                    .size(11.0)
                    .color(if c.peak_conflict > 0.7 { ERR } else { OK }),
            );
            ui.end_row();
        });

    ui.add_space(6.0);
    ui.label(RichText::new("双压热点").size(11.0).color(GOLD));
    let hot = app.design.conflict_hotspots(8);
    if hot.is_empty() {
        ui.label(
            RichText::new("没有明显重叠，节奏与出怪互相错开")
                .size(10.5)
                .color(OK),
        );
    } else {
        let mut jump = None;
        for (t, v) in hot {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(40.0, 4.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, BG_INPUT);
                let mut f = r;
                f.set_width(r.width() * v);
                ui.painter().rect_filled(f, 2.0, ERR);
                ui.label(
                    RichText::new(format!("{t:6.1}s"))
                        .size(10.5)
                        .color(TEXT)
                        .monospace(),
                );
                if soft_button(ui, "跳转").clicked() {
                    jump = Some(t);
                }
                if soft_button(ui, "在此处加怪")
                    .on_hover_text("往当前双压最高的时刻加一个事件")
                    .clicked()
                {
                    jump = Some(t);
                }
            });
        }
        if let Some(t) = jump {
            app.seek(t);
        }
    }

    ui.add_space(8.0);
    section(ui, "手感诊断");
    let st = app.design.stats();
    let mut lines: Vec<(String, Color32)> = Vec::new();
    if st.avg_nps > 3.2 {
        lines.push((
            format!("平均 {:.2} NPS 偏高，多数玩家难以稳定跟住", st.avg_nps),
            WARN,
        ));
    } else if st.avg_nps < 0.5 && st.notes > 20 {
        lines.push((
            format!("平均 {:.2} NPS 偏低，谱面显得空旷", st.avg_nps),
            TEXT_DIM,
        ));
    } else {
        lines.push((format!("平均 {:.2} NPS 处于合理区间", st.avg_nps), OK));
    }
    if st.peak_nps > 8.0 {
        lines.push((format!("峰值 {:.0} NPS，需要拆分连打", st.peak_nps), ERR));
    }
    let holds_ratio = st.holds as f64 / st.notes.max(1) as f64;
    if holds_ratio > 0.35 {
        lines.push((
            format!("长条占 {:.0}%，会削弱打击反馈", holds_ratio * 100.0),
            WARN,
        ));
    } else if holds_ratio < 0.03 && st.notes > 40 {
        lines.push(("几乎没有长条，层次单一".into(), TEXT_DIM));
    }
    let specials = app
        .design
        .waves
        .iter()
        .filter(|w| w.kind == WaveKind::Special)
        .count();
    if specials == 0 {
        lines.push((
            "没有特殊尘魄，玩家无法获得塔台（游戏会拒绝导入）".into(),
            ERR,
        ));
    } else if specials as f64 / app.design.duration.max(1.0) * 60.0 < 0.5 {
        lines.push((format!("{specials} 只特殊尘魄，塔台获取机会偏少"), WARN));
    }
    if !app.design.waves.iter().any(|w| w.kind.is_boss()) && app.design.duration >= 60.0 {
        lines.push(("缺少首领事件，关卡没有高潮".into(), WARN));
    }
    for (text, color) in lines {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("·").size(11.0).color(color));
            ui.label(RichText::new(text).size(10.5).color(color));
        });
    }
}

fn map_panel(app: &mut Studio, ui: &mut Ui) {
    section(ui, "地形预览");
    let size = ui.available_width().min(320.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size * 0.70 + 22.0), Sense::click());
    draw_mini_map(app, ui, rect, app.clock);
    if resp.clicked() {
        app.show_map_editor = true;
    }
    hint(ui, "点击地图打开「地形编辑」大画布");

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if primary_button(ui, "打开地形编辑").clicked() {
            app.show_map_editor = true;
        }
        if ghost_button(ui, "自动铺路").clicked() {
            app.auto_route();
        }
    });

    section(ui, "路线");
    let n = app.design.paths.len();
    for i in 0..n {
        let len = app.design.paths[i].len();
        let travel = (len.max(2) - 1) as f64 / app.design.speed.max(0.01);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(6.0, 14.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 3.0, [GOLD, PURPLE, TEAL][i % 3]);
            ui.label(
                RichText::new(format!("路线 {} · {len} 格 · {travel:.1}s", i + 1))
                    .size(11.0)
                    .color(TEXT),
            );
            right_aligned(ui, |ui| {
                let waves = app.design.waves.iter().filter(|w| w.route == i).count();
                ui.label(
                    RichText::new(format!("{waves} 只"))
                        .size(10.0)
                        .color(TEXT_FAINT),
                );
            });
        });
    }
    ui.label(
        RichText::new(format!("障碍 {} 格（上限 70）", app.design.blocked.len()))
            .size(10.0)
            .color(TEXT_FAINT),
    );
}

/// 迷你地图：静态地形 + 当前播放时刻的怪物位置。
pub fn draw_mini_map(app: &Studio, ui: &Ui, rect: Rect, clock: f64) {
    let painter = ui.painter_at(rect);
    let pal = app.design.palette();
    painter.rect_filled(rect, CornerRadius::same(5), hex(pal.bg[0]));

    let header = 18.0;
    let plot = Rect::from_min_max(
        pos2(rect.left() + 4.0, rect.top() + header),
        pos2(rect.right() - 4.0, rect.bottom() - 2.0),
    );
    let cell = (plot.width() / (COLS as f32 + 1.4))
        .min(plot.height() / ROWS as f32)
        .max(6.0);
    let ox = plot.left() + cell * 1.2;
    let oy = plot.center().y - cell * ROWS as f32 * 0.5;
    let cell_rect = |x: i32, y: i32| {
        Rect::from_min_size(
            pos2(ox + x as f32 * cell, oy + y as f32 * cell),
            vec2(cell - 1.0, cell - 1.0),
        )
    };
    for y in 0..ROWS {
        for x in 0..COLS {
            painter.rect_filled(
                cell_rect(x, y),
                CornerRadius::same(2),
                hex(pal.tile).gamma_multiply(0.6),
            );
        }
    }
    for b in &app.design.blocked {
        painter.rect_filled(
            cell_rect(b[0], b[1]),
            CornerRadius::same(2),
            with_alpha(BG_APP, 0.7),
        );
    }
    let colors = [GOLD, PURPLE, TEAL];
    // 用「每格方块 + 相邻格之间的连接桥」画路线：并集能完整填满拐角，
    // 不会像平头粗线段那样在转角处缺一个方块（看起来像路断开了）。
    let road_col = hex(pal.road);
    let t = (cell * 0.62).max(3.0);
    let half = t * 0.5;
    for (ri, path) in app.design.paths.iter().enumerate() {
        let c = colors[ri % 3];
        let pt = |g: &[i32; 2]| -> egui::Pos2 {
            if g[0] < 0 {
                // 入口格用与怪物一致的连续映射（x=-1 → 左移半格）
                pos2(ox - cell * 0.5, oy + (g[1] as f32 + 0.5) * cell)
            } else {
                cell_rect(g[0], g[1]).center()
            }
        };
        // 连接桥
        for w in path.windows(2) {
            let a = pt(&w[0]);
            let b = pt(&w[1]);
            let r = Rect::from_two_pos(a, b);
            let bridge = Rect::from_min_max(
                pos2(r.left() - half, r.top() - half),
                pos2(r.right() + half, r.bottom() + half),
            );
            painter.rect_filled(bridge, CornerRadius::same(2), with_alpha(road_col, 0.88));
            painter.rect_filled(bridge, CornerRadius::same(2), with_alpha(c, 0.13));
        }
        // 每格方块（填满转角）
        for g in path.iter() {
            let r = Rect::from_center_size(pt(g), vec2(t, t));
            painter.rect_filled(r, CornerRadius::same(2), with_alpha(road_col, 0.88));
            painter.rect_filled(r, CornerRadius::same(2), with_alpha(c, 0.13));
        }
        if let Some(last) = path.last() {
            if last[0] >= 0 {
                let ctr = cell_rect(last[0], last[1]).center();
                painter.circle_filled(ctr, cell * 0.30, GOLD);
                painter.circle_stroke(
                    ctr,
                    cell * 0.30,
                    Stroke::new(1.0_f32, with_alpha(Color32::WHITE, 0.5)),
                );
            }
        }
    }
    // 实时怪物
    let alive = app.design.enemies_alive_at(clock);
    for e in &alive {
        let c = wave_color(e.kind);
        // 血量越低画得越小，进度越靠后描边越亮
        let r = cell * (0.16 + 0.12 * (e.hp.max(1.0).ln() / 8.0).min(1.0) as f32);
        // 用连续映射（负数 x 也照样线性延伸），让尘魄从入口外平滑走进来；
        // 之前把 x<0 一律钉在入口点，会表现为「先静止一秒、然后突然跳到格子中心」。
        let p = pos2(ox + (e.x + 0.5) * cell, oy + (e.y + 0.5) * cell);
        painter.circle_filled(p, r, c);
        let edge = with_alpha(Color32::WHITE, 0.25 + 0.6 * e.progress);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, edge));
    }
    // 悬停某只怪时给出简报（路线 / 进度 / 血量）
    let mut tip: Option<String> = None;
    if let Some(hp) = ui.ctx().input(|i| i.pointer.hover_pos()) {
        if rect.contains(hp) {
            for e in &alive {
                let p = pos2(ox + (e.x + 0.5) * cell, oy + (e.y + 0.5) * cell);
                if p.distance(hp) < cell * 0.5 {
                    tip = Some(format!(
                        "事件 #{} · 路线 {} · 进度 {:.0}% · 血量 {:.0}",
                        e.wave + 1,
                        e.route + 1,
                        e.progress * 100.0,
                        e.hp
                    ));
                    break;
                }
            }
        }
    }
    painter.text(
        pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        tip.unwrap_or_else(|| format!("t = {:.2}s · 在场 {} 只", clock, alive.len())),
        font_id(9.5),
        if alive.is_empty() { TEXT_FAINT } else { GOLD },
    );
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}
