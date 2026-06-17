# Doubao Voice Input (豆包语音输入)

Windows 语音输入项目，基于豆包 ASR 实现实时识别。当前可运行版本是热键/托盘/悬浮按钮驱动的语音输入辅助工具；当前主线 milestone 是把它推进为系统级 Windows 输入法，也就是 TSF Text Input Processor (TIP)。

## 当前状态

| 方向 | 状态 | 说明 |
|------|------|------|
| ASR 识别核心 | 已有基础实现 | 设备注册、WebSocket ASR、音频采集和 Opus 编码已在现有 Rust 代码中实现 |
| 辅助工具入口 | 已有基础实现 | 支持热键、托盘、悬浮按钮，并通过 `SendInput` 向焦点窗口输入文本 |
| 系统级 IME/TIP | 骨架实现中 | 已有最小 TSF COM DLL skeleton 和 language profile 注册/诊断工具；仍需真实系统可见性验证、composition、候选/状态 UI |
| 发布能力 | 待完善 | 便携构建脚本已有基础，系统级 IME 还需要发布安装器、签名、卸载和 QA 矩阵 |

`SendInput` 路径保留为兼容/回退能力，但不再是系统级输入法主路径。后续主路径应通过 TSF composition 和 commit API 向目标应用提交文本。

## 目标能力

- 出现在 Windows 输入法/键盘列表中，并可由用户切换到该输入法。
- 注册 TSF language profile，激活后能触发 TIP `Activate`/`Deactivate` 生命周期。
- 将 ASR interim 结果映射为 TSF composition update，将 final 结果映射为文本提交。
- 提供录音、识别中、提交、错误等输入状态 UI，并支持候选窗/光标定位。
- 提供可重复执行的安装、卸载、升级、签名和兼容性验证流程。

## 当前可运行功能

- 基于豆包 ASR 的实时语音识别。
- 双击 Ctrl 或配置的热键启动/停止语音输入。
- 悬浮按钮和系统托盘入口。
- 通过 `SendInput` 向当前焦点窗口输入文本，并支持流式结果的增量修正。

## 配置文件

配置文件 `config.toml` 与程序同目录：

```toml
[general]
auto_start = false
language = "zh-CN"

[hotkey]
mode = "double_tap"
combo_key = "Ctrl+Shift+V"
double_tap_key = "Ctrl"
double_tap_interval = 300

[floating_button]
enabled = true
position_x = 100
position_y = 100

[asr]
vad_enabled = true
```

## 从源码构建

### 环境要求

- Rust stable
- Windows 10/11 x64
- Visual Studio Build Tools 2022
- CMake
- Protobuf Compiler (`protoc`)

### 构建

```powershell
cargo build
cargo build --release
```

当前 release 产物仍是辅助工具可执行文件：

```text
target/release/doubao-voice-input.exe
```

系统级 TSF TIP DLL 和开发期 profile 注册工具已可构建；发布安装包、签名和完整系统验证还在 milestone 实现范围内。

开发期 TIP 诊断命令：

```powershell
cargo build -p doubao-tsf-tip
.\target\debug\doubao-tip-tool.exe status
```

TIP DLL 被 TSF host 加载后，activation 诊断会同时写入 `OutputDebugStringW` 和 `%LOCALAPPDATA%\DoubaoVoiceInput\tsf-tip.log`。

## 文档

- [产品需求](PRD/windows-ime-requirements.md)
- [Milestone 1 路线图](PRD/milestone-1-roadmap.md)
- [技术架构](PRD/technical-architecture.md)
- [架构决策 ADR](PRD/adr-0001-tsf-tip-architecture.md)
- [Core/Shell 边界](PRD/core-shell-boundary.md)
- [任务清单](PRD/task-list.md)
- [项目结构](PRD/project-structure.md)

## 技术架构概览

| 模块 | 当前实现 | TSF milestone 目标 |
|------|----------|--------------------|
| ASR core | Rust async client、audio capture、protocol parsing | 抽成可被 TIP shell 调用的稳定 core API |
| 输入提交 | `SendInput` 文本注入 | TSF composition/edit session/commit |
| 入口 | 热键、托盘、悬浮按钮 | Windows 输入法切换和 TIP activation |
| UI | 悬浮按钮、托盘 | 候选窗、状态指示、光标定位、DPI/多显示器适配 |
| 分发 | 便携 exe | 安装/卸载、language profile 注册、签名发布 |

## 免责声明

本项目基于豆包输入法客户端协议分析实现，非官方 API，仅供学习研究使用。协议可能变更，使用时需遵守相关法律法规和服务条款。
