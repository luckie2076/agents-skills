# agents-skills 命令行参考

`agents-skills` CLI 的完整命令参考。功能概览与快速上手见 [README](../README.md)。

## 安装

```bash
cargo install agents-skills
```

## 全局选项

| 选项            | 说明         |
| --------------- | ------------ |
| `-v, --version` | 显示版本号   |

## 命令速查表

| 命令     | 别名                  | 说明                              |
| -------- | --------------------- | --------------------------------- |
| `add`    | `a`, `i`, `install`   | 从来源安装技能包                  |
| `remove` | `rm`, `r`             | 移除已安装技能                    |
| `list`   | `ls`                  | 列出已安装技能                    |
| `update`  | `upgrade`, `check`    | 将技能更新到最新版本              |
| `disable` | `d`                   | 禁用已安装技能                    |
| `enable`  | `e`                   | 重新启用已禁用的技能              |
| `agent`   |                       | 链接/解除链接/查看 agent 链接状态 |

## add

从本地路径、Git 仓库或 HTTPS 端点安装技能包到规范目录（项目 `.agents/skills`
或全局 `~/.agents/skills`）。`add` 不做任何 agent 链接——安装后运行 `agent --link`
即可让 agent 可见。

```
agents-skills add <source...> [options]
```

| 选项                   | 说明                                              |
| ---------------------- | ------------------------------------------------- |
| `-g, --global`         | 全局（用户级）安装，而非项目级                    |
| `-s, --skill <s>...`   | 要安装的技能名（`'*'` 表示全部）                  |
| `-l, --list`           | 仅列出可用技能，不安装                            |
| `-y, --yes`            | 跳过确认提示                                      |
| `--full-depth`         | 即使存在根 SKILL.md 也搜索所有子目录              |

示例：

```bash
# 安装仓库全部技能
agents-skills add anthropics/skills

# 仅安装指定技能
agents-skills add anthropics/skills@pdf

# 只列出仓库中的可用技能，不安装
agents-skills add anthropics/skills -l

# 安装后运行 agent --link，让 agent 可见
agents-skills add anthropics/skills
agents-skills agent --link
```

## remove

从规范目录移除已安装技能。

```
agents-skills remove [skills...] [options]
```

| 选项                 | 说明                                          |
| -------------------- | --------------------------------------------- |
| `-g, --global`       | 从全局作用域（`~/`）而非项目作用域移除        |
| `-s, --skill <s>...` | 要移除的技能（`'*'` 表示全部）                |
| `-y, --yes`          | 跳过确认提示                                  |
| `--all`              | `--skill '*' -y` 的简写                       |

示例：

```bash
# 移除指定技能
agents-skills remove pdf

# 移除全部技能
agents-skills remove --all
```

## list

列出已安装技能（技能名、规范目录路径、来源、启用状态）。`list` 始终展示
全部技能（启用 + 禁用），并附带 `enabled`/`disabled` 状态；`--json` 输出中
增加 `enabled` 布尔字段。各 agent 的链接状态由 `agent --status` 查询。

```
agents-skills list [options]
```

| 选项                 | 说明                                       |
| -------------------- | ------------------------------------------ |
| `-g, --global`       | 列出全局技能（默认：项目）                 |
| `-a, --agent <a>...` | 按指定 Agent 过滤                          |
| `--json`             | 以 JSON 输出（机器可读，无 ANSI 颜色码）   |

示例：

```bash
agents-skills list
agents-skills list --json
agents-skills list --global --agent claude-code
```

## update

根据 lockfile 记录将技能更新到最新版本。

```
agents-skills update [skills...] [options]
```

| 选项            | 说明                                                             |
| --------------- | ---------------------------------------------------------------- |
| `-g, --global`  | 仅更新全局技能                                                   |
| `-p, --project` | 仅更新项目技能                                                   |
| `-y, --yes`     | 跳过作用域提示（自动检测：项目内更新项目，否则更新全局）         |

示例：

```bash
# 更新全部技能（自动检测作用域）
agents-skills update

# 仅更新全局技能
agents-skills update --global

# 仅更新指定技能
agents-skills update pdf
```

## disable

临时禁用已安装技能：把技能目录从规范目录移到平级的 `disabled-skills/`，
使其对所有 agent 立即不可见；文件完整保留，`enable` 可无损恢复。已禁用的
技能 `update` 会跳过（保持禁用状态）。

```
agents-skills disable [skills...] [options]
```

| 选项                 | 说明                                       |
| -------------------- | ------------------------------------------ |
| `-g, --global`       | 禁用全局技能（默认：项目）                 |
| `-s, --skill <s>...` | 要禁用的技能（`'*'` 表示全部）             |
| `--all`              | 禁用所有已启用的技能                       |

示例：

```bash
# 禁用指定技能
agents-skills disable pdf

# 禁用全部已启用技能
agents-skills disable --all
```

## enable

重新启用已禁用的技能：把技能目录从 `disabled-skills/` 移回规范目录，恢复对
所有 agent 的可见性。是 `disable` 的逆操作。

```
agents-skills enable [skills...] [options]
```

| 选项                 | 说明                                       |
| -------------------- | ------------------------------------------ |
| `-g, --global`       | 启用全局技能（默认：项目）                 |
| `-s, --skill <s>...` | 要启用的技能（`'*'` 表示全部）             |
| `--all`              | 启用所有已禁用的技能                       |

示例：

```bash
# 启用指定技能
agents-skills enable pdf

# 启用全部已禁用技能
agents-skills enable --all
```

## agent

管理 agent 技能目录与规范目录的链接关系。必须通过 `--link` / `--unlink` /
`--status` 之一选择操作：`--link` 建立链接，`--unlink` 解除链接，`--status`
查看链接状态（只读）。

```
agents-skills agent [agents...] (--link | --unlink | --status) [options]
```

| 选项             | 说明                                             |
| ---------------- | ------------------------------------------------ |
| `-g, --global`   | 操作全局技能目录而非项目目录                     |
| `--link`         | 把 agent 技能目录链接到规范目录                  |
| `--unlink`       | 解除 agent 与规范目录的链接                      |
| `--status`       | 显示已安装 agent 的链接状态（不修改任何内容）    |
| `--migrate`      | 把 Agent 目录中的存量技能移入规范目录（仅 `--link`） |

`--link` / `--unlink` / `--status` 互斥，必须且只能指定其一。`--status` 区分
两种"可见"状态：原生读取规范目录的 agent（如 Codex、Cursor、Warp）标记为
`(canonical dir)`；通过符号链接接入的 agent 标记为 `(linked)`。`--migrate`
仅与 `--link` 配合。Agent 默认为自动探测的已安装 agent；`'*'` 表示全部。

示例：

```bash
# 链接全部已安装 agent
agents-skills agent --link

# 链接指定 agent，并迁移其存量技能
agents-skills agent --link claude-code --migrate

# 查看各 agent 的链接状态
agents-skills agent --status

# 解除指定 agent 的链接
agents-skills agent --unlink claude-code
```

## 相关概念

- 来源格式与安装位置见 [README · 功能说明](../README.md#功能说明)。
