# agents-skills 开发者文档

面向**本项目开发者**：项目结构、开发流程、测试与发布。功能概览与 CLI 用法见
[README](../README.md)，命令行参考见 [CLI.md](CLI.md)，库使用者见
[LIBRARY.md](LIBRARY.md)。

## 架构分层

项目刻意分层，库与 CLI 职责严格分离：

- **库**（`src/lib.rs` + `src/manager.rs` + `src/core/`）—— 纯数据：从不打印、
  从不调用 `process::exit`，错误通过 `Result` 上抛。
- **CLI**（`src/main.rs` + `src/cli.rs` + `src/commands/`）—— 库之上的薄渲染层：
  只负责 clap 参数拆解、把请求结构体交给 `Manager`、再把结果渲染成人类/机器可读
  输出并决定退出码。

每个 CLI 命令对应一个 `Manager` 方法，CLI 的 flag 对应请求结构体字段。新增能力时
应先在 `core`/`Manager` 层实现，再在 CLI 层渲染；不要让 CLI 层直接碰领域逻辑。

## 项目结构

```
src/
├── lib.rs              库根：Manager 门面 + 请求/结果类型 + core 模块
├── manager.rs          高层 Manager 门面（add/list/remove/update/disable/enable/link）
├── error.rs            统一错误类型与 Result 别名
├── core/               领域逻辑（纯函数、依赖可注入）
│   ├── mod.rs          模块组织与重导出
│   ├── source.rs       来源字符串解析
│   ├── agents.rs       Agent → 技能目录映射表
│   ├── discover.rs     SKILL.md 发现 + frontmatter 解析
│   ├── fetch.rs        git 克隆 / HTTP 下载 / 归档解包
│   ├── github.rs       GitHub API 单技能快速拉取
│   ├── install.rs      安装技能到规范目录 + 已装清单
│   ├── link.rs         目录级 agent 链接（link/unlink/migrate）
│   ├── lock.rs         skills-lock.json 读写 + 内容哈希
│   └── test_utils.rs   单元测试共享夹具
├── main.rs             bin 入口（库之上的薄 CLI）
├── cli.rs              clap 命令树（命令、flags，不设别名）
└── commands/           CLI 渲染层（仅参数拆解 + 输出）
    ├── mod.rs
    ├── add.rs
    ├── remove.rs
    ├── list.rs
    ├── update.rs
    ├── disable.rs
    ├── enable.rs
    └── agent.rs

examples/
├── add_skill.rs        通过 Manager 门面安装技能（真实用法）
└── manage.rs           在临时目录上演示 add → list → remove 生命周期

tests/
├── common/mod.rs       集成测试共享夹具
├── lib_api.rs          库 API 集成测试
├── cli_add.rs
├── cli_remove.rs
├── cli_list.rs
├── cli_agent.rs
├── cli_enable_disable.rs
└── cli_version.rs
```

## 开发

```bash
cargo build            # 构建
cargo test             # 运行全部测试
cargo clippy           # lint
cargo fmt              # 格式化
```

## 测试

测试遵循测试金字塔：

- **单元测试** —— 通过 `#[cfg(test)]` 内联在 `src/` 各模块中，快速、隔离；
  领域层夹具见 `src/core/test_utils.rs`。
- **集成测试** —— `tests/` 中的黑盒测试通过 `assert_cmd` 驱动真实 CLI；
  `lib_api.rs` 覆盖库 API。

示例程序作为补充：

```bash
cargo run --example manage      # 在临时目录上演示 add → list → remove（无副作用）
cargo run --example add_skill   # 通过 Manager 安装到你的真实环境
```

## 设计取舍

- **极简稳定** —— 刻意保持小而稳定，注重跨平台（macOS、Linux、Windows）。
- **纯数据** —— 库从不打印、从不调用 `process::exit`；结果结构化，错误通过
  `Result` 上抛。
- **无遥测** —— 不会有任何数据离开用户的机器。

## 发布

新版本一律通过 GitHub Actions 发布到 crates.io（见 `.github/workflows/`），
不要在本地手动 `cargo publish`。发布前确认 `Cargo.toml` 的 `version` 已按
语义化版本递增，并更新 [README](../README.md) / [CLI.md](CLI.md) /
[LIBRARY.md](LIBRARY.md) 中涉及的版本号与接口变更。
