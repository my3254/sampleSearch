# Agent 接入指南

一个二进制，三种入口，**同一份结果结构**：

| 入口 | 命令 | 谁用 | 是否需要常驻进程 |
| --- | --- | --- | --- |
| **MCP（stdio）** | `samplesearch mcp` | Claude Code / Cursor / Windsurf / Codex / DSH 等所有支持 MCP 的客户端 | 否，由客户端拉起 |
| **CLI** | `samplesearch search "…"` | 任何能执行命令的 agent（含不能配 MCP 的） | 否 |
| **HTTP** | `samplesearch serve` | 多 agent 共享、或非 Agent 程序调用 | 是 |

推荐做法：**MCP 为主 + CLI 兜底 + Skill 说明什么时候用哪个**。MCP 提供结构化调用，CLI 保证"任何 agent 都能用"；Skill 只是一份说明书，不承载逻辑。

先构建：

```powershell
$env:CARGO_HOME='$env:CARGO_HOME="$PWD\.cargo-home"'
cargo build --release
# 产物：D:\software\idea\samplesearch\target\release\samplesearch.exe
```

下文把该路径记作 `SAMPLESEARCH_EXE`。

---

## 1. MCP（首选）

### Claude Code

```powershell
claude mcp add samplesearch -- "$PWD\target\release\samplesearch.exe" mcp
claude mcp list
```

### Cursor / Windsurf / Claude Desktop（`mcpServers` 形态）

`~/.cursor/mcp.json`（Windsurf 为 `~/.codeium/windsurf/mcp_config.json`，Claude Desktop 为 `claude_desktop_config.json`）：

```json
{
  "mcpServers": {
    "samplesearch": {
      "command": "D:\\software\\idea\\samplesearch\\target\\release\\samplesearch.exe",
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
    "samplesearch": {
      "type": "stdio",
      "command": "D:\\software\\idea\\samplesearch\\target\\release\\samplesearch.exe",
      "args": ["mcp"]
    }
  }
}
```

> 各客户端键名（`mcpServers` vs `servers`）随版本变化，以你当前版本的官方文档为准；命令与参数不变。

### Codex

`~/.codex/config.toml`：

```toml
[mcp_servers.samplesearch]
command = 'D:\software\idea\samplesearch\target\release\samplesearch.exe'
args = ["mcp"]
```

### 验证 MCP 是否通

```powershell
Get-Content integrations\mcp-session.jsonl | & $env:SAMPLESEARCH_EXE mcp
```

应依次看到 `initialize` 回包、`tools/list`（3 个工具）、`tools/call` 的真实结果。

工具清单：`search`（query / max_results / explain）、`batch_search`（1–5 条）、`health`。

---

## 2. CLI（兜底，任何 agent 都能用）

```powershell
samplesearch search "大模型 推理 优化" -m 5
samplesearch search "600519 财报" -m 3 --explain
samplesearch health
samplesearch doc      # 给 agent 读的用法说明
```

- 输出是 JSON（`code` / `data.results` / `data.metadata`），可直接解析。
- 失败时 `code != 0` 且退出码非 0（`3`），便于 agent 判断。
- 不依赖 HTTP 服务，每次调用直接跑管线（约 200–800ms）。

---

## 3. HTTP（多 agent 共享）

```powershell
samplesearch serve          # 127.0.0.1:8787
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
| Claude Code | `~/.claude/skills/samplesearch/SKILL.md` |
| 通用共享 | `~/.agents/skills/samplesearch/SKILL.md` |
| OpenCode | `~/.config/opencode/skills/samplesearch/SKILL.md` |
| Cursor（项目级） | `<project>/.skills/samplesearch/SKILL.md` |

不支持 skill 的平台：把 `SKILL.md` 正文贴进项目的 `AGENTS.md` / `CLAUDE.md`，效果等价。

---

## 5. DeepSeek Harness（DSH）

DSH 里我们提供的是**原生插件**：直接 `ctx.tools.register(defineTool(...))` 把 `search` / `batch_search` / `search_health` 注册进工具表，**不需要 MCP 桥、也不需要单独装 `@deepseek-ai/dsh-mcp-client`**。插件名与仓库同名：`samplesearch`。

```powershell
# 一条命令装好插件（同时会把当前安装的二进制路径写进插件配置）
irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1 | iex
# 或者手动：
dsh plugin --profile desktop add 'file:<安装目录>\integrations\dsh-plugin'
# 然后重启 DeepSeek Harness，新开一个会话
```

插件包 `integrations/dsh-plugin/` 只有两个文件：

```jsonc
// package.json —— 它既是 bundle（dsh.bundle.patch）又是插件（index.js）
{ "name": "samplesearch", "type": "module", "main": "index.js",
  "dsh": { "bundle": { "patch": "./cordis.patch.yml" } } }
```

```yaml
# cordis.patch.yml —— insert 自己的插件条目
- insert:
    - id: samplesearch
      name: samplesearch
      config:
        binary: ''        # 空 = 从 PATH 找；installer 会写入绝对路径
        timeoutMs: 45000
```

为什么不用 `dsh-mcp-client`：那是一个 MCP 桥，链路是「DSH → MCP 客户端 → 子进程（MCP server）→ 搜索」。原生插件把中间两层去掉，直接 `spawn` 一次 CLI 拿 JSON，少一个依赖、少一层协议、少一次握手。

DSH 插件加载的两条硬规则（踩错只会看到"已安装，未生效"）：

1. **只有 `dsh.profile.bundles` 里、且声明了 `dsh.bundle` 的包才会被加载**；
2. **补丁条目分两种**：带 `id` 的是"覆盖已存在条目"（找不到会警告 `patch: entry "x" not found`）；带 `insert` 的才是"新增条目"。

**坑**：pnpm 对 `file:` 目录依赖是**复制**而非软链，改了插件代码后要么提升 `version` 重装，要么把新文件覆盖到 `<profile>\node_modules\samplesearch\`。

验证（不用启 GUI，与真实启动共用 `applyEntryPatches`）：

```powershell
dsh --profile mcpcheck --dump-config | Select-String -Pattern 'samplesearch|patch:' -Context 2,3
```

装好后工具出现在**新会话**的工具清单里（名字是 `search` / `batch_search` / `search_health`）；插件页显示的是插件包，不显示工具。

---

## 6. 为什么不是"只做 skill+CLI"或"只做 skill+MCP"

| 组合 | 能用在哪 | 短板 |
| --- | --- | --- |
| skill + CLI | 所有能执行命令的 agent | agent 要自己拼引号、解析 stdout；无结构化错误 |
| skill + MCP | 支持 MCP 的客户端 | 不支持 MCP 的 agent（部分轻量壳、脚本、旧版）完全用不了 |
| **skill + MCP + CLI（本方案）** | 两者并集 | 需要维护两个薄壳——但它们共用同一个 `Orchestrator`，薄壳各约 200 行 |

关键前提是**三个入口共用一套核心**：CLI 与 MCP 都直接调用 `Orchestrator::search`，HTTP 层只是再包一层。所以不存在"三个实现、三份行为不一致"的问题。
