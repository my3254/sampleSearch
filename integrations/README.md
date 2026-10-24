# Agent 接入指南

一个二进制，三种入口，**同一份结果结构**：

| 入口 | 命令 | 谁用 | 是否需要常驻进程 |
| --- | --- | --- | --- |
| **MCP（stdio）** | `mysearch mcp` | Claude Code / Cursor / Windsurf / Codex / DSH 等所有支持 MCP 的客户端 | 否，由客户端拉起 |
| **CLI** | `mysearch search "…"` | 任何能执行命令的 agent（含不能配 MCP 的） | 否 |
| **HTTP** | `mysearch serve` | 多 agent 共享、或非 Agent 程序调用 | 是 |

推荐做法：**MCP 为主 + CLI 兜底 + Skill 说明什么时候用哪个**。MCP 提供结构化调用，CLI 保证"任何 agent 都能用"；Skill 只是一份说明书，不承载逻辑。

先构建：

```powershell
$env:CARGO_HOME='D:\software\idea\mysearch\.cargo-home'
cargo build --release
# 产物：D:\software\idea\mysearch\target\release\mysearch.exe
```

下文把该路径记作 `MYSEARCH_EXE`。

---

## 1. MCP（首选）

### Claude Code

```powershell
claude mcp add mysearch -- "D:\software\idea\mysearch\target\release\mysearch.exe" mcp
claude mcp list
```

### Cursor / Windsurf / Claude Desktop（`mcpServers` 形态）

`~/.cursor/mcp.json`（Windsurf 为 `~/.codeium/windsurf/mcp_config.json`，Claude Desktop 为 `claude_desktop_config.json`）：

```json
{
  "mcpServers": {
    "mysearch": {
      "command": "D:\\software\\idea\\mysearch\\target\\release\\mysearch.exe",
      "args": ["mcp"],
      "env": { "RUST_LOG": "warn" }
    }
  }
}
```

### VS Code（Copilot / Continue 等）

工作区 `.vscode/mcp.json`：

```json
{
  "servers": {
    "mysearch": {
      "type": "stdio",
      "command": "D:\\software\\idea\\mysearch\\target\\release\\mysearch.exe",
      "args": ["mcp"]
    }
  }
}
```

> 各客户端键名（`mcpServers` vs `servers`）随版本变化，以你当前版本的官方文档为准；命令与参数不变。

### Codex

`~/.codex/config.toml`：

```toml
[mcp_servers.mysearch]
command = 'D:\software\idea\mysearch\target\release\mysearch.exe'
args = ["mcp"]
```

### 验证 MCP 是否通

```powershell
Get-Content integrations\mcp-session.jsonl | & $env:MYSEARCH_EXE mcp
```

应依次看到 `initialize` 回包、`tools/list`（3 个工具）、`tools/call` 的真实结果。

工具清单：`search`（query / max_results / explain）、`batch_search`（1–5 条）、`health`。

---

## 2. CLI（兜底，任何 agent 都能用）

```powershell
mysearch search "大模型 推理 优化" -m 5
mysearch search "600519 财报" -m 3 --explain
mysearch health
mysearch doc      # 给 agent 读的用法说明
```

- 输出是 JSON（`code` / `data.results` / `data.metadata`），可直接解析。
- 失败时 `code != 0` 且退出码非 0（`3`），便于 agent 判断。
- 不依赖 HTTP 服务，每次调用直接跑管线（约 200–800ms）。

---

## 3. HTTP（多 agent 共享）

```powershell
mysearch serve          # 127.0.0.1:8787
```

```bash
curl -s -X POST http://127.0.0.1:8787/v1/search -H 'content-type: application/json' \
  -d '{"query":"台风 路径 上海","max_results":5}'
curl -s http://127.0.0.1:8787/health
```

契约与 AnySearch 同构，任何按 AnySearch 写的客户端改 base_url 即可指向本机。

---

## 4. Skill（说明书）

`integrations/skill/SKILL.md` 是给 agent 读的"什么时候用、怎么读结果、别踩什么坑"。

安装位置（按平台任选其一）：

| 平台 | 目录 |
| --- | --- |
| Claude Code | `~/.claude/skills/mysearch/SKILL.md` |
| 通用共享 | `~/.agents/skills/mysearch/SKILL.md` |
| OpenCode | `~/.config/opencode/skills/mysearch/SKILL.md` |
| Cursor（项目级） | `<project>/.skills/mysearch/SKILL.md` |

不支持 skill 的平台：把 `SKILL.md` 正文贴进项目的 `AGENTS.md` / `CLAUDE.md`，效果等价。

---

## 5. DeepSeek Harness（DSH）

DSH 的插件加载有两条硬规则，踩错就只能看到"已安装，未生效"：

1. **只有 `dsh.profile.bundles` 里、且声明了 `dsh.bundle` 的包才会被加载**（loader 只遍历 bundles）。
2. **补丁条目分两种**：带 `id` 的是"覆盖已存在条目"（找不到就警告 `patch: entry "x" not found`）；带 `insert` 的才是"新增条目"（无 `id` 时追加到顶层）。

`@deepseek-ai/dsh-mcp-client` 是插件包、不是 bundle，所以必须由**我们自己做一个 bundle** 去 `insert` 它。仓库里已备好：`integrations/dsh-bundle/`。

```powershell
# 1) 安装本地 bundle（自动进入 dsh.profile.bundles）
dsh plugin --profile desktop add 'file:D:\software\idea\mysearch\integrations\dsh-bundle'
# 2) 重启 DeepSeek Harness
# 3) 验证 MCP 子进程
Get-CimInstance Win32_Process -Filter "Name='mysearch.exe'" | Select-Object ProcessId,CommandLine
```

bundle 补丁必须是 `insert` 形式：

```yaml
- insert:
    - id: mysearch-mcp
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: mysearch
        transport: stdio
        command: 'D:\software\idea\mysearch\target\release\mysearch.exe'
        args: ['mcp']
        env:
          RUST_LOG: warn
```

**坑**：pnpm 对 `file:` 目录依赖是**复制**而非软链，改了补丁后要么提升 `version` 重装，要么把新文件覆盖到 `<profile>\node_modules\dsh-mysearch-bundle\`。

不需要启 GUI 就能验证组合（与真实启动共用 `applyEntryPatches`）：

```powershell
dsh --profile mcpcheck --dump-config | Select-String -Pattern 'mysearch|patch:' -Context 2,3
```

装好后工具名是 `mcp__mysearch__search` / `mcp__mysearch__batch_search` / `mcp__mysearch__health`，只出现在**新会话**的工具清单里；插件页显示的是插件包，不会显示工具。

---

## 6. 为什么不是"只做 skill+CLI"或"只做 skill+MCP"

| 组合 | 能用在哪 | 短板 |
| --- | --- | --- |
| skill + CLI | 所有能执行命令的 agent | agent 要自己拼引号、解析 stdout；无结构化错误 |
| skill + MCP | 支持 MCP 的客户端 | 不支持 MCP 的 agent（部分轻量壳、脚本、旧版）完全用不了 |
| **skill + MCP + CLI（本方案）** | 两者并集 | 需要维护两个薄壳——但它们共用同一个 `Orchestrator`，薄壳各约 200 行 |

关键前提是**三个入口共用一套核心**：CLI 与 MCP 都直接调用 `Orchestrator::search`，HTTP 层只是再包一层。所以不存在"三个实现、三份行为不一致"的问题。
