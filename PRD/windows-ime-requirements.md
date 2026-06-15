# Doubao Voice Input - 系统级 IME / TSF TIP 产品需求

**版本**: v3.0  
**日期**: 2026-06-15  
**当前 milestone**: [系统级输入法 / TSF TIP](https://github.com/Tinnci/doubao-ime-win/milestone/1)

## 1. 背景

项目当前已经具备语音识别、热键、托盘、悬浮按钮和 `SendInput` 文本注入能力。这个形态适合做轻量辅助工具，但不能作为真正的 Windows 输入法出现在系统输入法列表中，也无法通过 TSF composition 与目标应用建立标准输入关系。

当前 milestone 的目标是把项目从辅助工具推进为系统级 Windows 输入法：注册 TSF language profile，成为可切换的 Text Input Processor (TIP)，并通过 TSF 管线完成组合态更新和最终文本提交。

## 2. 产品目标

- 输入法出现在 Windows 设置和任务栏输入法列表中。
- 用户切换到该输入法后，能触发 TSF TIP activation。
- 语音识别中间结果以 composition 形式显示在当前文本上下文中。
- 最终识别结果通过 TSF commit 正常提交到目标应用。
- 具备候选窗、录音/识别/错误状态指示和光标定位能力。
- 安装、卸载、升级、重启、禁用等系统级输入法生命周期可验证。

## 3. 非目标

- 不实现完整拼音/五笔输入法、词库管理或键盘候选转换引擎。
- 不把 `SendInput` 作为系统级 IME 主输入管线。
- 不要求第一个 TSF demo 支持完整候选窗、主题系统或复杂设置界面。
- 不存储用户语音数据，不做离线语音训练。
- 不在未明确验证前承诺 arm64、企业部署或 Microsoft Store 分发。

## 4. 用户场景

| 场景 | 用户结果 |
|------|----------|
| 安装输入法 | Windows 输入法列表出现 Doubao Voice Input |
| 切换输入法 | 任务栏输入指示器能切换到该输入法，并触发 TIP activation 日志 |
| 语音输入 | 录音过程中能看到组合文本更新，结束后文本提交到目标应用 |
| 取消输入 | composition 被清理，不留下半截文本 |
| 应用切换 | 焦点变化后候选/状态 UI 不残留，TSF 上下文不死锁 |
| 卸载 | language profile、COM registry 和安装文件清理干净 |

## 5. 功能需求

### 5.1 TSF TIP 注册和激活

| 优先级 | 需求 |
|--------|------|
| P0 | 提供可构建的 TIP DLL，导出 `DllGetClassObject`、`DllCanUnloadNow` 和注册/卸载入口 |
| P0 | 实现最小 `ITfTextInputProcessorEx` 生命周期 |
| P0 | 使用 `ITfInputProcessorProfiles` 注册 language profile |
| P0 | 输入法能在 Windows 设置和任务栏输入法列表显示并可切换 |
| P1 | 支持图标、描述、语言标识和启用/禁用状态验证 |

### 5.2 Composition 和文本提交

| 优先级 | 需求 |
|--------|------|
| P0 | 支持开始、更新、提交、取消 composition |
| P0 | ASR interim 结果映射为 composition update |
| P0 | ASR final 结果映射为 commit |
| P0 | 通过 TSF edit session 修改上下文，避免跨线程直接操作 TSF 对象 |
| P1 | 在 Notepad、Edge/Chrome、WinUI/WPF 文本框中完成基础验证 |

### 5.3 ASR core 复用

| 优先级 | 需求 |
|--------|------|
| P0 | 保留现有豆包 ASR 协议、音频采集、配置和凭据能力 |
| P0 | 把 ASR session 与 UI/输入提交解耦，输出稳定的状态事件 |
| P0 | 支持错误、取消、超时、认证失败和网络失败事件 |
| P1 | 保留现有辅助工具入口作为开发期 fallback |

### 5.4 候选窗和状态 UI

| 优先级 | 需求 |
|--------|------|
| P1 | 显示录音、识别中、提交中、错误等状态 |
| P1 | 候选/状态窗口跟随 TSF caret rectangle 定位 |
| P1 | 支持 DPI 缩放、多显示器和焦点变化后的清理 |
| P2 | 支持暗色模式和更完整的候选选择交互 |

### 5.5 安装、卸载和发布

| 优先级 | 需求 |
|--------|------|
| P0 | 提供开发期注册/卸载脚本或工具 |
| P0 | 卸载后清理 COM registry、profile 和安装文件 |
| P1 | 建立签名、Defender/SmartScreen、崩溃日志收集方案 |
| P1 | 建立升级和重启后的 profile 保持验证 |

## 6. 非功能需求

| 类别 | 要求 |
|------|------|
| 兼容性 | Windows 10/11 x64 是 P0；arm64 暂列 P2 |
| 稳定性 | TIP activation/deactivation 不应崩溃目标应用或导致 TSF 死锁 |
| 延迟 | ASR interim 到 composition update 的用户可见延迟目标 < 500ms |
| 安全 | 凭据继续使用本地安全存储；不记录原始语音 |
| 可诊断 | TIP 加载、activation、profile 注册、ASR 状态、错误路径必须有日志 |
| 可回滚 | 开发期必须能安全卸载并恢复系统输入法状态 |

## 7. MVP 验收标准

最小可行 demo 不要求完整候选窗，但必须满足：

- DLL 可构建并注册。
- Windows 输入法列表中能看到该输入法。
- 用户可切换到该输入法并触发 TIP activation。
- 在 Notepad 和现代浏览器输入框中能更新 composition 并提交 final 文本。
- 取消、错误、卸载路径不会遗留 composition、悬浮 UI 或 registry/profile 项。

## 8. Milestone issue 映射

| Issue | 需求覆盖 |
|-------|----------|
| [#1 调研 TSF TIP 最小可行架构和风险边界](https://github.com/Tinnci/doubao-ime-win/issues/1) | 架构决策、风险、demo 成功标准 |
| [#2 定义 Rust 核心与 TSF shell 的边界](https://github.com/Tinnci/doubao-ime-win/issues/2) | ASR core 复用、线程和生命周期边界 |
| [#3 搭建 TSF Text Input Processor COM DLL 骨架](https://github.com/Tinnci/doubao-ime-win/issues/3) | COM DLL、TIP 生命周期、日志 |
| [#4 注册 language profile 并显示在 Windows 输入法列表](https://github.com/Tinnci/doubao-ime-win/issues/4) | profile 注册、切换、卸载清理 |
| [#5 实现 TSF composition 会话和文本提交模型](https://github.com/Tinnci/doubao-ime-win/issues/5) | composition/update/commit/cancel |
| [#6 实现候选窗、模式状态和光标定位](https://github.com/Tinnci/doubao-ime-win/issues/6) | candidate/status UI、DPI、焦点清理 |
| [#7 把 ASR 流式结果接入 TSF 输入管线](https://github.com/Tinnci/doubao-ime-win/issues/7) | ASR event 到 TSF edit session 的桥接 |
| [#8 建立系统级 IME 兼容性和发布 QA 矩阵](https://github.com/Tinnci/doubao-ime-win/issues/8) | QA checklist、release blocker、发布验证 |

## 9. 旧路线处理

旧文档中的“绿色便携、无需 TSF、使用 `SendInput` 作为主路径”已不再代表当前产品目标。相关实现仍可保留为辅助工具和回退路径，用于：

- 开发期快速验证 ASR 协议和音频采集。
- 在 TSF TIP 尚不可用时提供临时输入能力。
- 对比 TSF composition 与键盘模拟输入的兼容性差异。
