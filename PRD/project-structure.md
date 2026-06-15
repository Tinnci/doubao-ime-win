# Doubao Voice Input - 项目结构

**版本**: v3.0  
**日期**: 2026-06-15  
**目标**: 支持当前辅助工具和后续系统级 TSF TIP 共存

## 1. 当前结构

当前仓库仍是单 Rust package，主要源码在 `src/`：

```text
src/
├── asr/                 # 豆包 ASR 协议、WebSocket client、响应解析
├── audio/               # 麦克风采集和音频编码
├── business/            # 当前辅助工具控制器、热键、SendInput 文本插入
├── data/                # 配置和凭据
├── ui/                  # 托盘和悬浮按钮
├── voice_core/          # ASR/audio session 事件边界，供 fallback 和 TSF shell 复用
├── lib.rs
└── main.rs
```

同时已经新增最小 TSF TIP crate：

```text
crates/
└── tsf-tip/
    ├── Cargo.toml
    └── src/
        ├── lib.rs
        └── windows_tip.rs
```

当前 `crates/tsf-tip` 能构建 `doubao_tsf_tip.dll`，包含 COM DLL 导出、class factory 和最小 `ITfTextInputProcessorEx` lifecycle。profile 注册工具和发布验证还未实现。

## 2. 目标结构

后续建议逐步迁移到 Cargo workspace，而不是在现有 `src/` 内继续堆 TSF 代码：

```text
doubao-ime-win/
├── crates/
│   ├── voice-core/
│   │   ├── src/
│   │   │   ├── asr/
│   │   │   ├── audio/
│   │   │   ├── config/
│   │   │   ├── credential/
│   │   │   └── session/
│   │   └── Cargo.toml
│   │
│   ├── voice-app/
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── hotkey/
│   │   │   ├── tray/
│   │   │   ├── floating_button/
│   │   │   └── sendinput_fallback/
│   │   └── Cargo.toml
│   │
│   ├── tsf-tip/
│   │   ├── src/
│   │   │   ├── dll_exports.rs
│   │   │   ├── class_factory.rs
│   │   │   ├── text_service.rs
│   │   │   ├── profile.rs
│   │   │   ├── composition.rs
│   │   │   ├── event_bridge.rs
│   │   │   └── ui/
│   │   └── Cargo.toml          # 已有最小 crate，后续继续补模块拆分
│   │
│   └── tip-installer/
│       ├── src/
│       │   ├── main.rs
│       │   ├── register.rs
│       │   ├── unregister.rs
│       │   └── diagnose.rs
│       └── Cargo.toml
│
├── assets/
│   ├── icon_idle.png
│   ├── icon_recording.png
│   ├── icon_processing.png
│   └── tip/
│       └── profile.ico
│
├── scripts/
│   ├── build-portable.ps1
│   ├── register-tip.ps1
│   └── unregister-tip.ps1
│
├── PRD/
│   ├── README.md
│   ├── windows-ime-requirements.md
│   ├── technical-architecture.md
│   ├── adr-0001-tsf-tip-architecture.md
│   ├── core-shell-boundary.md
│   ├── task-list.md
│   └── project-structure.md
│
├── tests/
│   ├── manual/
│   │   └── ime-qa-matrix.md
│   └── fixtures/
│
├── Cargo.toml
├── Cargo.lock
├── config.toml.example
└── README.md
```

## 3. 迁移原则

- `voice-core` 不依赖 TSF、COM、Win32 UI 或 `SendInput`。
- `voice-app` 承接当前热键/托盘/悬浮按钮体验，继续作为 fallback 和 ASR 调试入口。
- `tsf-tip` 只处理系统输入法 shell：COM、profile、activation、composition、候选/状态 UI。
- `tip-installer` 负责注册、卸载、诊断和开发期脚本入口。
- 公共 GUID、profile 名称、产品名和资源路径集中定义，避免注册和卸载不一致。

## 4. 当前模块迁移映射

| 当前路径 | 目标归属 | 说明 |
|----------|----------|------|
| `src/asr` | `crates/voice-core/src/asr` | 保留协议和 ASR client，输出 core event |
| `src/audio` | `crates/voice-core/src/audio` | 保留音频采集和编码 |
| `src/data` | `crates/voice-core/src/config` / `credential` | 拆分配置和凭据 |
| `src/voice_core` | `crates/voice-core/src/session` | 已抽出 ASR/audio session 事件边界，后续迁到 workspace |
| `src/business/voice_controller.rs` | `voice-app` adapter | 已改为 fallback adapter，订阅 core events 后调用 `TextInserter` |
| `src/business/text_inserter.rs` | `voice-app/sendinput_fallback` | 只作为 fallback，TSF 主路径不得依赖 |
| `src/business/hotkey_manager.rs` | `voice-app/hotkey` | 系统级 TIP 不依赖全局热键 |
| `src/ui` | `voice-app`，另建 `tsf-tip/ui` | 托盘/悬浮按钮和 TIP candidate/status UI 分离 |

## 5. TSF TIP 模块边界

`crates/tsf-tip` 内部建议分层：

| 模块 | 职责 |
|------|------|
| `dll_exports` | `DllGetClassObject`、`DllCanUnloadNow`、注册/卸载入口 |
| `class_factory` | COM class factory 和引用计数 |
| `text_service` | `ITfTextInputProcessorEx` activation/deactivation |
| `profile` | language profile 注册、卸载和诊断 |
| `composition` | TSF context、edit session、composition lifecycle |
| `event_bridge` | core event 到 TSF edit session 的队列和节流 |
| `ui` | 候选窗、状态 UI、DPI 和 caret 定位 |

## 6. 脚本和安装工具

开发期脚本：

```text
scripts/
├── register-tip.ps1       # 构建后注册 TIP DLL 和 language profile
├── unregister-tip.ps1     # 卸载 profile 和 COM registry
└── build-portable.ps1     # 当前辅助工具便携构建
```

发布期可以把脚本能力收敛到 `tip-installer`，并增加：

- 注册状态诊断。
- 重复安装/卸载幂等检查。
- 错误回滚。
- 签名和发布包检查。

## 7. 测试结构

系统级 IME 很难完全自动化，必须有手工 QA 文档：

```text
tests/
└── manual/
    ├── ime-qa-matrix.md
    ├── install-uninstall.md
    ├── composition.md
    └── app-compatibility.md
```

最小 QA 覆盖：

- Windows 10/11 x64。
- Notepad、Edge/Chrome、Office、WinUI/WPF、Electron。
- 安装、切换、activation、composition、commit、cancel、卸载。
- DPI、多显示器、焦点切换、重启后状态。

## 8. 不再推荐的结构

- 不再把 TSF 代码塞进当前 `business/` 模块。
- 不再让 core 直接调用 `TextInserter` 或 TSF COM 对象。
- 不再把候选窗和悬浮按钮复用为同一个 UI 模块。
- 不再维护独立的旧“绿色便携工具 PRD”；相关内容已合并到当前文档的 fallback 说明。
