# agents-skills 命令行参考

`agents-skills` CLI 的完整命令参考。功能概览见 [README](../README.md)，库使用者见 [LIBRARY.md](LIBRARY.md)。

## 安装

```bash
cargo install agents-skills
```

全局选项：`-v, --version` 显示版本号。

## 命令速查表

| 命令     | 别名                | 说明                            |
| -------- | ------------------- | ------------------------------- |
| `add`    | `a`, `i`, `install` | 从来源安装技能包                |
| `remove` | `rm`, `r`           | 移除已安装技能                  |
| `list`   | `ls`                | 列出已安装技能                  |
| `update` | `upgrade`, `check`  | 将技能更新到最新版本            |
| `disable`| `d`                 | 禁用已安装技能                  |
| `enable` | `e`                 | 重新启用已禁用的技能            |
| `agent`  |                     | 链接/解除链接/查看 agent 状态   |

通用说明：技能存放在规范目录（项目 `.agents/skills` 或全局 `~/.agents/skills`）；
`-g/--global` 切换全局作用域，`-y/--yes` 跳过确认提示。

## add

从本地路径、Git 仓库或 HTTPS 端点安装技能包。

```
agents-skills add <source...> [options]
```

| 选项                 | 说明                                      |
| -------------------- | ----------------------------------------- |
| `-g, --global`       | 全局安装（默认项目级）                    |
| `-s, --skill <s>...` | 要安装的技能名（`'*'` 表示全部）          |
| `-l, --list`         | 仅列出可用技能，不安装                    |
| `-y, --yes`          | 跳过确认提示                              |
| `--full-depth`       | 即使存在根 SKILL.md 也搜索所有子目录      |

```bash
agents-skills add anthropics/skills       # 安装仓库全部技能
agents-skills add anthropics/skills@pdf   # 仅安装指定技能
agents-skills add anthropics/skills -l    # 只列出可用技能
```

安装后运行 `agents-skills agent --link` 让 agent 可见（`add` 不自动链接）。

## remove

从规范目录移除已安装技能。

```
agents-skills remove [skills...] [options]
```

| 选项                 | 说明                                  |
| -------------------- | ------------------------------------- |
| `-g, --global`       | 从全局作用域移除                      |
| `-s, --skill <s>...` | 要移除的技能（`'*'` 表示全部）        |
| `-y, --yes`          | 跳过确认提示                          |
| `--all`              | `--skill '*' -y` 的简写               |

```bash
agents-skills remove pdf      # 移除指定技能
agents-skills remove --all    # 移除全部技能
```

## list

列出已安装技能（含启用/禁用状态）。各 agent 的链接状态用 `agent --status` 查询。

```
agents-skills list [options]
```

| 选项                 | 说明                                     |
| -------------------- | ---------------------------------------- |
| `-g, --global`       | 列出全局技能（默认项目）                 |
| `-a, --agent <a>...` | 按指定 Agent 过滤                        |
| `--json`             | JSON 输出（机器可读，含 `enabled` 字段） |

```bash
agents-skills list
agents-skills list --json
agents-skills list --global --agent claude-code
```

## update

根据 lockfile 记录将技能更新到最新版本。已禁用的技能会被跳过。

```
agents-skills update [skills...] [options]
```

| 选项            | 说明                                                     |
| --------------- | -------------------------------------------------------- |
| `-g, --global`  | 仅更新全局技能                                           |
| `-p, --project` | 仅更新项目技能                                           |
| `-y, --yes`     | 跳过作用域提示（项目内更新项目，否则更新全局）           |

```bash
agents-skills update             # 更新全部（自动检测作用域）
agents-skills update --global    # 仅更新全局技能
agents-skills update pdf         # 仅更新指定技能
```

## disable / enable

`disable` 把技能目录移到 `disabled-skills/`，使其对所有 agent 不可见；`enable`
移回规范目录恢复可见（`disable` 的逆操作）。文件完整保留，无损可逆。

```
agents-skills disable [skills...] [options]
agents-skills enable  [skills...] [options]
```

| 选项                 | 说明                                  |
| -------------------- | ------------------------------------- |
| `-g, --global`       | 全局作用域（默认项目）                |
| `-s, --skill <s>...` | 目标技能（`'*'` 表示全部）            |
| `--all`              | 禁用所有已启用 / 启用所有已禁用       |

```bash
agents-skills disable pdf      # 禁用指定技能
agents-skills disable --all    # 禁用全部已启用技能
agents-skills enable  pdf      # 启用指定技能
agents-skills enable  --all    # 启用全部已禁用技能
```

## agent

管理 agent 技能目录与规范目录的链接关系。

```
agents-skills agent [agents...] (--link | --unlink | --status) [options]
```

| 选项           | 说明                                            |
| -------------- | ----------------------------------------------- |
| `-g, --global` | 操作全局技能目录（默认项目）                    |
| `--link`       | 把 agent 技能目录链接到规范目录                 |
| `--unlink`     | 解除 agent 与规范目录的链接                     |
| `--status`     | 查看链接状态（只读）                            |
| `--migrate`    | 迁移 agent 目录中的存量技能（仅配合 `--link`）  |

`--link`/`--unlink`/`--status` 互斥，须指定其一。`--status` 区分两种可见状态：
原生读取规范目录的 agent（Codex、Cursor、Warp 等）标记 `(canonical dir)`，符号
链接接入的标记 `(linked)`。agent 默认为自动探测结果，`'*'` 表示全部。

```bash
agents-skills agent --link                       # 链接全部已安装 agent
agents-skills agent --link claude-code --migrate # 链接并迁移存量技能
agents-skills agent --status                     # 查看链接状态
agents-skills agent --unlink claude-code         # 解除指定 agent 链接
```

## 相关概念

- 来源格式与安装位置见 [README · 功能说明](../README.md#功能说明)。
- 库接口（每个命令对应的 `Manager` 方法与请求/结果类型）见 [LIBRARY.md](LIBRARY.md)。
