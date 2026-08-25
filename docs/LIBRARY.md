# agents-skills 库使用文档

面向**库使用者**：把技能管理能力嵌入自有 Rust 工具。CLI 用法见 [README](../README.md)，命令行参考见 [CLI.md](CLI.md)。

## 依赖引入

```toml
[dependencies]
agents-skills = "0.5"
```

## 快速开始

```rust
use agents_skills::{AgentRequest, Manager};

fn main() -> agents_skills::Result<()> {
    let manager = Manager::builder().build(); // 等价于 Manager::new()

    manager.agent(&AgentRequest::default())?;        // 链接所有已安装 agent
    let outcome = manager.add_source("anthropics/skills")?; // 安装技能包
    println!("installed {} skill(s)", outcome.installed.len());

    for s in manager.agent_status(false) {
        println!("{}: linked={}", s.name, s.linked);
        // 未链接的 agent 若自身目录已含技能，会通过 internal_skills 列出（便于随后 --migrate）。
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

| 方法                    | 请求                | 返回                                      |
| ----------------------- | ------------------- | ----------------------------------------- |
| [`Manager::add`]        | [`AddRequest`]      | [`AddOutcome`]（已安装 + 链接 + 失败）    |
| [`Manager::add_source`] | `impl Into<String>` | [`AddOutcome`]                            |
| [`Manager::agent`]      | [`AgentRequest`]    | [`AgentOutcome`]（逐 agent 结果）         |
| [`Manager::list`]       | [`ListRequest`]     | `Vec<`[`ListedSkill`]`>`（可序列化）      |
| [`Manager::remove`]     | [`RemoveRequest`]   | [`RemoveOutcome`]（已移除名称）           |
| [`Manager::update`]     | [`UpdateRequest`]   | [`UpdateOutcome`]（更新/失败计数）        |
| [`Manager::disable`]    | [`DisableRequest`]  | [`DisableOutcome`]（已禁用名称）          |
| [`Manager::enable`]     | [`EnableRequest`]   | [`EnableOutcome`]（已启用名称）           |

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

所有请求结构体带 `global: bool` 字段，对应 CLI 的 `-g/--global`：`false` 操作项目级
`./.agents/skills`，`true` 操作全局 `~/.agents/skills`。`agent` 与 `list` 的 `agents`
字段用于限定 agent。

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

## 底层：`core` 原语

如需细粒度控制，底层纯函数位于 `agents_skills::core`（未在 crate 根重导出）：

- **来源** —— [`core::source::parse_source`]、[`core::source::owner_repo`]
- **发现** —— [`core::discover::discover_skills`]、[`core::discover::filter_skills`]、[`core::discover::parse_skill_md`]
- **安装** —— [`core::install::install_skill`]、[`core::install::list_installed_skills`]、[`core::install::sanitize_name`]
- **禁用/启用** —— [`core::install::move_skill`]、[`core::install::list_disabled_skills`]、[`core::agents::disabled_skills_dir`]
- **链接** —— [`core::link::link_agent`]、[`core::link::unlink_agent`]、[`core::link::is_agent_linked`]
- **锁文件** —— [`core::lock::read_local_lock`]、[`core::lock::write_local_lock`]、[`core::lock::compute_folder_hash`]
- **Agent** —— [`core::agents::get_agent`]、[`core::agents::detect_installed_agents`]、[`core::agents::Agent`]、[`core::agents::Env`]

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
[`Manager::add_source`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.add_source
[`Manager::agent`]: https://docs.rs/agents-skills/latest/agents_skills/struct.Manager.html#method.agent
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
[`core::source::parse_source`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.parse_source.html
[`core::source::owner_repo`]: https://docs.rs/agents-skills/latest/agents_skills/core/source/fn.owner_repo.html
[`core::discover::discover_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.discover_skills.html
[`core::discover::filter_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.filter_skills.html
[`core::discover::parse_skill_md`]: https://docs.rs/agents-skills/latest/agents_skills/core/discover/fn.parse_skill_md.html
[`core::install::install_skill`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.install_skill.html
[`core::install::list_installed_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.list_installed_skills.html
[`core::install::sanitize_name`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.sanitize_name.html
[`core::install::move_skill`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.move_skill.html
[`core::install::list_disabled_skills`]: https://docs.rs/agents-skills/latest/agents_skills/core/install/fn.list_disabled_skills.html
[`core::agents::disabled_skills_dir`]: https://docs.rs/agents-skills/latest/agents_skills/core/agents/fn.disabled_skills_dir.html
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
