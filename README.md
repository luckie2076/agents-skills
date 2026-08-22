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
见 [docs/DEVELOPER.md](docs/DEVELOPER.md)。

## 快速开始

```bash
# 1. 核心一步：link 为所有已安装的 agent 创建指向规范目录的符号链接
agents-skills link

# 2. 安装技能包（装进规范目录后，所有 agent 立即可见）
agents-skills add anthropics/skills

# 查看已安装技能
agents-skills list

# 查看各 agent 的链接状态
agents-skills link --status
```

核心是 `link`：技能只在规范目录保存一份（项目级 `.agents/skills/` 或全局级
`~/.agents/skills/`），`link` 为每个已安装的 agent（Claude Code、Codex、Cursor…）
在其技能目录创建指向规范目录的符号链接；链接建立后，`add` 安装的技能所有
agent 立即可见，无需任何同步。

## 功能说明

### 核心：让 Agent 可见（link）

本质：技能只在规范目录保存一份真实副本；`link` 为每个 agent 在其技能目录创建
指向规范目录的符号链接（如 `.claude/skills` → `../.agents/skills`），使 70+ 编程
Agent 共享同一份技能，安装一次、处处可见。这是本项目最核心的能力：`add`/`remove`/
`update` 只操作规范目录，其余 agent 通过链接自动同步。`link` 一个命令负责
链接、查询状态与解除链接三种操作：

```bash
agents-skills link                           # 自动链接本机已安装的所有 agent
agents-skills link claude-code               # 只链接指定 agent
agents-skills link claude-code --migrate     # 链接并迁移存量技能
agents-skills link --status                  # 查看各 agent 的链接状态
agents-skills link --unlink claude-code      # 解除链接
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
```

### 列出技能（list）

本质：扫描规范目录，列出已安装技能（技能名、路径、来源）。各 agent 的
链接状态由 `link --status` 查询。

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

| 格式                | 示例                                                             |
| ------------------- | ---------------------------------------------------------------- |
| 本地路径            | `./my-skill`, `/abs/path/skill`                                  |
| GitHub 简写         | `owner/repo`, `owner/repo@skill`, `owner/repo/subpath`           |
| GitHub / GitLab URL | `https://github.com/owner/repo`, `https://gitlab.com/group/repo` |
| SSH / git URL       | `git@github.com:owner/repo.git`                                  |
| HTTPS（well-known） | `https://example.com/skills`（发现 → 下载兜底）                  |
| HTTPS（下载）       | `.../skill.zip`, `.../skill.tar.gz`, 原始 `SKILL.md`             |

仓库内按优先级容器目录发现技能（`skills/`、`.curated/`、`.experimental/`、
`.system/`），浅层遮蔽深层。

### 安装位置

- **规范目录（唯一真实副本）** —— 项目级 `./.agents/skills/<name>`，全局级
  `~/.agents/skills/<name>`。
- **Agent 集成** —— 不原生读取规范目录的 agent 获得一个目录级符号链接：
  `.claude/skills` → `../.agents/skills`（项目）或 `~/.claude/skills` →
  `~/.agents/skills`（全局）。已链接的 agent 共享规范目录，无需任何同步步骤。

### 命令速查表

| 命令     | 别名                | 说明                                |
| -------- | ------------------- | ----------------------------------- |
| `add`    | `a`, `i`, `install` | 从来源安装技能包                    |
| `remove` | `rm`, `r`           | 移除已安装技能                      |
| `list`   | `ls`                | 列出已安装技能                      |
| `update` | `upgrade`, `check`  | 将技能更新到最新版本                |
| `link`   | `ln`                | 链接/解除链接/查看 agent 链接状态   |

> 完整的命令行参数（每个命令的选项与更多示例）见 [docs/CLI.md](docs/CLI.md)。
> 开发者（库接口使用说明、项目结构、开发流程）见 [docs/DEVELOPER.md](docs/DEVELOPER.md)。

### License

在以下任一许可证下授权：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT license（[LICENSE-MIT](LICENSE-MIT)）

由你选择。
