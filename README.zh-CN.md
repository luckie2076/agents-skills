# agents-skills

[![crates.io](https://img.shields.io/crates/v/agents-skills.svg)](https://crates.io/crates/agents-skills)
[![docs.rs](https://img.shields.io/docsrs/agents-skills.svg)](https://docs.rs/agents-skills)
[![CI](https://github.com/luckie2076/agents-skills/actions/workflows/ci.yml/badge.svg)](https://github.com/luckie2076/agents-skills/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

一个极简、稳定的 **Rust 库**，用于安装与管理 AI Agent 技能，并附带一个基于该库
构建的命令行接口。

`agents-skills` 是**库优先**的：把它引入你的 Rust 项目，即可为
[Claude Code](https://claude.com/code)、Codex、Cursor 等 70+ 编程 Agent 安装、列出、
移除、更新 `SKILL.md` 包。同时附带一个小型 CLI（`agents-skills`），它只是同一套公开
API 之上的薄渲染层。

> 另见：[English README](README.md)

## 为什么是库？

- **把技能管理嵌入你自己的工具** —— 插件管理器、Agent 启动器、构建脚本都可以直接
  安装技能，无需再调用外部二进制。
- **纯数据、无 stdout 副作用** —— 每个 API 返回结构化结果，错误通过 `Result` 上抛；
  库从不打印、从不调用 `process::exit`。如何渲染、何时退出由你决定。
- **上下文可注入** —— `ManagerBuilder` 可指定任意的 `home`/`config`/`cwd`，让测试和
  沙箱场景变得简单。

## 快速开始

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
agents-skills = "1"
```

使用高层 [`Manager`] 门面安装并列出技能：

```rust
use agents_skills::{AddRequest, ListRequest, Manager, Result};

fn main() -> Result<()> {
    let manager = Manager::new();

    // 快捷方式：使用默认选项安装来源中的所有技能。
    let outcome = manager.add_source("anthropics/skills")?;
    println!("已安装 {} 个技能", outcome.installed.len());

    // 完整形式：安装到指定 Agent（其余字段取默认值）。
    let outcome = manager.add(&AddRequest {
        source: "anthropics/skills".to_string(),
        agents: vec!["*".to_string()],
        ..Default::default()
    })?;
    println!("已安装 {} 个技能", outcome.installed.len());

    // 列出已安装技能（可序列化；与 `list --json` 同构）。
    let skills = manager.list(&ListRequest::default())?;
    println!("{skills:?}");
    Ok(())
}
```

## API

### 高层：[`Manager`]

一站式操作。每个方法接收一个纯数据请求结构体，返回结构化结果。

| 方法                    | 请求                | 返回                                 |
| ----------------------- | ------------------- | ------------------------------------ |
| [`Manager::add`]        | [`AddRequest`]      | [`AddOutcome`]（已安装 + 失败）      |
| [`Manager::add_source`] | `impl Into<String>` | [`AddOutcome`]（已安装 + 失败）      |
| [`Manager::list`]       | [`ListRequest`]     | `Vec<`[`ListedSkill`]`>`（可序列化） |
| [`Manager::remove`]     | [`RemoveRequest`]   | [`RemoveOutcome`]（已移除名称）      |
| [`Manager::update`]     | [`UpdateRequest`]   | [`UpdateOutcome`]（更新/失败计数）   |

请求结构体均为 `Default + Clone`，可通过字段覆盖进行构建；结果是纯数据。

### 上下文注入：[`ManagerBuilder`]

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

### 底层：`core` 原语

如需更细粒度控制，底层的 `core` 函数已在 crate 根重新导出：

- **来源** —— [`parse_source`]、[`owner_repo`]
- **发现** —— [`discover_skills`]、[`filter_skills`]、[`parse_skill_md`]
- **安装** —— [`install_skill_for_agent`]、[`list_installed_skills`]、[`sanitize_name`]
- **锁文件** —— [`read_local_lock`]、[`write_local_lock`]、[`compute_folder_hash`]
- **Agent** —— [`get_agent`]、[`detect_installed_agents`]、[`Agent`]、[`Env`]

### 示例

运行内置示例查看真实用法：

```bash
cargo run --example manage      # 在临时目录上演示 add → list → remove（无副作用）
cargo run --example add_skill   # 通过 Manager 安装到你的真实环境
```

## 特性

- **从任意来源安装** —— 本地路径、GitHub 仓库/URL、GitLab、SSH/git URL，以及任意
  HTTPS 端点（well-known 发现或直接下载）。
- **70+ Agent** —— 静态目录映射表，数据驱动、依赖可注入，便于测试。
- **项目级与全局作用域** —— 安装到 `.agents/skills`（项目）或 `~/.agents/skills`
  （全局），支持 symlink 或 copy 模式。
- **Lockfile** —— `skills-lock.json` 记录每个已安装技能的来源与 SHA-256 内容哈希，
  使 `update` 可复现。
- **技能发现** —— 优先级容器目录（`skills/`、`.curated/`、`.experimental/`、
  `.system/`），浅层遮蔽深层。
- **跨平台** —— macOS、Linux、Windows（Windows 使用目录 symlink，`git2` 提供传输
  无关的克隆）。

## 来源格式

[`AddRequest`] 的 `source` 字段（以及 CLI 的 `<source>` 参数）支持：

| 格式                | 示例                                                      |
| ------------------- | --------------------------------------------------------- |
| 本地路径            | `./my-skill`, `/abs/path/skill`                           |
| GitHub 简写         | `owner/repo`, `owner/repo@skill`, `owner/repo/subpath`    |
| GitHub URL          | `https://github.com/owner/repo`, `.../tree/main/skills`   |
| GitLab URL          | `https://gitlab.com/group/repo`, `.../-/tree/main/skills` |
| SSH / git URL       | `git@github.com:owner/repo.git`                           |
| HTTPS（well-known） | `https://example.com/skills`（发现 → 下载兜底）           |
| HTTPS（下载）       | `.../skill.zip`, `.../skill.tar.gz`, 原始 `SKILL.md`      |

## 安装位置

- **项目作用域** —— `./.agents/skills/<name>`（canonical），symlink 到各 Agent 的
  项目技能目录。
- **全局作用域** —— `~/.agents/skills/<name>`（canonical），以及各 Agent 的用户级
  技能目录。

## 命令行接口

库之上附带一个小型 CLI：

```bash
# 安装（从 crates.io）
cargo install agents-skills

# 从 GitHub 仓库安装技能
agents-skills add anthropics/skills

# 安装某个技能到指定 Agent
agents-skills add anthropics/skills@pdf --agent claude-code

# 以机器可读的 JSON 输出
agents-skills list --json

# 根据 lockfile 来源更新所有技能
agents-skills update
```

| 命令     | 别名                | 说明                 |
| -------- | ------------------- | -------------------- |
| `add`    | `a`, `i`, `install` | 从来源安装技能包     |
| `remove` | `rm`, `r`           | 移除已安装技能       |
| `list`   | `ls`                | 列出已安装技能       |
| `update` | `upgrade`, `check`  | 将技能更新到最新版本 |

### `add`

```
agents-skills add <source> [options]

Options:
  -g, --global        全局（用户级）安装，而非项目级
  -a, --agent <a>...  要安装到的 Agent（'*' 表示全部）
  -s, --skill <s>...  要安装的技能名（'*' 表示全部）
  -l, --list          仅列出可用技能，不安装
      --copy          复制文件而非 symlink
      --all           --skill '*' --agent '*' -y 的简写
      --full-depth    即使存在根 SKILL.md 也搜索所有子目录
  -y, --yes           跳过确认提示
```

### `remove`

```
agents-skills remove [skills...] [options]

Options:
  -g, --global        从全局作用域而非项目作用域移除
  -a, --agent <a>...  从指定 Agent 移除（'*' 表示全部）
  -s, --skill <s>...  要移除的技能（'*' 表示全部）
      --all           --skill '*' --agent '*' -y 的简写
  -y, --yes           跳过确认提示
```

### `list`

```
agents-skills list [options]

Options:
  -g, --global        列出全局技能（默认：项目）
  -a, --agent <a>...  按指定 Agent 过滤
      --json          以 JSON 输出（机器可读，无 ANSI 颜色码）
```

### `update`

```
agents-skills update [skills...] [options]

Options:
  -g, --global        仅更新全局技能
  -p, --project       仅更新项目技能
  -y, --yes           跳过作用域提示（自动检测）
```

## 项目结构

```
src/
├── lib.rs              库根：重新导出 Manager + core 原语
├── manager.rs          高层 Manager 门面（add/list/remove/update）
├── error.rs            统一错误类型与 Result 别名
├── core/               领域逻辑（纯函数、依赖可注入）
│   ├── source.rs       来源字符串解析
│   ├── agents.rs       Agent → 技能目录映射表
│   ├── discover.rs     SKILL.md 发现 + frontmatter 解析
│   ├── fetch.rs        git 克隆 / HTTP 下载 / 归档解包
│   ├── install.rs      安装编排（canonical + symlink/copy）
│   └── lock.rs         skills-lock.json 读写 + 内容哈希
├── main.rs             bin 入口（库之上的薄 CLI）
├── cli.rs              clap 命令树（命令、别名、flags）
└── commands/           CLI 渲染层（仅参数拆解 + 输出）
    ├── add.rs
    ├── remove.rs
    ├── list.rs
    └── update.rs

examples/
├── add_skill.rs        通过 Manager 门面安装技能（真实用法）
└── manage.rs           在临时目录上演示 add → list → remove 生命周期

tests/
├── common/mod.rs       集成测试共享夹具
├── lib_api.rs          库 API 集成测试
├── cli_add.rs
├── cli_remove.rs
├── cli_list.rs
└── cli_version.rs
```

## 开发

```bash
cargo build            # 构建
cargo test             # 运行全部测试（61 单元 + 26 集成）
cargo run --example manage   # 运行库使用示例
cargo clippy           # lint
cargo fmt              # 格式化
```

测试遵循测试金字塔：快速、隔离的单元测试通过 `#[cfg(test)]` 内联在 `src/` 中，
而 `tests/` 中的黑盒集成测试通过 `assert_cmd` 驱动真实 CLI。

## 设计取舍

该 crate 刻意保持极简与稳定：

- **库优先** —— 库是主要接口，CLI 只是同一套公开 API 之上的薄渲染层。
- **纯数据** —— 库从不打印、从不调用 `process::exit`；它返回结构化结果，错误通过
  `Result` 上抛。
- **无遥测** —— 不会有任何数据离开你的机器。

## License

在以下任一许可证下授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT license（[LICENSE-MIT](LICENSE-MIT)）

由你选择。

[`Manager`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html
[`Manager::add`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add
[`Manager::add_source`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add_source
[`Manager::list`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.list
[`Manager::remove`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.remove
[`Manager::update`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.update
[`ManagerBuilder`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ManagerBuilder.html
[`AddRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddRequest.html
[`AddOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddOutcome.html
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
[`install_skill_for_agent`]: https://docs.rs/agents-skills/latest/agents_skills/fn.install_skill_for_agent.html
[`list_installed_skills`]: https://docs.rs/agents-skills/latest/agents_skills/fn.list_installed_skills.html
[`sanitize_name`]: https://docs.rs/agents-skills/latest/agents_skills/fn.sanitize_name.html
[`read_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/fn.read_local_lock.html
[`write_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/fn.write_local_lock.html
[`compute_folder_hash`]: https://docs.rs/agents-skills/latest/agents_skills/fn.compute_folder_hash.html
[`get_agent`]: https://docs.rs/agents-skills/latest/agents_skills/fn.get_agent.html
[`detect_installed_agents`]: https://docs.rs/agents-skills/latest/agents_skills/fn.detect_installed_agents.html
[`Agent`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Agent.html
[`Env`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Env.html
