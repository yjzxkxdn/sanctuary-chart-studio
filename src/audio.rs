//! 音频：symphonia 解码 + cpal 低延迟播放 + 波形峰值。
//!
//! * 解码后的采样存在 `Shared`（Arc）里，UI 线程只读；音频回调独占推进播放位置。
//! * 回调内做线性重采样，因此 44.1k / 48k / 22.05k 素材都能直接播放。
//! * 节拍器与试听提示音在回调内合成，保证与音频采样级对齐。

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn a2f(v: u64) -> f64 {
    f64::from_bits(v)
}
fn f2a(v: f64) -> u64 {
    v.to_bits()
}

pub struct Shared {
    samples: Vec<f32>,
    src_channels: usize,
    src_rate: f64,
    frames: usize,
    duration: f64,

    /// 播放位置（源采样帧，浮点）
    pos_bits: AtomicU64,
    playing: AtomicBool,
    gain_bits: AtomicU32,

    metronome: AtomicBool,
    bpm_bits: AtomicU64,
    phase_bits: AtomicU64,
    /// 变速播放倍率（不改变音高的简单重采样变速，用于慢速对谱）
    rate_bits: AtomicU64,
    /// 请求节拍器重新对拍
    resync: AtomicBool,
    /// 请求播放一声提示音
    blip_req: AtomicBool,
}

impl Shared {
    fn new(samples: Vec<f32>, channels: usize, rate: f64) -> Self {
        let channels = channels.max(1);
        let rate = rate.max(1.0);
        let frames = samples.len() / channels;
        Self {
            samples,
            src_channels: channels,
            src_rate: rate,
            frames,
            duration: frames as f64 / rate,
            pos_bits: AtomicU64::new(0f64.to_bits()),
            playing: AtomicBool::new(false),
            gain_bits: AtomicU32::new(0.85f32.to_bits()),
            metronome: AtomicBool::new(false),
            bpm_bits: AtomicU64::new(85f64.to_bits()),
            phase_bits: AtomicU64::new(0f64.to_bits()),
            rate_bits: AtomicU64::new(1f64.to_bits()),
            resync: AtomicBool::new(true),
            blip_req: AtomicBool::new(false),
        }
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }
}

/// 波形峰值缓存（每 4ms 一个桶，含 min/max/rms）
pub struct Waveform {
    pub bucket: f64,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    pub rms: Vec<f32>,
    pub duration: f64,
}

impl Default for Waveform {
    fn default() -> Self {
        Self::empty()
    }
}

impl Waveform {
    pub fn empty() -> Self {
        Self {
            bucket: 0.004,
            min: Vec::new(),
            max: Vec::new(),
            rms: Vec::new(),
            duration: 0.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.is_empty()
    }

    pub fn build(samples: &[f32], channels: usize, rate: f64) -> Self {
        let bucket_secs = 0.004;
        let per = ((rate * bucket_secs) as usize).max(1);
        let channels = channels.max(1);
        let frames = samples.len() / channels;
        let n = frames.div_ceil(per).max(1);
        let mut min = Vec::with_capacity(n);
        let mut max = Vec::with_capacity(n);
        let mut rms = Vec::with_capacity(n);
        for i in 0..n {
            let start = i * per;
            let end = ((i + 1) * per).min(frames);
            let mut lo = 0.0f32;
            let mut hi = 0.0f32;
            let mut acc = 0.0f64;
            let mut count = 0usize;
            for f in start..end {
                let mut v = 0.0f32;
                for c in 0..channels {
                    v += samples[f * channels + c];
                }
                v /= channels as f32;
                lo = lo.min(v);
                hi = hi.max(v);
                acc += (v as f64) * (v as f64);
                count += 1;
            }
            min.push(lo);
            max.push(hi);
            rms.push(if count > 0 {
                (acc / count as f64).sqrt() as f32
            } else {
                0.0
            });
            if end >= frames {
                break;
            }
        }
        Self {
            bucket: per as f64 / rate,
            min,
            max,
            rms,
            duration: frames as f64 / rate,
        }
    }

    /// 在 [t0, t1] 秒区间内聚合成 `width` 个峰值 (min, max, rms)，供绘制使用。
    /// 注意：绘制波形时应按音频桶边界遍历（见 ui_timeline::draw_waveform），
    /// 按屏幕列调用此函数会因列边界漂移导致画面抖动。
    #[allow(dead_code)]
    pub fn range_peaks(&self, t0: f64, t1: f64, width: usize) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::with_capacity(width);
        if self.min.is_empty() || width == 0 || t1 <= t0 {
            return out;
        }
        let len = self.min.len();
        if len == 0 {
            return out;
        }
        for x in 0..width {
            let a = t0 + (t1 - t0) * (x as f64 / width as f64);
            let b = t0 + (t1 - t0) * ((x + 1) as f64 / width as f64);
            let ia = ((a / self.bucket).floor().max(0.0) as usize).min(len - 1);
            let mut ib = ((b / self.bucket).ceil() as usize).min(len);
            // 列比一个桶还窄时（高倍缩放），直接取所在桶；补零会让定轴滚动的波形乱跳
            if ib <= ia {
                ib = (ia + 1).min(len);
            }
            if ia >= ib {
                let i = ia;
                out.push((self.min[i], self.max[i], self.rms[i]));
                continue;
            }
            let mut lo = 0.0f32;
            let mut hi = 0.0f32;
            let mut r = 0.0f32;
            for i in ia..ib {
                lo = lo.min(self.min[i]);
                hi = hi.max(self.max[i]);
                r = r.max(self.rms[i]);
            }
            out.push((lo, hi, r));
        }
        out
    }

    /// 量化后的能量包络（每 hop 秒一个值，归一到 0..1）
    pub fn energy_envelope(&self, hop_secs: f64) -> Vec<f32> {
        if self.rms.is_empty() {
            return Vec::new();
        }
        let step = (hop_secs / self.bucket).max(1.0);
        let n = (self.rms.len() as f64 / step).ceil() as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let a = (i as f64 * step) as usize;
            let b = (((i + 1) as f64 * step) as usize).min(self.rms.len());
            let mut m = 0.0f32;
            let mut j = a;
            while j < b.max(a + 1).min(self.rms.len()) {
                m = m.max(self.rms[j]);
                j += 1;
            }
            out.push(m);
        }
        let peak = out.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
        for v in out.iter_mut() {
            *v /= peak;
        }
        out
    }

    /// 音头包络（正向差分 + 衰减），与游戏 `analyzeBuffer` 的思路一致
    pub fn onset_envelope(&self, hop_secs: f64) -> Vec<f32> {
        let env = self.energy_envelope(hop_secs);
        let mut out = Vec::with_capacity(env.len());
        let mut prev = 0.0f32;
        for v in env {
            out.push((v - prev * 0.92).max(0.0));
            prev = v;
        }
        let peak = out.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
        for v in out.iter_mut() {
            *v /= peak;
        }
        out
    }
}

pub struct AudioEngine {
    shared: Option<Arc<Shared>>,
    _stream: Option<cpal::Stream>,
    pub waveform: Waveform,
    pub source_name: String,
    pub source_path: Option<std::path::PathBuf>,
    error: Option<String>,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            shared: None,
            _stream: None,
            waveform: Waveform::empty(),
            source_name: String::new(),
            source_path: None,
            error: None,
        }
    }

    pub fn has_audio(&self) -> bool {
        self.shared.is_some()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn duration(&self) -> f64 {
        self.shared.as_ref().map(|s| s.duration()).unwrap_or(0.0)
    }

    pub fn load_path(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("读取失败：{e}"))?;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        self.load_bytes(bytes, &name)?;
        self.source_path = Some(path.to_path_buf());
        Ok(())
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>, name: &str) -> Result<(), String> {
        let (samples, channels, rate) = decode(bytes)?;
        if samples.is_empty() {
            return Err("音频没有可解码的采样（可能是纯静音或格式不支持）".into());
        }
        self.waveform = Waveform::build(&samples, channels, rate);
        let shared = Arc::new(Shared::new(samples, channels, rate));
        self.shared = Some(Arc::clone(&shared));
        self.source_name = name.to_string();
        self.error = None;
        match build_stream(shared) {
            Ok(stream) => {
                self._stream = Some(stream);
                Ok(())
            }
            Err(e) => {
                self.error = Some(format!("音频设备不可用：{e}（仍可编辑谱面）"));
                Ok(())
            }
        }
    }

    pub fn position(&self) -> f64 {
        match &self.shared {
            Some(s) => a2f(s.pos_bits.load(Ordering::Relaxed)) / s.src_rate,
            None => 0.0,
        }
    }

    pub fn play(&self) {
        if let Some(s) = &self.shared {
            if self.position() >= s.duration - 0.01 {
                s.pos_bits.store(f2a(0.0), Ordering::Relaxed);
            }
            s.resync.store(true, Ordering::Relaxed);
            s.playing.store(true, Ordering::Relaxed);
        }
    }

    pub fn pause(&self) {
        if let Some(s) = &self.shared {
            s.playing.store(false, Ordering::Relaxed);
        }
    }

    pub fn seek(&self, t: f64) {
        if let Some(s) = &self.shared {
            let frame = (t * s.src_rate).clamp(0.0, s.frames as f64);
            s.pos_bits.store(f2a(frame), Ordering::Relaxed);
            s.resync.store(true, Ordering::Relaxed);
        }
    }

    pub fn set_gain(&self, g: f32) {
        if let Some(s) = &self.shared {
            s.gain_bits
                .store(g.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
        }
    }

    pub fn set_metronome(&self, on: bool) {
        if let Some(s) = &self.shared {
            s.metronome.store(on, Ordering::Relaxed);
            s.resync.store(true, Ordering::Relaxed);
        }
    }

    pub fn set_tempo(&self, bpm: f64) {
        if let Some(s) = &self.shared {
            s.bpm_bits
                .store(f2a(bpm.clamp(20.0, 400.0)), Ordering::Relaxed);
            s.resync.store(true, Ordering::Relaxed);
        }
    }

    /// 节拍相位（秒）：第一个拍点相对 0 秒的偏移
    pub fn set_phase(&self, phase: f64) {
        if let Some(s) = &self.shared {
            s.phase_bits.store(f2a(phase), Ordering::Relaxed);
            s.resync.store(true, Ordering::Relaxed);
        }
    }

    pub fn blip(&self) {
        if let Some(s) = &self.shared {
            s.blip_req.store(true, Ordering::Relaxed);
        }
    }

    /// 变速播放（0.25×—2×）：慢速用于对谱，快速用于检查后半段。
    pub fn set_rate(&self, rate: f64) {
        if let Some(s) = &self.shared {
            s.rate_bits
                .store(f2a(rate.clamp(0.25, 2.0)), Ordering::Relaxed);
            s.resync.store(true, Ordering::Relaxed);
        }
    }

    pub fn rate(&self) -> f64 {
        self.shared
            .as_ref()
            .map(|s| a2f(s.rate_bits.load(Ordering::Relaxed)))
            .unwrap_or(1.0)
    }
}

fn decode(bytes: Vec<u8>) -> Result<(Vec<f32>, usize, f64), String> {
    use symphonia::core::audio::{AudioBufferRef, Signal};
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let cursor = std::io::Cursor::new(bytes);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.mime_type("audio/mpeg");
    hint.mime_type("audio/mp4");
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("无法识别音频格式：{e}"))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "音频中没有可用的轨道".to_string())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("解码器初始化失败：{e}"))?;

    let mut samples: Vec<f32> = Vec::new();
    let mut channels = 0usize;
    let mut rate = 44100.0f64;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        };
        let spec = *decoded.spec();
        if channels == 0 {
            channels = spec.channels.count();
            rate = spec.rate as f64;
        }
        let n = decoded.frames();
        let ch = spec.channels.count();
        macro_rules! push_all {
            ($buf:expr, $conv:expr) => {{
                for f in 0..n {
                    for c in 0..ch {
                        let v: f32 = $conv($buf.chan(c)[f]);
                        samples.push(v);
                    }
                }
            }};
        }
        match decoded {
            AudioBufferRef::F32(buf) => push_all!(buf, |v: f32| v),
            AudioBufferRef::F64(buf) => push_all!(buf, |v: f64| v as f32),
            AudioBufferRef::S16(buf) => push_all!(buf, |v: i16| v as f32 / 32768.0),
            AudioBufferRef::S32(buf) => push_all!(buf, |v: i32| v as f32 / 2147483648.0),
            AudioBufferRef::U8(buf) => push_all!(buf, |v: u8| (v as f32 - 128.0) / 128.0),
            _ => {}
        }
    }
    if channels == 0 {
        return Err("解码结果为空".into());
    }
    Ok((samples, channels, rate))
}

fn build_stream(shared: Arc<Shared>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("没有默认输出设备")?;
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let out_rate = supported.sample_rate().0 as f64;
    let out_channels = supported.channels() as usize;
    let sample_format = supported.sample_format();
    let cfg: cpal::StreamConfig = supported.into();
    let err_fn = |e| eprintln!("[audio] stream error: {e}");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let sh = Arc::clone(&shared);
            let mut st = RenderState::default();
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [f32], _| {
                        render(&sh, &mut st, data, out_rate, out_channels, |v| v)
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
        }
        cpal::SampleFormat::I16 => {
            let sh = Arc::clone(&shared);
            let mut st = RenderState::default();
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [i16], _| {
                        render(&sh, &mut st, data, out_rate, out_channels, |v| {
                            (v.clamp(-1.0, 1.0) * 32767.0) as i16
                        })
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
        }
        cpal::SampleFormat::U16 => {
            let sh = Arc::clone(&shared);
            let mut st = RenderState::default();
            device
                .build_output_stream(
                    &cfg,
                    move |data: &mut [u16], _| {
                        render(&sh, &mut st, data, out_rate, out_channels, |v| {
                            ((v.clamp(-1.0, 1.0) * 0.5 + 0.5) * 65535.0) as u16
                        })
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| e.to_string())?
        }
        other => return Err(format!("不支持的输出格式 {other:?}")),
    };
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

#[derive(Default)]
struct RenderState {
    next_click: f64,
    click_env: f32,
    click_phase: f32,
    click_freq: f32,
}

fn render<T, F: Fn(f32) -> T>(
    sh: &Arc<Shared>,
    st: &mut RenderState,
    data: &mut [T],
    out_rate: f64,
    out_channels: usize,
    conv: F,
) {
    let playing = sh.playing.load(Ordering::Relaxed);
    let gain = f32::from_bits(sh.gain_bits.load(Ordering::Relaxed));
    let rate = a2f(sh.rate_bits.load(Ordering::Relaxed)).clamp(0.25, 2.0);
    let step = sh.src_rate / out_rate * rate;
    let mut pos = a2f(sh.pos_bits.load(Ordering::Relaxed));
    let src_ch = sh.src_channels;
    let frames = sh.frames;

    let metronome = sh.metronome.load(Ordering::Relaxed);
    let bpm = a2f(sh.bpm_bits.load(Ordering::Relaxed)).clamp(20.0, 400.0);
    let phase = a2f(sh.phase_bits.load(Ordering::Relaxed));
    let beat_secs = 60.0 / bpm;
    let beat_frames = beat_secs * sh.src_rate;

    if sh.resync.swap(false, Ordering::Relaxed) {
        st.next_click = -1.0;
    }
    if metronome && st.next_click < 0.0 && playing {
        let rel = pos / sh.src_rate - phase;
        let idx = (rel / beat_secs).ceil().max(0.0);
        st.next_click = ((phase + idx * beat_secs) * sh.src_rate).max(pos);
    }

    let blip = sh.blip_req.swap(false, Ordering::Relaxed);
    if blip {
        st.click_env = 0.9;
        st.click_phase = 0.0;
        st.click_freq = 2200.0;
    }

    let out_frames = data.len() / out_channels.max(1);
    for f in 0..out_frames {
        let mut l = 0.0f32;
        let mut r = 0.0f32;
        if playing && frames > 0 && pos < frames as f64 {
            let i0 = pos.floor() as usize;
            let i1 = (i0 + 1).min(frames - 1);
            let frac = (pos - i0 as f64) as f32;
            let a = sh.samples[i0 * src_ch];
            let b = sh.samples[i1 * src_ch];
            let s0 = a + (b - a) * frac;
            if src_ch >= 2 {
                let c = sh.samples[i0 * src_ch + 1];
                let d = sh.samples[i1 * src_ch + 1];
                l = s0;
                r = c + (d - c) * frac;
            } else {
                l = s0;
                r = s0;
            }
        }

        if metronome && playing && st.next_click > 0.0 && pos >= st.next_click {
            st.click_env = 1.0;
            st.click_phase = 0.0;
            let beat_index = ((st.next_click / sh.src_rate - phase) / beat_secs).round() as i64;
            st.click_freq = if beat_index.rem_euclid(4) == 0 {
                2400.0
            } else {
                1500.0
            };
            st.next_click += beat_frames;
        }
        if st.click_env > 0.002 {
            st.click_phase += st.click_freq / sh.src_rate as f32;
            if st.click_phase > 1.0 {
                st.click_phase -= 1.0;
            }
            let s = (st.click_phase * std::f32::consts::TAU).sin() * st.click_env * 0.2;
            l += s;
            r += s;
            st.click_env *= 0.9975;
        } else {
            st.click_env = 0.0;
        }

        l = (l * gain).clamp(-1.0, 1.0);
        r = (r * gain).clamp(-1.0, 1.0);
        let base = f * out_channels;
        if out_channels == 1 {
            data[base] = conv((l + r) * 0.5);
        } else {
            data[base] = conv(l);
            for c in 1..out_channels {
                data[base + c] = conv(if c == 1 { r } else { (l + r) * 0.5 });
            }
        }
        if playing {
            pos += step;
            if pos >= frames as f64 {
                sh.playing.store(false, Ordering::Relaxed);
                break;
            }
        }
    }

    sh.pos_bits.store(f2a(pos), Ordering::Relaxed);
    let _ = AtomicI64::new(0);
}
