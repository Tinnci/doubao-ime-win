# PRD 文档说明

本目录维护 Doubao Voice Input 的当前产品和技术文档。文档已经从旧的“绿色便携语音输入辅助工具”路线收敛到当前 milestone：系统级 Windows 输入法 / TSF TIP。

## 文档列表

| 文档 | 作用 |
|------|------|
| [windows-ime-requirements.md](./windows-ime-requirements.md) | 产品目标、范围、非目标、验收标准和 milestone issue 映射 |
| [milestone-1-roadmap.md](./milestone-1-roadmap.md) | milestone 目标、阶段、当前状态、下一步和退出标准 |
| [technical-architecture.md](./technical-architecture.md) | TSF TIP 技术架构、Rust core 边界、COM/profile/composition 设计 |
| [adr-0001-tsf-tip-architecture.md](./adr-0001-tsf-tip-architecture.md) | #1 架构决策：推荐方案、放弃方案、接口、注册路径、风险和 demo 标准 |
| [core-shell-boundary.md](./core-shell-boundary.md) | #2 Rust core 与 TSF shell 的 API、事件、错误和线程边界 |
| [task-list.md](./task-list.md) | 按 GitHub milestone issue 拆分的推进顺序和验收检查 |
| [project-structure.md](./project-structure.md) | 当前目录与目标 workspace/TSF 模块结构 |
| [tsf-tip-completion-plan.md](./tsf-tip-completion-plan.md) | TSF 注册、composition、UI service、IPC 和 QA 的完整完成计划 |

## 已合并的旧文档

| 旧文档 | 处理方式 |
|--------|----------|
| `doubao-voice-input-prd.md` | 内容并入产品需求；旧的单纯便携工具定位已过时 |
| `语音输入增量更新优化.md` | 内容并入技术架构的 `SendInput` 兼容路径说明；独立文档已删除 |

## 阅读顺序

1. 先读 [产品需求](./windows-ime-requirements.md)，确认当前要做的是系统级输入法，而不是只做文本注入工具。
2. 再读 [Milestone 1 路线图](./milestone-1-roadmap.md)，确认阶段目标、当前状态和下一步。
3. 再读 [架构决策 ADR](./adr-0001-tsf-tip-architecture.md)，确认为什么选择当前路线。
4. 阅读 [技术架构](./technical-architecture.md) 和 [Core/Shell 边界](./core-shell-boundary.md)，确认 TSF shell、Rust core、ASR worker 和 UI 的边界。
5. 用 [任务清单](./task-list.md) 对齐 GitHub milestone 的实现顺序。
6. 需要新增模块或调整目录时，参考 [项目结构](./project-structure.md)。

## 维护规则

- 文档中的主路线以 TSF TIP milestone 为准。
- `SendInput` 只作为现有辅助工具和兼容回退路径记录，不再作为系统级输入法主设计。
- 新增实现任务优先挂到 GitHub issue，并在 [task-list.md](./task-list.md) 保持同步。
- 如果实现路线变化，先更新架构决策，再改任务清单。

## 更新记录

| 日期 | 版本 | 更新内容 |
|------|------|----------|
| 2026-06-17 | v3.3 | 补充 TSF TIP 完整完成计划，明确 keyboard category、UI service 和 IPC 路线 |
| 2026-06-16 | v3.2 | 细化 Milestone 1 gate，补 #4 开发期注册/卸载/status 工具和脚本状态 |
| 2026-06-15 | v3.1 | 细化 Milestone 1 / #4 language profile 注册目标，同步最小 COM/TSF 注册实现状态 |
| 2026-06-15 | v3.0 | 合并旧 PRD，删除重复文档，切换到系统级 TSF TIP 路线 |
| 2026-02-05 | v2.0 | 旧简化版语音输入辅助工具文档 |

## 相关链接

- [项目 README](../README.md)
- [GitHub milestone: 系统级输入法 / TSF TIP](https://github.com/Tinnci/doubao-ime-win/milestone/1)
- [Windows TSF 官方文档](https://learn.microsoft.com/en-us/windows/win32/tsf/text-services-framework)
