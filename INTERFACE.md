# agents-skills 对外接口设计（黑盒视角）

> 本文档从**使用者角度**描述 `agents-skills` 的功能设计与对外接口：有哪些入口、各自的输入/输出、以及对文件系统和网络的副作用。不涉及任何内部实现细节。
>
> 项目形态：**库优先**的 Rust crate（`agents-skills`）+ 一个薄 CLI 二进制（`agents-skills`）。CLI 只是库 API 的渲染层，二者能力一一对应。

---

## 1. 产品定位

为 AI 编码 agent（Claude Code、Codex、Cursor 等 70+ 种）**安装、列出、移除、更新** `SKILL.md` 技能包。使用者有两种身份：

| 身份        | 使用方式                                                              |
| ----------- | --------------------------------------------------------------------- |
| 终端用户    | 通过 `agents-skills` CLI 命令                                         |
| Rust 开发者 | 将 `agents-skills` 作为库引入（插件管理器、构建脚本、agent 启动器等） |

### 核心概念

| 概念         | 含义                                                                                                                       |
| ------------ | -------------------------------------------------------------------------------------------------------------------------- |
| **Skill**    | 一个包含 `SKILL.md` 的目录。`SKILL.md` 需有 YAML frontmatter 且包含 `name` 与 `description`，否则视为无效                  |
| **Agent**    | 技能的宿主。每个 agent 有唯一的 kebab-case 标识符（如 `claude-code`、`codex`、`amp`）和自己的技能目录，共 70+ 个，静态内置 |
| **Source**   | 技能来源：本地路径 / GitHub / GitLab / 任意 git / HTTPS 下载地址                                                           |
| **Scope**    | 安装范围：`project`（当前项目）或 `global`（用户级）                                                                       |
| **Lockfile** | `skills-lock.json`，记录每个已装技能的来源与内容哈希，支撑可复现的 `update`                                                |

---

## 2. 接口总览

| 层      | 入口                                                                        | 适用场景              |
| ------- | --------------------------------------------------------------------------- | --------------------- |
| CLI     | `agents-skills <command> [flags]`（4 个命令：add / remove / list / update） | 终端操作、脚本调用    |
| 库·高层 | `Manager` 门面（add / add_source / list / remove / update）                 | Rust 程序内嵌技能管理 |
| 库·低层 | `agents_skills::core::*` 纯函数（解析/发现/安装/锁）                        | 需要精细控制的场景    |

**库的契约（重要）**：库层是纯数据接口——从不向 stdout/stderr 打印、从不调用 `process::exit`，一切结果通过返回值（结构化 outcome）和 `Result` 传递；渲染与退出码是 CLI 层独有的职责。

---

## 3. 输入约定

### 3.1 Source 格式

`source` 字符串（CLI `<source>` 参数或 `AddRequest::source`）接受：

| 格式                | 示例                                                           |
| ------------------- | -------------------------------------------------------------- |
| 本地路径            | `./my-skill`、`/abs/path/skill`                                |
| GitHub 简写         | `owner/repo`、`owner/repo@skill`、`owner/repo/subpath`         |
| GitHub URL          | `https://github.com/owner/repo`、`.../tree/main/skills`        |
| GitLab URL          | `https://gitlab.com/group/repo`、`.../-/tree/main/skills`      |
| SSH / git URL       | `git@github.com:owner/repo.git`                                |
| HTTPS（well-known） | `https://example.com/skills`（先探测约定路径，失败则直接下载） |
| HTTPS（直接下载）   | `.../skill.zip`、`.../skill.tar.gz`、裸 `SKILL.md`             |

### 3.2 通用选择语义

- **技能选择**：列表为空 → 全部；含 `"*"` → 全部；否则按名过滤。
- **agent 选择**：列表为空 → 自动探测本机已安装的 agent（并始终包含通用 agent）；含 `"*"` → 全部已知 agent；否则按名匹配，出现未知名称时报错并列出全部合法值。
- **技能发现深度**：默认只在约定容器目录（仓库根、`skills/`、`.curated/`、`.experimental/`、`.system/`、各 agent 项目技能目录）内递归至多 3 层，浅层遮蔽深层；`full_depth`/`--full-depth` 则搜索整棵目录树（跳过 `node_modules`、`.git`、`dist`、`build`、`__pycache__`）。
- **内部技能**：默认隐藏，仅当显式按名选择（或环境变量 `INSTALL_INTERNAL_SKILLS=1`）时可见。

---

## 4. CLI 接口

### 4.0 全局行为

| 调用                             | 行为                            | 退出码 |
| -------------------------------- | ------------------------------- | ------ |
| `agents-skills`（无参数）        | 打印 ASCII banner 与命令速览    | 0      |
| `agents-skills -v` / `--version` | 打印裸语义化版本号              | 0      |
| `agents-skills --help` 等        | clap 帮助信息（stdout）         | 0      |
| 未知子命令                       | `Unknown command: <cmd>` + 提示 | 1      |
| 命令执行出错                     | `Error: <msg>` 到 stderr        | 1      |

### 4.1 `add`（别名 `a` / `i` / `install`）— 安装技能

```
agents-skills add <source...> [options]
```

| 输入（flag）               | 类型/默认          | 作用                                       |
| -------------------------- | ------------------ | ------------------------------------------ |
| `<source>`（必填，可多个） | 路径               | **仅第一个生效**（多余的被忽略）           |
| `-g, --global`             | bool，默认 project | 安装到用户级                               |
| `-a, --agent <a>...`       | 名单               | 目标 agent（见 §3.2）                      |
| `-s, --skill <s>...`       | 名单               | 目标技能（见 §3.2）                        |
| `-l, --list`               | bool               | 只列出源中可用技能，不安装                 |
| `--copy`                   | bool               | 复制文件替代符号链接                       |
| `--all`                    | bool               | 等价于 `--skill '*' --agent '*' -y`        |
| `--full-depth`             | bool               | 深度搜索子目录                             |
| `-y, --yes`                | bool               | 接受但当前无实际行为（当前版本无交互确认） |

**输出（stdout）**：源信息 → 发现技能数 → 选中/目标 agent → 每项安装结果（`✓ 路径` 或 `✗ 技能 → agent: 原因`）→ 汇总。错误消息用红色打印。

**退出码**：0（成功，含部分安装失败——仅打印不置错码）；1（源无效/无有效技能/选中技能不存在/未知 agent）。

**副作用**：网络拉取（git clone 或 HTTP 下载，临时目录用完即删）；写入规范目录与 agent 目录（§7）；成功项写入 lockfile。

### 4.2 `remove`（别名 `rm` / `r`）— 移除技能

```
agents-skills remove [skills...] [options]
```

| 输入（flag）                  | 作用                                   |
| ----------------------------- | -------------------------------------- |
| `[skills...]` + `-s, --skill` | 待移除技能名（两处合并）               |
| `-g, --global`                | 操作全局范围                           |
| `-a, --agent <a>...`          | 限定 agent；默认全部（含幽灵链接清理） |
| `--all`                       | 移除全部已安装技能 + 全部 lockfile 键  |
| `-y, --yes`                   | 同 add，当前无实际行为                 |

**输出**：无参数时列出已安装技能与用法提示；成功时 `Successfully removed N skill(s)`；无匹配时红色提示。

**退出码**：0（成功、无匹配、空列表）；1（未知 agent）。

**副作用**：删除各 agent 目录下的技能（含遗留旧位置与失效符号链接）；无其他 agent 使用时删除规范目录；从 lockfile 移除对应条目。

### 4.3 `list`（别名 `ls`）— 列出已安装技能

```
agents-skills list [options]
```

| 输入（flag）         | 作用                              |
| -------------------- | --------------------------------- |
| `-g, --global`       | 列全局技能（默认 project）        |
| `-a, --agent <a>...` | 按 agent 过滤                     |
| `--json`             | 输出机器可读 JSON（无 ANSI 颜色） |

**输出**：人类可读模式下逐技能打印名称、路径（`~`/`.` 缩写）、关联 agent（最多显示 5 个）、来源。`--json` 输出 pretty JSON 数组，元素形状（camelCase，与库层 `ListedSkill` 完全一致）：

```json
{
  "name": "pdf",
  "path": "/abs/.agents/skills/pdf",
  "scope": "project",
  "agents": ["Claude Code", "Codex"],
  "source": "anthropics/skills",
  "sourceUrl": "https://github.com/anthropics/skills",
  "sourceType": "github"
}
```

`source`/`sourceUrl`/`sourceType` 无锁记录时为 `null`。**退出码**：0（含空列表）；1（未知 agent）。

**副作用**：只读，无。

### 4.4 `update`（别名 `upgrade` / `check`）— 更新技能

```
agents-skills update [skills...] [options]
```

| 输入（flag）    | 作用                                                                                 |
| --------------- | ------------------------------------------------------------------------------------ |
| `[skills...]`   | 按名过滤；空 = 全部                                                                  |
| `-g, --global`  | 仅更新全局                                                                           |
| `-p, --project` | 仅更新项目                                                                           |
| `-y, --yes`     | 跳过范围询问；当前实际行为：自动探测范围（项目有技能/锁文件 → project，否则 global） |

**行为**：读取 lockfile，跳过 `local` 来源的技能；相同来源只 clone 一次；对每个技能重新发现并以符号链接方式重装到自动探测的 agent。

**输出**：逐技能 `✓ Updated <name>` / `✗ <失败原因>`，末尾汇总。**退出码**：0（成功或无可更新）；1（存在失败项、来源解析失败）。

**副作用**：网络拉取；覆写安装目录；更新 lockfile 哈希。

---

## 5. Rust 库 API（`agents_skills` crate）

### 5.1 构造与上下文

```rust
let real     = Manager::new();                        // 真实环境（home/config/cwd）
let sandbox  = Manager::builder()                     // 注入式上下文（测试/沙箱）
    .home("/tmp/home")                                // 影响全局目录与全局锁
    .config("/tmp/config")                            // 影响 agent 配置查找
    .cwd("/tmp/project")                              // 影响项目目录与范围探测
    .env_var("CLAUDE_CONFIG_DIR", "/tmp/claude")      // 注入环境变量（不改真实进程环境）
    .build();
let env: &Env = real.env();                           // 读取已解析的上下文
```

未设置的字段在 `build()` 时回退到真实进程环境。

### 5.2 `Manager::add` / `add_source` — 安装

**输入** `AddRequest`（`Default + Clone`，可用 `AddRequest::new(source)` 快捷构造）：

| 字段         | 类型          | 默认语义                   |
| ------------ | ------------- | -------------------------- |
| `source`     | `String`      | 来源字符串（§3.1）         |
| `global`     | `bool`        | false → 项目范围           |
| `agents`     | `Vec<String>` | 空 → 自动探测              |
| `skills`     | `Vec<String>` | 空 → 全部技能              |
| `list_only`  | `bool`        | false；true 时只发现不安装 |
| `copy`       | `bool`        | false → 符号链接模式       |
| `full_depth` | `bool`        | false → 限定容器目录深度   |

**输出** `AddOutcome`：`source`（解析结果）、`skills`（全部发现）、`selected`（选中）、`target_agents`（目标 agent 显示名）、`installed: Vec<InstallSuccess { name, agent, canonical_path: Option<PathBuf> }>`（copy 模式下 `canonical_path` 为 `None`）、`failed: Vec<InstallFailure { skill, agent, error }>`、`list_only`。

**错误**：`Message`（源无效/不存在/无有效技能）、`InvalidAgents`、`Git`/`Http`/`Io`/`Zip`/`Json`/`Yaml`（传输与文件系统失败）。

**副作用**：同 CLI `add`；仅成功项写入 lockfile。

### 5.3 `Manager::list` — 列出

**输入** `ListRequest { global: bool, agents: Vec<String> }`（默认：项目范围、全部 agent）。
**输出** `Vec<ListedSkill>`（字段同 §4.3 JSON，serde 可序列化）。
**错误**：`InvalidAgents`。**副作用**：无。

### 5.4 `Manager::remove` — 移除

**输入** `RemoveRequest { skills, global, agents, all }`。语义：`skills` 为空且 `all` = false → **no-op**，仅返回已安装名单（CLI 借此打印提示）；`all` = true → 全部已安装 + 全部锁键；`agents` 为空 → 全部 agent。

**输出** `RemoveOutcome { installed, requested, removed }`。
**错误**：`InvalidAgents`。**副作用**：同 CLI `remove`。

### 5.5 `Manager::update` — 更新

**输入** `UpdateRequest { skills: Vec<String>, scope: Scope }`，其中 `Scope` ∈ `Auto`（默认：项目有技能或锁文件 → 项目，否则全局）/ `Global` / `Project`。

**输出** `UpdateOutcome { global: bool, updated: usize, failed: usize, updated_names: Vec<String>, failures: Vec<String> }`——注意 `updated`/`failed` 按 **技能 × agent** 计数。

**错误**：`Message`（记录的来源重新解析失败）；单个技能的 clone/安装失败计入 `failures` 而非返回 `Err`。

### 5.6 错误类型

统一 `Error`（= `SkillsError`）与 `Result<T>` 别名：

| 变体                                   | 触发场景                               |
| -------------------------------------- | -------------------------------------- |
| `Message(String)`                      | 用户可读的普通错误（源无效、无技能等） |
| `InvalidAgents(String)`                | 未知 agent 名（逗号分隔列表）          |
| `Io` / `Json` / `Yaml` / `Git` / `Zip` | 文件系统与格式错误                     |
| `Http`                                 | HTTP 请求错误                          |

### 5.7 低层 `core` API（crate 根重新导出）

| 模块             | 主要函数                                                            | 作用                                      |
| ---------------- | ------------------------------------------------------------------- | ----------------------------------------- |
| `core::source`   | `parse_source`, `owner_repo`                                        | 来源字符串 → 结构化 `Source`              |
| `core::discover` | `discover_skills`, `filter_skills`, `parse_skill_md`                | SKILL.md 发现/过滤/解析                   |
| `core::install`  | `install_skill_for_agent`, `list_installed_skills`, `sanitize_name` | 安装编排与目录解析                        |
| `core::lock`     | `read_local_lock`, `write_local_lock`, `compute_folder_hash`        | 锁文件读写与 SHA-256 内容哈希             |
| `core::agents`   | `get_agent`, `detect_installed_agents`, `Agent`, `Env`, `AGENTS`    | agent 表查询与安装探测                    |
| `core::fetch`    | `clone_repo`, `download_and_extract`                                | git clone / HTTP 下载解压（返回临时目录） |

均为纯函数或依赖注入式（`Env`）设计。

---

## 6. 副作用清单（黑盒合约）

### 文件系统写入

| 项                           | project 范围                    | global 范围                                                |
| ---------------------------- | ------------------------------- | ---------------------------------------------------------- |
| 规范目录（内容的权威副本）   | `<cwd>/.agents/skills/<name>`   | `~/.agents/skills/<name>`                                  |
| agent 目录（符号链接或副本） | `<cwd>/<agent 技能目录>/<name>` | agent 各自的用户级技能目录（如 `~/.claude/skills/<name>`） |
| 锁文件                       | `<cwd>/skills-lock.json`        | `~/.agents/.skill-lock.json`                               |

锁文件格式（键为净化后的技能名，排序输出，字段 camelCase）：

```json
{
  "version": 1,
  "skills": {
    "pdf": {
      "source": "anthropics/skills",
      "sourceUrl": "https://github.com/anthropics/skills",
      "ref": null,
      "sourceType": "github",
      "skillPath": "skills/pdf",
      "computedHash": "<SHA-256>"
    }
  }
}
```

### 网络访问

仅在被要求拉取远程来源时：git clone（经 libgit2）或 HTTPS 下载（zip/tar.gz/裸 SKILL.md）。**无遥测**，不做任何后台上报。

### 环境变量（只读）

- `HOME` / `XDG_CONFIG_HOME`：解析 home 与配置目录。
- `CLAUDE_CONFIG_DIR` 等 agent 专属变量：解析个别 agent 的目录。
- `INSTALL_INTERNAL_SKILLS=1`：显示内部技能。

库层通过 `ManagerBuilder::env_var` 注入的变量只在该 `Manager` 实例内生效，不修改真实进程环境。

### 明确不做的事

- 库层不打印、不 `process::exit`、不写上述路径之外的任何位置。
- CLI 对 `list` 及一切只读路径无写入副作用。
- 移除操作对"仍被其他 agent 使用的规范目录"不删除。
