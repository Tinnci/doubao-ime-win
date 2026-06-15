# Milestone 1 Roadmap: 系统级输入法 / TSF TIP

**状态**: Open  
**目标仓库**: `Tinnci/doubao-ime-win`  
**GitHub milestone**: [系统级输入法 / TSF TIP](https://github.com/Tinnci/doubao-ime-win/milestone/1)

## 目标

把当前语音输入辅助工具推进为系统级 Windows 输入法：

- 出现在 Windows 输入法列表和任务栏输入指示器中。
- 注册 TSF language profile，并能被 Windows TSF manager 加载。
- 通过 TSF composition/update/commit 完成语音输入文本提交。
- 支持录音、识别、提交、错误等状态 UI。
- 提供安装、卸载、诊断、签名和兼容性 QA 路径。

## 当前状态

| 区域 | 状态 | 说明 |
|------|------|------|
| #1 架构决策 | Done | 已产出 ADR，明确 Rust core + TSF shell 路线和 C++ fallback |
| #2 Core/Shell 边界 | Done | 已抽出 `src/voice_core`，fallback adapter 订阅 core events |
| #3 TIP DLL 骨架 | In progress | 已新增 `crates/tsf-tip`，可构建 DLL，已导出 COM 入口；真实注册和 TSF manager 加载验证仍未完成 |
| #4 Language profile 注册 | Not started | 下一步，包含 COM registry、profile 注册、卸载和诊断 |
| #5 Composition | Not started | 先用固定文本验证 TSF composition，再接 ASR |
| #6 候选/状态 UI | Not started | 依赖 #5 的上下文和 caret rectangle |
| #7 ASR 接入 TSF | Not started | 依赖 #5 的 event bridge 和 composition 生命周期 |
| #8 QA 矩阵 | Not started | 可先建 checklist，完整验证依赖 #4-#7 |

## Phase 1: Foundation

### 已完成

- 文档主线从旧 `SendInput` 辅助工具切到 TSF TIP。
- `SendInput` 被限定为 fallback app，不进入系统级 IME 主路径。
- 新增 ADR：[adr-0001-tsf-tip-architecture.md](./adr-0001-tsf-tip-architecture.md)。
- 新增 core/shell 边界：[core-shell-boundary.md](./core-shell-boundary.md)。
- 新增 `src/voice_core`，将 ASR/audio session 事件化。
- `VoiceController` 改成 fallback adapter。

### Foundation 退出标准

- `cargo check --workspace --locked` 通过。
- 当前 fallback exe 仍可构建。
- core event 模型不依赖 TSF、COM、`SendInput` 或 UI。

## Phase 2: TIP Loadability

### 已完成

- 新增 workspace 成员 `crates/tsf-tip`。
- `doubao_tsf_tip.dll` 可构建。
- DLL 导出：
  - `DllGetClassObject`
  - `DllCanUnloadNow`
  - `DllRegisterServer`
  - `DllUnregisterServer`
- 已实现手写 COM vtable skeleton：
  - `IClassFactory`
  - `ITfTextInputProcessor`
  - `ITfTextInputProcessorEx`
- `Activate` / `ActivateEx` / `Deactivate` 有日志调用点。

### 剩余

- `DllRegisterServer` / `DllUnregisterServer` 当前仍返回 `SELFREG_E_CLASS` 占位。
- 需要写入真实 COM registry。
- 需要通过 `ITfInputProcessorProfiles` 注册 language profile。
- 需要验证 TSF manager 能创建 TIP 实例并触发 activation。

### Phase 2 退出标准

- COM DLL 可注册和卸载。
- `DllGetClassObject` 可被 COM 创建路径调用。
- Windows TSF manager 能创建 TIP 实例。
- 切换输入法时能看到 activation/deactivation 日志。

## Phase 3: Language Profile

下一步执行 #4。

目标：

- 定义最终 CLSID、profile GUID、语言标识、描述、图标路径。
- 实现开发期注册/卸载工具或 DLL 自注册逻辑。
- 注册 text service 和 language profile。
- 输入法出现在 Windows 设置和任务栏输入列表。
- 卸载后 profile、registry 和文件清理干净。

非目标：

- 不在 #4 实现 composition。
- 不接 ASR。
- 不处理完整候选窗 UI。

## Phase 4: Composition MVP

目标：

- 管理 TSF context、edit session 和 composition。
- 先用固定文本验证 composition update 和 final commit。
- 支持 cancel 和 error cleanup。
- 验证 Notepad 和现代浏览器输入框。

关闭标准：

- 不使用 `SendInput` 也能更新组合文本并提交最终文本。
- 焦点变化或取消不会遗留 composition。

## Phase 5: ASR Event Bridge

目标：

- 将 core `InterimText` 映射到 composition update。
- 将 core `FinalText` 映射到 commit。
- 错误、取消、超时、认证失败都能清理 composition。
- event bridge 支持节流、合并和过期事件丢弃。

关闭标准：

- 录音期间持续更新 composition。
- final 文本提交到目标应用。
- 错误路径不造成 TSF 死锁或目标应用崩溃。

## Phase 6: Candidate/Status UI

目标：

- 显示录音、识别中、提交、错误状态。
- 候选/状态窗口跟随 TSF caret rectangle。
- 处理 DPI、多显示器和焦点变化。

关闭标准：

- UI 不遮挡正在输入的文本。
- 停用、取消、失焦后无悬浮 UI 残留。

## Phase 7: QA and Release Readiness

目标：

- 建立 Windows 10/11 x64 QA checklist。
- 覆盖 Notepad、Edge/Chrome、Office、WinUI/WPF、Electron。
- 验证安装、升级、卸载、重启后 profile 保持和清理。
- 明确签名、Defender/SmartScreen、崩溃日志和诊断要求。

Release blocker：

- 无法卸载或 profile 残留。
- TIP activation 崩溃目标应用。
- TSF edit session 死锁。
- 错误路径遗留 composition。
- 凭据泄露或日志记录敏感 token。

## Milestone 退出标准

- 输入法能注册到 Windows 输入法列表。
- 用户可以切换到该输入法并触发 TSF activation。
- 至少在 Notepad 和现代浏览器输入框内完成 composition 更新和 final commit。
- ASR interim/final 结果能通过 TSF 管线更新和提交。
- 安装、卸载、重启后状态可验证。
- QA checklist 有明确结果，release blocker 全部关闭或有降级说明。
