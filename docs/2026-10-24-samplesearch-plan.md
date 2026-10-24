# samplesearch：自建 AI 搜索基础设施 — 实施计划

- 版本：v0.4（新增：引擎路由策略、反爬边界、海外源开关；据实测调整主引擎）
- 日期：2026-10-24
- 工作区：`D:\software\idea\samplesearch`
- 环境：Windows / rustc 1.98.0 / Node v22.22.2 / DSH 沙箱 workspace-write

---

## 0. 结论与决策

**做。** 免费公开数据源 + 一层聚合与路由，自托管一个接口对齐 AnySearch 的搜索服务：
`POST /v1/search`、`GET /v1/sub-domains`、`POST /v1/extract`，同时提供 MCP server 与 CLI。

| 决策项 | 结论 | 说明 |
| --- | --- | --- |
| 用途 | 纯自用，不对外 | 省掉鉴权/计费/多租户 |
| 部署 | 本机 Windows | — |
| 语言 | **Rust**（rustc 1.98 可用，无需 MSVC） | §3 |
| TLS | **只用 rustls，禁用 native-tls** | 沙箱内 schannel 不可用 |
| 交付形态 | HTTP API + MCP 同时 | — |
| **反爬源** | **一律不做**（验证码绕过 / cookie 池 / 签名逆向 / 登录态，任一命中即排除） | §7.1 |
| **海外源** | **默认关闭，一个开关打开**（`intl.enabled`），开启后可配代理 | §7.5 |
| 引擎选择 | **不"都搜一遍"**：1 主 + ≤2 附加 + 不足才兜底，上限 4 个请求 | §7.2 |
| 主引擎（中文） | **360（so.com）**；Bing 降为附加/兜底 | 实测 Bing 3/8 查询"实体塌缩"，360 8/8 命中（§2.3） |

---

## 1. 参考产品拆解（AnySearch，2026-10-24 实测）

来源：`anysearch.com`、GitHub `anysearch-ai/anysearch-skill`（6753★，Apache-2.0）、其 `scripts/shared/doc_spec.md`。

- 通用搜索 + 17 个垂直域（`general, resource, social_media, finance, academic, legal, health, business, security, ip, code, energy, environment, agriculture, travel, film, gaming`）。
- 垂直搜索先 `GET /v1/sub-domains` 取目录与参数 schema，再带 `sub_domain` + `params` 搜索。
- `batch_search` 1–5 条并发，逐条独立限流/配额。
- `POST /v1/extract` 返回 Markdown（HTML/文本/JSON/Markdown；50k 截断；PDF 等不支持）。
- 接入：API / MCP / Skill；匿名可用，邮箱注册即发 key，宣称每日 1000 次免费。

```json
{"code":0,"message":"success","request_id":"46031268-…",
 "data":{"results":[{"title":"…","url":"https://…","snippet":"…","content":"…"}],
         "metadata":{"total_results":2,"search_time_ms":1616}}}
```

请求参数：`query`（必填）、`max_results`（1–10）、`tag` 或 `domain`+`sub_domain`、`params`、`zone`(`cn`/`intl`)、`language`；`Authorization: Bearer <key>` 可选。

---

## 2. 环境与源实测

### 2.1 沙箱对出网的影响

| 场景 | 结果 |
| --- | --- |
| 沙箱内 `curl.exe` / `Invoke-WebRequest` / `cargo` | TLS 失败 `SEC_E_NO_CREDENTIALS`（受限令牌下 schannel 无凭据） |
| 沙箱内 Node `fetch` | ✅ 正常（OpenSSL），baidu/github/crates.io 均 200 |
| **完全权限**下 `curl` | ✅ 200 |
| **完全权限**下 `cargo build`（含 ring/rustls/native-tls） | ✅ 49.76s 通过，**不需要 MSVC** |

→ 开发期出网统一走 Node；Rust 依赖先 `cargo vendor` 再 `--offline` 构建；服务自身必须用 rustls。

### 2.2 源可达性（无代理，2026-10-24）

| 源 | 状态 | 延迟 | 解析 | 用途 |
| --- | --- | --- | --- | --- |
| `so.com`（360） | 200 | ~390ms | `li.res-list`，**`data-mdurl` 直给真实 URL** | **中文通用主引擎** |
| `cn.bing.com` HTML | 200 | ~300ms | `li.b_algo` 10 条 + `b_lineclamp` 摘要 | 通用附加/兜底 |
| `cn.bing.com` RSS | 200 | 145–290ms | 10 条 `<item>` | 解析兜底（与 HTML 同源） |
| GitHub Search | 200 | ~1.8s | JSON，10 次/分 | code |
| HN Algolia | 200 | ~1.9s | JSON | code/social |
| StackExchange | 200 | ~2.2s | JSON | code/QA |
| npm / PyPI | 200 | ~1–1.8s | JSON | code/pkg |
| arXiv Atom / Crossref | 200 | ~1.7s | XML / JSON | academic |
| 腾讯行情 / 东财 | 200 | ~150ms | 文本(GBK)/JSON | finance.quote |
| IT之家 RSS | 200 | 111ms | 60 条 `<item>` | news |
| 新浪 RSS / 少数派 RSS | 200 | ~200ms | XML | news |
| 中国政府网·最新政策 | 200 | 122ms | HTML 21KB | legal |
| 澎湃首页 | 200 | 296ms | 仅 10 条链接、5 个标题（门户页，非搜索） | news（限量补充，非按词检索） |
| 百度网页 / 搜狗网页 / 百度新闻 | 200 但"安全验证"空页 | — | 反爬 | ❌ 不做 |
| 知乎 401 / 微博 403 / 贴吧 403 / B站 412 | — | — | 需登录/签名 | ❌ 不做 |
| V2EX / Reddit / RSSHub / 豆瓣(待验) | 超时 / 登录墙 | — | — | ❌ 无代理不可用 |
| 裁判文书网 | 405（POST+加密） | — | — | ❌ 不做 |
| Google / DDG / Brave / Startpage / SearXNG / 维基 | 超时 | — | — | 由 `intl.enabled` 开关控制 |
| OpenAlex / Semantic Scholar | 429 | — | 共享 IP 限流 | 需 key，v2 |

**RSS 不是搜索**：IT之家/新浪/少数派是"最新条目流"，只能本地按词过滤，语义＝"最近新闻里命中关键词的条目"。`news` 域必须如实标注这一点。

### 2.3 相关性实测（决定谁当主引擎）

8 条中英混合查询，指标：top-3 标题是否命中查询词。

| 查询 | Bing(HTML/RSS) | 360 |
| --- | --- | --- |
| 大模型 推理 优化 | **0/3**（结果全在讲汉字"大"） | 3/3 ✅ |
| vLLM 部署 显存 | 2/3 | 3/3 ✅ |
| Rust 异步 运行时 | 3/3 | 3/3 ✅ |
| 2026 新能源 汽车 销量 | 3/3（返回"2026年"百科） | 3/3 ✅ |
| 劳动法 试用期 规定 | 3/3 | 3/3 ✅ |
| tokio select 用法 | **0/3**（返回 Tokio 官网首页） | 2/3 ✅ |
| nvidia 财报 2026 | **0/3**（返回 NVIDIA 官网首页） | 3/3 ✅ |
| 台风 路径 上海 | 3/3 | 3/3 ✅ |
| **合计** | **5/8 可用，3 次"实体塌缩"** | **8/8** |

英文查询两边都差：`rust async runtime` → Bing 给菜鸟教程、360 给论文页；`windows sandbox acl` → 360 给"360翻译"、Bing 给 Windows 下载页。→ 技术类英文交给 GitHub/HN/StackExchange，通用英文靠 `intl` 开关。

**结论**：中文通用主引擎换成 360；Bing 保留为附加与解析兜底；新增**相关性闸门**（§7.4）兜住"塌缩"这类静默降级——它不报错，只给你一屏无关结果。

---

## 3. Rust 技术路线

### 3.1 性能预期

瓶颈是上游 RTT（150ms–2.2s）。解析 10–30 条、去重、RRF 都是毫秒级。Rust 带来的是单 exe、~20MB 常驻、长期不抖动，不是"搜得更快"。

### 3.2 工具链（已实测）

| 项 | 状态 |
| --- | --- |
| rustc / cargo | 1.98.0，host msvc |
| MSVC link.exe / cl.exe | 不存在；Rust 1.9x 自带 `rust-lld`，链接 OK |
| `ring 0.17.14` | 无需 cl.exe 即可编译 |
| crates 下载 | 沙箱内不可，完全权限可 |
| 依赖版本 | axum 0.8.9 · tokio 1.54.1 · reqwest 0.13.5 · scraper 0.29 · serde 1.0.229 · quick-xml 0.42 · moka 0.12 · redb 4.4 · governor 0.10 · rmcp 3.7 · clap 4.6 · encoding_rs 0.8 · async-trait 0.1.92 · arc-swap 1.9 · notify 8.2 · jsonschema 0.60 · serde_json_path 0.7 · chrono 0.4 |

### 3.3 依赖与 TLS

```powershell
$env:CARGO_HOME='$env:CARGO_HOME="$PWD\.cargo-home"'
cargo vendor vendor      # 完全权限下执行一次
cargo build --offline    # 之后沙箱内可重复
```

`reqwest --no-default-features --features rustls-tls,json,gzip,charset`；禁止 native-tls。正文抽取用 `scraper` + 文本密度打分 + `htmd` 转 Markdown（Rust 的 readability 移植版质量一般，接受降级）。

---

## 4. 目标与非目标

**目标**：一条命令起服务；契约对齐 AnySearch；中英查询可用；P95<2.5s；零付费 key；MCP+CLI；缓存/限流/熔断/指标齐备。

**非目标**：不对外、不全域覆盖、不建索引、不翻页、**不绕过任何反爬**、不承诺与商业索引同质量。

---

## 5. 架构

> 扩展性与打分细节见独立文档：[架构设计：可插拔渠道 + 可扩展打分](2026-10-24-architecture.md)。
> 要点：渠道分 L1 声明式（改 JSON 热加载）/ L2 原生（编译）/ L3 外挂进程（加 URL 热加载）；排序为固定 7 段骨架 + 可插拔 `Fusion`/`Feature`/`Diversifier`。

```
Agent / CLI / curl / MCP client
        │
┌───────▼──────────────────────────────────────────────┐
│ HTTP 层   axum 0.8 + tower-http（trace / request-id）  │
│  /v1/search · /v1/sub-domains · /v1/extract           │
└───────┬──────────────────────────────────────────────┘
        │ SearchRequest
┌───────▼──────────────────────────────────────────────┐
│ Router（§7）                                           │
│  显式域? → 锁定域引擎 : 信号打分 → 主引擎 + 附加引擎     │
└───────┬──────────────────────────────────────────────┘
        │ EnginePlan { tier_a, tier_b, tier_c, budget }
┌───────▼──────────────────────────────────────────────┐
│ Orchestrator                                          │
│  JoinSet 并发 → 相关性闸门 → 归一化 → 去重 → RRF        │
│  → rerank → MMR → 不足则触发 tier_c → 截断              │
└───────┬───────────────────────────┬───────────────────┘
        │                           │
┌───────▼──────────┐   ┌────────────▼──────────────────┐
│ engines/          │   │ cache(moka+redb) / ratelimit  │
│ Engine trait      │   │ (governor) / health / metrics │
└───────┬──────────┘   └───────────────────────────────┘
        │ reqwest + rustls（可选代理）
```

### 5.1 目录结构

```
src/
├─ main.rs  config.rs  model.rs
├─ api/{mod,search,subdomains,extract}.rs
├─ core/{orchestrator,router,gates,dedupe,merge,rerank,cache,ratelimit,health}.rs
├─ engines/{mod,bing,so360,github,hn,stackexchange,npm,pypi,arxiv,crossref,
│           quote_cn,ithome,sina,sspai,gov_cn,ddg,brave,wikipedia}.rs
├─ extract/{fetch,readabl,md}.rs
├─ vertical/subdomains.rs
├─ mcp/server.rs
└─ cli.rs
tests/{fixtures,eval}
```

---

## 6. API 契约

```jsonc
// POST /v1/search
{ "query": "600519 财报", "max_results": 5, "zone": "cn", "language": "zh-CN",
  "domain": "finance", "sub_domain": "finance.quote", "params": {"symbol":"600519"} }

// 200
{ "code": 0, "message": "success", "request_id": "…",
  "data": { "results": [{"title":"","url":"","snippet":"","content":""}],
            "metadata": { "total_results": 5, "search_time_ms": 640, "cached": false,
              "routing": {"intent":"finance","tier_a":["quote_cn"],"tier_b":["so360"],
                          "tier_c":[],"degraded":[]},
              "sources": [{"name":"quote_cn","count":1,"ms":152,"status":"ok"},
                          {"name":"so360","count":5,"ms":380,"status":"ok"}] } } }
```

`metadata.routing` 是**路由解释**（AnySearch 无此字段，属增量）；`?explain=1` 时每条结果额外带 `signals`。

| code | 场景 |
| --- | --- |
| 0 | 成功 |
| 400 | 参数非法 / 未知 sub_domain / 不支持的 content-type |
| 404 | 路由不存在 |
| 429 | 本地限流 |
| 502 | 全部上游失败（部分成功仍 200 + degraded） |
| 504 | 全局超时 |
| -1 | 内部错误 |

其他：`GET /health`（各引擎成功率/延迟/熔断状态）、`GET /metrics`、`GET /v1/sub-domains?domain=x&format=markdown`。

---

## 7. 引擎选择策略（回答"什么时候用哪个源、要不要都搜一遍"）

### 7.1 先划边界：什么源根本不接（反爬）

**判据（命中任一条即排除）**：需要验证码绕过 / 需要 cookie 池或账号池 / 需要逆向签名或 token / 需要登录态。

| 排除 | 原因 |
| --- | --- |
| 百度网页、搜狗网页、百度新闻 RSS | 返回"安全验证"空页，需绕过 |
| 微博、知乎、贴吧、B站、抖音、小红书 | 需登录/签名（401/403/412） |
| 裁判文书网 | POST + 加密参数 |
| 需破解的公共 SearXNG / RSSHub 镜像 | 多为绕过型，且不可达 |

**保留**（无验证码、无登录、无签名逆向）：360、Bing 公开结果页、GitHub/HN/StackExchange/arXiv/Crossref/npm/PyPI 官方 API、IT之家/新浪/少数派官方 RSS、政府网静态页、腾讯/东财公开行情端点。

> 区分两件事：**Bing 的"实体塌缩"不是反爬**——它照常 200、照常给 10 条，只是内容无关，用 §7.4 的闸门处理；"需要反爬"是指必须绕过防护才拿得到内容——那类直接不接。

### 7.2 核心原则：不都搜一遍

每次都扇出到 12 个源，代价是实打实的：

| 策略 | 上游请求/次 | 端到端 P95 | GitHub 配额 | 结果质量 |
| --- | --- | --- | --- | --- |
| 全搜 12 源 | 12 | ~2.2s（最慢者决定） | 10 分钟内打空（10 次/分） | 大量无关条目，靠重排硬滤 |
| **本文路由** | **1–3**（典型 2） | **~1.2s** | 仅代码类查询触发 | 高（实测 8/8 命中） |

**三层决策**：

```
L0 请求里带了 domain/sub_domain
    → 只打该域的引擎（1–2 个），不做意图猜测
L1 没带
    → 信号打分（正则/字符统计，微秒级，不调模型）
    → 选出 1 个主引擎(tier_a) + 0–2 个附加(tier_b)，并发发出
L2 结果不够
    → 去重后条数 < ceil(max_results × 0.5) 且未超预算
    → 追加 tier_c 兜底（最多 2 次，串行）
```

**预算**（可配）：全局软截止 2500ms；tier_a 硬超时 1200ms；tier_b 1800ms；tier_c 1200ms；**单次请求上游上限 4 个**；缓存命中则 0 个请求。

### 7.3 信号 → 引擎（v1 规则表，自上而下首个命中生效）

| # | 信号（正则/统计） | tier_a 主 | tier_b 附加 | 备注 |
| --- | --- | --- | --- | --- |
| 1 | 显式 `domain`/`sub_domain` | 该域引擎 | 无 | 不做意图猜测 |
| 2 | `site:` 限定 | 360(中) / bing(en) | 无 | 尊重站点限定，不加垂直源 |
| 3 | 股票：`\b(6\d{5}\|0\d{5}\|3\d{5})\b`、`\$[A-Z]{1,5}`、`财报\|市盈率\|涨停\|ETF` | `quote_cn` | `so360` | 行情 + 中文解读 |
| 4 | 代码：`Error:\|Exception\|Traceback\|npm i \|pip install\|cargo \|fn \|#issue\|\.(py\|ts\|rs\|go)\b` | `stackexchange` | `hn`,`github` | GitHub 仅在前两条命中 repo/包语义时加入 |
| 5 | 学术：`arxiv\|doi\|paper\|survey\|论文\|综述\|et al\.\|20\d\d\.\d{4,5}` | `crossref`+`arxiv` | `so360` | 429 退避并降级 |
| 6 | 新闻：`最新\|今天\|刚刚\|本周\|发布会\|通报\|news` 或"当年年份+事件词" | `ithome`（本地词过滤） | `so360`,`sina` | RSS 语义＝最新条目过滤 |
| 7 | 法律：`法条\|民法典\|第\d+条\|司法解释\|劳动合同\|行政处罚\|判例` | `gov_cn` | `so360` | 政策/法规页 |
| 8 | 中文（CJK 字符占比 ≥30%） | **`so360`** | `bing` | 主引擎依据 §2.3 实测确定 |
| 9 | 其余（英文/符号） | `bing` | `hn`,`github`（技术词命中时） | `intl.enabled` 时前置 `ddg`/`brave` |

规则表是**数据不是代码**：放 `config/routing.json`，支持热加载；`?explain=1` 返回命中的规则号、选中的引擎与理由。

### 7.4 相关性闸门（对付"静默降级"）

每个引擎返回后立即计算：`hits = top3 中命中查询词（CJK 2-gram 或 ASCII 词）的条数`

- `hits = 0` → 该引擎标 `degraded`，RRF 权重 ×0.3，并在 `metadata.sources` 显式标注；
- 主引擎 degraded 且 tier_b 结果不足 → 立即触发 tier_c 换另一个通用源重试一次；
- 全部 degraded → 仍 `code=0`，但 `metadata.degraded_all=true`（不假装成功）。

这条规则正是为今天实测到的 Bing 塌缩设计的：它不报错，只返回一屏无关结果，只有闸门能发现。

### 7.5 海外源开关

```jsonc
// config/routing.json（节选）
{ "intl": { "enabled": false, "proxy": null, "engines": ["ddg", "brave", "wikipedia"] } }
```

| 状态 | 行为 |
| --- | --- |
| `enabled: false`（默认） | intl 引擎不构造、不探测、不出现在 `/health`、不消耗请求；代码里只留桩 |
| `enabled: true` + `proxy` | 英文查询时 intl 进 tier_a（优先级 ddg > brave > bing）；中文查询时仅作 tier_b；代理失败自动回退国内源并在 `/health` 报警 |
| 代理范围 | `reqwest::Proxy::all(...)` **只作用于 intl 引擎**，国内源直连（避免无谓绕行） |

`zone` 与开关互不影响：`zone` 只改排序与语言倾向，`intl.enabled` 才决定"能不能出网找海外源"。

### 7.6 五个可验证的例子

| 请求 | 命中规则 | 实际发起 | 期望行为 |
| --- | --- | --- | --- |
| `大模型 推理 优化` | #8 中文 | 360 + Bing | Bing 闸门判 degraded，最终 5 条几乎全来自 360 |
| `TypeError: Cannot read properties of undefined` | #4 代码 | StackExchange + HN + GitHub | 中文通用源完全不参与 |
| `arxiv 2301.12345` | #5 学术 | arXiv + Crossref + 360 | 首条为论文页 |
| `600519 财报` | #3 金融 | 行情 + 360 | 行情数据 + 财报新闻 |
| `tokio select usage`（intl 关） | #9 英文技术 | Bing + HN + GitHub | Bing 若塌缩 → 闸门触发 tier_c(360) |

### 7.7 成本上限

- 上游请求 ≤ 4/次；同一引擎调用间隔 ≥ 配置（360 200ms、Bing 200ms、GitHub 6s、HN 1s、arXiv 3s）。
- 缓存：query+参数指纹，通用 5–10 分钟、结构化域 1–5 分钟；同 key 并发合并（singleflight）。
- 熔断：连续 5 次失败 → 冷却 5 分钟，Router 自动跳过，`/health` 可见。

---

## 8. 引擎清单（v1 落地）

| 域 | 引擎 | 角色 | 备注 |
| --- | --- | --- | --- |
| general | `so360` | **中文主引擎** | `li.res-list` + `data-mdurl` 真实 URL |
| general | `bing` | 附加/兜底 | HTML 优先，解析失败降级 RSS |
| code | `stackexchange`、`hn`、`github`、`npm`、`pypi` | 主/附 | GitHub 10 req/min |
| academic | `arxiv`、`crossref` | 主 | 429 退避 |
| finance | `quote_cn`（腾讯+东财） | 主 | GBK 解码 |
| news | `ithome`、`sina`、`sspai` | 主/附 | **RSS＝最新条目过滤，非全文搜索** |
| legal | `gov_cn` | 主 | 政策/法规页；`flk.npc.gov.cn` API 待验证 |
| social | `hn` | 主 | 知乎/微博/V2EX 不可用；不接反爬源 |
| intl（默认关） | `ddg`、`brave`、`wikipedia` | 条件 | 需 `intl.enabled=true`（+ 代理） |

垂直目录 v1：`code.repo`、`code.package`、`code.qa`、`academic.paper`、`finance.quote`、`news.tech`、`legal.policy`、`social.tech`。

---

## 9. 质量、缓存、限流

- 去重：URL 规范化（小写 host、去 `utm_*`/`spm`/fragment、排序 query）+ SimHash 近重复（汉明 ≤3）。
- 融合：RRF `Σ w/(k+rank)`，k=60；权重 so360 1.0、bing 0.8、垂直 0.9；degraded 引擎 ×0.3。
- 重排：词覆盖（中文 2-gram）、标题命中、权威域白名单、时效、zone 偏好。
- 多样性：MMR λ=0.7 + 同域 ≤3 条。
- 评测：`tests/eval/queries.json` ≥40 条（含 §2.3 的回归集），输出 top1/top3 命中率、平均延迟、平均上游请求数、degraded 率。

---

## 10. extract 安全边界

仅 http/https；拒绝私有/回环/链路本地地址（SSRF）；重定向 ≤5 次且每次重校验；≤5MB、超时 15s；content-type 白名单；Markdown 截断 50k；返回内容视为不可信数据。

---

## 11. 里程碑

| 阶段 | 内容 | 验收 | 预估 |
| --- | --- | --- | --- |
| **M0 解锁+骨架** ✅ | vendor 落盘（206 crates）、axum 骨架、`model.rs`、`Engine` trait、`so360`+`bing` 原生适配、`/v1/search`+`/health` 打通、22 个单测 | 2026-10-24 实测：中文查询返回真实 URL 结果；`cargo test` 22/22 绿；沙箱内离线构建 3.2s；`probe/e2e.mjs` 通过 | 1.5 天（已完成） |
| **M1 路由+聚合** | `router`+`routing.json`、**L1 声明式渠道 + 热加载**（`/admin/reload`）、相关性闸门、去重/RRF/特征打分/MMR、缓存、限流、熔断、`/health`、`/metrics` | §7.6 五个例子符合预期；新增一个 RSS 源只改 JSON；P95<1.5s；上游请求均值 ≤2.5 | 2.5 天 |
| **M2 垂直+抽取** | 9 个垂直源**用 L1 配置实现**（行情/Bing 留 L2）、`/v1/sub-domains`、`/v1/extract`、`engine check` 体检 | 六类查询各有结构化结果；加源零核心改动；extract 出 Markdown | 2 天 |
| **M3 Agent 接入** | `rmcp` stdio、CLI、`doc`、README、计划任务自启 | 在 DSH/Claude Code 挂 MCP 能搜到 | 1 天 |
| **M4 intl+质量** | `intl` 开关与代理、评测集、权重调优 | 关闭时零海外请求；开启后英文质量提升可量化 | 1 天 |

到可用版本（M0–M3）约 6.5 天。

---

## 12. 风险与对策

| 风险 | 影响 | 对策 |
| --- | --- | --- |
| 360 单点（中文唯一高质量源） | 挂掉即中文不可用 | Bing 兜底 + 闸门 + `/health` 报警 |
| Bing 实体塌缩 | 静默给无关结果 | 相关性闸门 + degraded 权重 + tier_c 换源 |
| HTML 结构变动 | 解析归零 | 结构断言（结果 ≥1）+ fixture 回归 + `probe/` 复核 |
| 抓公开结果页的 ToS 灰色地带 | 合规 | 只自用、低频、缓存、保留来源链接、不绕过任何防护 |
| 依赖拉取依赖完全权限 | 新增 crate 需再解锁 | `vendor/` 入库；升级依赖时重跑一次 |
| 沙箱 schannel | 运行时 TLS | 只用 rustls |
| RSS 不是搜索 | news 覆盖有限 | 接口与文档如实标注"最新条目过滤" |
| 正文抽取质量 | extract 一般 | 密度打分 + 降级纯文本 |

---

## 13. 下一步

M0 已完成（代码可跑，见 AGENTS.md 的构建/运行命令）。接下来 M1：

1. `config/routing.json` + `core/router.rs`：把 §7.3 的 9 条信号规则从代码里的"中英二分"升级为数据驱动，支持热加载。
2. `engines/declarative.rs` + `config/engines/*.json`：L1 声明式渠道（RSS/JSON/HTML 选择器 + JSONPath），第一批把 ithome / arXiv / Crossref / GitHub / StackExchange / 政府网迁过去，验证"加源不改码"。
3. `core/cache.rs`（moka + redb）+ `core/ratelimit.rs`（governor）+ `engines/external.rs`（L3 外挂协议）。
4. `/admin/reload`、`/admin/engines`、`/metrics`，以及 tier_c 兜底（当前是主+副并发一次，不做预算内追加）。

---

## 附录：探针

```powershell
node probe\endpoints.mjs <q>    # 15 个通用/海外源
node probe\vertical.mjs         # 14 个结构化源
node probe\verticals2.mjs       # legal/news/social 第一批
node probe\verticals3.mjs       # RSSHub 等候选
node probe\relevance.mjs        # 引擎相关性（8 条查询）
node probe\bing-variants.mjs    # Bing 参数/编码/新闻 RSS
node probe\bing-cookies.mjs     # cookie 能否修复塌缩（结论：不能）
node probe\so360-urls.mjs       # 360 真实 URL 提取（data-mdurl）
node probe\en-quality.mjs       # 英文查询在 Bing/360 上的质量
node probe\crates.mjs           # Rust 依赖版本
```

沙箱内禁止 `Invoke-WebRequest` / `curl.exe` / `cargo` 直接出网；统一 Node。
