//! 自动打谱 / 自动编排：节拍网格、相位推断、音符生成、出怪建议。

use crate::audio::Waveform;
use crate::model::{Design, Note, NoteKind, Wave, WaveKind, MIN_NOTE_GAP};

// 吸附与网格

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Snap {
    Off,
    Bar,
    Beat,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

impl Snap {
    pub const ALL: [Snap; 7] = [
        Snap::Off,
        Snap::Bar,
        Snap::Beat,
        Snap::Half,
        Snap::Quarter,
        Snap::Eighth,
        Snap::Sixteenth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Snap::Off => "关闭",
            Snap::Bar => "1/1",
            Snap::Beat => "1/4",
            Snap::Half => "1/8",
            Snap::Quarter => "1/12",
            Snap::Eighth => "1/16",
            Snap::Sixteenth => "1/24",
        }
    }

    /// 每拍切分数
    pub fn per_beat(self) -> f64 {
        match self {
            Snap::Off => 0.0,
            Snap::Bar => 0.25,
            Snap::Beat => 1.0,
            Snap::Half => 2.0,
            Snap::Quarter => 3.0,
            Snap::Eighth => 4.0,
            Snap::Sixteenth => 6.0,
        }
    }

    pub fn step(self, bpm: f64) -> Option<f64> {
        let per = self.per_beat();
        if per <= 0.0 {
            None
        } else {
            Some(60.0 / bpm / per)
        }
    }
}

/// 把时间吸附到网格（phase 为第一个拍点的偏移）
pub fn snap_time(t: f64, snap: Snap, bpm: f64, phase: f64) -> f64 {
    match snap.step(bpm) {
        Some(step) if step > 1e-6 => {
            let k = ((t - phase) / step).round();
            (phase + k * step).max(0.0)
        }
        _ => t.max(0.0),
    }
}

// 相位推断

/// 在 [0, beat) 内寻找让拍点能量最大的相位偏移。
pub fn infer_phase(waveform: &Waveform, bpm: f64) -> f64 {
    if waveform.is_empty() {
        return 0.0;
    }
    let beat = 60.0 / bpm.max(1.0);
    let hop = 0.02;
    let onset = waveform.onset_envelope(hop);
    if onset.is_empty() {
        return 0.0;
    }
    let dur = waveform.duration;
    let mut best = -1.0f32;
    let mut best_phase = 0.0;
    let steps = 120;
    for i in 0..steps {
        let phase = beat * i as f64 / steps as f64;
        let mut score = 0.0f32;
        let mut t = phase;
        while t < dur {
            let k = (t / hop).round() as isize;
            if k >= 0 && (k as usize) < onset.len() {
                score += onset[k as usize];
            }
            t += beat;
        }
        if score > best {
            best = score;
            best_phase = phase;
        }
    }
    best_phase
}

/// 用音头包络的自相关估计 BPM，返回 (BPM, 置信度 0—1)。
///
/// 做法：10ms 步长的音头包络 → 去均值 → 对 60—200 BPM 对应的延迟求归一化自相关，
/// 再用「以 120 BPM 为中心的对数先验」抑制倍频 / 半频误判，最后抛物线插值细化。
pub fn estimate_bpm(waveform: &Waveform) -> Option<(f64, f32)> {
    const HOP: f64 = 0.01;
    let env = waveform.onset_envelope(HOP);
    let n = env.len();
    if n < 400 {
        return None;
    }
    let mean = env.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
    let x: Vec<f64> = env.iter().map(|v| *v as f64 - mean).collect();

    // 搜索区间：BPM 下限 60 / 上限 200 对应的自相关滞后（单位 hop）
    const BPM_MIN: f64 = 60.0;
    const BPM_MAX: f64 = 200.0;
    let lag_min = (60.0 / BPM_MAX / HOP).round() as usize; // 30
    let lag_max = (60.0 / BPM_MIN / HOP).round() as usize; // 100
    let max_lag = lag_max.min(n / 2);
    if max_lag <= lag_min + 1 {
        return None;
    }

    let mut scores = vec![0f64; max_lag + 2];
    for lag in lag_min..=max_lag {
        let m = n - lag;
        let mut s = 0.0;
        for i in 0..m {
            s += x[i] * x[i + lag];
        }
        scores[lag] = s / m as f64;
    }
    let peak = scores[lag_min..=max_lag]
        .iter()
        .cloned()
        .fold(0f64, f64::max)
        .max(1e-12);

    // 速度先验：以 120 BPM 为中心，±1 个八度内衰减
    let mut best_lag = 0usize;
    let mut best_val = f64::MIN;
    for (lag, score) in scores.iter().enumerate().take(max_lag + 1).skip(lag_min) {
        let bpm = 60.0 / (lag as f64 * HOP);
        let oct = (bpm / 120.0).log2();
        let prior = (-(oct * oct) / (2.0 * 0.60 * 0.60)).exp();
        let v = (score / peak) * prior;
        if v > best_val {
            best_val = v;
            best_lag = lag;
        }
    }
    if best_lag == 0 {
        return None;
    }

    // 抛物线插值到亚采样精度
    let (a, b, c) = (scores[best_lag - 1], scores[best_lag], scores[best_lag + 1]);
    let denom = a - 2.0 * b + c;
    let delta = if denom.abs() > 1e-12 {
        (0.5 * (a - c) / denom).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let bpm = 60.0 / ((best_lag as f64 + delta) * HOP);

    // 置信度：峰值与「中位数」的比值。用中位数而不是均值，
    // 因为周期信号在 1 拍 / 2 拍 / 4 拍处都会出现高峰，均值会被这些谐波抬高。
    let mut sorted: Vec<f64> = scores[lag_min..=max_lag].to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2].max(1e-12);
    let ratio = (scores[best_lag] / median).max(0.0);
    // 比值 1.5 以下不可信，4 倍以上非常确定
    let conf = (((ratio - 1.5) / 2.5).clamp(0.0, 1.0)) as f32;
    Some((bpm.clamp(30.0, 240.0), conf))
}

// 打点测速（Tap Tempo）

/// 用户连续敲击得到的 BPM。超过 2 秒没敲视为重新开始。
#[derive(Default, Clone)]
pub struct TapTempo {
    taps: Vec<std::time::Instant>,
}

impl TapTempo {
    pub fn reset(&mut self) {
        self.taps.clear();
    }

    pub fn count(&self) -> usize {
        self.taps.len()
    }

    /// 记一次敲击，返回当前的 BPM 估计（至少需要 3 次敲击）。
    pub fn tap(&mut self) -> Option<f64> {
        let now = std::time::Instant::now();
        if let Some(last) = self.taps.last() {
            if now.duration_since(*last).as_secs_f64() > 2.0 {
                self.taps.clear();
            }
        }
        self.taps.push(now);
        if self.taps.len() > 16 {
            self.taps.remove(0);
        }
        self.bpm()
    }

    pub fn bpm(&self) -> Option<f64> {
        if self.taps.len() < 3 {
            return None;
        }
        // 用首尾总时长 / 间隔数，比逐段平均更稳
        let first = *self.taps.first()?;
        let last = *self.taps.last()?;
        let span = last.duration_since(first).as_secs_f64();
        let beats = (self.taps.len() - 1) as f64;
        if span <= 0.0 {
            return None;
        }
        Some((60.0 * beats / span).clamp(30.0, 240.0))
    }
}

// 自动打谱

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Density {
    Sparse,
    Normal,
    Dense,
}

impl Density {
    pub fn label(self) -> &'static str {
        match self {
            Density::Sparse => "稀疏",
            Density::Normal => "标准",
            Density::Dense => "密集",
        }
    }
    pub fn subdiv(self) -> f64 {
        match self {
            Density::Sparse => 2.0,
            Density::Normal => 2.0,
            Density::Dense => 4.0,
        }
    }
    /// 相对每拍的间隔（1 = 每拍一下）
    pub fn stride(self) -> f64 {
        match self {
            Density::Sparse => 2.0,
            Density::Normal => 1.0,
            Density::Dense => 0.5,
        }
    }
}

#[cfg(test)]
mod bpm_tests {
    use super::*;

    /// 合成一段带节拍的音频包络，检查 BPM 估计能否还原。
    fn fake_waveform(bpm: f64, secs: f64) -> Waveform {
        let rate = 44100.0;
        let frames = (rate * secs) as usize;
        let samples_per_beat = 60.0 / bpm * rate;
        let mut s = vec![0.0f32; frames];
        let mut t = 0.0f64;
        while (t as usize) < frames {
            let start = t as usize;
            // 每拍一个短促的衰减音头
            for k in 0..((rate * 0.02) as usize) {
                if start + k >= frames {
                    break;
                }
                let env = (-(k as f64) / (rate * 0.004)).exp() as f32;
                s[start + k] = env * 0.9;
            }
            t += samples_per_beat;
        }
        Waveform::build(&s, 1, rate)
    }

    #[test]
    fn estimate_bpm_recovers_tempo() {
        for &target in &[85.0f64, 100.0, 128.0, 150.0] {
            let wf = fake_waveform(target, 40.0);
            let (bpm, conf) = estimate_bpm(&wf).expect("应能估出 BPM");
            // 允许八度折返（音乐上等价），或直接命中
            let cands = [bpm, bpm * 2.0, bpm * 0.5];
            let hit = cands.iter().any(|c| (c - target).abs() < 3.0);
            assert!(hit, "目标 {target}，估计 {bpm:.2}（置信度 {conf:.2}）");
            assert!(conf > 0.5, "置信度应较高，实际 {conf:.2}");
        }
    }

    #[test]
    fn tap_tempo_needs_three_taps() {
        let mut t = TapTempo::default();
        assert!(t.tap().is_none(), "1 次敲击不应给出结果");
        assert!(t.tap().is_none(), "2 次敲击不应给出结果");
        // 模拟 120 BPM：手动塞入时间戳
        t.reset();
        let now = std::time::Instant::now();
        t.taps = vec![
            now,
            now + std::time::Duration::from_millis(500),
            now + std::time::Duration::from_millis(1000),
        ];
        let bpm = t.bpm().expect("3 次敲击应给出结果");
        assert!((bpm - 120.0).abs() < 1.0, "应约 120 BPM，实际 {bpm:.2}");
    }
}

#[derive(Clone, Debug)]
pub struct ChartGenOptions {
    pub density: Density,
    pub phase: f64,
    pub start: f64,
    pub end: f64,
    pub use_energy: bool,
    /// 0..1，越高只保留越强的音头
    pub gate: f32,
    /// 每 N 小节放一个长条，0 表示不放
    pub hold_every_bars: u32,
    pub hold_beats: f64,
    pub accent_downbeat: bool,
    pub min_gap: f64,
}

impl Default for ChartGenOptions {
    fn default() -> Self {
        Self {
            density: Density::Normal,
            phase: 0.0,
            start: 2.4,
            end: 0.0,
            use_energy: true,
            gate: 0.32,
            hold_every_bars: 4,
            hold_beats: 2.0,
            accent_downbeat: true,
            min_gap: MIN_NOTE_GAP,
        }
    }
}

/// 依照 BPM / 相位 / 能量生成谱面（结果已排序、去重叠）。
pub fn generate_notes(
    design: &Design,
    waveform: Option<&Waveform>,
    opt: &ChartGenOptions,
) -> Vec<Note> {
    let bpm = design.bpm.max(1.0);
    let beat = 60.0 / bpm;
    let sub = opt.density.subdiv();
    let step = beat / sub;
    let end = if opt.end > 0.0 {
        opt.end.min(design.duration)
    } else {
        design.duration
    };
    let start = opt.start.max(0.0);
    let onset = waveform
        .filter(|w| !w.is_empty())
        .map(|w| w.onset_envelope(0.02));

    let energy_at = |t: f64| -> f32 {
        match &onset {
            Some(env) => {
                let k = (t / 0.02).round() as isize;
                if k >= 0 && (k as usize) < env.len() {
                    env[k as usize]
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    };

    let mut notes: Vec<Note> = Vec::new();
    let mut blocked_until = f64::NEG_INFINITY;
    let mut idx: i64 = ((start - opt.phase) / step).ceil() as i64;
    let stride = opt.density.stride().max(0.25);
    let stride_steps = (sub * stride).max(1.0).round() as i64;

    if idx < 0 {
        idx = 0;
    }
    loop {
        let t = opt.phase + idx as f64 * step;
        if t > end - 0.05 {
            break;
        }
        idx += 1;
        if t < start - 1e-6 || t < blocked_until + opt.min_gap {
            continue;
        }
        let on_grid = idx % stride_steps.max(1) == 0;
        if !on_grid {
            continue;
        }
        let bar_pos = ((t - opt.phase) / beat).rem_euclid(4.0);
        let is_downbeat = bar_pos < 1e-3;

        // 能量门限：强拍永远保留，弱位按能量强弱筛选
        if opt.use_energy && onset.is_some() && !is_downbeat {
            let e = energy_at(t);
            let need = opt.gate
                * if bar_pos.fract().abs() < 1e-3 {
                    0.6
                } else {
                    1.0
                };
            if e < need {
                continue;
            }
        }

        // 长条
        let bar_index = ((t - opt.phase) / (beat * 4.0)).floor() as i64;
        let want_hold = opt.hold_every_bars > 0
            && is_downbeat
            && bar_index >= 0
            && bar_index % opt.hold_every_bars as i64 == 0
            && t > 4.0
            && t + opt.hold_beats * beat < end - 0.5;
        if want_hold {
            let hold_end = t + opt.hold_beats * beat;
            let mut n = Note::hold(round3(t), round3(hold_end));
            n.accent = opt.accent_downbeat;
            notes.push(n);
            blocked_until = hold_end;
            continue;
        }

        let mut n = Note::tap(round3(t));
        n.accent = opt.accent_downbeat && is_downbeat;
        notes.push(n);
        blocked_until = t;
    }

    dedupe(&mut notes, opt.min_gap);
    notes
}

pub fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// 去重并保证最小间隔
pub fn dedupe(notes: &mut Vec<Note>, min_gap: f64) {
    notes.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<Note> = Vec::with_capacity(notes.len());
    for n in notes.drain(..) {
        if let Some(prev) = out.last_mut() {
            if n.time - prev.tail() < min_gap {
                // 保留更靠后的那个，但长条不覆盖
                if prev.kind == NoteKind::Hold && prev.tail() > n.time {
                    continue;
                }
                if n.tail() < prev.time + min_gap {
                    continue;
                }
            }
        }
        out.push(n);
    }
    *notes = out;
}

// 出怪编排

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveIntensity {
    Relaxed,
    Standard,
    Intense,
}

impl WaveIntensity {
    pub fn label(self) -> &'static str {
        match self {
            WaveIntensity::Relaxed => "舒缓",
            WaveIntensity::Standard => "标准",
            WaveIntensity::Intense => "紧张",
        }
    }
    pub fn gap_scale(self) -> f64 {
        match self {
            WaveIntensity::Relaxed => 1.35,
            WaveIntensity::Standard => 1.0,
            WaveIntensity::Intense => 0.72,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WaveGenOptions {
    pub intensity: WaveIntensity,
    pub first_at: f64,
    pub special_every: usize,
    pub boss_at_ratio: f64,
    pub align_to_beat: bool,
    pub phase: f64,
}

impl Default for WaveGenOptions {
    fn default() -> Self {
        Self {
            intensity: WaveIntensity::Standard,
            first_at: 6.0,
            special_every: 5,
            boss_at_ratio: 0.82,
            align_to_beat: false,
            phase: 0.0,
        }
    }
}

/// 生成一条节奏递进的出怪曲线（复刻游戏 `recommendWaves` 并加入强度与对齐选项）。
pub fn suggest_waves(design: &Design, opt: &WaveGenOptions) -> Vec<Wave> {
    let duration = design.duration.max(1.0);
    let routes = design.paths.len().max(1);
    let beat = design.beat();
    let scale = opt.intensity.gap_scale();
    let mut waves = Vec::new();
    let mut t = opt.first_at.max(1.0);
    let mut i: usize = 0;

    while t < duration - 1.0 && waves.len() < 1000 {
        let special = i % opt.special_every.max(1) == 0;
        let late = t > duration * 0.73;
        let boss = !special && late && i % 13 == 8;
        let kind = if special {
            WaveKind::Special
        } else if boss {
            if i % 2 == 1 {
                WaveKind::Thief
            } else {
                WaveKind::Boss
            }
        } else if i % 5 == 2 {
            WaveKind::Ranged
        } else if i % 4 == 3 {
            WaveKind::Armored
        } else {
            WaveKind::Normal
        };

        let mut time = t;
        if opt.align_to_beat {
            time = snap_time(t, Snap::Beat, design.bpm, opt.phase);
        }
        // 后半段血量/速度递进
        let progress = (t / duration).clamp(0.0, 1.0);
        let health = (1.0 + progress * 1.6).min(2.6);
        let speed = 1.0 + progress * 0.15;

        waves.push(Wave {
            id: 0,
            time: round3(time),
            route: i % routes,
            kind,
            health: (health * 10.0).round() / 10.0,
            speed: (speed * 10.0).round() / 10.0,
        });

        let base = (3.4 - t / duration * 1.9).max(0.8);
        t += base * scale;
        i += 1;
    }

    if duration >= 30.0 && !waves.iter().any(|w| w.kind == WaveKind::Boss) {
        waves.push(Wave {
            id: 0,
            time: round3(duration * opt.boss_at_ratio),
            route: 0,
            kind: WaveKind::Boss,
            health: 2.0,
            speed: 0.9,
        });
    }
    // 保证至少一只特殊尘魄
    if !waves.iter().any(|w| w.kind == WaveKind::Special) {
        waves.push(Wave {
            id: 0,
            time: round3((duration * 0.25).max(4.0)),
            route: 0,
            kind: WaveKind::Special,
            health: 1.0,
            speed: 1.0,
        });
    }
    let _ = beat;
    waves.sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    waves.truncate(1000);
    for w in waves.iter_mut() {
        w.id = crate::model::next_id();
    }
    waves
}
