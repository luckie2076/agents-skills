# agents-skills 开发者文档

面向开发者的文档：项目结构、库接口使用说明与开发流程。功能概览与 CLI 用法见
[README](../README.md)，命令行参考见 [CLI.md](CLI.md)。

## 依赖引入

```toml
[dependencies]
agents-skills = "0.3"
```

CLI 只是同一套公开 Rust API 之上的薄渲染层，两者共享全部能力——每个 CLI 命令
对应一个 [`Manager`] 方法，CLI 的 flag 对应请求结构体字段。

## 快速开始

```rust
use agents_skills::{LinkRequest, Manager};

fn main() -> agents_skills::Result<()> {
    // 基于真实环境解析的 Manager（等价于 Manager::new()）
    let manager = Manager::builder().build();

    // 1. link: 为所有已安装的 agent 创建指向规范目录的符号链接
    manager.link(&LinkRequest::default())?;

    // 2. 安装技能包（装进规范目录后，所有 agent 立即可见）
    let outcome = manager.add_source("anthropics/skills")?;
    println!("installed {} skill(s)", outcome.installed.len());

    // 3. 查看各 agent 的链接状态
    for s in manager.link_status(false) {
        println!("{}: linked={} canonical={}", s.name, s.linked, s.canonical);
    }
    Ok(())
}
```

## 库接口使用说明

核心是 `link`：技能只在规范目录保存一份，`link` 为每个已安装的 agent 在其技能
目录创建指向规范目录的符号链接。以下按功能给出库用法，与 CLI 命令一一对应。

### 让 Agent 可见（link）

对应 CLI：`agents-skills link [agents...] [--status] [--unlink] [--migrate] [-g]`

```rust
// 链接所有已安装 agent（项目级）
manager.link(&LinkRequest::default())?;

// 只链接指定 agent
manager.link(&LinkRequest {
    agents: vec!["claude-code".into()],
    ..Default::default()
})?;

// 链接并迁移存量技能（--migrate）
manager.link(&LinkRequest {
    agents: vec!["claude-code".into()],
    migrate: true,
    ..Default::default()
})?;

// 解除链接（--unlink）：移除符号链接并重建空目录；规范目录与技能不受影响
manager.link(&LinkRequest {
    agents: vec!["claude-code".into()],
    unlink: true,
    ..Default::default()
})?;

// 查看各 agent 的链接状态（--status，只读）
for s in manager.link_status(false) {
    if s.canonical {
        println!("{} -> canonical dir", s.display);
    } else if s.linked {
        println!("{} -> canonical dir (linked)", s.display);
    } else {
        println!("{}: not linked", s.display);
    }
}
```

`link_status` 返回 [`LinkStatus`]（`name` / `display` / `linked` / `canonical`），
CLI 的 `link --status` 即其渲染。两种"可见"状态含义不同：

- **`canonical: true`** —— 该 agent（universal agent）**原生**就读取规范目录，
  `link` 对其是无操作（`AlreadyLinked`）；CLI 显示为 `(canonical dir)`。
  判断优先于 `linked`（universal agent 的 `linked` 恒为 true）。
- **`canonical: false` + `linked: true`** —— 该 agent 的技能目录是指向规范目录的
  **符号链接**（通过 `link` 建立）；CLI 显示为 `(linked)`。

只报告"已安装或已链接"的 agent：未安装且未链接的非 universal agent、以及未安装的
universal agent 都不会出现在结果里。

顺序保证：`link_status` 返回的顺序与 CLI 的 `link --status` 渲染顺序完全一致——
`canonical: true`（universal）的 agent 恒在前，其余 agent 在后；两段各自保持静态
agent 表顺序。CLI 不做二次排序，库调用方也无需自行排序。

每个 agent 的结果在 `LinkManagerOutcome.results` 中，`outcome` 字段为
[`LinkOutcome`]：`Linked` / `AlreadyLinked` / `Migrated` / `Refused` / `Skipped` /
`Unlinked` / `NotLinked` / `Failed`。其中 `Refused` 的 `skills` 字段会列出
agent 目录内已存在的技能名，CLI 会据此提示用户；`Migrated` 的 `moved`
字段列出被移入规范目录的技能名，`skipped` 字段列出因规范目录已有同名
而跳过的技能名（规范目录的副本优先）。迁移是**非全有或全无**的：同名冲突
安全跳过（canonical 有副本），但游离文件（普通文件、指向非目录的链接）会
中止整个迁移——它们没有副本，删掉即数据丢失，须由用户先处理。

### 安装技能（add）

对应 CLI：`agents-skills add <source> [options]`

```rust
// 安装全部技能到规范目录（项目级）
let outcome = manager.add_source("anthropics/skills")?;
for s in &outcome.installed {
    println!("installed {}", s.name);
}

// 仅安装仓库中的指定技能（-s/--skill）
let outcome = manager.add(&AddRequest {
    source: "anthropics/skills".into(),
    skills: vec!["pdf".into()],
    ..Default::default()
})?;

// 只列出可用技能，不安装（-l/--list）
let outcome = manager.add(&AddRequest {
    source: "anthropics/skills".into(),
    list_only: true,
    ..Default::default()
})?;
// outcome.skills 为发现到的全部技能
```

`add` 不做任何 agent 链接——让 agent 可见是 `link` 的职责。

### 列出技能（list）

对应 CLI：`agents-skills list [-g] [--json] [-a agent]`

```rust
let skills = manager.list(&ListRequest::default())?;
for s in &skills {
    println!("{} ({}): {}", s.name, s.scope, s.path.display());
}

// ListedSkill 是 serde 可序列化的，JSON 结构即 CLI 的 `list --json` 输出
let json = serde_json::to_string_pretty(&skills)?;

// 只列出指定 agent 可见的技能（CLI 的 -a/--agent）
let filtered = manager.list(&ListRequest {
    agents: vec!["codex".into()],
    ..Default::default()
})?;
```

每个 [`ListedSkill`] 的 `agents` 字段列出**可见该技能的 agent 显示名**：
universal agent 恒在；非 universal agent 仅当其技能目录中存在该技能（即已链接）
时出现。`agents` 过滤（`-a`）只影响该字段的填充——技能本身始终列出，过滤后
`agents` 可能为空数组，与 CLI 的 `-a` 行为一致。

CLI 的 `list` 默认输出（技能名 / 路径 / `Source:`）**不显示** `agents` 列——
agent 链接状态是 `link --status` 的职责，避免重复噪音；`--json` 保留完整字段
供机器读取。

### 移除技能（remove）

对应 CLI：`agents-skills remove <name> [-g] [--all]`

```rust
// 移除指定技能
let outcome = manager.remove(&RemoveRequest {
    skills: vec!["pdf".into()],
    ..Default::default()
})?;
println!("removed: {:?}", outcome.removed);

// 移除全部技能
manager.remove(&RemoveRequest { all: true, ..Default::default() })?;
```

### 更新技能（update）

对应 CLI：`agents-skills update [-g]`

```rust
let outcome = manager.update(&UpdateRequest::default())?;
println!("updated={} failed={}", outcome.updated, outcome.failed);
```

### 项目级与全局作用域

所有请求结构体都带 `global: bool` 字段，与 CLI 的 `-g/--global` 对应：
`false` 操作项目级 `./.agents/skills`，`true` 操作全局 `~/.agents/skills`。

### 错误处理

库从不打印、从不调用 `process::exit`；错误通过 `agents_skills::error::Result`
上抛，如何渲染、何时退出由调用方决定。

## 高层：[`Manager`]

一站式操作。每个方法接收一个纯数据请求结构体，返回结构化结果。

| 方法                    | 请求                | 返回                                      |
| ----------------------- | ------------------- | ----------------------------------------- |
| [`Manager::add`]        | [`AddRequest`]      | [`AddOutcome`]（已安装 + 链接 + 失败）    |
| [`Manager::add_source`] | `impl Into<String>` | [`AddOutcome`]（已安装 + 链接 + 失败）    |
| [`Manager::link`]       | [`LinkRequest`]     | [`LinkManagerOutcome`]（逐 agent 结果；`unlink: true` 断开） |
| [`Manager::list`]       | [`ListRequest`]     | `Vec<`[`ListedSkill`]`>`（可序列化）      |
| [`Manager::remove`]     | [`RemoveRequest`]   | [`RemoveOutcome`]（已移除名称）           |
| [`Manager::update`]     | [`UpdateRequest`]   | [`UpdateOutcome`]（更新/失败计数）        |

请求结构体均为 `Default + Clone`，可通过字段覆盖构建；结果是纯数据。

## 上下文注入：[`ManagerBuilder`]

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

## 底层：`core` 原语

如需更细粒度控制，底层纯函数位于 `agents_skills::core`（未在 crate 根重导出）：

- **来源** —— [`core::source::parse_source`]、[`core::source::owner_repo`]
- **发现** —— [`core::discover::discover_skills`]、[`core::discover::filter_skills`]、
  [`core::discover::parse_skill_md`]
- **安装** —— [`core::install::install_skill`]、[`core::install::list_installed_skills`]、
  [`core::install::sanitize_name`]
- **链接** —— [`core::link::link_agent`]、[`core::link::unlink_agent`]、
  [`core::link::is_agent_linked`]
- **锁文件** —— [`core::lock::read_local_lock`]、[`core::lock::write_local_lock`]、
  [`core::lock::compute_folder_hash`]
- **Agent** —— [`core::agents::get_agent`]、[`core::agents::detect_installed_agents`]、
  [`core::agents::Agent`]、[`core::agents::Env`]

## 项目结构

```
src/
├── lib.rs              库根：Manager 门面 + 请求/结果类型 + core 模块
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
    └── link.rs

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

## 示例

```bash
cargo run --example manage      # 在临时目录上演示 add → list → remove（无副作用）
cargo run --example add_skill   # 通过 Manager 安装到你的真实环境
```

## 开发

```bash
cargo build            # 构建
cargo test             # 运行全部测试
cargo clippy           # lint
cargo fmt              # 格式化
```

测试遵循测试金字塔：快速、隔离的单元测试通过 `#[cfg(test)]` 内联在 `src/` 中，
而 `tests/` 中的黑盒集成测试通过 `assert_cmd` 驱动真实 CLI。

## 设计取舍

- **极简稳定** —— 刻意保持小而稳定，注重跨平台（macOS、Linux、Windows）。
- **纯数据** —— 库从不打印、从不调用 `process::exit`；结果结构化，错误通过
  `Result` 上抛。
- **无遥测** —— 不会有任何数据离开你的机器。

[`Manager`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html
[`Manager::add`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add
[`Manager::add_source`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add_source
[`Manager::link`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.link
[`Manager::list`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.list
[`Manager::remove`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.remove
[`Manager::update`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.update
[`ManagerBuilder`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ManagerBuilder.html
[`AddRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddRequest.html
[`AddOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddOutcome.html
[`LinkRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.LinkRequest.html
[`LinkManagerOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.LinkManagerOutcome.html
[`LinkOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/enum.LinkOutcome.html
[`ListRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListRequest.html
[`ListedSkill`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListedSkill.html
[`LinkStatus`]: https://docs.rs/agents-skills/latest/agents_skills/struct.LinkStatus.html
[`RemoveRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveRequest.html
[`RemoveOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveOutcome.html
[`UpdateRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateRequest.html
[`UpdateOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateOutcome.html
[`core::source::parse_source`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.parse_source.html
[`core::source::owner_repo`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.owner_repo.html
[`core::discover::discover_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.discover_skills.html
[`core::discover::filter_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.filter_skills.html
[`core::discover::parse_skill_md`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.parse_skill_md.html
[`core::install::install_skill`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.install_skill.html
[`core::install::list_installed_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.list_installed_skills.html
[`core::install::sanitize_name`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.sanitize_name.html
[`core::link::link_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.link_agent.html
[`core::link::unlink_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.unlink_agent.html
[`core::link::is_agent_linked`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.is_agent_linked.html
[`core::lock::read_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.read_local_lock.html
[`core::lock::write_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.write_local_lock.html
[`core::lock::compute_folder_hash`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.compute_folder_hash.html
[`core::agents::get_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.get_agent.html
[`core::agents::detect_installed_agents`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.detect_installed_agents.html
[`core::agents::Agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/struct.Agent.html
[`core::agents::Env`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/struct.Env.html
