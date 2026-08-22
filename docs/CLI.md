# agents-skills 命令行参考

`agents-skills` CLI 的完整命令参考。功能概览与快速上手见 [README](../README.zh-CN.md)。

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
| `update` | `upgrade`, `check`    | 将技能更新到最新版本              |
| `link`   | `ln`                  | 链接/解除链接/查看 agent 链接状态 |

## add

从本地路径、Git 仓库或 HTTPS 端点安装技能包到规范目录（项目 `.agents/skills`
或全局 `~/.agents/skills`）。`add` 不做任何 agent 链接——安装后运行 `link`
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

# 安装后运行 link，让 agent 可见
agents-skills add anthropics/skills
agents-skills link
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

列出已安装技能（每个技能下显示可见 agents）。各 agent 的链接状态由
`link --status` 查询。

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

## link

管理 agent 技能目录与规范目录的链接关系：默认链接，`--status` 查看链接状态，
`--unlink` 解除链接。

```
agents-skills link [agents...] [options]
```

| 选项             | 说明                                             |
| ---------------- | ------------------------------------------------ |
| `-g, --global`   | 链接全局技能目录而非项目目录                     |
| `--status`       | 显示已安装 agent 的链接状态（不修改任何内容）    |
| `--unlink`       | 解除 agent 与规范目录的链接                      |
| `--migrate`      | 把 Agent 目录中的存量技能移入规范目录            |

`--status` 与 `--unlink` 互斥；`--migrate` 仅对默认链接行为有效。
Agent 默认为自动探测的已安装 agent；`'*'` 表示全部。

示例：

```bash
# 链接全部已安装 agent
agents-skills link

# 链接指定 agent，并迁移其存量技能
agents-skills link claude-code --migrate

# 查看各 agent 的链接状态
agents-skills link --status

# 解除指定 agent 的链接
agents-skills link --unlink claude-code
```

## 相关概念

- 来源格式与安装位置见 [README · 功能说明](../README.zh-CN.md#功能说明)。
