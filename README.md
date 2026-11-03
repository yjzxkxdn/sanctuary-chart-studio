# 圣土 · 关卡编排台 — Sanctuary Chart Studio

> 为《圣土 · 白塔乐章》网页版设计的 **Rust 桌面打谱与关卡编辑器**。
> 不模仿原版「关卡编纂室」的三个标签页，而是把 **谱面 / 出怪 / 难度** 放在同一条时间轴上同时显示。

[![CI](https://github.com/yjzxkxdn/sanctuary-chart-studio/actions/workflows/ci.yml/badge.svg)](https://github.com/yjzxkxdn/sanctuary-chart-studio/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/yjzxkxdn/sanctuary-chart-studio)](https://github.com/yjzxkxdn/sanctuary-chart-studio/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

*A Rust desktop chart & level editor for the web rhythm-tower-defense game
**Sanctuary · White Tower Movement**. It puts the note chart, the enemy waves and the
difficulty curve on one shared timeline, so you can see 「how many notes must be hit
while how many enemies are on screen」 at the same moment — the metric that decides
whether a level feels good. The whole UI is in Chinese.*

---

## 为什么要重做

原版编辑器把「地图」「出怪」「谱面」拆成三个互不可见的标签页，因此你没法在写谱的时候看到
「这里正好有一大波怪」，也没法判断「这一段是不是手忙脚乱」。本项目围绕一个核心问题设计：

> **同一时刻，玩家要按多少音符，场上又有多少怪？**

这就是「双压」——音游难度与塔防压力的重叠程度。它是决定关卡好不好玩的第一指标。

---

## 核心设计

### 1. 统一时间轴（一个视图看四件事）

从左栏向右是一条连续时间线，自上而下四条轨道：

| 轨道 | 内容 |
|---|---|
| **波形** | 载入音频后的峰值 + 能量曲线，音头位置一目了然 |
| **难度** | 红色=在场怪物总血量，金色线=每秒判定数，紫色=双压重叠度 |
| **出怪** | 按路线分成泳道，怪物按类型着色，首领是菱形 |
| **谱面** | 音符本体，**按拍位分音着色** |

全部共用同一套 `x = f(t)` 映射与同一条贯穿全高的播放头。

### 2. 分音着色

正拍金色、拍点紫色、半拍青色、十六分音符灰蓝。缩到整曲时能一眼看出节奏骨架，
放大后又可以精确对位。这一条在多数简易编辑器里没有，但对读谱体验影响最大。

### 3. 双压平衡面板

- **峰值 NPS** / **峰值在场血量** / **双压峰值**
- **双压热点列表**：直接列出「87.8s、88.5s…」这些最手忙脚乱的位置，可一键跳转
- **手感诊断**：NPS 是否过高、长条占比是否失衡、特殊尘魄是否够、有没有首领高潮

### 4. 实时战斗推演

选中任意出怪事件，立刻按游戏真实公式（`enemySpawnStats`）算出：

- 实际生命值（含后半段 3.5× 血量递进）
- 实际移速（含类型修正：BOSS ×0.58、披甲 ×0.8）
- **单程时间**与**抵达目标时刻**（超出关卡时长会标红）
- **全部漏过会扣多少血**、目标血量够漏几只

### 5. 怪物图鉴

游戏里 9 种尘魄的真实数值（血 ×1.0—×7.0、速 ×0.5—×1.9、漏过 1—3 点）做成可点击条目，
点一下就套用到当前选中的出怪事件——不用再记数值。

### 6. 专业打谱功能

- **录制模式**：开启后空格 = 在播放头处落音符，长按 = 长条；配合框选 + 量化，打谱飞快
- **打点录音**：播放中按 `F` 即时落一个音符
- **打击音**：每个音符经过时发提示音
- **变速播放**：0.5× / 0.75× / 1× / 1.25×，慢速对不了的地方就慢下来对
- **吸附**：1/1 到 1/24，配合相位偏移
- **BPM 工具**：载入音频自动检测 BPM；BPM 数值可直接改；「打点测速」跟着节奏连点 3 次以上即可测速并应用
- **相位自动推断**：用音头能量对齐节拍网格
- **一键生成**：按音头能量铺谱面、按强度生成出怪曲线
- **框选 / 批量量化 / 复制粘贴到播放头 / 微调**

### 7. 播放与视图

- **走带 / 定轴**：走带 = 背景不动播放头右移；定轴 = 播放头固定、背景左移（类 DAW）
- **跟随**：走带模式下播放头超出视口时自动翻页
- **轨道高度**：拖动轨道之间的分隔条即可调整（1:1 跟随鼠标），谱面轨道自动填充剩余高度；工具栏「轨道」菜单里也有滑杆
- **难度曲线窗口自适应**：红/紫/金三条曲线按当前可见窗口的峰值各自归一化——后期血量堆积不会把 y 轴撑高、压扁其余数据；左下角读数是当前窗口的实时峰值
- **同一时刻堆叠的怪物自动上下散开**，并标出 ×N 数量徽章

### 8. 音频

只支持加载本地音频文件（mp3 / wav / ogg / flac），不内置任何曲目。
载入后自动分析 BPM 与节拍相位，二者均可手动修正。

### 9. 地形编辑

10×7 战场，从入口条开始拖动画路线，带方向箭头、格序号、终点目标标记。
障碍格用斜线填充。播放时地图上会实时显示怪物当前位置。

---

## 快捷键

| 键 | 作用 |
|---|---|
| `空格` | 播放 / 暂停；录制模式下 = 落音符（长按出长条） |
| `Enter` | 播放 / 暂停（录制模式下专用） |
| `T` | 打点测速 |
| `1` `2` `3` `4` | 切换右侧面板：音符 / 出怪 / 双压 / 地形 |
| `F` | 打点录音 |
| `←` `→` | 移动播放头；`Shift + ←/→` 微调选中对象 |
| `Home` / `End` | 跳到开头 / 结尾 |
| `Ctrl+Z` / `Ctrl+Y` | 撤销 / 重做 |
| `Ctrl+S` / `Ctrl+E` / `Ctrl+O` | 保存 / 导出 / 打开 |
| `Ctrl+C` / `Ctrl+V` | 复制 / 粘贴到播放头 |
| `Ctrl+A` | 全选 |
| `Ctrl+Q` | 量化选中音符 |
| `Delete` | 删除选中 |
| `H` / 双击音符 | 点按 ↔ 长条 |
| `Shift + 拖拽` / 空白处直接拖拽 | 框选（谱面与出怪轨都支持） |
| 右键 | 删除指针下的音符 / 出怪 |
| 滚轮 / `Ctrl+滚轮` | 横向滚动 / 缩放 |
| `Esc` | 取消选择 |

---

## 导出

导出的是 **与游戏内置编辑器完全兼容的 JSON**，字段与 `level-schema.mjs::validateDesign` 一一对应：

```json
{
  "version": 1, "key": "level-…", "name": "…",
  "duration": 90, "bpm": 85, "palette": "amethyst",
  "goalHp": 16, "enemyHealth": 65, "speed": 0.7,
  "paths": [[[-1,2],[0,2], …]], "blocked": [],
  "waves": [{ "time": 6, "route": 0, "kind": "special", "health": 1, "speed": 1 }],
  "chart": [{ "time": 2.4, "kind": "tap", "accent": false }],
  "audioName": "", "audioKey": null, "trackId": "local"
}
```

导出前会跑与游戏**逐条一致**的校验（≤3 条路线、每条 3—100 格、≤70 障碍、1—1000 出怪、
1—3000 音符、≥1 只 `special`、音符间隔 ≥0.18s、长条尾 ≥ 头 +0.25s）。
有任何「错误」就拒绝导出，避免拿到游戏里才发现进不去。

游戏内的真实数值公式也已复刻并**用单元测试锁死**：

```
血 量 = enemyHealth × health × (1 + max(0, t/duration − 0.3) × 3.5) × 类型系数
移 速 = speed × speed倍率 × 类型修正
类型系数：普通 1 / 披甲 3 / 特殊 0.9 / BOSS 19
```

---

## 构建与运行

需要 Rust **1.85+**（`rust-version` 已写进 `Cargo.toml`）。无其他系统依赖。

```bash
git clone https://github.com/yjzxkxdn/sanctuary-chart-studio.git
cd sanctuary-chart-studio

cargo run              # 开发
cargo run --release    # 发布
cargo test             # 兼容性自测（6 项，锁定与游戏一致的规则）
```

产物：`target/release/sanctuary-chart-studio`（Windows 下带 `.exe`）。
也可以直接从 [Releases](https://github.com/yjzxkxdn/sanctuary-chart-studio/releases) 下载预编译包。

Linux 下若编译报缺少系统库：

```bash
sudo apt-get install -y libgl1-mesa-dev libxkbcommon-dev libxcb1-dev \
  libxrandr-dev libxi-dev libxcursor-dev libasound2-dev libudev-dev pkg-config
```

### 无界面自检

在没有图形环境（或截图不可靠）的机器上，可以直接验证「生成 → 校验 → 导出」整条链路：

```bash
SCS_EXPORT=demo.json sanctuary-chart-studio
# 输出 EXPORT_OK / EXPORT_INVALID，exit code 0
```

若程序意外崩溃，调用栈会写入系统临时目录下的 `sanctuary-crash.txt`
（窗口模式看不到控制台，这是唯一的现场线索）。

**字体**：自动从系统加载中文字体（`msyh.ttc` 等）。若想换字体，把字体文件放到
可执行文件同目录的 `assets/font.ttf`（或 `.otf` / `.ttc`）即可覆盖。

---

## 代码结构

| 文件 | 职责 |
|---|---|
| `src/model.rs` | 关卡数据模型、战斗推演、双压曲线、校验；含兼容性单元测试 |
| `src/ui_timeline.rs` | **统一时间轴**：波形/难度/出怪/谱面四轨绘制与全部交互 |
| `src/ui_inspector.rs` | 左栏（关卡/音乐/生成/统计/检查）、右栏（音符/出怪/双压/地形四面板 + 怪物图鉴） |
| `src/ui_map.rs` | 地形编辑大画布 |
| `src/audio.rs` | symphonia 解码 + cpal 低延迟播放、波形峰值、节拍器、变速 |
| `src/autochart.rs` | 相位推断、BPM 估计、打点测速、自动打谱、出怪编排 |
| `src/widgets.rs` | 自绘控件：卡片、内嵌数值框、色板、统计格、芯片 |
| `src/theme.rs` | 深紫金视觉主题与中文字体装载 |
| `src/app.rs` | 布局、走带、快捷键、文件操作 |
| `src/undo.rs` | 快照式撤销栈（600ms 合并） |

`_ref/` 目录仅用于开发期对照游戏原始逻辑，属于第三方代码，**不随本仓库分发**。

---

## 许可

[MIT](LICENSE)。本项目与《圣土 · 白塔乐章》的开发者无隶属关系，仅为该游戏制作的第三方工具；
游戏本体及其素材的版权归原作者所有。
