# codex-fold —— 本 fork 的定制说明

这是 [openai/codex](https://github.com/openai/codex) 的个人 fork，目的是把 TUI 的输出改得更**简洁、低调**，参考 Claude Code 的观感：把命令输出折叠成计数摘要，去掉多余的高亮、横线和留白。

命令名安装为 `codex-fold`（不覆盖官方 `codex`）。

## 改了什么

所有改动集中在 `codex-rs/tui/src/`：

| 功能 | 文件 | 说明 |
| --- | --- | --- |
| 命令输出折叠为计数摘要 | `exec_cell/model.rs`、`exec_cell/render.rs` | 连续的 agent 工具调用（shell / read / search / list）聚合进同一个 cell，渲染成 `Ran 2 shell commands`、`Searched for 1 pattern, ran 2 shell commands` 这类单行摘要；命令的完整 stdout/stderr/diff 不再内联，只在 `ctrl + t` transcript 里查看。失败信息也不在折叠视图里展示（不显示 `(N failed)`、不打印错误输出）——保持简洁，完整输出仍走 `ctrl + t`。 |
| 并行/复合命令也合并 | `chatwidget/command_lifecycle.rs` | 复合 shell（如 `git status && git log`）被解析成 `Unknown`，走 unified-exec 早返回路径、没有 begin 事件；其 end 到达时改为合并进当前 agent 组（不再要求组内仍有命令在跑），避免"flush 重建"把连续活动切成多条 `Ran 1 shell command`。 |
| 多 agent `Started` 合并 | `multi_agents.rs`（`StartedAgentsCell`）、`chatwidget/tool_lifecycle.rs` | 连续的 `Started <agent>` 聚合成一条 `Started N agents` + 缩进路径列表；单个时仍显示 `Started \`/root/xxx\``。作为 active cell 累积，遇其它活动 / 助手消息自动 flush。 |
| 摘要低调化 | `exec_cell/render.rs` | 摘要文本用暗灰（dim）、去掉前面的 `•`、缩进 2 格对齐消息正文。 |
| 去掉 turn 之间的横线 | `history_cell/separators.rs`、`chatwidget/streaming.rs`、`chatwidget/turn_runtime.rs` | 不再画整宽 `─` 分隔线，靠 cell 间原有的单行空行分隔；罕见的 "Worked for / metrics" 标签保留为纯文本、不带短横。 |
| 输入框 / 历史用户消息去灰底 | `style.rs` | `user_message_style` 改为透明，输入框和已发送消息与终端背景一致，不再是满宽灰带。 |
| 用户消息去上下内边距 | `history_cell/messages.rs` | 去掉灰底时代遗留的上下空行内边距。 |
| 输入框上方紧凑 | `bottom_pane/chat_composer.rs` | 去掉输入行上方的空行（top inset 1→0、`Min(3)→Min(2)`、高度常量 `+2→+1`）。 |

> ⚠️ 折叠改变了 flush 时机：完成的 agent 命令不再逐条滚入历史，而是留在底部累积，等助手开始说话 / 回合结束时提交为 `Ran N shell commands`。因此 `codex-rs/tui/src/chatwidget/tests/` 里若干断言旧渲染/旧时机的用例会失败——**不影响运行**，尚未更新。

## 构建与安装

本机（8GB 内存）用 **debug** 构建：release 开了 `lto = "thin"`，链接峰值内存会 OOM 被杀。debug 版功能一致，仅启动略慢，TUI 使用无感。

一键重建脚本（改完源码后运行即可）：

```bash
rebuild-codex-fold
```

脚本位于 `~/.local/bin/rebuild-codex-fold`，等价于：

```bash
source "$HOME/.cargo/env"
cargo build --bin codex -j 4 --manifest-path <此仓库>/codex-rs/Cargo.toml
cp <此仓库>/codex-rs/target/debug/codex ~/.local/bin/codex-fold
codesign --force --sign - ~/.local/bin/codex-fold   # 必须重签，否则复制后被 macOS SIGKILL（退出码 137）
```

`~/.local/bin` 已在 PATH 中，所以直接运行 `codex-fold` 即可（新开终端或 `hash -r` 生效）。官方 `codex`（npm 版）保持不变、互不影响。

## 环境

- Rust 工具链：rustup，仓库固定 `codex-rs/rust-toolchain.toml`（1.95.0）。
- 需 `source "$HOME/.cargo/env"` 才能用 cargo（脚本已自动处理）。
