# [DEPRECATED] Doubao Voice Input (豆包语音输入)

> [!WARNING]
> **本项目已废弃并归档 (Deprecated & Archived)**
>
> 豆包输入法官方 Windows 版本（由北京春田智云 / 字节跳动官方出品）已正式发布并开启内测，提供了完善的系统级原生输入、实时语音识别与 AI 助手体验。
> 本项目作为早期的开源逆向探索与系统级 TSF TIP 原型实验，其历史探索使命已经完成，现已全面停止新功能迭代与维护。
>
> - **推荐使用官方版本**：建议前往官方渠道体验豆包官方输入法正式/内测版本。
> - **归档参考价值**：本仓库代码与架构设计文档永久归档保留，供广大开发者作为 **Rust 开发 Windows TSF (Text Services Framework) TIP 原生输入法**、COM 组件交互、WebSocket 实时语音流式处理的开源研究参考。
> - **残留清理**：若您此前在系统中注册过本项目的开发版 TIP，请参见下方 [开发版注销与清理](#开发版注销与清理) 彻底移除注册表残留。

## 项目定位（历史）

Windows 语音输入项目，早期基于豆包 ASR 协议实现实时识别。最初版本是热键/托盘/悬浮按钮驱动的语音输入辅助工具；后续曾推进为系统级 Windows 输入法 (TSF Text Input Processor, TIP) 原型。现已随官方版本的发布正式归档。

## 归档状态说明

| 方向 | 归档前状态 | 归档处置说明 |
|------|------------|--------------|
| 官方产品替代 | 官方正式内测 | 字节跳动官方已发布 Windows 豆包输入法，提供原生系统集成与模型能力，无需自研 |
| ASR 识别核心 | 已有基础实现 | 设备注册、WebSocket ASR、音频采集和 Opus 编码代码保留供学习 |
| 辅助工具入口 | 已有基础实现 | 热键、托盘、悬浮按钮及 `SendInput` 注入逻辑保留供参考 |
| 系统级 IME/TIP | 骨架验证通过 | 验证了 Rust 编写 TSF COM DLL、注册 Keyboard Category 及 Language Profile 的完整链路，代码归档保留 |
| 维护状态 | **已停止维护 (Deprecated)** | 不再接收新功能与 Bug 修复，milestone 计划已关闭 |

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

## 开发版注销与清理

如果此前曾在系统中测试注册过本项目的开发版 TSF TIP（在注册表和输入法列表中显示为 `Doubao Voice Input`），可通过以下方式彻底清理注销，避免与系统输入法或官方版本产生冲突：

### 方式 1：使用项目自带注销脚本（需管理员权限）
```powershell
# 以管理员权限打开 PowerShell
.\scripts\unregister-tip.ps1
```

### 方式 2：手动注册表清理（PowerShell 管理员模式）
如果无需重新编译工具，可直接执行如下命令移除注册表残留项：
```powershell
$paths = @(
    "HKLM:\SOFTWARE\Microsoft\CTF\TIP\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\CTF\TIP\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}",
    "HKCU:\Software\Microsoft\CTF\TIP\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}",
    "HKLM:\SOFTWARE\Classes\CLSID\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}",
    "HKLM:\SOFTWARE\Classes\WOW6432Node\CLSID\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}",
    "HKCU:\Software\Classes\CLSID\{8F5C8C59-2A4D-4DDF-8EBF-F2AB0E9B5A31}"
)
foreach ($p in $paths) {
    if (Test-Path $p) { Remove-Item -Path $p -Recurse -Force }
}
```

## 文档

- [产品需求](PRD/windows-ime-requirements.md)
- [后续路线图和功能计划](PRD/product-roadmap.md)
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
