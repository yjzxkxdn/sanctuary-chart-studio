// 圣土·白塔乐章 — 关卡编排台
// 以打谱为核心的统一时间轴编辑器（Rust + egui）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod autochart;
mod model;
mod theme;
mod ui_inspector;
mod ui_map;
mod ui_timeline;
mod undo;
mod widgets;

use app::Studio;

fn main() -> eframe::Result<()> {
    // 把 panic 写进日志文件：windows_subsystem="windows" 时看不到控制台输出，
    // 出问题时没有日志会完全失去线索。
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "?".into());
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "未知 panic".to_string()
        };
        let bt = std::backtrace::Backtrace::force_capture();
        let text = format!("PANIC at {loc}\n{msg}\n\n{bt}\n");
        let path = std::env::temp_dir().join("sanctuary-crash.txt");
        let _ = std::fs::write(&path, text);
        eprintln!("{msg}\n{loc}");
    }));

    // 无界面自检必须放在创建窗口之前：CI 运行器没有 GPU/OpenGL，
    // 一旦进入 eframe 就会以 PainterError 退出。
    if let Ok(target) = std::env::var("SCS_EXPORT") {
        std::process::exit(app::headless_export(&target));
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            // with_inner_size 是「逻辑点」，在 150% 缩放下会被放大 1.5 倍；
            // 这里给保守值，保证任何缩放下窗口都不会超出屏幕（超出会让右侧面板跑到画面外）。
            .with_inner_size([1060.0, 680.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("圣土 · 关卡编排台 — Sanctuary Chart Studio"),
        ..Default::default()
    };
    eframe::run_native(
        "sanctuary-chart-studio",
        options,
        Box::new(|cc| Ok(Box::new(Studio::new(cc)))),
    )
}
