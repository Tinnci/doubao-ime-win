# TSF TIP 完整完成计划

**日期**: 2026-06-17
**范围**: Milestone 1 `系统级输入法 / TSF TIP`

## 当前结论

`Doubao Voice Input` 出现在 Windows 键盘布局/输入法列表属于 Milestone 1 的 #4。当前 profile 已能注册并触发 TSF host `ActivateEx`，但还缺 keyboard TIP category：

- 必须注册 `GUID_TFCAT_TIP_KEYBOARD`，否则 Windows 可能不会把该 text service 归类到键盘/输入法布局列表。
- profile 描述和 icon path 必须用 NUL 结尾的 UTF-16 字符串传给 TSF 注册 API，否则 registry 中 `Description` 可能出现脏数据。
- 已注册 DLL 被 TSF host 加载后会锁住 `doubao_tsf_tip.dll`，开发脚本需要能只刷新注册工具和 TSF 元数据，而不是每次都重写 DLL。

相关代码：

- `crates/tsf-tip/src/windows_tip.rs`
  - `register_tsf_profile`
  - `register_tsf_categories`
  - `unregister_tsf_categories`
  - `query_registration_status`
- `crates/tsf-tip/src/bin/doubao-tip-tool.rs`
- `scripts/register-tip.ps1`
- `scripts/check-tip-registration.ps1`
- `scripts/unregister-tip.ps1`

## Windows API / 示例索引

### 注册和键盘列表可见性

| API / 常量 | 用途 | 当前策略 |
|------------|------|----------|
| `DllRegisterServer` / `DllUnregisterServer` | COM DLL 自注册入口 | 保留，调用统一注册/卸载逻辑 |
| `HKCR\CLSID\{TIP_CLSID}\InProcServer32` | COM in-proc server 路径 | 已写入 DLL path 和 `ThreadingModel=Apartment` |
| `ITfInputProcessorProfiles::Register` | 注册 text service CLSID | 已使用 |
| `ITfInputProcessorProfiles::AddLanguageProfile` | 添加 language profile | 已使用，字符串改为 NUL 结尾 |
| `ITfInputProcessorProfiles::EnableLanguageProfile` | 启用 language profile | 已使用 |
| `ITfCategoryMgr::RegisterCategory` | 给 TIP 注册 category | 必须注册 `GUID_TFCAT_TIP_KEYBOARD` |
| `GUID_TFCAT_TIP_KEYBOARD` | 声明该 TIP 是 keyboard text service | #4 键盘布局可见性的关键补项 |
| `ITfInputProcessorProfileMgr::RegisterProfile` | 较新的 profile manager 注册 API | 若 category + AddLanguageProfile 仍不显示，再切换/补充 |
| `ITfInputProcessorProfileMgr::ActivateProfile` | 激活当前用户 profile | 若 HKCU language list 不出现，再用于用户侧启用 |

官方文档：

- [Text Services Framework](https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework)
- [ITfInputProcessorProfiles::AddLanguageProfile](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofiles-addlanguageprofile)
- [ITfInputProcessorProfiles::EnableLanguageProfile](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofiles-enablelanguageprofile)
- [ITfInputProcessorProfileMgr::RegisterProfile](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-registerprofile)
- [ITfInputProcessorProfileMgr::ActivateProfile](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfinputprocessorprofilemgr-activateprofile)
- [ITfCategoryMgr::RegisterCategory](https://learn.microsoft.com/en-us/windows/win32/api/msctf/nf-msctf-itfcategorymgr-registercategory)

示例参考：

- `nathancorvussolis/code-gallery-tsf-samples`
  - `Keyboard/Register.cpp`: `RegisterProfiles` + `RegisterCategories`
  - `Keyboard/Readme.txt`: 明确 keyboard text service 必须注册 `GUID_TFCAT_TIP_KEYBOARD`
  - `Composition/*`: edit session 和 composition 生命周期
  - `CandidateList/*`: candidate window 和 composition 结合
  - `IconInLanguageBar` / `Keyboard/LanguageBar.cpp`: language bar item

### TIP 生命周期和输入

| API / 接口 | 用途 | 对应 issue |
|------------|------|------------|
| `ITfTextInputProcessorEx::ActivateEx` | TIP 激活入口，保存 `ITfThreadMgr` 和 client id | #3 |
| `ITfTextInputProcessor::Deactivate` | TIP 停用，释放 sinks、UI、worker/session | #3/#5/#6 |
| `ITfThreadMgr` | TSF thread manager | #5 |
| `ITfDocumentMgr` / `ITfContext` | 焦点文档和编辑上下文 | #5 |
| `ITfContext::RequestEditSession` | 请求 edit session | #5 |
| `ITfEditSession::DoEditSession` | 读写 TSF context 的唯一位置 | #5 |
| `ITfContextComposition::StartComposition` | 开始 composition | #5 |
| `ITfComposition::EndComposition` | 结束 composition | #5 |
| `ITfCompositionSink` | composition 被系统终止时回调 | #5 |
| `ITfKeyEventSink` / `ITfKeystrokeMgr` | keyboard TIP 拦截按键、保留快捷键 | #5 后按需 |
| `ITfThreadMgrEventSink` / `ITfTextEditSink` | 焦点/context/text 变化监听 | #5/#6 |

官方文档：

- [Compositions](https://learn.microsoft.com/en-us/windows/win32/tsf/compositions)
- [Edit Sessions](https://learn.microsoft.com/en-us/windows/win32/tsf/edit-sessions)
- [Language Bar](https://learn.microsoft.com/en-us/windows/win32/tsf/language-bar)

## 悬浮窗 / 状态 UI 设计

### 推荐架构

第一版不要把复杂 UI 直接放进 TIP DLL。TIP DLL 跑在目标应用/TSF host 进程内，复杂窗口、网络、音频和崩溃风险都应隔离。

推荐拆成：

```text
TSF host process
└── doubao_tsf_tip.dll
    ├── TSF lifecycle / edit sessions
    ├── composition state
    ├── lightweight diagnostics
    └── IPC client

User session process
└── doubao-tip-ui.exe
    ├── floating status window
    ├── candidate/status panels
    ├── settings / account prompts
    ├── ASR session orchestration if需要隔离
    └── IPC server
```

### IPC 方案

| 方案 | 优点 | 风险 | 结论 |
|------|------|------|------|
| Named Pipe | Windows 原生、支持同用户会话 ACL、Rust 支持好、适合命令/事件流 | 需要设计重连和版本协议 | 推荐第一版 |
| Local TCP (`127.0.0.1`) | 调试方便、跨语言简单 | 防火墙/端口占用/安全边界弱 | 不推荐作为默认 |
| COM local server | Windows 语义强 | 实现成本高，注册复杂 | 发布期可评估 |
| Shared memory + event | 低延迟 | 协议复杂，收益不大 | 暂不需要 |
| Window messages | 简单 | 跨完整性级别、焦点和句柄生命周期麻烦 | 只适合简单唤醒 |

Named Pipe 初始协议：

```text
TipHello { tip_instance_id, pid, session_id, client_id }
TipActivated { langid, profile_guid }
TipDeactivated {}
FocusChanged { hwnd?, rect?, dpi? }
CompositionState { idle | recording | recognizing | committing | error }
CandidateUpdate { revision, text, alternatives[] }
Command { start_recording | stop_recording | cancel | commit_selected | open_settings }
UiReady {}
UiClosed {}
```

### UI 功能范围

#6 候选/状态 UI 应覆盖：

- 录音状态：idle / recording / recognizing / committing / error。
- 候选文本：interim 文本、final 文本、可选 alternatives。
- 光标跟随：基于 TSF context/view 的 caret rectangle；失败时退回到屏幕右下/上次位置。
- DPI 和多显示器：按 monitor DPI 缩放，避免跨屏错位。
- 焦点变化清理：TIP deactivate、context change、session cancel 后隐藏窗口。
- 失败提示：认证失败、网络错误、麦克风权限、ASR 超时。
- 隐私：日志不得写 token、音频内容、完整凭据。

## 完成计划

### Gate A: #4 键盘布局可见

1. 注册 `GUID_TFCAT_TIP_KEYBOARD`。
2. 修复 `AddLanguageProfile` 字符串 NUL 结尾。
3. elevated 运行 `scripts/register-tip.ps1` 刷新注册。
4. `doubao-tip-tool status` 必须显示：
   - `COM key present: yes`
   - `TSF profile registered: yes`
   - `TSF profile enabled: yes`
   - `keyboard category registered: yes`
5. Windows 设置/任务栏输入法列表能看到 `Doubao Voice Input`。
6. 切换进出能看到 `ActivateEx` / `Deactivate`。
7. `scripts/unregister-tip.ps1` 后无残留。

### Gate B: #5 固定文本 composition

1. 保存 `ITfThreadMgr`、client id、当前 `ITfContext`。
2. 实现 edit session COM object。
3. 用固定文本开始 composition、更新 composition、commit final。
4. Notepad 和浏览器输入框验证。
5. deactivate/cancel/context lost 必须结束 composition。

### Gate C: #7 ASR event bridge

1. `voice_core` event 进入 TIP 队列。
2. interim 合并/节流后更新 composition。
3. final commit。
4. recoverable error 显示状态，fatal error 清理 composition。
5. ASR worker 不持有 TSF COM 指针。

### Gate D: #6 UI service

1. 新增 `doubao-tip-ui.exe`。
2. Named Pipe IPC。
3. 悬浮状态窗和候选窗。
4. caret 跟随、DPI、多显示器。
5. TIP deactivate / UI crash / IPC reconnect 全覆盖。

### Gate E: #8 QA / Release

1. Windows 10/11 x64。
2. Notepad、Edge/Chrome、Office、WinUI/WPF、Electron。
3. 安装、升级、卸载、重启。
4. 签名、Defender/SmartScreen。
5. release blocker 清单归零或有降级说明。
