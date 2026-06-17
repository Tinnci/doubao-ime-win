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
| #3 TIP DLL 骨架 | In progress | 已新增 `crates/tsf-tip`，可构建 DLL，已导出 COM 入口；注册代码和 activation 诊断已落地，TSF manager 加载验证仍未完成 |
| #4 Language profile 注册 | In progress | 已实现 COM registry 写入/清理和 `ITfInputProcessorProfiles` 注册/卸载；系统可见性、切换和清理验证待执行 |
| #5 Composition | Not started | 先用固定文本验证 TSF composition，再接 ASR |
| #6 候选/状态 UI | Not started | 依赖 #5 的上下文和 caret rectangle |
| #7 ASR 接入 TSF | Not started | 依赖 #5 的 event bridge 和 composition 生命周期 |
| #8 QA 矩阵 | Not started | 可先建 checklist，完整验证依赖 #4-#7 |

## 阶段目标总览

Milestone 1 按可验证 gate 推进，每个阶段只关闭会解锁下一阶段的系统能力：

| 阶段 | 主 issue | 阶段目标 | 完成 gate | 当前状态 |
|------|----------|----------|-----------|----------|
| Phase 1 Foundation | #1, #2 | 锁定 TSF TIP 路线，隔离 Rust core 与 TSF shell | workspace 可构建，core event 模型不依赖 TSF/COM/UI | Done |
| Phase 2 TIP Loadability | #3 | 构建可注册、可由 COM 创建的 TIP DLL | COM 注册成功，TSF manager 能创建实例并触发 activation 日志 | In progress |
| Phase 3 Language Profile | #4 | 注册、启用、诊断和卸载 zh-CN language profile | Windows 输入法列表可见，重复注册幂等，卸载无 profile/registry 残留 | In progress |
| Phase 4 Composition MVP | #5 | 不依赖 `SendInput`，用固定文本验证 TSF composition/update/commit | Notepad 和现代浏览器可更新 composition 并提交 final 文本 | Not started |
| Phase 5 ASR Event Bridge | #7 | 将 core interim/final/error 映射到 TSF edit session | 语音 interim 更新 composition，final commit，错误路径清理干净 | Not started |
| Phase 6 Candidate/Status UI | #6 | 实现录音/识别/错误状态和 caret 跟随 UI | DPI、多显示器、焦点切换下 UI 不残留、不遮挡输入 | Not started |
| Phase 7 QA/Release Readiness | #8 | 建立安装、升级、卸载、兼容性和 release blocker 验证 | Windows 10/11 QA checklist 有结果，blocker 关闭或有降级说明 | Not started |

### 近期推进顺序

1. 关闭 #4 的真实系统验证：管理员注册、输入法列表可见、切换触发 activation、卸载清理。
2. 用 #4 的注册脚本复测 #3：确认 TSF manager 能创建 TIP 实例。
3. 进入 #5：先做固定文本 composition，不接 ASR、不做完整 UI。

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
- `DllGetClassObject` / `CreateInstance` / `Activate` / `ActivateEx` / `Deactivate` 有诊断输出。
- 诊断同时写入 `OutputDebugStringW` 和 `%LOCALAPPDATA%\DoubaoVoiceInput\tsf-tip.log`，避免 TSF host 没有 tracing subscriber 时不可观测。

### 剩余

- 需要在开发机上执行真实注册/卸载路径。
- 需要验证 TSF manager 能创建 TIP 实例并触发 activation。
- 需要用现有诊断输出定位 registry/profile 残留和加载失败原因。

### Phase 2 退出标准

- COM DLL 可注册和卸载。
- `DllGetClassObject` 可被 COM 创建路径调用。
- Windows TSF manager 能创建 TIP 实例。
- 切换输入法时能看到 activation/deactivation 日志。

## Phase 3: Language Profile

当前执行 #4。

目标：

- [x] 定义 CLSID、profile GUID、语言标识、描述和初始图标路径策略。
- [x] 实现 DLL 自注册逻辑：`HKCR\CLSID\{TIP_CLSID}` 和 `InProcServer32`。
- [x] 通过 `ITfInputProcessorProfiles::Register` 注册 text service。
- [x] 通过 `AddLanguageProfile` / `EnableLanguageProfile` 注册并启用 zh-CN profile。
- [x] 通过 `RemoveLanguageProfile` / `Unregister` 和 registry 删除实现卸载清理。
- [x] 提供开发期命令和脚本，封装管理员权限下的注册、卸载和状态检查。
- [ ] 输入法出现在 Windows 设置和任务栏输入列表。
- [ ] 卸载后 profile、registry 和文件清理干净。

### #4 细化目标

| 子目标 | 交付物 | 验收方式 |
|--------|--------|----------|
| #4.1 标识和元数据 | `TIP_CLSID`、`TIP_PROFILE_GUID`、`TIP_LANGID=0x0804`、描述字符串 | 代码集中定义，文档记录 zh-CN 初始策略 |
| #4.2 COM 自注册 | `DllRegisterServer` 写入 `HKCR\CLSID\{TIP_CLSID}\InProcServer32` | `reg query` 可看到 DLL 路径和 `ThreadingModel=Apartment` |
| #4.3 TSF profile 注册 | `Register`、`AddLanguageProfile`、`EnableLanguageProfile` | Windows 设置和语言栏可见 `Doubao Voice Input` |
| #4.4 卸载清理 | `RemoveLanguageProfile`、`Unregister`、`RegDeleteTreeW` | 重复卸载不失败，profile 和 CLSID key 无残留 |
| #4.5 诊断路径 | `doubao-tip-tool status` 与 `scripts/check-tip-registration.ps1` | 输出 DLL 路径、CLSID key、profile registry key、枚举注册状态、启用状态和错误码 |
| #4.6 系统加载验证 | TSF manager 创建 TIP 实例 | 切换输入法时在 DebugView/WinDbg 或 `%LOCALAPPDATA%\DoubaoVoiceInput\tsf-tip.log` 看到 `ActivateEx` / `Deactivate` |

### #4 开发期命令

管理员 PowerShell：

```powershell
.\scripts\register-tip.ps1
.\scripts\check-tip-registration.ps1
.\scripts\unregister-tip.ps1
```

直接调用工具：

```powershell
cargo build -p doubao-tsf-tip
.\target\debug\doubao-tip-tool.exe register --dll-path .\target\debug\doubao_tsf_tip.dll
.\target\debug\doubao-tip-tool.exe status
.\target\debug\doubao-tip-tool.exe unregister
```

### #4 退出标准

- `cargo check --workspace --locked` 通过。
- 管理员权限下注册 DLL 后，Windows 能枚举该 language profile。
- 语言栏切换到该输入法时，TSF manager 调用 `DllGetClassObject` 和 `ActivateEx`，并能通过 debug/file log 观察。
- 重复注册不会产生重复 profile。
- 卸载后再次打开 Windows 输入法列表不再显示该 profile。
- 注册失败时返回非成功 HRESULT，并保留可诊断日志。

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
