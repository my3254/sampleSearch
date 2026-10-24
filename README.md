# mysearch

自托管、零 API 费用的 **AI 搜索聚合服务**：用一个本地二进制，给各种 AI Agent 提供「检索 + 结构化结果」，接口与 [AnySearch](https://www.anysearch.com) 契约同构。

- **一个二进制，三种入口**：`serve`（HTTP）/ `mcp`（MCP stdio server）/ `search`（CLI）
- **零付费 key**：聚合 360（中文主引擎）、Bing（英文/兜底）等公开源
- **精度优先**：概念匹配 + 7 道过滤链 + 引擎级相关性闸门，宁缺毋滥
- **Rust**：单文件分发、常驻内存约 20MB

> 状态：M0 + 接入层可用（真实上游验证通过）。垂直域、缓存、热加载在 M1/M2。

## 快速开始

```powershell
$env:CARGO_HOME="$PWD\.cargo-home"
cargo vendor vendor          # 一次性：把依赖落到 vendor/（需要能访问 crates.io）
cargo build --release        # 产物：target\release\mysearch.exe
cargo test                   # 29 个单测
```

```powershell
mysearch serve                                # 127.0.0.1:8787
mysearch search "大模型 推理 优化" -m 5        # CLI，输出 JSON
mysearch mcp                                  # MCP stdio server
```

```bash
curl -s -X POST http://127.0.0.1:8787/v1/search \
  -H 'content-type: application/json' \
  -d '{"query":"贵州茅台 2025 年报","max_results":3}'
```

## 接入方式

| 入口 | 命令 | 适用 |
| --- | --- | --- |
| MCP (stdio) | `mysearch mcp` | Claude Code / Cursor / Windsurf / Codex / DeepSeek Harness |
| CLI | `mysearch search "…" -m 5` | 任何能执行命令的 agent |
| HTTP | `mysearch serve` | 多 agent 共享、非 Agent 程序 |

客户端配置与 Skill 安装见 [integrations/README.md](integrations/README.md)。
DSH 用户：`integrations/dsh-bundle/` 是一个可直接安装的 profile bundle。

## API

`POST /v1/search`

```jsonc
{ "query": "600519 财报", "max_results": 5, "zone": "cn", "language": "zh-CN" }
```

```jsonc
{ "code": 0, "message": "success", "request_id": "…",
  "data": {
    "results": [{ "title": "", "url": "", "snippet": "", "content": "", "source": "so360", "score": 0.81 }],
    "metadata": {
      "total_results": 5, "search_time_ms": 640, "cached": false,
      "routing": { "intent": "general_cn", "primary": ["so360"], "secondary": ["bing"], "degraded": ["bing"] },
      "sources": [{ "name": "so360", "count": 5, "ms": 615, "status": "ok" }],
      "filters": [{ "reason": "low_match", "count": 3 }]
    }
  } }
```

`metadata.routing` / `filters` / `sources` 是增量字段（AnySearch 无），客户端可忽略。

## 设计要点

- **不"都搜一遍"**：按语言/意图选 1 个主引擎 + ≤2 个附加，结果不足才兜底，单次上游请求上限 4；平均端到端 ~200–700ms。
- **概念匹配**：查询切组、去停用词、去修饰前缀；1 组需 1 命中，2 组需 2，3–4 组需 2。用来干掉"大模型 推理 优化 → 大（汉语文字）"这类实体塌缩。
- **7 道过滤链**：noise_host / aggregator_title / portal_root / low_match / title_mismatch / entity_page / script_mismatch / dup_content，全部原因回传在 `metadata.filters`。
- **实测精度**：`node probe/eval.mjs`：过滤前 precision 0.79（34/43）→ 过滤后 1.00（35/35），平均返回 3.9/5（召回靠 M1 加源，不靠放宽过滤）。
- **不接反爬源**：需要验证码绕过 / cookie 池 / 签名逆向 / 登录态的数据源一律不做。

## 文档

| 文档 | 内容 |
| --- | --- |
| [docs/2026-10-24-mysearch-plan.md](docs/2026-10-24-mysearch-plan.md) | 源可达性实测矩阵、引擎路由、里程碑 |
| [docs/2026-10-24-architecture.md](docs/2026-10-24-architecture.md) | 插件化渠道（L1 声明式 / L2 原生 / L3 外挂）、热加载、打分管线 |
| [docs/2026-10-24-precision.md](docs/2026-10-24-precision.md) | 精确度手册：过滤链、阈值、实测数字、误删边界 |
| [integrations/README.md](integrations/README.md) | MCP / CLI / HTTP / Skill / DSH 安装 |
| [AGENTS.md](AGENTS.md) | 给编码 Agent 的仓库说明（构建陷阱、约定） |

## 环境注意

- `vendor/` 未入库（187MB）。构建前执行一次 `cargo vendor vendor`。
- 本机 Windows 在沙箱内 `curl`/`cargo` 走 schannel 会报 `SEC_E_NO_CREDENTIALS`：出网探测请用 `node probe/*.mjs`（Node 走 OpenSSL），cargo 依赖拉取需在完全权限会话执行。
- HTTP 客户端固定使用 **rustls**（`rustls-no-provider` + ring），不要换 native-tls。

## 目录

```
src/
├─ main.rs        子命令分发 serve / mcp / search / health / doc
├─ app.rs         AppState + 编排器构造（三入口共用）
├─ cli.rs  mcp.rs API/CLI/MCP 三个薄壳
├─ api/           POST /v1/search、GET /health
├─ engines/       Engine trait + so360 + bing
└─ core/          orchestrator、relevance、filter、fusion、score、dedupe、normalize
probe/            Node 探针与评测（可达性、相关性、精度）
integrations/     各客户端 MCP 配置、Skill、DSH bundle
docs/             计划、架构、精度手册
```

## License

MIT
