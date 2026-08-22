# agents-skills

[![crates.io](https://img.shields.io/crates/v/agents-skills.svg)](https://crates.io/crates/agents-skills)
[![docs.rs](https://img.shields.io/docsrs/agents-skills.svg)](https://docs.rs/agents-skills)
[![CI](https://github.com/luckie2076/agents-skills/actions/workflows/ci.yml/badge.svg)](https://github.com/luckie2076/agents-skills/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

一个极简的 AI Agent 技能安装与管理工具：所有技能集中存放在一个**规范目录**，
通过一次 `link` 即可让 [Claude Code](https://claude.com/code)、Codex、Cursor 等
70+ 编程 Agent 全部可见可用——安装一次，处处生效。

```bash
cargo install agents-skills
```

同时提供可嵌入的 Rust 库，适合把技能管理集成进自有工具的场景，
见[面向开发者](#面向开发者)。

> 另见：[English README](README.md)

## 快速开始

```bash
# 1. 核心一步：link 为所有已安装的 agent 创建指向规范目录的符号链接
agents-skills link

# 2. 安装技能包（装进规范目录后，所有 agent 立即可见）
agents-skills add anthropics/skills

# 查看已安装技能与链接状态
agents-skills list
```

核心是 `link`：技能只在规范目录保存一份（项目级 `.agents/skills/` 或全局级
`~/.agents/skills/`），`link` 为每个已安装的 agent（Claude Code、Codex、Cursor…）
在其技能目录创建指向规范目录的符号链接；链接建立后，`add` 安装的技能所有
agent 立即可见，无需任何同步。

## 功能说明

### 核心：让 Agent 可见（link / unlink）

本质：技能只在规范目录保存一份真实副本；`link` 为每个 agent 在其技能目录创建
指向规范目录的符号链接（如 `.claude/skills` → `../.agents/skills`），使 70+ 编程
Agent 共享同一份技能，安装一次、处处可见。这是本项目最核心的能力：`add`/`remove`/
`update` 只操作规范目录，其余 agent 通过链接自动同步。

```bash
agents-skills link                           # 自动链接本机已安装的所有 agent
agents-skills link claude-code               # 只链接指定 agent
agents-skills link claude-code --migrate     # 链接并迁移存量技能
agents-skills unlink claude-code             # 解除链接
```

### 安装技能（add）

本质：从来源拉取技能包，发现其中的 `SKILL.md` 后复制进规范目录（`.agents/skills`
或 `~/.agents/skills`），并把来源与内容哈希写入 `skills-lock.json`。`add` 不做任何
agent 链接——让 agent 可见是 `link` 的职责（见上文）。

```bash
# 安装全部技能到规范目录
agents-skills add anthropics/skills

# 仅安装仓库中的指定技能
agents-skills add anthropics/skills@pdf

# 安装后运行 link 即可让 agent 可见
agents-skills link
```

### 列出技能（list）

本质：扫描规范目录，读取已安装技能与各 agent 的链接状态。

```bash
agents-skills list             # 项目级
agents-skills list --json      # 机器可读输出
```

### 移除技能（remove）

本质：删除规范目录中的技能副本并同步 lockfile；agent 通过符号链接共享规范目录，
一处删除处处生效。

```bash
agents-skills remove pdf       # 移除指定技能
agents-skills remove --all     # 移除全部技能
```

### 更新技能（update）

本质：按 lockfile 记录重新拉取来源，哈希比对后替换规范目录中的过时副本。

```bash
agents-skills update
```

### 项目级与全局作用域

本质：技能只在规范目录保存一份，`-g/--global` 决定这份副本属于当前项目
（`./.agents/skills`）还是用户目录（`~/.agents/skills`）。

### 来源格式

`add` 的 `<source>` 参数支持：

| 格式                | 示例                                                      |
| ------------------- | --------------------------------------------------------- |
| 本地路径            | `./my-skill`, `/abs/path/skill`                           |
| GitHub 简写         | `owner/repo`, `owner/repo@skill`, `owner/repo/subpath`    |
| GitHub / GitLab URL | `https://github.com/owner/repo`, `https://gitlab.com/group/repo` |
| SSH / git URL       | `git@github.com:owner/repo.git`                           |
| HTTPS（well-known） | `https://example.com/skills`（发现 → 下载兜底）           |
| HTTPS（下载）       | `.../skill.zip`, `.../skill.tar.gz`, 原始 `SKILL.md`      |

仓库内按优先级容器目录发现技能（`skills/`、`.curated/`、`.experimental/`、
`.system/`），浅层遮蔽深层。

### 安装位置

- **规范目录（唯一真实副本）** —— 项目级 `./.agents/skills/<name>`，全局级
  `~/.agents/skills/<name>`。
- **Agent 集成** —— 不原生读取规范目录的 agent 获得一个目录级符号链接：
  `.claude/skills` → `../.agents/skills`（项目）或 `~/.claude/skills` →
  `~/.agents/skills`（全局）。已链接的 agent 共享规范目录，无需任何同步步骤。

### 命令速查表

| 命令     | 别名                | 说明                            |
| -------- | ------------------- | ------------------------------- |
| `add`    | `a`, `i`, `install` | 从来源安装技能包                |
| `remove` | `rm`, `r`           | 移除已安装技能                  |
| `list`   | `ls`                | 列出已安装技能与 agent 链接     |
| `update` | `upgrade`, `check`  | 将技能更新到最新版本            |
| `link`   | `ln`                | 将 agent 技能目录链接到规范目录 |
| `unlink` | `un`                | 解除 agent 与规范目录的链接     |

> 完整的命令行参数（每个命令的选项与更多示例）见 [docs/CLI.md](docs/CLI.md)。

## 面向开发者

### 项目结构

```
src/
├── lib.rs              库根：重新导出 Manager + core 原语
├── manager.rs          高层 Manager 门面（add/list/remove/update/link/unlink）
├── error.rs            统一错误类型与 Result 别名
├── core/               领域逻辑（纯函数、依赖可注入）
│   ├── source.rs       来源字符串解析
│   ├── agents.rs       Agent → 技能目录映射表
│   ├── discover.rs     SKILL.md 发现 + frontmatter 解析
│   ├── fetch.rs        git 克隆 / HTTP 下载 / 归档解包
│   ├── install.rs      安装技能到规范目录 + 已装清单
│   ├── link.rs         目录级 agent 链接（link/unlink/migrate）
│   └── lock.rs         skills-lock.json 读写 + 内容哈希
├── main.rs             bin 入口（库之上的薄 CLI）
├── cli.rs              clap 命令树（命令、别名、flags）
└── commands/           CLI 渲染层（仅参数拆解 + 输出）
    ├── add.rs
    ├── remove.rs
    ├── list.rs
    ├── update.rs
    ├── link.rs
    └── unlink.rs

examples/
├── add_skill.rs        通过 Manager 门面安装技能（真实用法）
└── manage.rs           在临时目录上演示 add → list → remove 生命周期

tests/
├── common/mod.rs       集成测试共享夹具
├── lib_api.rs          库 API 集成测试
├── cli_add.rs
├── cli_remove.rs
├── cli_list.rs
├── cli_link.rs
└── cli_version.rs
```

### 库接口

CLI 只是同一套公开 Rust API 之上的薄渲染层，两者共享全部能力。

#### 高层：[`Manager`]

一站式操作。每个方法接收一个纯数据请求结构体，返回结构化结果。

| 方法                    | 请求                | 返回                                      |
| ----------------------- | ------------------- | ----------------------------------------- |
| [`Manager::add`]        | [`AddRequest`]      | [`AddOutcome`]（已安装 + 链接 + 失败）    |
| [`Manager::add_source`] | `impl Into<String>` | [`AddOutcome`]（已安装 + 链接 + 失败）    |
| [`Manager::link`]       | [`LinkRequest`]     | [`LinkManagerOutcome`]（逐 agent 结果）   |
| [`Manager::unlink`]     | [`UnlinkRequest`]   | [`UnlinkManagerOutcome`]（逐 agent 结果） |
| [`Manager::list`]       | [`ListRequest`]     | `Vec<`[`ListedSkill`]`>`（可序列化）      |
| [`Manager::remove`]     | [`RemoveRequest`]   | [`RemoveOutcome`]（已移除名称）           |
| [`Manager::update`]     | [`UpdateRequest`]   | [`UpdateOutcome`]（更新/失败计数）        |

请求结构体均为 `Default + Clone`，可通过字段覆盖构建；结果是纯数据。

#### 上下文注入：[`ManagerBuilder`]

```rust
use agents_skills::Manager;

let manager = Manager::builder()
    .home("/tmp/home")
    .config("/tmp/config")
    .cwd("/tmp/project")
    .env_var("CLAUDE_CONFIG_DIR", "/tmp/claude")
    .build();
```

`Manager::new()` 等价于 `Manager::builder().build()`，基于真实环境解析。

#### 底层：`core` 原语

如需更细粒度控制，底层的 `core` 函数已在 crate 根重新导出：

- **来源** —— [`parse_source`]、[`owner_repo`]
- **发现** —— [`discover_skills`]、[`filter_skills`]、[`parse_skill_md`]
- **安装** —— [`install_skill`]、[`list_installed_skills`]、[`sanitize_name`]
- **链接** —— [`link_agent`]、[`unlink_agent`]
- **锁文件** —— [`read_local_lock`]、[`write_local_lock`]、[`compute_folder_hash`]
- **Agent** —— [`get_agent`]、[`detect_installed_agents`]、[`Agent`]、[`Env`]

#### 库的定位

- **纯数据** —— 库从不打印、从不调用 `process::exit`；返回结构化结果，错误通过
  `Result` 上抛，如何渲染、何时退出由调用方决定。
- **上下文可注入** —— `ManagerBuilder` 可指定任意 `home`/`config`/`cwd`，
  测试与沙箱场景简单。
- **CLI 与库共享同一套 API** —— 所有命令行为在库层实现，CLI 只做参数拆解与输出。

#### 示例

```bash
cargo run --example manage      # 在临时目录上演示 add → list → remove（无副作用）
cargo run --example add_skill   # 通过 Manager 安装到你的真实环境
```

### 开发

```bash
cargo build            # 构建
cargo test             # 运行全部测试
cargo clippy           # lint
cargo fmt              # 格式化
```

测试遵循测试金字塔：快速、隔离的单元测试通过 `#[cfg(test)]` 内联在 `src/` 中，
而 `tests/` 中的黑盒集成测试通过 `assert_cmd` 驱动真实 CLI。

### 设计取舍

- **极简稳定** —— 刻意保持小而稳定，注重跨平台（macOS、Linux、Windows）。
- **纯数据** —— 库从不打印、从不调用 `process::exit`；结果结构化，错误通过
  `Result` 上抛。
- **无遥测** —— 不会有任何数据离开你的机器。

### License

在以下任一许可证下授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT license（[LICENSE-MIT](LICENSE-MIT)）

由你选择。

[`Manager`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html
[`Manager::add`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add
[`Manager::add_source`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add_source
[`Manager::link`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.link
[`Manager::unlink`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.unlink
[`Manager::list`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.list
[`Manager::remove`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.remove
[`Manager::update`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.update
[`ManagerBuilder`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ManagerBuilder.html
[`AddRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddRequest.html
[`AddOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddOutcome.html
[`LinkRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.LinkRequest.html
[`LinkManagerOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.LinkManagerOutcome.html
[`UnlinkRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UnlinkRequest.html
[`UnlinkManagerOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UnlinkManagerOutcome.html
[`ListRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListRequest.html
[`ListedSkill`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListedSkill.html
[`RemoveRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveRequest.html
[`RemoveOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveOutcome.html
[`UpdateRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateRequest.html
[`UpdateOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateOutcome.html
[`parse_source`]: https://docs.rs/agents-skills/latest/agents_skills/fn.parse_source.html
[`owner_repo`]: https://docs.rs/agents-skills/latest/agents_skills/fn.owner_repo.html
[`discover_skills`]: https://docs.rs/agents-skills/latest/agents_skills/fn.discover_skills.html
[`filter_skills`]: https://docs.rs/agents-skills/latest/agents_skills/fn.filter_skills.html
[`parse_skill_md`]: https://docs.rs/agents-skills/latest/agents_skills/fn.parse_skill_md.html
[`install_skill`]: https://docs.rs/agents-skills/latest/agents_skills/fn.install_skill.html
[`link_agent`]: https://docs.rs/agents-skills/latest/agents_skills/fn.link_agent.html
[`unlink_agent`]: https://docs.rs/agents-skills/latest/agents_skills/fn.unlink_agent.html
[`list_installed_skills`]: https://docs.rs/agents-skills/latest/agents_skills/fn.list_installed_skills.html
[`sanitize_name`]: https://docs.rs/agents-skills/latest/agents_skills/fn.sanitize_name.html
[`read_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/fn.read_local_lock.html
[`write_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/fn.write_local_lock.html
[`compute_folder_hash`]: https://docs.rs/agents-skills/latest/agents_skills/fn.compute_folder_hash.html
[`get_agent`]: https://docs.rs/agents-skills/latest/agents_skills/fn.get_agent.html
[`detect_installed_agents`]: https://docs.rs/agents-skills/latest/agents_skills/fn.detect_installed_agents.html
[`Agent`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Agent.html
[`Env`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Env.html
