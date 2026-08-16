# agent-skill

[![crates.io](https://img.shields.io/crates/v/agent-skill.svg)](https://crates.io/crates/agent-skill)
[![CI](https://github.com/luckie2076/agent-skill/actions/workflows/ci.yml/badge.svg)](https://github.com/luckie2076/agent-skill/actions)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

一个极简、稳定、易于理解的 AI Agent 技能（skills）安装与管理工具，使用 Rust 编写。

安装和管理 AI Agent 的 **技能**（skills）—— 可复用、带版本的 `SKILL.md` 包，支持
[Claude Code](https://claude.com/code)、Codex、Cursor 等 70+ 编程 Agent。

界面刻意保持小巧：4 个主要命令、可复现更新的 lockfile，以及主流编程 Agent 共享的
安装位置。实现是基于成熟 crate 的、干净且地道的 Rust 程序。

> 另见：[English README](README.md)

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

## 安装

```bash
# 从 crates.io（推荐）
cargo install agent-skill

# 从源码
git clone https://github.com/luckie2076/agent-skill
cd agent-skill
cargo install --path .

# 验证
agent-skill --version   # 1.5.22
```

## 快速上手

```bash
# 从 GitHub 仓库安装技能（简写）
agent-skill add anthropics/skills

# 安装仓库中的某个技能
agent-skill add anthropics/skills@pdf

# 从本地路径安装到指定 Agent
agent-skill add ./my-skill --agent claude-code

# 列出已安装技能（项目作用域）
agent-skill list

# 以机器可读的 JSON 输出
agent-skill list --json

# 根据 lockfile 来源更新所有技能
agent-skill update
```

## 作为库使用

`agent-skill` 同时以 Rust 库的形式提供。在 `Cargo.toml` 中添加：

```toml
[dependencies]
agent-skill = "1"
```

使用高层 `Manager` 门面：

```rust
use agent_skill::{AddRequest, ListRequest, Manager, Result};

fn main() -> Result<()> {
    let manager = Manager::new();

    // 将 GitHub 仓库中的所有技能安装到所有检测到的 Agent。
    manager.add(&AddRequest {
        source: "anthropics/skills".to_string(),
        agents: vec!["*".to_string()],
        ..Default::default()
    })?;

    // 列出已安装技能（可序列化；与 `list --json` 同构）。
    let skills = manager.list(&ListRequest::default())?;
    println!("{skills:?}");
    Ok(())
}
```

如需更细粒度控制，底层 `core` 原语已在 crate 根重新导出
（`parse_source`、`discover_skills`、`install_skill_for_agent`、
`read_local_lock` 等）。

## 命令

| 命令 | 别名 | 说明 |
| ------- | ------- | ----------- |
| `add` | `a`, `i`, `install` | 从来源安装技能包 |
| `remove` | `rm`, `r` | 移除已安装技能 |
| `list` | `ls` | 列出已安装技能 |
| `update` | `upgrade`, `check` | 将技能更新到最新版本 |

### `add`

```
agent-skill add <source> [options]

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
agent-skill remove [skills...] [options]

Options:
  -g, --global        从全局作用域而非项目作用域移除
  -a, --agent <a>...  从指定 Agent 移除（'*' 表示全部）
  -s, --skill <s>...  要移除的技能（'*' 表示全部）
      --all           --skill '*' --agent '*' -y 的简写
  -y, --yes           跳过确认提示
```

### `list`

```
agent-skill list [options]

Options:
  -g, --global        列出全局技能（默认：项目）
  -a, --agent <a>...  按指定 Agent 过滤
      --json          以 JSON 输出（机器可读，无 ANSI 颜色码）
```

### `update`

```
agent-skill update [skills...] [options]

Options:
  -g, --global        仅更新全局技能
  -p, --project       仅更新项目技能
  -y, --yes           跳过作用域提示（自动检测）
```

## 来源格式

`<source>` 参数支持：

| 格式 | 示例 |
| ------ | ------- |
| 本地路径 | `./my-skill`, `/abs/path/skill` |
| GitHub 简写 | `owner/repo`, `owner/repo@skill`, `owner/repo/subpath` |
| GitHub URL | `https://github.com/owner/repo`, `.../tree/main/skills` |
| GitLab URL | `https://gitlab.com/group/repo`, `.../-/tree/main/skills` |
| SSH / git URL | `git@github.com:owner/repo.git` |
| HTTPS（well-known） | `https://example.com/skills`（发现 → 下载兜底） |
| HTTPS（下载） | `.../skill.zip`, `.../skill.tar.gz`, 原始 `SKILL.md` |

## 安装位置

- **项目作用域** —— `./.agents/skills/<name>`（canonical），symlink 到各 Agent 的
  项目技能目录。
- **全局作用域** —— `~/.agents/skills/<name>`（canonical），以及各 Agent 的用户级
  技能目录。

## 项目结构

```
src/
├── main.rs             bin 入口：banner、version、子命令分发
├── lib.rs              库根：重新导出 Manager + core 原语
├── manager.rs          高层 Manager 门面（add/list/remove/update）
├── cli.rs              clap 命令树（命令、别名、flags）
├── error.rs            统一错误类型与 Result 别名
├── commands/           CLI 渲染层（薄编排）
│   ├── add.rs
│   ├── remove.rs
│   ├── list.rs
│   └── update.rs
└── core/               领域逻辑（与 CLI 无关、依赖可注入）
    ├── source.rs       来源字符串解析
    ├── agents.rs       Agent → 技能目录映射表
    ├── discover.rs     SKILL.md 发现 + frontmatter 解析
    ├── fetch.rs        git 克隆 / HTTP 下载 / 归档解包
    ├── install.rs      安装编排（canonical + symlink/copy）
    └── lock.rs         skills-lock.json 读写 + 内容哈希

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

工具刻意保持极简与稳定：

- **四个命令** —— `add`、`remove`、`list`、`update` 覆盖完整的安装/管理流程；其他
  常见命令（搭建 `SKILL.md` 脚手架、不安装直接生成 prompt、搜索技能注册表）不在
  范围内。
- **默认非交互式** —— 每个命令都可用脚本驱动；移除了确认提示（`-y` flag 作为保持
  CLI 兼容的 no-op 保留）。
- **无遥测** —— 不会有任何数据离开你的机器。

## License

在以下任一许可证下授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT license（[LICENSE-MIT](LICENSE-MIT)）

由你选择。
