# Doubao Voice Input - TSF TIP 任务清单

**版本**: v3.0  
**日期**: 2026-06-15  
**来源**: [GitHub milestone #1 系统级输入法 / TSF TIP](https://github.com/Tinnci/doubao-ime-win/milestone/1)

## 当前原则

- 按依赖顺序推进，不直接从 ASR 接 TSF 开始。
- 先验证 TIP 可注册、可加载、可激活，再做 composition 和 ASR event bridge。
- 当前 `SendInput` 输入路径保留为 fallback，不再扩展成主架构。
- 每个实现任务结束时同步更新对应 issue 的验收情况。

## 推进顺序

| 顺序 | Issue | 优先级 | 目标 |
|------|-------|--------|------|
| 1 | [#1 调研 TSF TIP 最小可行架构和风险边界](https://github.com/Tinnci/doubao-ime-win/issues/1) | P0 | 锁定最小架构、风险、demo 成功标准 |
| 2 | [#2 定义 Rust 核心与 TSF shell 的边界](https://github.com/Tinnci/doubao-ime-win/issues/2) | P0 | 抽出 ASR/core API，避免 TSF 细节污染业务逻辑 |
| 3 | [#3 搭建 TSF Text Input Processor COM DLL 骨架](https://github.com/Tinnci/doubao-ime-win/issues/3) | P0 | 构建可注册、可被 TSF manager 加载的 TIP DLL |
| 4 | [#4 注册 language profile 并显示在 Windows 输入法列表](https://github.com/Tinnci/doubao-ime-win/issues/4) | P0 | 注册 profile，支持切换和卸载清理 |
| 5 | [#5 实现 TSF composition 会话和文本提交模型](https://github.com/Tinnci/doubao-ime-win/issues/5) | P1 | 用 TSF composition/update/commit 替代主路径输入注入 |
| 6 | [#7 把 ASR 流式结果接入 TSF 输入管线](https://github.com/Tinnci/doubao-ime-win/issues/7) | P1 | 将 interim/final/error 映射到 TSF event bridge |
| 7 | [#6 实现候选窗、模式状态和光标定位](https://github.com/Tinnci/doubao-ime-win/issues/6) | P1 | 添加候选/状态 UI、caret 定位、DPI 和焦点清理 |
| 8 | [#8 建立系统级 IME 兼容性和发布 QA 矩阵](https://github.com/Tinnci/doubao-ime-win/issues/8) | P1 | 建立可重复执行的手工 QA 和 release blocker 标准 |

[#9 Epic: 系统级输入法 / TSF TIP 路线图](https://github.com/Tinnci/doubao-ime-win/issues/9) 只做总控跟踪，不承载具体实现。

## Phase 0: 文档和架构收敛

- [x] 合并旧 PRD，删除过时的“无需 TSF”主路线。
- [x] 明确 `SendInput` 是 fallback，不是系统级 IME 主路径。
- [x] 建立 TSF TIP 产品需求、技术架构、任务清单和目标目录结构。
- [x] 在 #1 中记录最终架构决策：Rust TIP shell、C++ TIP shell，或混合方案。

## Phase 1: P0 架构和边界

### #1 调研 TSF TIP 最小可行架构和风险边界

- [x] 列出必须实现的 TSF/COM 接口。
- [x] 明确 TIP DLL 注册和 language profile 注册路径。
- [x] 比较 Rust `cdylib` shell、C++ shell、混合方案的成本。
- [x] 明确 app-container、签名、Defender、权限和 Windows 版本风险。
- [x] 定义 demo 成功标准：输入法可见、可切换、可提交固定文本。

### #2 定义 Rust 核心与 TSF shell 的边界

- [x] 定义 core API：initialize/start/stop/cancel/subscribe/shutdown。
- [x] 把 ASR session、配置、凭据、状态事件从当前控制器中解耦。
- [x] 定义 TSF shell 接收的事件类型和错误模型。
- [x] 明确线程模型：ASR worker 不持有 TSF COM 指针。
- [x] 确保现有 exe/fallback 路径仍可构建和运行。

## Phase 2: 最小 TIP 可加载

### #3 搭建 TSF Text Input Processor COM DLL 骨架

- [ ] 新增 TIP DLL 工程或 crate。
- [ ] 实现 COM class factory。
- [ ] 导出 `DllGetClassObject`、`DllCanUnloadNow`、`DllRegisterServer`、`DllUnregisterServer`。
- [ ] 实现最小 `ITfTextInputProcessorEx` activation/deactivation。
- [ ] 添加 TIP 加载、激活、停用日志。

### #4 注册 language profile 并显示在 Windows 输入法列表

- [ ] 定义 CLSID、profile GUID、描述、图标和语言标识。
- [ ] 使用 `ITfInputProcessorProfiles` 注册 profile。
- [ ] 提供开发期注册/卸载脚本或工具。
- [ ] 验证 Windows 设置和任务栏输入指示器可见。
- [ ] 验证卸载后 profile 和 registry 清理干净。

## Phase 3: TSF 输入主路径

### #5 实现 TSF composition 会话和文本提交模型

- [ ] 管理 document manager、context 和 edit session。
- [ ] 建立 composition 生命周期状态机。
- [ ] 支持固定文本的 composition update 和 final commit。
- [ ] 支持 cancel 和错误清理。
- [ ] 在 Notepad、Edge/Chrome、WinUI/WPF 文本框中验证。

### #7 把 ASR 流式结果接入 TSF 输入管线

- [ ] 把 core `InterimText` 转成 composition update。
- [ ] 把 core `FinalText` 转成 commit。
- [ ] 处理认证失败、网络错误、超时、取消和重试。
- [ ] 给 event bridge 增加节流、合并和过期事件丢弃。
- [ ] 验证错误路径不会造成 TSF 死锁、崩溃或遗留 composition。

## Phase 4: UI 和发布验证

### #6 实现候选窗、模式状态和光标定位

- [ ] 实现候选/状态窗口显示、更新、隐藏。
- [ ] 根据 TSF layout/caret rectangle 定位。
- [ ] 显示录音、识别中、提交、错误状态。
- [ ] 处理 DPI、多显示器、暗色模式和窗口焦点变化。
- [ ] 验证焦点切换、窗口移动、DPI 缩放下不残留 UI。

### #8 建立系统级 IME 兼容性和发布 QA 矩阵

- [ ] 覆盖 Windows 10/11、管理员/普通用户、x64。
- [ ] 覆盖 Notepad、Edge/Chrome、Office、WinUI/WPF、Electron、终端类应用。
- [ ] 覆盖安装、升级、卸载、重启后 profile 保持和清理。
- [ ] 定义 release blocker：崩溃、死锁、profile 残留、无法卸载、凭据泄露。
- [ ] 记录签名、Defender/SmartScreen、崩溃日志和诊断收集要求。

## 关闭 milestone 的标准

- 输入法能注册到 Windows 输入法列表。
- 用户可以切换到该输入法并触发 TSF activation。
- 至少在 Notepad 和现代浏览器输入框内完成 composition 更新和 final commit。
- 安装、卸载、重启后状态可验证。
- QA checklist 有明确结果，release blocker 全部关闭或降级说明。
