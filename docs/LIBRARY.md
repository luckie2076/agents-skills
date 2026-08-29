# agents-skills 库使用文档

面向**库使用者**：把技能管理能力嵌入自有 Rust 工具。CLI 用法见 [README](../README.md)，命令行参考见 [CLI.md](CLI.md)。

## 依赖引入

```toml
[dependencies]
agents-skills = "0.6"
```

## 快速开始

```rust
use agents_skills::{AddRequest, AgentRequest, Manager};

fn main() -> agents_skills::Result<()> {
    let manager = Manager::builder().build(); // 等价于 Manager::new()

    manager.agent(&AgentRequest::default())?;        // 链接所有已安装 agent
    let outcome = manager.add(&AddRequest::new("anthropics/skills"))?; // 安装技能包
    println!("installed {} skill(s)", outcome.installed.len());

    // agent_status 列出每个 agent 的链接状态；未链接且自带技能的 agent
    // 会通过 internal_skills 暴露那些技能名（便于随后 --migrate）。
    for s in manager.agent_status(false) {
        println!("{}: linked={}", s.name, s.linked);
        if !s.internal_skills.is_empty() {
            println!("  internal: {}", s.internal_skills.join(", "));
        }
    }
    Ok(())
}
```

## 高层 API：[`Manager`]

每个方法接收一个纯数据请求结构体，返回结构化结果；请求结构体均为
`Default + Clone`，可用字段覆盖构建。

| 方法                      | 请求               | 返回                                   |
| ------------------------- | ------------------ | -------------------------------------- |
| [`Manager::add`]          | [`AddRequest`]     | [`AddOutcome`]（已安装 + 链接 + 失败） |
| [`Manager::agent`]        | [`AgentRequest`]   | [`AgentOutcome`]（逐 agent 结果）      |
| [`Manager::agent_status`] | `bool`（global）   | `Vec<`[`AgentStatus`]`>`               |
| [`Manager::list`]         | [`ListRequest`]    | `Vec<`[`ListedSkill`]`>`（可序列化）   |
| [`Manager::remove`]       | [`RemoveRequest`]  | [`RemoveOutcome`]（已移除名称）        |
| [`Manager::update`]       | [`UpdateRequest`]  | [`UpdateOutcome`]（更新/失败计数）     |
| [`Manager::disable`]      | [`DisableRequest`] | [`DisableOutcome`]（已禁用名称）       |
| [`Manager::enable`]       | [`EnableRequest`]  | [`EnableOutcome`]（已启用名称）        |

### 请求结构体字段

| 结构体             | 字段（除 `global: bool` 外）                                                                                |
| ------------------ | ----------------------------------------------------------------------------------------------------------- |
| [`AddRequest`]     | `source: String`、`skills: Vec<String>`（`"*"` 或具体名，空 = 全部）、`list_only: bool`                     |
| [`AgentRequest`]   | `agents: Vec<String>`、`unlink: bool`、`migrate: bool`                                                      |
| [`ListRequest`]    | `agents: Vec<String>`（空 = 全部 agent）                                                                    |
| [`RemoveRequest`]  | `skills: Vec<String>`、`all: bool`                                                                          |
| [`UpdateRequest`]  | `skills: Vec<String>`、`scope: Scope`                                                                       |
| [`DisableRequest`] | `skills: Vec<String>`、`all: bool`                                                                          |
| [`EnableRequest`]  | `skills: Vec<String>`、`all: bool`                                                                          |

所有请求结构体带 `global: bool` 字段，对应 CLI 的 `-g/--global`：`false` 操作项目级
`./.agents/skills`，`true` 操作全局 `~/.agents/skills`。`AgentRequest` 与
`ListRequest` 的 `agents` 字段用于限定 agent（`"*"` 或具体名，空 = 自动探测）。

[`UpdateRequest`] 的 `scope` 用于覆盖自动作用域判定，取值
[`Scope::Auto`]（默认，项目有技能/锁文件则项目级，否则全局）、[`Scope::Global`]、
[`Scope::Project`]。

### 与 CLI 的对应约定

- **`add` 单 source**：CLI 的 `add <source...>` 可一次装多个源，库的
  [`AddRequest`] 只接受单个 `source: String`。要装多个源请多次调用
  `manager.add(...)`，每次返回独立的 [`AddOutcome`]。
- **`AgentRequest` 的 link 约定**：CLI 的 `agent` 命令 `--link`/`--unlink`/`--status`
  三选一互斥；库把 `--status` 拆为独立的 [`Manager::agent_status`]，因此
  [`AgentRequest`] 只需区分 link 与 unlink：`unlink: false`（默认）即 link，
  `unlink: true` 即 unlink，`migrate: true` 仅在 link 时生效（对应 CLI
  `--link --migrate`）。

### 常见操作

```rust
use agents_skills::{AddRequest, DisableRequest, EnableRequest, ListRequest, RemoveRequest};

// 安装指定技能 / 只列出不安装
let outcome = manager.add(&AddRequest {
    source: "anthropics/skills".into(),
    skills: vec!["pdf".into()],   // 省略则安装全部
    list_only: false,             // true 则只列出可用技能
    ..Default::default()
})?;

// 列出技能（支持 -g / --json / -a agent）
let skills = manager.list(&ListRequest::default())?;
let json = serde_json::to_string_pretty(&skills)?; // CLI 的 list --json

// 移除技能
manager.remove(&RemoveRequest { skills: vec!["pdf".into()], ..Default::default() })?;

// 更新技能
let outcome = manager.update(&UpdateRequest::default())?;

// 禁用 / 启用（把技能目录移出 / 移回规范目录）
manager.disable(&DisableRequest { skills: vec!["pdf".into()], ..Default::default() })?;
manager.enable(&EnableRequest { skills: vec!["pdf".into()], ..Default::default() })?;
```

## 上下文注入：[`ManagerBuilder`]

```rust
let manager = Manager::builder()
    .home("/tmp/home")
    .config("/tmp/config")
    .cwd("/tmp/project")
    .env_var("CLAUDE_CONFIG_DIR", "/tmp/claude")
    .build();
```

用于沙箱/测试，避免触碰真实环境；`Manager::new()` 等价于 `Manager::builder().build()`。
沙箱中再加 `.probe_system_dirs(false)` 可让 agent 探测完全不读取系统位置
（如 `/Applications/ZCode.app`），保证结果封闭可复现。

## 底层：`core` 原语

如需细粒度控制，底层纯函数位于 `agents_skills::core`（未在 crate 根重导出）：

- **来源**：[`parse_source`]、[`owner_repo`]
- **发现**：[`discover_skills`]、[`filter_skills`]、[`parse_skill_md`]
- **安装**：[`install_skill`]、[`list_installed_skills`]、[`sanitize_name`]、[`move_skill`]、[`list_disabled_skills`]
- **Agent**：[`get_agent`]、[`detect_installed_agents`]、[`Agent`]、[`Env`]、[`disabled_skills_dir`]
- **链接**：[`link_agent`]、[`unlink_agent`]、[`is_agent_linked`]
- **锁文件**：[`read_local_lock`]、[`write_local_lock`]、[`compute_folder_hash`]

## 示例

```bash
cargo run --example manage      # 在临时目录上演示 add → list → remove（无副作用）
cargo run --example add_skill   # 通过 Manager 安装到真实环境
```

## 行为契约

库保持**纯数据**：从不打印、从不调用 `process::exit`，结果结构化，错误通过 `Result`
上抛；渲染与退出码由调用方决定。库**无遥测**——不会有任何数据离开你的机器。

[`Manager`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html
[`Manager::add`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add
[`Manager::agent`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.agent
[`Manager::agent_status`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.agent_status
[`Manager::list`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.list
[`Manager::remove`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.remove
[`Manager::update`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.update
[`Manager::disable`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.disable
[`Manager::enable`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.enable
[`ManagerBuilder`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ManagerBuilder.html
[`AddRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddRequest.html
[`AddOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AddOutcome.html
[`AgentRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AgentRequest.html
[`AgentOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AgentOutcome.html
[`AgentStatus`]: https://docs.rs/agents-skills/latest/agents_skills/struct.AgentStatus.html
[`ListRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListRequest.html
[`ListedSkill`]: https://docs.rs/agents-skills/latest/agents_skills/struct.ListedSkill.html
[`RemoveRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveRequest.html
[`RemoveOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.RemoveOutcome.html
[`UpdateRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateRequest.html
[`UpdateOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.UpdateOutcome.html
[`DisableRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.DisableRequest.html
[`DisableOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.DisableOutcome.html
[`EnableRequest`]: https://docs.rs/agents-skills/latest/agents_skills/struct.EnableRequest.html
[`EnableOutcome`]: https://docs.rs/agents-skills/latest/agents_skills/struct.EnableOutcome.html
[`Scope::Auto`]: https://docs.rs/agents-skills/latest/agents_skills/enum.Scope.html
[`Scope::Global`]: https://docs.rs/agents-skills/latest/agents_skills/enum.Scope.html
[`Scope::Project`]: https://docs.rs/agents-skills/latest/agents_skills/enum.Scope.html
[`parse_source`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.parse_source.html
[`owner_repo`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.owner_repo.html
[`discover_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.discover_skills.html
[`filter_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.filter_skills.html
[`parse_skill_md`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.parse_skill_md.html
[`install_skill`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.install_skill.html
[`list_installed_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.list_installed_skills.html
[`sanitize_name`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.sanitize_name.html
[`move_skill`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.move_skill.html
[`list_disabled_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.list_disabled_skills.html
[`disabled_skills_dir`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.disabled_skills_dir.html
[`get_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.get_agent.html
[`detect_installed_agents`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.detect_installed_agents.html
[`Agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/struct.Agent.html
[`Env`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/struct.Env.html
[`link_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.link_agent.html
[`unlink_agent`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.unlink_agent.html
[`is_agent_linked`]: https://docs.rs/agents-skills/latest/agents_skills/core/link/fn.is_agent_linked.html
[`read_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.read_local_lock.html
[`write_local_lock`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.write_local_lock.html
[`compute_folder_hash`]: https://docs.rs/agents-skills/latest/agents_skills/core/lock/fn.compute_folder_hash.html
