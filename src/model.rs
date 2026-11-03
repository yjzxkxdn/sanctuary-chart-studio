//! 关卡数据模型（与《圣土·白塔乐章》`design.mjs` / `level-schema.mjs` 完全对齐）
//!
//! 导出的 JSON 可直接被游戏内置的「关卡编纂室」导入使用。

use serde::{Deserialize, Serialize};

pub const COLS: i32 = 10;
pub const ROWS: i32 = 7;

pub const MIN_DURATION: f64 = 15.0;
pub const MAX_DURATION: f64 = 600.0;
pub const MIN_BPM: f64 = 30.0;
pub const MAX_BPM: f64 = 240.0;
pub const MIN_NOTE_GAP: f64 = 0.18;
pub const MIN_HOLD: f64 = 0.25;

// 配色

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    /// 背景渐变（3 段）
    pub bg: [&'static str; 3],
    pub tile: &'static str,
    pub road: &'static str,
    pub accent: &'static str,
}

pub const PALETTES: [Palette; 5] = [
    Palette {
        id: "amethyst",
        name: "紫金圣仪",
        bg: ["#100b19", "#251c32", "#453c32"],
        tile: "#362b45",
        road: "#70624f",
        accent: "#c5a3ee",
    },
    Palette {
        id: "rose",
        name: "绛月玫瑰",
        bg: ["#160d18", "#342135", "#483238"],
        tile: "#473047",
        road: "#80646d",
        accent: "#edb2d0",
    },
    Palette {
        id: "indigo",
        name: "靛海星砂",
        bg: ["#0c101f", "#202a43", "#3b3848"],
        tile: "#303b59",
        road: "#74798c",
        accent: "#acbdff",
    },
    Palette {
        id: "ivory",
        name: "月白鎏金",
        bg: ["#18151f", "#37313d", "#544b3b"],
        tile: "#514959",
        road: "#938471",
        accent: "#eee0bc",
    },
    Palette {
        id: "eclipse",
        name: "日蚀赤金",
        bg: ["#170e17", "#35202c", "#514034"],
        tile: "#4a3040",
        road: "#927052",
        accent: "#eab28c",
    },
];

pub fn palette_of(id: &str) -> Palette {
    PALETTES
        .iter()
        .copied()
        .find(|p| p.id == id)
        .unwrap_or(PALETTES[0])
}

// 音频
// 只支持载入本地音频文件；`track_id` / `audio_name` 仅用于导出兼容，
// 游戏读取时会自行回退到内置曲目。

// 出怪类型

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum WaveKind {
    Normal,
    Special,
    Armored,
    Boss,
    Thief,
    Ranged,
    Decoy,
}

impl WaveKind {
    pub const ALL: [WaveKind; 7] = [
        WaveKind::Normal,
        WaveKind::Special,
        WaveKind::Armored,
        WaveKind::Ranged,
        WaveKind::Boss,
        WaveKind::Thief,
        WaveKind::Decoy,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WaveKind::Normal => "普通史莱姆",
            WaveKind::Special => "特殊尘魄",
            WaveKind::Armored => "重装史莱姆",
            WaveKind::Boss => "BOSS",
            WaveKind::Thief => "窃取BOSS",
            WaveKind::Ranged => "远程尘魄",
            WaveKind::Decoy => "伪装普通怪",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            WaveKind::Normal => "普通",
            WaveKind::Special => "特殊",
            WaveKind::Armored => "重装",
            WaveKind::Ranged => "远程",
            WaveKind::Boss => "BOSS",
            WaveKind::Thief => "窃取",
            WaveKind::Decoy => "伪装",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            WaveKind::Normal => "基础尘魄，掉落共鸣资源",
            WaveKind::Special => "击杀后可获得塔台；每关至少一只",
            WaveKind::Armored => "高护甲，需要重击",
            WaveKind::Ranged => "远程攻击塔台与单位",
            WaveKind::Boss => "高血量首领，漏过扣 5 点生命",
            WaveKind::Thief => "窃取能量的首领，漏过代价最高",
            WaveKind::Decoy => "伪装成普通怪，诱导误判",
        }
    }

    pub fn is_boss(self) -> bool {
        matches!(self, WaveKind::Boss | WaveKind::Thief)
    }
}

// 出怪事件 / 音符

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wave {
    #[serde(default = "next_note_id", skip)]
    pub id: u64,
    pub time: f64,
    pub route: usize,
    pub kind: WaveKind,
    pub health: f64,
    pub speed: f64,
}

impl Wave {
    pub fn new(time: f64, route: usize, kind: WaveKind) -> Self {
        Self {
            id: next_note_id(),
            time,
            route,
            kind,
            health: 1.0,
            speed: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Tap,
    Hold,
}

impl NoteKind {
    pub fn label(self) -> &'static str {
        match self {
            NoteKind::Tap => "点按",
            NoteKind::Hold => "长条",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    #[serde(default = "next_note_id", skip)]
    pub id: u64,
    pub time: f64,
    pub kind: NoteKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    #[serde(default)]
    pub accent: bool,
}

fn next_note_id() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// 生成一个稳定 id（用于选择集）
pub fn next_id() -> u64 {
    next_note_id()
}

impl Note {
    pub fn tap(time: f64) -> Self {
        Self {
            id: next_note_id(),
            time,
            kind: NoteKind::Tap,
            end: None,
            accent: false,
        }
    }
    pub fn hold(time: f64, end: f64) -> Self {
        Self {
            id: next_note_id(),
            time,
            kind: NoteKind::Hold,
            end: Some(end),
            accent: false,
        }
    }
    pub fn tail(&self) -> f64 {
        match self.kind {
            NoteKind::Hold => self.end.unwrap_or(self.time),
            NoteKind::Tap => self.time,
        }
    }
    pub fn span(&self) -> f64 {
        self.tail() - self.time
    }
}

// 关卡

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    pub version: u32,
    pub key: String,
    pub name: String,
    pub duration: f64,
    pub bpm: f64,
    pub palette: String,
    #[serde(rename = "goalHp")]
    pub goal_hp: u32,
    #[serde(rename = "enemyHealth")]
    pub enemy_health: u32,
    pub speed: f64,
    pub paths: Vec<Vec<[i32; 2]>>,
    pub blocked: Vec<[i32; 2]>,
    pub waves: Vec<Wave>,
    pub chart: Vec<Note>,
    #[serde(rename = "audioName", default)]
    pub audio_name: String,
    #[serde(rename = "audioKey", default)]
    pub audio_key: Option<String>,
    #[serde(rename = "trackId", default)]
    pub track_id: String,
}

impl Default for Design {
    fn default() -> Self {
        Self::new_design()
    }
}

impl Design {
    /// 与游戏 `newDesign()` 一致的默认关卡。
    pub fn new_design() -> Self {
        Self {
            version: 1,
            key: format!("level-{}", now_ms()),
            name: "未命名的星途".to_string(),
            duration: 90.0,
            bpm: 85.0,
            palette: "amethyst".to_string(),
            goal_hp: 16,
            enemy_health: 65,
            speed: 0.7,
            paths: vec![vec![
                [-1, 2],
                [0, 2],
                [1, 2],
                [2, 2],
                [3, 2],
                [3, 3],
                [3, 4],
                [4, 4],
                [5, 4],
                [6, 4],
                [7, 4],
                [7, 3],
                [7, 2],
                [8, 2],
                [9, 2],
            ]],
            blocked: Vec::new(),
            waves: Vec::new(),
            chart: Vec::new(),
            audio_name: String::new(),
            audio_key: None,
            track_id: "local".to_string(),
        }
    }

    pub fn palette(&self) -> Palette {
        palette_of(&self.palette)
    }

    pub fn beat(&self) -> f64 {
        60.0 / self.bpm.max(1.0)
    }

    /// 小节长度（4/4 拍）
    pub fn bar(&self) -> f64 {
        self.beat() * 4.0
    }

    pub fn sort(&mut self) {
        self.waves.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.chart.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    pub fn is_road(&self, x: i32, y: i32) -> bool {
        self.paths.iter().any(|p| p.contains(&[x, y]))
    }

    pub fn is_blocked(&self, x: i32, y: i32) -> bool {
        self.blocked.contains(&[x, y])
    }

    pub fn route_len(&self, route: usize) -> usize {
        self.paths.get(route).map(|p| p.len()).unwrap_or(0)
    }

    /// 关卡统计
    pub fn stats(&self) -> Stats {
        let mut s = Stats::default();
        s.notes = self.chart.len();
        s.holds = self
            .chart
            .iter()
            .filter(|n| n.kind == NoteKind::Hold)
            .count();
        s.taps = s.notes - s.holds;
        s.time_judgements = self
            .chart
            .iter()
            .map(|n| if n.kind == NoteKind::Hold { 2 } else { 1 })
            .sum();
        s.waves = self.waves.len();
        for w in &self.waves {
            match w.kind {
                WaveKind::Normal => s.wave_kinds[0] += 1,
                WaveKind::Special => s.wave_kinds[1] += 1,
                WaveKind::Armored => s.wave_kinds[2] += 1,
                WaveKind::Ranged => s.wave_kinds[3] += 1,
                WaveKind::Boss => s.wave_kinds[4] += 1,
                WaveKind::Thief => s.wave_kinds[5] += 1,
                WaveKind::Decoy => s.wave_kinds[6] += 1,
            }
        }
        s.avg_nps = if self.duration > 0.0 {
            s.time_judgements as f64 / self.duration
        } else {
            0.0
        };
        // 峰值：1 秒滑窗
        let mut sorted: Vec<f64> = self.chart.iter().map(|n| n.time).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut peak = 0usize;
        let mut j = 0usize;
        for i in 0..sorted.len() {
            while sorted[i] - sorted[j] > 1.0 {
                j += 1;
            }
            peak = peak.max(i - j + 1);
        }
        s.peak_nps = peak as f64;
        s.roads = self.paths.len();
        s.road_cells = self.paths.iter().map(|p| p.len()).sum();
        s.blocked = self.blocked.len();
        s
    }

    /// 谱面密度直方图（每秒判定数），用于背景热度条
    pub fn density_histogram(&self, bins: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; bins];
        if self.duration <= 0.0 || bins == 0 {
            return out;
        }
        for n in &self.chart {
            let i = ((n.time / self.duration) * bins as f64) as usize;
            if i < bins {
                out[i] += if n.kind == NoteKind::Hold { 2.0 } else { 1.0 };
            }
        }
        out
    }
}

#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub notes: usize,
    pub taps: usize,
    pub holds: usize,
    pub time_judgements: usize,
    pub waves: usize,
    pub wave_kinds: [usize; 7],
    pub avg_nps: f64,
    pub peak_nps: f64,
    pub roads: usize,
    pub road_cells: usize,
    pub blocked: usize,
}

pub fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// 校验（与游戏规则一致，并补充编曲层面的提示）

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 游戏会拒绝导入
    Error,
    /// 可以导入但体验有问题
    Warning,
    /// 建议
    Tip,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "错误",
            Severity::Warning => "警告",
            Severity::Tip => "建议",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Level,
    Map,
    Waves,
    Chart,
}

impl Section {
    pub fn label(self) -> &'static str {
        match self {
            Section::Level => "关卡",
            Section::Map => "地形",
            Section::Waves => "出怪",
            Section::Chart => "谱面",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Issue {
    pub severity: Severity,
    pub section: Section,
    pub message: String,
    pub time: Option<f64>,
}

impl Issue {
    pub fn err(section: Section, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            section,
            message: msg.into(),
            time: None,
        }
    }
    pub fn warn(section: Section, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            section,
            message: msg.into(),
            time: None,
        }
    }
    pub fn tip(section: Section, msg: impl Into<String>) -> Self {
        Self {
            severity: Severity::Tip,
            section,
            message: msg.into(),
            time: None,
        }
    }
    pub fn at(mut self, t: f64) -> Self {
        self.time = Some(t);
        self
    }
}

impl Design {
    /// 严格校验：只返回会让游戏拒绝导入的问题（用于导出前检查）。
    pub fn validate(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let finite = |v: f64, min: f64, max: f64| v.is_finite() && v >= min && v <= max;

        // 基本字段
        if self.name.trim().is_empty() || self.name.chars().count() > 60 {
            out.push(Issue::err(Section::Level, "关卡名称需要 1—60 个字。"));
        }
        if !finite(self.duration, MIN_DURATION, MAX_DURATION) {
            out.push(Issue::err(Section::Level, "时长需在 15—600 秒之间。"));
        }
        if !finite(self.bpm, MIN_BPM, MAX_BPM) {
            out.push(Issue::err(Section::Level, "BPM 需在 30—240 之间。"));
        }
        if !finite(self.goal_hp as f64, 1.0, 999.0) {
            out.push(Issue::err(Section::Level, "目标血量需在 1—999 之间。"));
        }
        if !finite(self.enemy_health as f64, 10.0, 9999.0) || !finite(self.speed, 0.1, 3.0) {
            out.push(Issue::err(Section::Level, "尘魄血量 10—9999、移速 0.1—3。"));
        }
        if PALETTES.iter().all(|p| p.id != self.palette) {
            out.push(Issue::err(Section::Level, "请选择有效配色。"));
        }

        // 路线
        if self.paths.is_empty() || self.paths.len() > 3 {
            out.push(Issue::err(Section::Map, "路线数量需为 1—3 条。"));
        }
        let mut roads: std::collections::HashSet<(i32, i32)> = std::collections::HashSet::new();
        for (r, path) in self.paths.iter().enumerate() {
            if path.len() < 3 || path.len() > 100 {
                out.push(Issue::err(
                    Section::Map,
                    format!("路线 {} 至少 3 格，最多 100 格。", r + 1),
                ));
                continue;
            }
            let mut seen = std::collections::HashSet::new();
            for (i, p) in path.iter().enumerate() {
                if p[0] < -1 || p[0] >= COLS || p[1] < 0 || p[1] >= ROWS {
                    out.push(Issue::err(
                        Section::Map,
                        format!("路线 {} 含无效地块 ({},{}).", r + 1, p[0], p[1]),
                    ));
                    continue;
                }
                if !seen.insert((p[0], p[1])) {
                    out.push(Issue::err(
                        Section::Map,
                        format!("路线 {} 不能重复经过同一格 ({},{}).", r + 1, p[0], p[1]),
                    ));
                }
                roads.insert((p[0], p[1]));
                if i > 0 {
                    let q = path[i - 1];
                    if (p[0] - q[0]).abs() + (p[1] - q[1]).abs() != 1 {
                        out.push(Issue::warn(
                            Section::Map,
                            format!(
                                "路线 {} 第 {} 段不连续（({},{})) → ({},{}))）",
                                r + 1,
                                i,
                                q[0],
                                q[1],
                                p[0],
                                p[1]
                            ),
                        ));
                    }
                }
            }
        }

        // 障碍
        if self.blocked.len() > 70 {
            out.push(Issue::err(Section::Map, "不可建造地块最多 70 个。"));
        }
        for p in &self.blocked {
            if p[0] < 0 || p[0] >= COLS || p[1] < 0 || p[1] >= ROWS || roads.contains(&(p[0], p[1]))
            {
                out.push(Issue::err(
                    Section::Map,
                    format!("障碍必须位于路线外的有效地块 ({},{}).", p[0], p[1]),
                ));
            }
        }

        // 出怪
        if self.waves.is_empty() || self.waves.len() > 1000 {
            out.push(Issue::err(Section::Waves, "需要 1—1000 个出怪事件。"));
        }
        for w in &self.waves {
            if !finite(w.time, 0.0, self.duration)
                || w.route >= self.paths.len()
                || !finite(w.health, 0.1, 20.0)
                || !finite(w.speed, 0.1, 4.0)
            {
                out.push(
                    Issue::err(
                        Section::Waves,
                        format!("{:.2}s 的出怪事件参数不正确。", w.time),
                    )
                    .at(w.time),
                );
            }
        }
        if !self.waves.iter().any(|w| w.kind == WaveKind::Special) {
            out.push(Issue::err(
                Section::Waves,
                "至少需要一只特殊尘魄，才能获得塔台。",
            ));
        }

        // 谱面
        if self.chart.is_empty() || self.chart.len() > 3000 {
            out.push(Issue::err(Section::Chart, "需要 1—3000 个音符。"));
        }
        let mut sorted = self.chart.clone();
        sorted.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, n) in sorted.iter().enumerate() {
            if !finite(n.time, 0.0, self.duration) {
                out.push(
                    Issue::err(Section::Chart, format!("音符时间无效：{:.3}s", n.time)).at(n.time),
                );
            }
            if n.kind == NoteKind::Hold {
                match n.end {
                    Some(e) if finite(e, n.time + MIN_HOLD, self.duration) => {}
                    _ => {
                        out.push(
                            Issue::err(Section::Chart, format!("长条尾部时间无效：{:.3}s", n.time))
                                .at(n.time),
                        );
                    }
                }
            } else if n.end.is_some() {
                out.push(
                    Issue::warn(
                        Section::Chart,
                        format!("点按音符不应带尾部时间：{:.3}s", n.time),
                    )
                    .at(n.time),
                );
            }
            if i > 0 {
                let prev = &sorted[i - 1];
                if n.time - prev.tail() < MIN_NOTE_GAP {
                    out.push(
                        Issue::err(
                            Section::Chart,
                            format!(
                                "音符重叠：{:.3}s 与 {:.3}s 间隔不足 0.18s。",
                                prev.time, n.time
                            ),
                        )
                        .at(n.time),
                    );
                }
            }
        }
        out
    }

    /// 编辑期提示：不阻断导出，但会提醒手感 / 平衡问题。
    pub fn lint(&self) -> Vec<Issue> {
        let mut out = Vec::new();
        let st = self.stats();

        if self.chart.len() > 20 && st.avg_nps > 3.2 {
            out.push(Issue::warn(
                Section::Chart,
                format!("平均 {:.2} NPS 偏高，判定箭头密集，建议删减。", st.avg_nps),
            ));
        }
        if st.peak_nps > 8.0 {
            out.push(Issue::warn(
                Section::Chart,
                format!("峰值 {:.0} NPS，可能出现无法手按的连打。", st.peak_nps),
            ));
        }
        if let (Some(first), Some(last)) = (
            self.chart
                .iter()
                .map(|n| n.time)
                .fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.min(t)))),
            self.chart
                .iter()
                .map(|n| n.time)
                .fold(None, |a: Option<f64>, t| Some(a.map_or(t, |x| x.max(t)))),
        ) {
            if first > 5.0 {
                out.push(Issue::tip(
                    Section::Chart,
                    "开场 5 秒无音符，可作为前奏留白（此游戏前 2.4 秒固定不计谱）。",
                ));
            }
            if self.duration - last > 8.0 {
                out.push(Issue::tip(
                    Section::Chart,
                    format!(
                        "结尾 {} 秒无音符，考虑缩短时长。",
                        (self.duration - last).round()
                    ),
                ));
            }
        }
        if st.holds == 0 && st.notes > 60 {
            out.push(Issue::tip(
                Section::Chart,
                "没有任何长条，节奏层次会比较单一。",
            ));
        }

        // 出怪节奏
        if self.waves.len() >= 4 {
            let gaps: Vec<f64> = self
                .waves
                .windows(2)
                .map(|w| w[1].time - w[0].time)
                .collect();
            let min_gap = gaps.iter().cloned().fold(f64::INFINITY, f64::min);
            if min_gap < 0.35 {
                out.push(Issue::warn(
                    Section::Waves,
                    format!("出怪最小间隔 {:.2}s，容易瞬间堆叠。", min_gap),
                ));
            }
        }
        if !self.waves.iter().any(|w| w.kind.is_boss()) && self.duration >= 60.0 {
            out.push(Issue::tip(
                Section::Waves,
                "没有首领事件，关卡缺少高潮；可在 70%—90% 处加入 BOSS。",
            ));
        }
        let specials = self
            .waves
            .iter()
            .filter(|w| w.kind == WaveKind::Special)
            .count();
        if specials == 1 && self.duration > 90.0 {
            out.push(Issue::tip(
                Section::Waves,
                "长关卡只有一只特殊尘魄，塔台获取机会偏少。",
            ));
        }
        if self.paths.iter().any(|p| p.len() < 6) {
            out.push(Issue::tip(
                Section::Map,
                "存在很短的路线，敌人会很快抵达目标，检查目标血量是否够撑。",
            ));
        }
        if self.blocked.is_empty() {
            out.push(Issue::tip(
                Section::Map,
                "没有不可建造地块，塔台铺法会缺少取舍。",
            ));
        }

        // 平衡
        let ttk = self.enemy_health as f64 / (self.goal_hp.max(1) as f64);
        if ttk > 20.0 {
            out.push(Issue::tip(
                Section::Level,
                "尘魄血量相对目标血量偏高，前期容易漏怪。",
            ));
        }
        for (i, p) in self.paths.iter().enumerate() {
            if p.len() > 2 {
                let last = p.last().unwrap();
                if last[0] < 0 || last[1] < 0 || last[0] >= COLS || last[1] >= ROWS {
                    out.push(Issue::warn(
                        Section::Map,
                        format!(
                            "路线 {} 的终点 ({},{})) 不在场地内，无法抵达目标。",
                            i + 1,
                            last[0],
                            last[1]
                        ),
                    ));
                }
                if p[0][0] != -1 {
                    out.push(Issue::tip(
                        Section::Map,
                        format!(
                            "路线 {} 起点不在左侧入口（x=-1），敌人会从场地内部出现。",
                            i + 1
                        ),
                    ));
                }
            }
        }
        out
    }

    /// 全部问题（错误在前）
    pub fn all_issues(&self) -> Vec<Issue> {
        let mut v = self.validate();
        v.extend(self.lint());
        v.sort_by_key(|i| i.severity);
        v
    }

    pub fn export_json(&self) -> Result<String, String> {
        let errs = self.validate();
        if errs.iter().any(|e| e.severity == Severity::Error) {
            return Err(errs
                .iter()
                .filter(|e| e.severity == Severity::Error)
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let mut d = self.clone();
        d.sort();
        serde_json::to_string_pretty(&d).map_err(|e| e.to_string())
    }

    pub fn import_json(text: &str) -> Result<Design, String> {
        let mut d: Design =
            serde_json::from_str(text).map_err(|e| format!("JSON 解析失败：{e}"))?;
        if d.name.trim().is_empty() {
            d.name = "导入的星途".to_string();
        }
        if d.palette.is_empty() {
            d.palette = "amethyst".to_string();
        }
        if d.track_id.is_empty() {
            d.track_id = "local".to_string();
        }
        d.key = format!("level-{}", now_ms());
        Ok(d)
    }
}

// 怪物图鉴（与游戏 enemy-types.mjs 的真实数值一致）

pub struct EnemyType {
    pub id: &'static str,
    pub name: &'static str,
    /// 相对基础血量的倍率
    pub health: f64,
    /// 相对基础移速的倍率
    pub speed: f64,
    /// 漏过时扣掉的目标血量
    pub leak: f64,
    /// 攻击距离（格）
    pub range: f64,
    pub desc: &'static str,
}

pub const ENEMY_TYPES: [EnemyType; 9] = [
    EnemyType {
        id: "slime",
        name: "游离尘魄",
        health: 1.0,
        speed: 1.0,
        leak: 1.0,
        range: 2.0,
        desc: "基础近程个体，沿道路接近保护目标。",
    },
    EnemyType {
        id: "armored",
        name: "披甲尘魄",
        health: 3.0,
        speed: 0.8,
        leak: 2.0,
        range: 2.0,
        desc: "体表覆甲，生命较高、移动较慢。",
    },
    EnemyType {
        id: "ranged",
        name: "射影尘魄",
        health: 1.0,
        speed: 1.0,
        leak: 1.0,
        range: 5.0,
        desc: "携带法球，可从远处攻击同行者。",
    },
    EnemyType {
        id: "runner",
        name: "疾羽尘魄",
        health: 1.2,
        speed: 1.9,
        leak: 1.0,
        range: 2.0,
        desc: "移速为普通个体的 1.9 倍，适合用减速与控制拦截。",
    },
    EnemyType {
        id: "bulwark",
        name: "重垒尘魄",
        health: 7.0,
        speed: 0.5,
        leak: 3.0,
        range: 2.0,
        desc: "生命是普通个体的 7 倍，漏过扣 3 点目标生命。",
    },
    EnemyType {
        id: "walker",
        name: "负甲尘魄",
        health: 4.5,
        speed: 0.65,
        leak: 3.0,
        range: 2.0,
        desc: "双足重装轮廓，移动缓慢，生命与攻击较高。",
    },
    EnemyType {
        id: "prism",
        name: "浮棱尘魄",
        health: 2.1,
        speed: 1.1,
        leak: 2.0,
        range: 5.0,
        desc: "浮游棱核，移动较快，能远程攻击同行者。",
    },
    EnemyType {
        id: "longbow",
        name: "贯星尘魄",
        health: 3.2,
        speed: 0.65,
        leak: 2.0,
        range: 6.0,
        desc: "悬浮长弓与金色星矢，蓄力后锁定落点。",
    },
    EnemyType {
        id: "bell",
        name: "震铃尘魄",
        health: 4.0,
        speed: 0.55,
        leak: 2.0,
        range: 5.5,
        desc: "淡玫瑰色悬铃，对落点周围造成范围伤害。",
    },
];

/// 把图鉴条目套用到出怪事件上（保留路线与时间，只改数值）。
pub fn apply_enemy_template(w: &mut Wave, e: &EnemyType) {
    w.health = ((e.health * 10.0).round() / 10.0).clamp(0.1, 20.0);
    w.speed = ((e.speed * 100.0).round() / 100.0).clamp(0.1, 4.0);
}

pub fn fmt_time(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor() as i64;
    let s = t - m as f64 * 60.0;
    format!("{:02}:{:06.3}", m, s)
}

// 战斗推演：单个出怪事件的实际数值（与游戏 `enemySpawnStats` 完全一致）

#[derive(Clone, Copy, Debug)]
pub struct WaveStats {
    /// 实际生命值
    pub hp: f64,
    /// 实际移速（格 / 秒）
    pub speed: f64,
    /// 漏过时扣掉的目标血量
    pub damage: f64,
    /// 从入口走到目标所需时间（秒）
    pub travel: f64,
    /// 后半段血量递进系数
    pub ramp: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct LiveEnemy {
    pub wave: usize,
    pub route: usize,
    pub kind: WaveKind,
    /// 网格坐标（入口 x = -1）
    pub x: f32,
    pub y: f32,
    /// 0..1 行程进度
    pub progress: f32,
    pub hp: f64,
}

/// 沿时间轴的三条曲线，用于「难度平衡」视图。
#[derive(Clone, Debug, Default)]
pub struct Curves {
    /// 每个采样点代表的秒数
    pub bin: f64,
    /// 每秒音符数
    pub nps: Vec<f32>,
    /// 在场尘魄的总血量
    pub threat: Vec<f32>,
    /// 两者归一化后的乘积（双压程度）
    pub conflict: Vec<f32>,
    pub peak_nps: f32,
    pub peak_threat: f32,
    pub peak_conflict: f32,
    pub avg_nps: f32,
}

impl Design {
    pub fn wave_stats(&self, w: &Wave) -> WaveStats {
        let dur = self.duration.max(1.0);
        let ramp = 1.0 + ((w.time / dur) - 0.3).max(0.0) * 3.5;
        let kind_hp = if w.kind.is_boss() {
            19.0
        } else if w.kind == WaveKind::Special {
            0.9
        } else if w.kind == WaveKind::Armored {
            3.0
        } else {
            1.0
        };
        let kind_speed = if w.kind.is_boss() {
            0.58
        } else if w.kind == WaveKind::Armored {
            0.8
        } else {
            1.0
        };
        let damage = if w.kind.is_boss() {
            5.0
        } else if w.kind == WaveKind::Armored {
            2.0
        } else {
            1.0
        };
        let hp = self.enemy_health as f64 * w.health.max(0.1) * ramp * kind_hp;
        let speed = (self.speed * w.speed.max(0.1) * kind_speed).max(0.01);
        let len = self.route_len(w.route).max(2) as f64;
        WaveStats {
            hp,
            speed,
            damage,
            travel: (len - 1.0) / speed,
            ramp,
        }
    }

    /// 路线上的插值坐标，progress 0 = 入口，1 = 目标。
    pub fn point_on_route(&self, route: usize, progress: f64) -> Option<(f32, f32)> {
        let path = self.paths.get(route)?;
        if path.len() < 2 {
            return None;
        }
        let total = (path.len() - 1) as f64;
        let d = (progress.clamp(0.0, 1.0) * total).min(total);
        let i = (d.floor() as usize).min(path.len() - 2);
        let f = (d - i as f64) as f32;
        let a = path[i];
        let b = path[i + 1];
        Some((
            a[0] as f32 + (b[0] - a[0]) as f32 * f,
            a[1] as f32 + (b[1] - a[1]) as f32 * f,
        ))
    }

    /// 某一时刻仍在路线上的尘魄。
    pub fn enemies_alive_at(&self, t: f64) -> Vec<LiveEnemy> {
        let mut out = Vec::new();
        for (i, w) in self.waves.iter().enumerate() {
            let st = self.wave_stats(w);
            if t < w.time || t > w.time + st.travel {
                continue;
            }
            let progress = ((t - w.time) / st.travel.max(1e-3)).clamp(0.0, 1.0);
            if let Some((x, y)) = self.point_on_route(w.route, progress) {
                out.push(LiveEnemy {
                    wave: i,
                    route: w.route,
                    kind: w.kind,
                    x,
                    y,
                    progress: progress as f32,
                    hp: st.hp,
                });
            }
        }
        out
    }

    /// 一拍的位置（0..1 表示在拍内的相对位置），用于分音着色。
    pub fn beat_phase(&self, t: f64, phase: f64) -> f64 {
        let beat = self.beat().max(1e-6);
        let k = (t - phase) / beat;
        k - k.floor()
    }

    pub fn bar_index(&self, t: f64, phase: f64) -> f64 {
        (t - phase).max(0.0) / self.bar().max(1e-6)
    }

    /// 时间轴吸附
    pub fn snap_to(&self, t: f64, step: Option<f64>, phase: f64) -> f64 {
        match step {
            Some(s) if s > 1e-6 => {
                let k = ((t - phase) / s).round();
                (phase + k * s).max(0.0)
            }
            _ => t.clamp(0.0, self.duration),
        }
    }

    /// 三条平衡曲线。
    pub fn curves(&self, bins: usize) -> Curves {
        let bins = bins.max(16);
        let mut c = Curves {
            bin: self.duration.max(0.1) / bins as f64,
            nps: vec![0.0; bins],
            threat: vec![0.0; bins],
            conflict: vec![0.0; bins],
            ..Default::default()
        };
        let w = c.bin.max(1e-6);
        for n in &self.chart {
            let i = (n.time / w).floor() as isize;
            if i >= 0 && (i as usize) < bins {
                c.nps[i as usize] += 1.0 / w as f32;
            }
        }
        for wave in &self.waves {
            let st = self.wave_stats(wave);
            let t0 = wave.time.max(0.0);
            let t1 = (wave.time + st.travel).min(self.duration);
            if t1 <= t0 {
                continue;
            }
            let i0 = (t0 / w).floor().max(0.0) as usize;
            let i1 = ((t1 / w).ceil() as usize).min(bins);
            for i in i0..i1 {
                let a = (i as f64 * w).max(t0);
                let b = ((i + 1) as f64 * w).min(t1);
                let overlap = (b - a).max(0.0);
                c.threat[i] += (st.hp * (overlap / w)) as f32;
            }
        }
        c.peak_nps = c.nps.iter().cloned().fold(0.0f32, f32::max);
        c.peak_threat = c.threat.iter().cloned().fold(0.0f32, f32::max);
        c.avg_nps = if c.nps.is_empty() {
            0.0
        } else {
            c.nps.iter().sum::<f32>() / c.nps.len() as f32
        };
        let nn = c.peak_nps.max(1e-3);
        let tn = c.peak_threat.max(1e-3);
        for i in 0..bins {
            c.conflict[i] = (c.nps[i] / nn) * (c.threat[i] / tn);
        }
        c.peak_conflict = c.conflict.iter().cloned().fold(0.0f32, f32::max);
        c
    }

    /// 双压最严重的若干时间点（用于「平衡」视图给出可点击建议）。
    pub fn conflict_hotspots(&self, count: usize) -> Vec<(f64, f32)> {
        let c = self.curves(600);
        let mut peaks: Vec<(f64, f32)> = c
            .conflict
            .iter()
            .enumerate()
            .map(|(i, v)| (i as f64 * c.bin, *v))
            .filter(|(_, v)| *v > 0.55)
            .collect();
        peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        peaks.truncate(count);
        peaks
    }
}

// 兼容性自测：导出的 JSON 必须能通过游戏 `validateDesign` 的全部规则

#[cfg(test)]
mod compat_tests {
    use super::*;

    /// 复刻游戏 `design.mjs::newDesign()` 的默认态 + 自动生成内容，
    /// 检查导出的 JSON 完全符合 `level-schema.mjs::validateDesign`。
    fn demo() -> Design {
        let mut d = Design::new_design();
        let opt = crate::autochart::ChartGenOptions {
            phase: 0.0,
            end: d.duration,
            ..Default::default()
        };
        d.chart = crate::autochart::generate_notes(&d, None, &opt);
        let ow = crate::autochart::WaveGenOptions {
            phase: 0.0,
            ..Default::default()
        };
        d.waves = crate::autochart::suggest_waves(&d, &ow);
        d.sort();
        d
    }

    #[test]
    fn exported_json_satisfies_game_rules() {
        let d = demo();
        let json = d.export_json().expect("示例关卡应可导出");

        // 字段名必须与游戏一致
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        for key in [
            "version",
            "key",
            "name",
            "duration",
            "bpm",
            "palette",
            "goalHp",
            "enemyHealth",
            "speed",
            "paths",
            "blocked",
            "waves",
            "chart",
            "audioName",
            "audioKey",
            "trackId",
        ] {
            assert!(v.get(key).is_some(), "导出缺少字段 {key}");
        }

        // 数值范围
        assert!((15.0..=600.0).contains(&d.duration));
        assert!((30.0..=240.0).contains(&d.bpm));
        assert!((1..=999).contains(&d.goal_hp));
        assert!((10..=9999).contains(&d.enemy_health));
        assert!((0.1..=3.0).contains(&d.speed));
        assert!(!d.name.trim().is_empty() && d.name.chars().count() <= 60);
        assert!(PALETTES.iter().any(|p| p.id == d.palette));

        // 路线
        assert!((1..=3).contains(&d.paths.len()));
        let mut roads = std::collections::HashSet::new();
        for (r, path) in d.paths.iter().enumerate() {
            assert!(
                (3..=100).contains(&path.len()),
                "路线 {} 长度 {} 非法",
                r + 1,
                path.len()
            );
            let mut seen = std::collections::HashSet::new();
            for (i, p) in path.iter().enumerate() {
                assert!(
                    (-1..COLS).contains(&p[0]) && (0..ROWS).contains(&p[1]),
                    "路线 {} 地块越界",
                    r + 1
                );
                assert!(seen.insert((p[0], p[1])), "路线 {} 重复经过同一格", r + 1);
                roads.insert((p[0], p[1]));
                if i > 0 {
                    let q = path[i - 1];
                    assert_eq!(
                        (p[0] - q[0]).abs() + (p[1] - q[1]).abs(),
                        1,
                        "路线 {} 第 {} 段不相邻",
                        r + 1,
                        i
                    );
                }
            }
        }

        // 障碍
        assert!(d.blocked.len() <= 70);
        for b in &d.blocked {
            assert!((0..COLS).contains(&b[0]) && (0..ROWS).contains(&b[1]));
            assert!(!roads.contains(&(b[0], b[1])), "障碍压在了路线上");
        }

        // 出怪
        assert!((1..=1000).contains(&d.waves.len()));
        for w in &d.waves {
            assert!((0.0..=d.duration).contains(&w.time), "出怪时间越界");
            assert!(w.route < d.paths.len(), "出怪路线越界");
            assert!((0.1..=20.0).contains(&w.health));
            assert!((0.1..=4.0).contains(&w.speed));
        }
        assert!(
            d.waves.iter().any(|w| w.kind == WaveKind::Special),
            "必须至少有一只特殊尘魄"
        );

        // 谱面
        assert!((1..=3000).contains(&d.chart.len()));
        let mut sorted = d.chart.clone();
        sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        for (i, n) in sorted.iter().enumerate() {
            assert!((0.0..=d.duration).contains(&n.time));
            if n.kind == NoteKind::Hold {
                let e = n.end.expect("长条必须有尾部");
                assert!(e >= n.time + MIN_HOLD && e <= d.duration, "长条尾部非法");
            }
            if i > 0 {
                let prev = &sorted[i - 1];
                assert!(
                    n.time - prev.tail() >= MIN_NOTE_GAP - 1e-9,
                    "音符间隔 < 0.18s：{:.3} 与 {:.3}",
                    prev.time,
                    n.time
                );
            }
        }
    }

    #[test]
    fn wave_stats_match_game_formula() {
        // 游戏 enemySpawnStats：ramp = 1 + max(0, t/duration - 0.3) * 3.5
        let mut d = Design::new_design();
        d.duration = 100.0;
        d.enemy_health = 100;
        d.speed = 1.0;
        let w = Wave {
            id: 1,
            time: 30.0,
            route: 0,
            kind: WaveKind::Normal,
            health: 1.0,
            speed: 1.0,
        };
        let st = d.wave_stats(&w);
        // t/duration = 0.3 → ramp = 1.0 → hp = 100
        assert!((st.hp - 100.0).abs() < 1e-6, "hp = {}", st.hp);
        assert!((st.speed - 1.0).abs() < 1e-6);

        // 披甲：×3 血量、×0.8 移速
        let mut wa = w.clone();
        wa.kind = WaveKind::Armored;
        let sa = d.wave_stats(&wa);
        assert!((sa.hp - 300.0).abs() < 1e-6);
        assert!((sa.speed - 0.8).abs() < 1e-6);
        assert!((sa.damage - 2.0).abs() < 1e-6);

        // BOSS：×19 血量、×0.58 移速、漏过扣 5
        let mut wb = w.clone();
        wb.kind = WaveKind::Boss;
        let sb = d.wave_stats(&wb);
        assert!((sb.hp - 1900.0).abs() < 1e-6);
        assert!((sb.speed - 0.58).abs() < 1e-6);
        assert!((sb.damage - 5.0).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_import_export() {
        let d = demo();
        let json = d.export_json().unwrap();
        let back = Design::import_json(&json).unwrap();
        assert_eq!(back.paths, d.paths);
        assert_eq!(back.chart.len(), d.chart.len());
        assert_eq!(back.waves.len(), d.waves.len());
        assert_eq!(back.duration, d.duration);
    }

    /// 自动编排必须产出「能通关」的骨架：特殊尘魄拿塔台 + 首领做高潮。
    #[test]
    fn generated_waves_contain_boss_and_special() {
        let d = demo();
        assert!(
            d.waves.iter().any(|w| w.kind == WaveKind::Special),
            "缺少特殊尘魄"
        );
        assert!(
            d.waves.iter().any(|w| w.kind.is_boss()),
            "缺少首领事件（{} 个事件）",
            d.waves.len()
        );
        // 后半段血量必须显著高于前半段（递进感）
        let early = d
            .waves
            .iter()
            .find(|w| w.time < d.duration * 0.3 && w.kind == WaveKind::Normal);
        let late = d.waves.iter().rev().find(|w| w.kind == WaveKind::Normal);
        if let (Some(a), Some(b)) = (early, late) {
            assert!(
                b.health >= a.health,
                "血量没有递进：{:.2} → {:.2}",
                a.health,
                b.health
            );
        }
    }
}
