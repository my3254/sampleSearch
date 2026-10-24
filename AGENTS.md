# AGENTS.md — mysearch

**当前状态**：**M0 + 接入层已落地并验证（2026-10-24）**——可编译、可运行、可搜索；**同一二进制三种入口**：`serve`（HTTP）/ `mcp`（stdio MCP server）/ `search|health|doc`（CLI）。M1（路由表 + L1 声明式渠道 + 热加载）待做。

## 项目用途

自建、零 API 费用的 AI 搜索聚合服务：本地起一个 HTTP 服务，把免费公开数据源（Bing、360、GitHub、HN、StackExchange、npm、PyPI、arXiv、Crossref、国内行情、IT之家/新浪/少数派 RSS、政府网政策等）聚合成结构化结果，供自己与 AI Agent 通过 HTTP、MCP 或 CLI 调用。

参考产品 AnySearch（`anysearch.com`）。接口对齐其契约：`POST /v1/search`、`GET /v1/sub-domains`、`POST /v1/extract`，响应信封 `{code, message, request_id, data:{results[], metadata}}`。

**已确认决策**：纯自用 · 本机 Windows · HTTP API + MCP 同交付 · **Rust** · **不接任何需要反爬的源** · **海外源默认关闭、一个开关打开** · **不"都搜一遍"（1 主 + ≤2 附加 + 不足才兜底，上限 4 请求）**。

**完整设计**：[实施计划](docs/2026-10-24-mysearch-plan.md)（源矩阵、路由、里程碑） + [架构设计](docs/2026-10-24-architecture.md)（插件化渠道、热加载、打分体系） + [精度手册](docs/2026-10-24-precision.md)（过滤链、概念匹配、实测数字）。

## Agent 接入（三种入口，共用同一核心）

| 入口 | 命令 | 适用 | 常驻 |
| --- | --- | --- | --- |
| MCP (stdio) | `mysearch mcp` | Claude Code / Cursor / Windsurf / Codex / DSH 等 MCP 客户端 | 否（客户端拉起） |
| CLI | `mysearch search "…" -m 5` | 任何能执行命令的 agent | 否 |
| HTTP | `mysearch serve` | 多 agent 共享 / 非 Agent 程序 | 是 |

三个入口都直接调用 `Orchestrator::search`，**行为一致**：CLI 与 HTTP 返回同一个 AnySearch 信封，MCP 额外给 markdown + `structuredContent`。

- MCP 工具：`search`（query / max_results / explain）、`batch_search`（1–5 条）、`health`。
- 协议版本协商支持 `2024-11-05` / `2025-03-26` / `2025-06-18`；日志一律走 stderr，stdout 只放 JSON-RPC。
- 各客户端配置片段、Skill 安装位置见 [integrations/README.md](integrations/README.md)；给 agent 读的说明书是 [integrations/skill/SKILL.md](integrations/skill/SKILL.md)。
- 验证 MCP：`Get-Content integrations\mcp-session.jsonl | .\target\release\mysearch.exe mcp`

## 扩展方式（加渠道/调权重前先看这里）

| 想做的事 | 改哪里 | 重编译 | 生效 |
| --- | --- | --- | --- |
| 加 RSS/JSON/HTML 源（**首选**） | `config/engines/<id>.json`（L1 声明式） | ❌ | 热加载 |
| 调源权重/限流/超时/选择器 | 同上 | ❌ | 热加载 |
| 需要 GBK/签名/多步请求的源 | `src/engines/native/<id>.rs` + `native_engines()`（L2） | ✅ | 重启 |
| 非 Rust 或需进程隔离 | 外挂进程（L3，`POST /search` + `GET /health`） | ❌ | 热加载 + 健康检查 |
| 改路由信号 | `config/routing.json` | ❌ | 热加载 |
| 调打分权重/profile | `config/scoring.json` | ❌ | 热加载 |
| 加新打分特征 | `src/core/features/<id>.rs` 实现 `Feature` | ✅ | 重启 |

热加载入口：文件监听、`POST /admin/reload`（仅回环）、SIGHUP。校验失败一律**保留上一版**并在 `/health` 报 `config_error`，不允许半更新。

## 项目结构

```
mysearch/
├─ AGENTS.md
├─ docs/2026-10-24-mysearch-plan.md   实施计划 v0.4
├─ docs/2026-10-24-architecture.md    架构基线：插件化渠道 / 热加载 / 打分体系
└─ probe/                             Node ESM 探针（沙箱内唯一可靠出网通道）
   ├─ endpoints.mjs   15 个通用/海外源可达性
   ├─ extract.mjs     AnySearch 画像 + 各引擎 HTML/RSS 结构
   ├─ vertical.mjs    14 个结构化源
   ├─ verticals2.mjs  legal / news / social 第一批
   ├─ verticals3.mjs  RSSHub / 其余候选
   ├─ relevance.mjs   引擎相关性对照（8 条查询，得出 360 > Bing 的结论）
   ├─ bing-variants.mjs / bing-cookies.mjs   Bing 参数与 cookie 是否修复塌缩
   ├─ so360-urls.mjs  360 真实 URL 提取（data-mdurl）
   ├─ en-quality.mjs  英文查询在 Bing/360 的质量
   ├─ api.mjs         AnySearch 信封 + Bing 结构 + RSS 稳定性
   ├─ eval.mjs        **精度评测：9 条查询的 precision / 返回条数 / 过滤原因**
   ├─ e2e.mjs         本地服务端到端自测（/health + 3 条查询 + 参数校验）
   ├─ crates.mjs      Rust 依赖版本核查
   ├─ plugin-crates.mjs  插件/打分相关 crate 版本（arc-swap、notify、jsonschema、serde_json_path…）
   └─ anysource*.mjs  读取 AnySearch 仓库 README / doc_spec / 常量
```

现状源码结构：

```
src/
├─ main.rs          子命令分发：serve / mcp / search / health / doc
├─ app.rs           AppState + build_orchestrator/build_state（三入口共用同一核心）
├─ cli.rs           CLI 参数解析 + `doc` 文案 + 统一参数校验
├─ mcp.rs           手写 MCP stdio（initialize / tools.list / tools.call / ping）
├─ config.rs  model.rs
├─ api/{mod,search,health}.rs
├─ engines/{mod,so360,bing}.rs
└─ core/{mod,orchestrator,gates,relevance,filter,dedupe,fusion,score,normalize}.rs
```

M1 计划新增：`core/router.rs`（routing.json）、`engines/{declarative,external}.rs`、`engines/native/quote_cn.rs`、`core/{cache,ratelimit}.rs`、`core/features/*`、`admin/`、`extract/`、`config/*`。集成物在 `integrations/`（`README.md` 各客户端配置、`skill/SKILL.md`、`mcp-session.jsonl` 验证样本）。

## 技术架构

- Rust stable（1.98）。**M0 实际依赖**：tokio 1.54 · axum 0.8 · tower-http 0.7（仅 trace）· reqwest 0.13（`rustls-no-provider` + json/gzip/charset）· rustls 0.23（ring 后端）· scraper 0.29 · serde/serde_json · async-trait · url · anyhow · thiserror · tracing · tracing-subscriber。
- **计划引入（尚未加）**：moka + redb（缓存）、governor（限流）、jsonschema + serde_json_path（L1 spec 校验）、notify + arc-swap（热加载）、encoding_rs（GBK）。
- **刻意不引入**：`rmcp`（MCP SDK）与 `clap` —— MCP stdio 只用到 4 个方法，手写约 200 行即可，且能保证 stdout 只有 JSON-RPC、不受 SDK 版本变化影响；CLI 参数只有一个查询串和两个开关，手写解析足够。
- 分层：HTTP 层 → Router（选引擎）→ Orchestrator（并发 → 相关性闸门 → 归一化 → 去重 → RRF → rerank → MMR → 不足则兜底）→ 引擎适配层 → 上游。
- 契约：`code/message/request_id/data{results,metadata}`；额外字段 `metadata.routing`、`metadata.cached`、`metadata.sources` 为增量，客户端可忽略。

**引擎选择（不都搜一遍）**：显式 `domain/sub_domain` → 只打该域引擎；否则按 `config/routing.json` 的信号表选 1 主 + ≤2 附；去重后不足 `max_results×0.5` 才追加兜底；单次上游请求上限 4。中文通用主引擎是 **360**（实测相关性 8/8，Bing 5/8 且存在"实体塌缩"），Bing 降为附加/解析兜底。`?explain=1` 可看路由决策。

**反爬边界**：验证码绕过 / cookie 池 / 签名逆向 / 登录态——任一命中即不接。因此百度、搜狗网页版、微博、知乎、贴吧、B站、裁判文书网均排除。

**海外源**：`intl.enabled` 默认 `false`，关闭时不构造、不探测、不消耗请求；开启并配 `proxy` 后，英文查询前置 ddg/brave，中文查询仅作附加，代理只作用于 intl 引擎。

**核心模块职责**

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `src/api/*` | 路由、参数校验、错误码信封、request_id | ✅ 已实现 |
| `src/app.rs` | `build_orchestrator` / `build_state`，三入口共用 | ✅ 已实现 |
| `src/cli.rs` | CLI 参数解析、`doc` 文案、统一校验 | ✅ 已实现 |
| `src/mcp.rs` | MCP stdio：initialize / tools.list / tools.call / ping | ✅ 已实现 |
| `src/core/orchestrator.rs` | 选源、并发、闸门、融合、打分、组装 | ✅ 已实现 |
| `src/core/{relevance,filter}.rs` | 概念匹配；7 道精度过滤链 | ✅ 已实现 |
| `src/core/{dedupe,fusion,score,normalize,gates}.rs` | 去重、RRF、特征打分、URL 规范化、语言判定 | ✅ 已实现 |
| `src/engines/{mod,so360,bing}.rs` | Engine trait + 两个原生适配器 | ✅ 已实现 |
| `src/core/router.rs` | 信号表 → EnginePlan（routing.json） | ⏳ M1 |
| `src/engines/{declarative,external}.rs` | L1 声明式渠道 / L3 外挂渠道 | ⏳ M1 |
| `src/core/{cache,ratelimit}.rs`、`core/features/*` | 缓存、限流、可插拔特征 | ⏳ M1 |
| `src/engines/native/quote_cn.rs` | 行情（GBK） | ⏳ M2 |
| `src/extract/*` | `/v1/extract`：安全取页 + 正文转 Markdown | ⏳ M2 |
| `src/admin/*` | `/admin/reload`、`/admin/engines`（仅回环） | ⏳ M1 |
| `src/config/*` | spec 校验 + notify 热加载 | ⏳ M1 |

## 安装、启动、构建、测试（已验证）

依赖已 `cargo vendor` 到 `vendor/`（206 个 crate），`.cargo/config.toml` 做了 source replacement + `offline = true`。**必须把 `CARGO_HOME` 指到工作区内**（`~/.cargo` 在沙箱中只读）：

```powershell
$env:CARGO_HOME='D:\software\idea\mysearch\.cargo-home'
cargo build          # ~3s（改代码后增量）
cargo test           # 29 个单测，全绿
cargo run            # 服务监听 127.0.0.1:8787
```

接口自测（Node 起，沙箱内唯一可靠出网通道）：

```powershell
node probe\e2e.mjs   # /health + 3 条真实查询 + 参数校验
```

```powershell
# 手工调用
curl.exe -s -X POST http://127.0.0.1:8787/v1/search -H "content-type: application/json" -d "{\"query\":\"大模型 推理 优化\",\"max_results\":5}"
```

环境变量：`MYSEARCH_BIND`、`MYSEARCH_TIER_A_TIMEOUT_MS`、`MYSEARCH_TIER_B_TIMEOUT_MS`、`MYSEARCH_MAX_UPSTREAM`、`MYSEARCH_DEFAULT_MAX`、`MYSEARCH_INTL`（默认 0）、`MYSEARCH_PROXY`。

### 已实现（M0）

- `POST /v1/search`：AnySearch 兼容信封（`code/message/request_id/data{results,metadata}`），`metadata.routing`/`sources` 为增量字段；`?explain` 可通过请求体 `explain:true` 打开逐条特征分。
- `GET /health`：引擎名册、uptime、intl 开关状态。
- 引擎：`so360`（HTML，`data-mdurl` 取真实 URL）、`bing`（HTML，失败降级 RSS；RSS 用手写极简解析，无 XML 依赖）。
- 管线（精度优先，顺序固定）：URL 规范化去追踪参数 → **引擎级闸门**（top3 无概念命中 → degraded，权重 ×0.3）→ RRF 融合 → 标题近重复去重 → **7 道过滤链**（noise_host / aggregator_title / portal_root / low_match / title_mismatch / entity_page / script_mismatch / dup_content）→ 6 特征加权打分（rrf、coverage、title_coverage、phrase、authority、length）→ 同域 ≤3 → 截断。
- **概念匹配**：查询切组 + 去停用词 + 去修饰前缀（"最新政策"→"政策"）；1 组需 1 命中，2 组需 2 命中，3–4 组需 2，5+ 组需 ⌈n/2⌉；ASCII 整词优先、中文允许 2-gram 覆盖 ≥0.6。
- 过滤原因回传在 `metadata.filters`（`[{reason,count}]`），`explain:true` 回传每条结果的分数贡献。
- 参数校验：query 1..=512 字符、max_results 1..=10，错误返回 400 + `{code:400}`。
- 实测精度：`node probe/eval.mjs` → 过滤前 0.79（34/43）→ 过滤后 **1.00（35/35）**，平均返回 3.9/5（召回靠 M1 加源补，不靠放宽过滤）。

## 开发注意事项（重要）

1. **沙箱内禁止用 `Invoke-WebRequest`、`curl.exe`、`cargo` 直接出网**：受限令牌下 schannel 报 `SEC_E_NO_CREDENTIALS`。出网统一走 Node。
2. `~/.rustup`、`~/.cargo` 在工作区内**只读**；`CARGO_HOME` 必须指向工作区内（`.cargo-home`），并用 `vendor/` + `--offline` 构建。
3. **HTTP 客户端必须用 rustls，禁止 native-tls**：reqwest 0.13 的写法是 `default-features = false, features = ["rustls-no-provider", ...]`，并在 `main` 里先调用 `rustls::crypto::ring::default_provider().install_default()`；`rustls` 依赖 **必须用 ring**（`default-features=false, features=["ring","std","tls12"]`），不能用默认的 aws-lc-rs（本机无 C 工具链）。
4. 上游限流必须尊重：GitHub 未登录 10 req/min；bing 5 req/s、hn 1 req/s、arxiv 1 req/3s。
5. 测试不打真网：新增引擎必须带 `tests/fixtures/` 录制样本；新增渠道先跑 `mysearch engine check <id>`，字段覆盖率 <90% 或频繁 0 条不要入库。
6. **新渠道优先写 L1 声明式 spec**（`config/engines/*.json`），只有 GBK/签名/多步请求才写 L2；配置里的 `meta.hosts` 是硬白名单，不得绕过。
7. **调打分权重必须用 `mysearch eval` 给前后对比**，禁止手调后不做回归；`tests/golden/ranking_*.json` 是排序契约，改动需说明原因。
8. `/admin/*` 默认仅回环可访问，不得暴露到公网；外挂渠道默认只允许 `127.0.0.1`。
9. 合规边界：个人自用、不公开、不商用、低频；只缓存结果元数据与短摘要，不缓存全文；结果一律附来源链接。
10. 中文网页注意编码：腾讯行情等为 GBK，用 `encoding_rs` 解码；其余按 `content-type` 判断。
11. 网络可达性随环境变化：无代理时 Google/DDG/Brave/知乎/微博/V2EX/贴吧/RSSHub/裁判文书网均不可用。改引擎清单前先跑 `probe/`。
12. 本机曾出现工作区目录权限问题（DSH 沙箱无法授予写权限），已修复；回滚脚本在 `D:\software\idea\mysearch-acl-recovery\`。

## 已确认的事实（2026-10-24 实测）

- AnySearch：三端点、信封字段、17 个垂直域、匿名可用、免费额度口径。
- 沙箱内：schannel / cargo 出网失败；Node `fetch` 正常；`node:sqlite`、`node:test` 可用；npm registry 可达。
- 完全权限下：`curl` 200；`cargo build` 49.76s 通过（含 ring/rustls/native-tls，无需 MSVC）。
- Rust 1.9x 自带 `rust-lld`，无 MSVC 也能链接。
- 可达源与延迟见计划 §2.2；不可达源清单同上。

## 待确认

- `flk.npc.gov.cn`（国家法律法规库）API 参数与加密方式；豆瓣 HTML 是否可用（可能登录墙）。
- 代理若日后提供，需重跑 `probe/endpoints.mjs` 并启用 `intl_enabled`。
