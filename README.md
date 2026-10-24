# samplesearch

A self-hosted search backend for AI agents. One binary serves an HTTP API, an MCP (stdio) server and a CLI, and all three return the same ranked, filtered results — no search API key, no per-query billing.

[中文说明](#中文说明) · [Documentation](docs/) · [Releases](https://github.com/my3254/sampleSearch/releases)

---

## What it does

Most "AI search" products are a hosted index with a per-query price. samplesearch takes a different position: it queries public result pages and official data APIs that are free to read, then spends its effort on the part that actually determines answer quality — deciding which results to keep.

- **One binary, three entry points.** `serve` (HTTP), `mcp` (MCP stdio), `search` (one-shot CLI). Same pipeline underneath, so behaviour cannot diverge between clients.
- **Nine engines, two kinds.** 360 and Bing read public result pages for general queries; GitHub, Stack Exchange, Hacker News, arXiv, Crossref, npm and PyPI are queried through their official APIs, which publish quotas and never answer with a captcha.
- **Precision before recall.** Query terms are matched as concepts, not as character fragments; seven filters remove aggregator pages, site homepages, entity-only titles and unrelated hits before scoring. Results that would be filler are dropped, and `metadata.filters` reports what was dropped and why.
- **Rust, single file.** ~6 MB release binary, no runtime, no resident sidecar beyond the one process.

Status: v0.1.0. Nine engines, filtering, caching and all three transports are implemented and tested against live upstreams.

## Install

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1 | iex
```

macOS / Linux:

```bash
curl -fsSL https://raw.githubusercontent.com/my3254/sampleSearch/main/install.sh | bash
```

The installer downloads the release asset for your platform, installs it under a per-user directory, adds it to `PATH`, installs the agent skill, and prints a ready-to-paste MCP configuration.

DeepSeek Harness users can add the native plugin in the same step:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1))) -InstallDshPlugin
```

Build from source:

```bash
cargo build --release          # requires Rust 1.80+
cargo test
```

## Use it

```bash
samplesearch search "大模型 推理 优化" -m 5      # JSON envelope on stdout
samplesearch serve                              # HTTP API on 127.0.0.1:8787
samplesearch mcp                                # MCP server over stdio
samplesearch doc                                # usage notes written for agents
```

MCP clients (Claude Code, Cursor, Codex, Windsurf, DeepSeek Harness):

```jsonc
{ "mcpServers": { "samplesearch": { "command": "samplesearch", "args": ["mcp"] } } }
```

## API

`POST /v1/search`

```jsonc
{ "query": "600519 财报", "max_results": 5, "zone": "cn", "language": "zh-CN" }
```

```jsonc
{
  "code": 0, "message": "success", "request_id": "…",
  "data": {
    "results": [
      { "title": "…", "url": "https://…", "snippet": "…", "content": "…",
        "source": "so360", "score": 0.816 }
    ],
    "metadata": {
      "total_results": 5, "search_time_ms": 640, "cached": false,
      "routing": { "intent": "general_cn", "primary": ["so360"],
                   "secondary": ["bing"], "degraded": ["bing"] },
      "sources": [{ "name": "so360", "count": 5, "ms": 615, "status": "ok" }],
      "filters": [{ "reason": "low_match", "count": 3 }]
    }
  }
}
```

The envelope is intentionally compatible with AnySearch's contract, so an existing client only needs its `base_url` changed. `routing`, `sources` and `filters` are additive fields; clients that ignore them still work.

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 400 | Invalid argument (query length, `max_results` range) |
| 429 | Local rate limit |
| 502 | Every upstream failed (partial failures still return 0 + `degraded`) |
| -1 | Internal error |

## How accuracy is handled

1. **Concept matching.** The query is split into concepts; stop words and modifiers are dropped ("最新政策" → "政策"). Two-concept queries must match both; three or four concepts must match two.
2. **Engine gate.** If none of an engine's top three results mentions a query concept, that engine is marked `degraded` and its weight is reduced — this is what catches a search engine silently answering with an entity page.
3. **Filter chain.** Noise hosts, aggregator titles, site homepages, low matches, title-only matches, entity pages, script mismatches and duplicate snippets are removed in a fixed order.
4. **Scoring and diversification.** Reciprocal-rank fusion across sources, then weighted features (concept coverage, phrase hit, domain authority, freshness, length), then a per-domain cap.

Measured on a nine-query set (`node probe/eval.mjs`): precision rose from 0.79 to 1.00 while the average result count fell from 4.8 to 3.9. That trade is deliberate — see [the precision handbook](docs/2026-10-24-precision.md).

## Limits

- **Upstream quota is the real limit.** 360 and Bing are public result pages, not licensed APIs. Sustained volume triggers captchas or connection resets; this is why the service caches repeated queries for five minutes, cools an engine down for ten minutes after a block, and retries a slow but working engine with a longer budget instead of failing at the fixed tier timeout.
- **Free sources are not complete.** A query can legitimately return fewer results than requested; the API reports that instead of padding.
- **No anti-bot evasion.** Sources that require CAPTCHA solving, cookie pools or signature reversing are excluded by design.
- **Personal use.** Not built or licensed for public hosting or resale; result pages belong to their respective sites.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SAMPLESEARCH_BIND` | `127.0.0.1:8787` | HTTP listen address |
| `SAMPLESEARCH_TIER_A_TIMEOUT_MS` | `1500` | Primary engine timeout |
| `SAMPLESEARCH_TIER_B_TIMEOUT_MS` | `2000` | Secondary engine timeout |
| `SAMPLESEARCH_MAX_UPSTREAM` | `4` | Upstream requests per query |
| `SAMPLESEARCH_DEFAULT_MAX` | `10` | Default `max_results` |
| `SAMPLESEARCH_INTL` | `0` | Enable overseas engines |
| `SAMPLESEARCH_PROXY` | — | Proxy for overseas engines only |
| `SAMPLESEARCH_GITHUB_TOKEN` | — | Raises GitHub search from 10 to 30 requests/minute |
| `SAMPLESEARCH_SE_KEY` | — | Raises Stack Exchange from 300 to 10,000 requests/day |
| `SAMPLESEARCH_CONTACT` | — | Email sent to Crossref as `mailto` (polite pool) |

## Integrations

| Client | How |
| --- | --- |
| Claude Code, Cursor, Codex, Windsurf | MCP stdio: `samplesearch mcp` |
| DeepSeek Harness | Native plugin (`integrations/dsh-plugin`), registers tools on `ctx.tools` directly — no MCP bridge, no extra package |
| Any shell-based agent | `samplesearch search "…" -m 5` |
| Your own code | `POST /v1/search` |

Per-client snippets: [integrations/README.md](integrations/README.md).

## Documentation

| Document | Contents |
| --- | --- |
| [docs/search-sources-survey.md](docs/search-sources-survey.md) | Where search MCP servers get their results, and what we adopt from that |
| [docs/2026-10-24-architecture.md](docs/2026-10-24-architecture.md) | Pluggable channels (declarative / native / external), hot reload, scoring pipeline |
| [docs/2026-10-24-precision.md](docs/2026-10-24-precision.md) | Filter chain, thresholds, measured numbers, known false drops |
| [docs/2026-10-24-samplesearch-plan.md](docs/2026-10-24-samplesearch-plan.md) | Source reachability matrix, routing rules, milestones |

## License

MIT

---

## 中文说明

自托管的搜索后端，服务对象是 AI Agent。一个二进制提供三种入口（HTTP / MCP stdio / CLI），三者返回同一份排序并过滤后的结果；不需要搜索 API key，也不按次计费。

### 定位

市面上多数"AI 搜索"是托管索引 + 按次收费。samplesearch 走另一条路：查询公开结果页与免费官方数据接口，把工程投入放在真正决定回答质量的一环——**决定哪些结果留下来**。

- **一个二进制，三种入口**：`serve`（HTTP）、`mcp`（MCP stdio）、`search`（一次性 CLI）。共用同一套管线，客户端之间不会出现行为差异。
- **九个引擎，两类来源**：360 与 Bing 读公开结果页，负责通用查询；GitHub、Stack Exchange、Hacker News、arXiv、Crossref、npm、PyPI 走官方 API，有公开配额、不会弹验证码。
- **精度优先于召回**：查询词按"概念"匹配而非字符片段；七道过滤器在打分前剔除聚合页、站点首页、纯实体标题与无关命中。被丢弃的结果会带原因回报在 `metadata.filters` 里。
- **Rust 单文件**：release 约 6 MB，无运行时依赖。

当前版本 v0.1.0：九个引擎、过滤、缓存与三种入口均已实现，并对真实上游验证。

### 安装

```powershell
# Windows
irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1 | iex
```

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/my3254/sampleSearch/main/install.sh | bash
```

安装脚本会下载对应平台的发布包、装到用户目录、加入 `PATH`、安装 Agent Skill，并打印可直接粘贴的 MCP 配置。

DeepSeek Harness 用户可以同时注册原生插件（不需要额外安装 MCP 桥）：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1))) -InstallDshPlugin
```

从源码构建：

```bash
cargo build --release   # Rust 1.80+
cargo test
```

### 使用

```bash
samplesearch search "大模型 推理 优化" -m 5   # 输出 JSON 信封
samplesearch serve                           # HTTP 服务，默认 127.0.0.1:8787
samplesearch mcp                             # MCP stdio server
samplesearch doc                             # 面向 Agent 的用法说明
```

MCP 客户端配置：

```jsonc
{ "mcpServers": { "samplesearch": { "command": "samplesearch", "args": ["mcp"] } } }
```

### 接口

`POST /v1/search`，请求体 `{"query": "...", "max_results": 5}`，返回信封与 AnySearch 契约同构，现有客户端只需改 `base_url`。`routing` / `sources` / `filters` 为增量字段，忽略它们的客户端不受影响。

### 精度是怎么做的

1. **概念匹配**：查询切分为概念，去掉停用词与修饰前缀（"最新政策" → "政策"）；两个概念必须都命中，三到四个概念命中两个。
2. **引擎闸门**：某引擎前三名结果都不含任何查询概念时，该引擎标记为 `degraded` 并降权——这是用来识别"搜索引擎悄悄返回实体页"的手段。
3. **过滤链**：噪声域名、聚合页标题、站点首页、低命中、标题无概念、纯实体页、语种不匹配、重复摘要，按固定顺序剔除。
4. **融合与重排**：跨源 RRF 融合，再按概念覆盖、整句命中、域名权威度、时效、长度加权排序，最后限制单域名条数。

在九条查询的评测集上（`node probe/eval.mjs`）：准确率从 0.79 提升到 1.00，平均返回条数从 4.8 降到 3.9。这个取舍是刻意的，细节见[精度手册](docs/2026-10-24-precision.md)。

### 已知限制

- **上游配额就是真正的上限**：360 与 Bing 是公开结果页，不是授权 API。持续高频会触发验证码或连接重置；因此我们对同一查询缓存五分钟、被拦后冷却十分钟、并在只剩一个可用引擎时给足超时预算而不是直接失败。
- **免费源覆盖不全**：结果条数可能少于请求条数，接口如实返回，不拿无关结果凑数。
- **不做反爬绕过**：需要验证码破解、Cookie 池或签名逆向的源一律不接。
- **个人使用**：不面向公网托管或转售；结果页版权归各站点所有。

### 配置

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `SAMPLESEARCH_BIND` | `127.0.0.1:8787` | HTTP 监听地址 |
| `SAMPLESEARCH_TIER_A_TIMEOUT_MS` | `1500` | 主引擎超时 |
| `SAMPLESEARCH_TIER_B_TIMEOUT_MS` | `2000` | 附加引擎超时 |
| `SAMPLESEARCH_MAX_UPSTREAM` | `4` | 单次查询上游请求上限 |
| `SAMPLESEARCH_DEFAULT_MAX` | `10` | 默认 `max_results` |
| `SAMPLESEARCH_INTL` | `0` | 是否启用海外引擎 |
| `SAMPLESEARCH_PROXY` | — | 仅海外引擎使用的代理 |
| `SAMPLESEARCH_GITHUB_TOKEN` | — | 把 GitHub 搜索配额从 10 次/分提到 30 次/分 |
| `SAMPLESEARCH_SE_KEY` | — | 把 Stack Exchange 配额从 300 次/天提到 10,000 次/天 |
| `SAMPLESEARCH_CONTACT` | — | 作为 `mailto` 发给 Crossref（polite pool） |

### 许可

MIT
