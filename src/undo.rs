//! 快照式撤销栈（每次离散编辑前压栈，拖拽类操作在开始时只压一次）。

use crate::model::Design;

pub struct UndoStack {
    past: Vec<(String, Design)>,
    future: Vec<(String, Design)>,
    limit: usize,
    last_push: Option<std::time::Instant>,
    last_label: String,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            limit: 120,
            last_push: None,
            last_label: String::new(),
        }
    }
}

impl UndoStack {
    /// 在任何修改之前调用。同名操作在 600ms 内会被合并，避免拖拽产生大量快照。
    pub fn push(&mut self, design: &Design, label: &str) {
        let now = std::time::Instant::now();
        let coalesce = self
            .last_push
            .map(|t| now.duration_since(t).as_millis() < 600)
            .unwrap_or(false)
            && self.last_label == label;
        self.last_push = Some(now);
        self.last_label = label.to_string();
        if coalesce {
            return;
        }
        if self.past.last().map(|(_, d)| d == design).unwrap_or(false) {
            return;
        }
        self.past.push((label.to_string(), design.clone()));
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// 强制压栈（不合并）
    pub fn push_force(&mut self, design: &Design, label: &str) {
        if self.past.last().map(|(_, d)| d == design).unwrap_or(false) {
            return;
        }
        self.past.push((label.to_string(), design.clone()));
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        self.future.clear();
        self.last_push = Some(std::time::Instant::now());
        self.last_label = label.to_string();
    }

    pub fn undo(&mut self, current: &Design) -> Option<(String, Design)> {
        let (label, prev) = self.past.pop()?;
        self.future.push((label.clone(), current.clone()));
        self.last_push = None;
        Some((label, prev))
    }

    pub fn redo(&mut self, current: &Design) -> Option<(String, Design)> {
        let (label, next) = self.future.pop()?;
        self.past.push((label.clone(), current.clone()));
        self.last_push = None;
        Some((label, next))
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.last_push = None;
    }
}
