# Doubao Voice Input - 后续路线图和功能计划

**版本**: v1.0  
**日期**: 2026-06-17  
**范围**: 从当前 TSF TIP milestone 到可发布的系统级 Windows 语音输入法

## 1. 产品方向

Doubao Voice Input 的主路线是系统级 Windows 语音输入法，而不是单纯的热键文本注入工具。

最终产品应满足：

- 用户能像切换普通输入法一样切换到 `Doubao Voice Input`。
- 语音识别的中间结果通过 TSF composition 显示在当前输入位置。
- 最终识别结果通过 TSF commit 提交到目标应用。
- 录音、识别、提交、错误和候选状态有明确 UI。
- 安装、升级、卸载、重启后的系统输入法状态可验证、可恢复。
- fallback app 继续保留，但只作为开发、排障和兼容回退路径。

## 2. 当前基线

截至 2026-06-17：

- `cargo check --workspace --locked` 通过。
- `doubao_tsf_tip.dll` 已存在，并导出 COM/TIP 必要入口。
- COM class factory、`ITfTextInputProcessorEx::ActivateEx` 和 `Deactivate` skeleton 已实现。
- `DllRegisterServer` / `DllUnregisterServer` 已覆盖 COM registry 和 TSF profile 注册/卸载。
- `GUID_TFCAT_TIP_KEYBOARD` 已注册，Windows 能把该 TIP 归类为键盘 text service。
- 本机注册检查显示 COM key、TSF profile、enabled state、keyboard category 均为 yes。
- 用户已确认 Windows 输入法/键盘布局列表能看到 `Doubao Voice Input`。
- `voice_core` 已抽出 ASR interim/final/error 事件，fallback app 仍通过 `SendInput` 工作。
- #5 已开始：F6 fixed-text TSF edit session / composition commit 代码路径已实现并编译，待注册新 DLL 后做真实输入框验证。

仍未完成的系统验收：

- 执行卸载脚本后确认 COM/profile/category registry 和 Windows 输入法列表无残留。
- 用 TSF composition/update/commit 取代主输入路径并通过 Notepad/浏览器验证。
- ASR interim/final 接入 TSF edit session。
- 候选/状态 UI、安装器、签名和 QA 矩阵尚未完成。

## 3. 路线图总览

路线图按能力依赖推进，不按日期硬切。每个 milestone 的完成标准必须能在真实 Windows 系统上复测。

| Milestone | 目标 | 主要交付物 | 退出标准 |
|-----------|------|------------|----------|
| M1: TSF TIP MVP | 把项目从辅助工具推进为可切换、可提交文本的 TSF TIP | 注册/卸载、activation、composition、ASR bridge、基础 UI、QA checklist | Windows 输入法可见，可切换，Notepad/浏览器可 composition + commit，ASR final 走 TSF |
| M2: 安装和发布硬化 | 让开发期脚本升级为可交付安装流程 | installer、签名、升级/卸载、诊断收集、崩溃日志 | 普通用户按安装包完成安装/卸载，重启和升级后状态可验证 |
| M3: 语音输入体验 v1 | 做出日常可用的语音输入体验 | 设置页、状态窗、候选/改写、热键策略、错误恢复 | 用户能稳定录音、取消、提交、重试，UI 不打断输入 |
| M4: 识别质量和可靠性 | 提升 ASR 稳定性、延迟和文本质量 | 重连、token 刷新、标点/分段、节流、网络错误恢复 | 长时间使用不会卡死，弱网可恢复，日志能定位 ASR 问题 |
| M5: 兼容性和安全 | 覆盖更多应用和系统环境 | 兼容矩阵、隐私审计、凭据保护、最小权限 | release blocker 归零，敏感数据不进日志，主流应用通过 |
| M6: 发布运营 | 支撑公开分发和持续维护 | release channel、更新策略、问题反馈、版本策略 | 可重复发布，用户问题可诊断，回滚路径明确 |

## 4. M1: TSF TIP MVP

M1 是当前 GitHub milestone `系统级输入法 / TSF TIP`。它的目标不是做完整产品，而是把主输入路径从 `SendInput` 切到 TSF。

### 4.1 #3/#4 系统注册收尾

当前代码已经完成注册实现，且已经过部分系统验证。

已完成：

- Windows 输入法 UI 中能看到 `Doubao Voice Input`。
- 切换到该输入法时，已能观察到 `%LOCALAPPDATA%\DoubaoVoiceInput\tsf-tip.log` 中出现 `ActivateEx`。
- 通过 `doubao-tip-tool switch-test` 已验证切出输入法时出现 `Deactivate`，并能恢复到原 active keyboard profile。

剩余 gate：

- 执行 `.\scripts\unregister-tip.ps1`，再执行 `.\scripts\check-tip-registration.ps1`。
- 确认 `COM key present`、`TSF profile registered`、`keyboard category registered` 都回到 no。
- 再打开 Windows 输入法列表，确认没有残留条目。

完成后可关闭：

- [#3 搭建 TSF Text Input Processor COM DLL 骨架](https://github.com/Tinnci/doubao-ime-win/issues/3)
- [#4 注册 language profile 并显示在 Windows 输入法列表](https://github.com/Tinnci/doubao-ime-win/issues/4)

### 4.2 #5 TSF composition MVP

这是下一个主要实现阶段。必须先用固定文本验证 TSF 管线，不能一开始接 ASR。

功能切片：

1. 保存 activation 期间的 `ITfThreadMgr` 和 client id。
2. 获取当前 `ITfDocumentMgr` / `ITfContext`。
3. 实现 `ITfEditSession` COM object。
4. 所有上下文写入只发生在 `DoEditSession` 内。
5. 实现 composition 状态机：
   - idle
   - composing
   - committed
   - cancelled
   - context_lost
6. 支持固定文本的 start/update/commit。
7. 支持 cancel、deactivate、focus lost 时清理 composition。
8. 在 Notepad 和 Edge/Chrome 输入框验证。

验收标准：

- 不调用 `SendInput`，也能在目标应用显示组合文本。
- fixed interim 文本能更新 composition。
- fixed final 文本能 commit。
- 取消或切换输入法后没有半截 composition 残留。
- 目标应用拒绝 edit session 时不会崩溃或死锁。

### 4.3 #7 ASR event bridge

在 #5 固定文本 composition 稳定后，再接入 `voice_core`。

功能切片：

1. TIP activation 后初始化 core event bridge，但不在 TSF 线程等待网络或音频。
2. `RecordingStarted` 更新状态，不直接写 TSF context。
3. `InterimText` 合并和节流后请求 edit session 更新 composition。
4. `FinalText` 请求 edit session commit 文本并结束 composition。
5. `Cancelled` / `Error` / `SessionEnded` 清理 composition 和 UI。
6. 给每个 ASR session 增加 revision/session id，丢弃过期事件。
7. 处理网络失败、认证失败、麦克风权限失败和 ASR 超时。

验收标准：

- 录音过程中目标应用内能看到 interim composition 更新。
- 停止录音后 final 文本通过 TSF commit。
- 连续开始/取消/重试不会遗留 composition。
- ASR worker 不持有 TSF COM 指针。
- TSF 线程不等待 WebSocket、音频采集或 token 刷新。

### 4.4 #6 UI service 和候选/状态 UI

复杂 UI 不应直接塞进 TIP DLL。TIP DLL 会被目标应用或 TSF host 加载，复杂窗口、网络和崩溃风险应隔离到用户会话进程。

目标进程结构：

```text
Target app / TSF host
└── doubao_tsf_tip.dll
    ├── TSF lifecycle
    ├── composition manager
    ├── lightweight diagnostics
    └── IPC client

User session
└── doubao-tip-ui.exe
    ├── floating status window
    ├── candidate/status panel
    ├── settings/account prompts
    ├── optional ASR orchestration
    └── IPC server
```

推荐 IPC：

- 第一版使用 Named Pipe。
- 每个用户 session 建立独立 pipe。
- TIP 侧必须支持 UI process 未启动、重启、崩溃和断线重连。
- 协议需要版本号、session id、tip instance id 和 message revision。

第一版消息范围：

| 消息 | 方向 | 用途 |
|------|------|------|
| `TipHello` | TIP -> UI | 注册 TIP 实例、pid、client id、profile |
| `TipActivated` | TIP -> UI | 显示状态入口 |
| `TipDeactivated` | TIP -> UI | 隐藏 UI 并释放状态 |
| `FocusChanged` | TIP -> UI | 更新 caret rect、dpi、monitor |
| `CompositionState` | TIP -> UI | idle/recording/recognizing/committing/error |
| `CandidateUpdate` | TIP -> UI | interim/final text 和 alternatives |
| `Command` | UI -> TIP | start/stop/cancel/commit_selected/open_settings |
| `UiReady` | UI -> TIP | UI process ready |
| `UiClosed` | UI -> TIP | UI process 正常退出 |

UI 功能范围：

- 小型状态窗：idle、recording、recognizing、committing、error。
- 候选/状态 panel：interim 文本、final 文本、alternatives。
- 跟随 TSF caret rectangle，失败时退回到屏幕右下或上次位置。
- 支持 DPI、多显示器、暗色模式和焦点切换清理。
- 右键/菜单入口打开设置、诊断日志、退出 UI service。

验收标准：

- 输入法停用、目标应用失焦、取消、错误后 UI 不残留。
- UI process 崩溃不影响目标应用。
- DPI 缩放和多显示器下窗口位置合理。
- UI 不遮挡当前输入文字。

### 4.5 #8 QA 和 release blocker

M1 退出前必须有可重复执行的 QA 矩阵。

覆盖环境：

- Windows 10 x64
- Windows 11 x64
- 管理员安装和普通用户使用
- 重启前后

覆盖应用：

- Notepad
- Edge / Chrome
- Office 文档输入框
- WinUI / WPF 输入框
- Electron 应用
- Windows Terminal 或终端类应用，作为兼容风险记录

必须验证：

- 安装注册
- 输入法列表可见
- 切换 activation/deactivation
- composition update/commit
- cancel/error cleanup
- ASR interim/final
- 卸载无残留
- 重复安装/卸载幂等
- 日志不含 token、完整凭据或原始语音数据

Release blocker：

- TIP activation 崩溃目标应用。
- TSF edit session 死锁。
- 卸载后 profile 或 registry 残留。
- 错误路径遗留 composition。
- UI 残留在桌面或遮挡输入。
- 日志泄露 token、设备凭据或用户语音内容。

## 5. M2: 安装和发布硬化

M2 的目标是把开发期注册脚本变成可交付安装能力。

功能计划：

- 安装包：
  - 安装 `doubao_tsf_tip.dll`
  - 安装 `doubao-tip-ui.exe`
  - 安装 fallback app
  - 注册 TSF profile 和 keyboard category
  - 写入卸载信息
- 卸载：
  - 停止 UI service 和 fallback app
  - 注销 profile/category/COM registry
  - 删除安装文件
  - 保留或删除用户配置由卸载选项决定
- 升级：
  - 检测已注册 DLL path
  - 处理 TSF host 锁 DLL 的情况
  - 升级后重新注册 profile metadata
  - 保留配置和凭据
- 签名：
  - DLL、EXE、installer 都需要签名
  - 明确证书来源和时间戳策略
- 诊断：
  - 安装日志
  - 注册状态检查
  - 一键收集 non-sensitive 诊断包

验收标准：

- 普通用户按安装向导完成安装。
- 安装后输入法可见并可切换。
- 重启后 profile 仍存在。
- 升级后原配置和凭据保留。
- 卸载后输入法列表无残留。

## 6. M3: 语音输入体验 v1

M3 的目标是让产品达到日常可用，而不是只完成系统 API 验证。

功能计划：

- 设置页：
  - 登录/设备注册状态
  - 麦克风设备选择
  - 热键设置
  - 悬浮状态窗开关
  - 日志级别和诊断导出
- 输入交互：
  - 按住说话或点击开始/停止
  - 取消当前识别
  - 重试上一段
  - 提交候选文本
  - 快速切换中英文标点策略
- 状态反馈：
  - 麦克风权限缺失
  - 网络不可用
  - 认证失败
  - ASR 超时
  - 当前应用不支持 TSF composition 的降级提示
- fallback 策略：
  - TSF 主路径失败时不自动静默退回 `SendInput`
  - 需要明确提示或开发配置开关，避免用户误以为仍是系统级输入法

验收标准：

- 用户能不看日志完成开始、取消、提交、重试。
- 错误提示可行动，不只显示失败。
- 设置变更不用重装输入法。
- fallback 行为可解释、可关闭、可诊断。

## 7. M4: ASR 质量和可靠性

M4 的目标是提升长时间使用体验和弱网恢复能力。

功能计划：

- ASR session 管理：
  - token 过期前刷新
  - WebSocket 断线重连
  - 超时和取消分层处理
  - 并发 session 防重入
- 文本质量：
  - 标点策略
  - 分段策略
  - interim 抖动抑制
  - final 文本去重
  - 可选热词/上下文提示
- 延迟：
  - 音频 frame 到 interim 的链路计时
  - edit session 排队延迟计时
  - UI update 节流
- 资源：
  - 麦克风占用释放
  - 后台 worker shutdown
  - 日志滚动和大小限制

验收标准：

- 30 分钟连续使用无资源泄漏或卡死。
- 弱网断开后能清理当前 session 并允许重试。
- interim 变化不会导致 composition 闪烁或 edit session 堆积。
- 诊断日志能定位 ASR、音频、TSF 或 UI 哪一层出问题。

## 8. M5: 兼容性和安全

M5 的目标是减少系统级输入法发布风险。

功能计划：

- 兼容性：
  - Office、浏览器、Electron、WinUI/WPF、传统 Win32 控件
  - 高 DPI、多显示器、远程桌面
  - UAC 提权窗口和不同完整性级别
  - Windows 输入法切换快捷键
- 安全：
  - 凭据只保存在安全存储
  - 日志脱敏
  - IPC ACL 限定当前用户
  - Named Pipe message 校验和版本兼容
  - 防止低完整性进程伪造敏感命令
- 稳定性：
  - TIP DLL 内禁止长时间阻塞
  - UI process crash isolation
  - panic boundary 和 HRESULT 映射
  - crash dump / minidump 策略

验收标准：

- 主流目标应用 QA 通过或有明确降级说明。
- 不记录 token、完整凭据、原始音频或完整隐私文本。
- IPC 不能被其他用户会话直接控制。
- 崩溃后能重启 UI，不拖垮目标应用。

## 9. M6: 发布运营

M6 的目标是让项目能持续迭代。

功能计划：

- release channel：
  - dev
  - beta
  - stable
- 版本策略：
  - TIP DLL、UI service、core protocol 版本分离
  - IPC protocol version
  - 配置 schema version
- 更新策略：
  - 手动检查更新
  - 安装包覆盖升级
  - 回滚上一版本
- 用户反馈：
  - 诊断包导出
  - 问题模板
  - 崩溃和安装失败排查指南

验收标准：

- 每个 release 有变更记录、验证矩阵和已知问题。
- 失败安装能回滚。
- 用户能导出不含敏感数据的诊断信息。

## 10. 功能模块计划

### 10.1 TSF shell

职责：

- COM DLL 导出和 class factory。
- `ITfTextInputProcessorEx` lifecycle。
- profile/category 注册。
- edit session 和 composition。
- TSF sinks 和 focus/context tracking。

近期优先级：

1. 完成 #3/#4 剩余系统验证。
2. 实现 #5 fixed-text composition。
3. 接入 #7 ASR event bridge。
4. 暴露 #6 UI service 所需的 focus/caret/state 事件。

### 10.2 Voice core

职责：

- 音频采集。
- Opus 编码。
- ASR WebSocket。
- token/device credential。
- session 状态机。
- 事件输出。

近期优先级：

1. 保持 core 不依赖 TSF/COM/UI。
2. 给 events 增加 session id 和 revision。
3. 增加 error kind 和 retryability。
4. 支持 ASR session cancel/shutdown 的确定性清理。

### 10.3 UI service

职责：

- 状态窗。
- 候选/识别文本 panel。
- 设置页。
- 诊断入口。
- 与 TIP 的 IPC。

近期优先级：

1. 新增 `doubao-tip-ui.exe`。
2. 建立 Named Pipe 协议。
3. 实现最小状态窗。
4. 再实现 caret 跟随候选/状态 panel。

### 10.4 Installer 和工具

职责：

- 安装、卸载、升级。
- 注册状态诊断。
- 日志收集。
- 签名和发布包。

近期优先级：

1. 保留开发期 PowerShell 脚本。
2. 把 `doubao-tip-tool status` 做成 release 诊断入口。
3. 设计安装器前先固定注册/卸载的幂等行为。
4. 处理 DLL 被 TSF host 锁定时的升级策略。

### 10.5 QA 和诊断

职责：

- 手工测试矩阵。
- 自动化 smoke checks。
- 日志和错误码。
- release blocker 管理。

近期优先级：

1. 为 #3/#4 记录手工验证结果。
2. 为 #5 增加 Notepad/浏览器固定文本 checklist。
3. 为 #7 增加 ASR 成功、取消、错误、超时 checklist。
4. 为 #6 增加 DPI、多屏、焦点切换 checklist。

## 11. 建议新增 GitHub issue

当前 milestone 已有 #3 到 #8 覆盖 M1。M1 之后建议新增 issue，而不是继续塞进 #9 Epic。

建议：

| 标题 | Milestone | 类型 |
|------|-----------|------|
| 设计并实现 `doubao-tip-ui.exe` Named Pipe IPC 协议 | M1 或 M2 | implementation |
| 建立 TSF composition fixed-text smoke test 手册 | M1 | qa |
| 设计 release installer 的安装/升级/卸载状态机 | M2 | architecture |
| 引入代码签名和安装包签名流程 | M2 | release |
| 实现设置页和诊断导出入口 | M3 | product |
| ASR session id、revision 和错误分类完善 | M4 | implementation |
| IPC ACL 和日志脱敏安全审计 | M5 | security |
| 发布渠道和版本策略 | M6 | release |

## 12. 明确延期的事项

以下事项不进入当前主线，避免影响 TSF 主路径完成：

- 完整拼音/五笔输入法引擎。
- 离线语音识别模型训练。
- 云端同步词库或用户语音数据。
- Microsoft Store 分发。
- arm64 原生支持。
- 企业集中部署策略。
- 自动静默回退到 `SendInput` 作为默认产品行为。

## 13. 最近三步

1. 用管理员 PowerShell 完成 #4 的 unregister cleanup 验证。
2. 进入 #5，实现 fixed-text TSF composition/update/commit。
3. #5 通过后接 #7，把 `voice_core` interim/final/error 映射到 TSF edit session。

这三步完成后，项目才真正从“输入法可见”进入“系统级输入法可用”的阶段。
