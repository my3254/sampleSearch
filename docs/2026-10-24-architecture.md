# samplesearch 架构设计：可插拔渠道 + 可扩展打分

- 版本：v1.0（架构基线）
- 日期：2026-10-24
- 关联：[实施计划](2026-10-24-samplesearch-plan.md)、[AGENTS.md](../AGENTS.md)
- 目标：**新增搜索渠道不改核心代码、不重启进程；打分/排序可换、可调、可解释、可测**

---

## 0. 设计约束（先划清"热插拔"到底能做到什么）

Rust 没有稳定 ABI，`dlopen` 加载 Rust 动态库会随编译器版本崩，所以**不承诺"任意 Rust 代码热插拔"**。真实可交付的能力分三档：

| 档位 | 能力 | 生效方式 | 需要编译 |
| --- | --- | --- | --- |
| **L1 声明式渠道** | RSS / Atom / JSON API / HTML 选择器来源 | 改 JSON → 热加载 | ❌ |
| **L2 原生适配器** | 需签名、GBK、多步请求、自定义分页的源 | 编译进二进制 | ✅ |
| **L3 外挂渠道** | 任意语言/任意逻辑，独立进程 | 加 URL → 热加载 + 健康检查 | ❌ |

经验比例：常见搜索/数据源里 **L1 能覆盖 80%**，L2 留给"必须写代码"的少数，L3 用来保证"以后想接什么都能接"。

明确不采用：`libloading` 加载 Rust cdylib（ABI 不稳）、v1 引入 `wasmtime`（50.0.0 可用，但对本项目的收益/复杂度比不划算，留作 L3 的替代实现方案）。

---

## 1. 渠道（Engine）抽象

### 1.1 Trait

```rust
// src/engines/mod.rs
#[async_trait::async_trait]
pub trait Engine: Send + Sync {
    fn id(&self) -> &str;                       // 稳定标识，进 metadata.sources
    fn meta(&self) -> &EngineMeta;              // 域、zone、权重、间隔、hosts 白名单
    fn supports(&self, req: &SearchRequest) -> bool;
    async fn search(&self, req: &SearchRequest, cx: &EngineCtx)
        -> Result<EngineOutput, EngineError>;
}

pub struct EngineMeta {
    pub domains: Vec<Domain>,           // general / code / academic / finance / news / legal / social
    pub zones: Vec<Zone>,               // cn / intl
    pub weight: f32,                    // RRF 源权重（可由 profile 覆盖）
    pub min_interval: Duration,         // 上游礼貌间隔
    pub hosts: Vec<String>,             // 只允许访问这些 host（SSRF 与越权防护）
    pub kind: EngineKind,               // Declarative / Native / External
}

pub struct EngineOutput {
    pub items: Vec<RawItem>,
    pub stats: EngineStats,             // http_status、bytes、latency、parsed、assert_ok
}

pub struct RawItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub content: Option<String>,
    pub published: Option<DateTime<Utc>>,
    pub rank: u16,                      // 源内排名（1 起）
    pub extras: BTreeMap<String, String>, // 行情价、star 数、包版本等结构化附加
}
```

**错误分类**（决定行为，而不是笼统 `Err`）：

```rust
pub enum EngineError {
    Timeout,                       // → degraded，不重试
    Upstream { status: u16 },      // 5xx 退避重试 1 次；4xx 直接 degraded
    RateLimited { retry_after: Option<Duration> },
    Parse { reason: String },      // 结构断言失败 → degraded
    Blocked { evidence: String },  // 检测到验证码/空壳页 → 自动禁用 10 分钟并告警
    Config { reason: String },     // 只在加载期出现
}
```

`Blocked` 是刻意设计的：反爬源我们不做，但"上游突然返回验证码"必须能被识别并快速熔断，否则每次请求都白花 300ms。

### 1.2 注册表与热替换

```rust
pub struct Registry {
    engines: ArcSwap<Vec<Arc<dyn Engine>>>,      // arc-swap 1.9：读无锁
    index:   ArcSwap<HashMap<EngineId, Arc<dyn Engine>>>,
    profiles: ArcSwap<ScoringProfiles>,
    routing:  ArcSwap<RoutingTable>,
    revision: AtomicU64,                          // 每次热加载 +1，进 /health
}
```

替换流程（进程不重启、在途请求不受影响）：

```
notify 监听到 config/**/*.json   ┐
POST /admin/reload               ├→ 解析 → JSON Schema 校验 → 构造新引擎实例
SIGHUP / CLI reload              ┘        │
                                  校验失败 → 保留旧版本，/health 报 config_error（含行号/字段）
                                  校验通过 → 预热（hosts 校验、CSS 选择器预编译）
                                            → Registry::swap(new) → 旧 Arc 待引用归零后释放
                                            → /admin/reload 返回 {revision, engines_added, engines_removed}
```

热插拔语义，逐条说清：

| 操作 | 行为 | 需要重启 |
| --- | --- | --- |
| 新增 L1 渠道（`config/engines/xxx.json`） | 立即进入可选集，`/admin/engines` 可见 | 否 |
| 停用渠道（`enabled:false`） | Router 不再选中；已发出的请求自然完成 | 否 |
| 改权重 / 限流 / 超时 / 选择器 | 下一次请求生效 | 否 |
| 新增 L3 外挂 | 加 endpoint → 健康检查通过后入池 | 否 |
| 新增 L2 原生适配器 | 必须重新编译（诚实说明） | 是 |
| 改路由规则 / 打分权重 | 下一次请求生效 | 否 |

---

## 2. L1：声明式渠道（主力扩展方式）

一份 JSON 描述"怎么请求 → 怎么取条目 → 怎么映射字段 → 怎么断言"。

```jsonc
// config/engines/ithome.json
{
  "id": "ithome",
  "kind": "declarative",
  "version": 1,
  "enabled": true,
  "meta": {
    "domains": ["news"], "zones": ["cn"], "weight": 0.9,
    "min_interval_ms": 1000, "timeout_ms": 6000, "max_bytes": 5000000,
    "hosts": ["www.ithome.com"]
  },
  "request": {
    "method": "GET",
    "url": "https://www.ithome.com/rss/",
    "headers": { "accept-language": "zh-CN,zh;q=0.9" },
    "encoding": "auto"                     // auto | utf-8 | gbk | gb18030
  },
  "extract": {
    "format": "rss",                       // rss | atom | json | html
    "items": "/rss/channel/item",
    "fields": {
      "title": "title", "url": "link", "snippet": "description", "published": "pubDate"
    },
    "dates": ["%a, %d %b %Y %H:%M:%S %z"]
  },
  "post": {
    "filter_requires_query_terms": true,   // RSS 不是搜索：本地按词过滤（news 域语义）
    "max_items": 20
  },
  "assert": { "min_items": 1 }             // 结构断言：0 条即判 Parse 失败
}
```

三个真实形态（已按实测结构写，不是示意）：

```jsonc
// HTML：360（真实 URL 在 data-mdurl，不用跟跳转）
{ "request": { "url": "https://www.so.com/s?q={{query_urlencoded}}" },
  "extract": { "format": "html", "items": "li.res-list",
    "fields": { "title": "h3@text", "url": "self@data-mdurl", "snippet": "p.res-desc@text" } },
  "post": { "drop_rows_missing": ["url", "title"] } }

// JSON：GitHub Search
{ "request": { "url": "https://api.github.com/search/repositories?q={{query_urlencoded}}&per_page={{max_results}}",
               "headers": { "accept": "application/vnd.github+json" } },
  "extract": { "format": "json", "items": "$.items[*]",
    "fields": { "title": "$.full_name", "url": "$.html_url", "snippet": "$.description",
                "published": "$.pushed_at", "extras.stars": "$.stargazers_count" } } }

// 模板变量：{{query}} {{query_urlencoded}} {{max_results}} {{page}} {{params.<name>}} {{zone}} {{language}}
```

**字段路径语法**

| 形态 | 语法 | 例子 |
| --- | --- | --- |
| RSS/Atom | 简单 XPath 子集 | `/rss/channel/item`、`title`、`enclosure@url` |
| JSON | JSONPath（`serde_json_path` 0.7） | `$.items[*]`、`$.data.list` |
| HTML | CSS + 取值后缀（`scraper` 0.29） | `li.b_algo`、`h2 a@text`、`h3@text`、`self@data-mdurl`、`a@href`、`p@html` |

**加载期校验**：用 `jsonschema 0.60` 校验 spec 结构 → 预编译 CSS 选择器 / JSONPath → 校验 `hosts`（只允许 https 且非私网）→ 失败即拒绝加载并保留旧版本。

**声明式引擎的表达力边界**（诚实标注，超出就走 L2/L3）：多步请求、签名/加密参数、需要 cookie 会话、分页聚合、二进制响应。

---

## 3. L2：原生适配器

实现 `Engine` trait 即可；注册在一个静态函数里：

```rust
// src/engines/mod.rs
pub fn native_engines(cfg: &Config, http: Arc<HttpClient>) -> Vec<Arc<dyn Engine>> {
    vec![
        Arc::new(QuoteCn::new(cfg, http.clone())),   // GBK + 多交易所路由
        Arc::new(Bing::new(cfg, http.clone())),      // HTML 解析 + RSS 降级
    ]
}
```

适用：腾讯/东财行情（GBK、多市场代码映射）、Bing（HTML→RSS 自动降级）、未来需要登录/签名的源（若判定不算反爬）。

---

## 4. L3：外挂渠道（语言无关、进程隔离）

协议极小，任何语言都能实现：

```
POST /search
  {"query":"…","max_results":10,"zone":"cn","language":"zh-CN","params":{…}}
→ 200 {"items":[{"title":"…","url":"https://…","snippet":"…",
                 "published":"2026-10-20T08:00:00Z","rank":1,"extras":{}}],
       "stats":{"upstream_ms":120}}

GET /health → {"ok":true,"version":"1.0"}
```

接入配置：

```jsonc
{ "id": "my-crawler", "kind": "external", "enabled": true,
  "meta": { "domains": ["legal"], "zones": ["cn"], "weight": 0.8,
            "min_interval_ms": 2000, "timeout_ms": 8000, "hosts": [] },
  "endpoint": "http://127.0.0.1:9310",     // 默认只允许回环地址
  "failures_before_disable": 5, "cooldown_s": 300 }
```

约束：默认只允许 `127.0.0.1`/`::1`；超时 ≤8s；响应大小 ≤4MB；连续失败 5 次自动下线并在 `/health` 报警；`/admin/engines/{id}/enable` 手动带回。

---

## 5. 排序与打分管线（可换、可调、可解释）

### 5.1 固定骨架 + 可插拔环节

```
RawItem[] ──Normalizer──▶ ──Gate──▶ ──Deduper──▶ ──Fusion──▶ ──FeatureScorer──▶ ──Diversifier──▶ ──Truncator──▶ Result[]
              URL 规范化    结构断言    精确+近似      跨源融合       加权特征            MMR/同域上限      取 top-k
                            相关性闸门
```

每个环节都是 trait，默认实现可替换：

```rust
pub trait Fusion: Send + Sync {
    fn id(&self) -> &'static str;                       // "rrf" | "weighted_sum" | "borda"
    fn fuse(&self, lists: &[SourceList], cfg: &FusionCfg) -> Vec<Candidate>;
}

pub trait Feature: Send + Sync {
    fn id(&self) -> &'static str;
    fn score(&self, cx: &ScoreCtx, c: &Candidate) -> f32;  // 归一化到 0..=1
}

pub trait Diversifier: Send + Sync {
    fn id(&self) -> &'static str;                        // "mmr" | "domain_cap" | "none"
    fn diversify(&self, ranked: Vec<Candidate>, target: usize, cfg: &DiversityCfg) -> Vec<Candidate>;
}
```

### 5.2 两段式：先融合、再打分

- **融合用 RRF**（`Σ w_src / (k + rank)`，k=60）：各源分数不可比，但名次可比，且对新源零校准成本。
- **重排用特征加权**：`final = Σ w_i · f_i`，权重按 profile 配置，可为负（惩罚）。

内置特征：

| id | 含义 | 参数 |
| --- | --- | --- |
| `rrf` | 融合得分归一化 | — |
| `term_coverage` | 查询词覆盖率（中文 2-gram / 英文词） | `min_token_len` |
| `title_match` | 全部词命中且在标题中相邻 | — |
| `authority` | 域名权威分（政府/教育/官方/代码托管白名单） | 表可配 |
| `freshness` | 时效衰减 | `half_life_hours` |
| `language` | 与 `language` 参数一致 | — |
| `length` | 摘要长度（过短/过长降权） | `min,max` |
| `source_trust` | 源信任度（可被评测网格校准） | 表可配 |
| `exact_entity` | 查询含实体且命中官方域 | — |
| `param_consistency` | 结构化域（如行情）参数匹配度 | — |

新增特征 = 实现 trait + 注册（编译期），配置里即可赋权。**不改管线代码**。

### 5.3 Profile：按意图换权重

```jsonc
// config/scoring.json
{
  "fusion": { "kind": "rrf", "k": 60 },
  "diversity": { "kind": "mmr", "lambda": 0.7, "max_per_domain": 3 },
  "profiles": {
    "default": { "features": { "rrf": 1.0, "term_coverage": 0.8, "title_match": 0.5,
                               "authority": 0.3, "freshness": 0.2, "length": -0.1 } },
    "code":    { "features": { "rrf": 1.0, "term_coverage": 1.0, "authority": 0.4, "freshness": 0.1 } },
    "academic":{ "features": { "rrf": 1.0, "term_coverage": 0.9, "authority": 0.5 } },
    "news":    { "features": { "rrf": 1.0, "freshness": 1.2, "term_coverage": 0.6, "authority": 0.2 } },
    "finance": { "features": { "rrf": 1.0, "param_consistency": 1.0, "freshness": 0.8 } }
  },
  "engine_trust": { "so360": 1.0, "bing": 0.7, "github": 0.9, "ithome": 0.85 }
}
```

- profile 由路由结果决定（`intent → profile`），也可用请求参数 `?profile=` 覆盖（调试用）。
- `final = Σ w_i f_i / Σ|w_i|`，排序键 `(score desc, rank asc, url asc)` —— 完全确定性，可复现。

### 5.4 可解释

`?explain=1` 时每条结果附：

```json
{"url":"…","score":0.83,
 "contrib":{"rrf":0.41,"term_coverage":0.22,"authority":0.12,"freshness":0.05,"length":0.03},
 "source":"so360","rank":2,"gate":"ok"}
```

`metadata.routing` 给出选源理由（命中规则号、tier 划分、degraded 列表）。**任何一次"为什么这条排第一"都能当场回答**，不靠猜。

### 5.5 校准（把权重调好，而不是拍脑袋）

```powershell
samplesearch eval --set tests/eval/queries.json --grid 0.2        # 粗网格搜索权重
samplesearch eval --set tests/eval/queries.json --compare baseline.json
```

输出：top1/top3 命中率、MRR、平均延迟、平均上游请求数、degraded 率。调参依据是数字，不是手感。

---

## 6. 扩展点速查（想加东西改哪里）

| 需求 | 改动 | 重编译 | 生效 |
| --- | --- | --- | --- |
| 加 RSS/JSON/HTML 源 | `config/engines/<id>.json` | ❌ | 热加载 |
| 调某源权重/限流/超时 | 同上 `meta` | ❌ | 热加载 |
| 加"需要签名/GBK/多步"的源 | `src/engines/<id>.rs` + `native_engines()` | ✅ | 重启 |
| 用别的语言写渠道 | 外挂进程 + `config/engines/<id>.json` | ❌ | 热加载 |
| 改路由信号 | `config/routing.json` | ❌ | 热加载 |
| 调打分权重/profile | `config/scoring.json` | ❌ | 热加载 |
| 加新特征 | `src/core/features/<id>.rs` + 注册 | ✅ | 重启 |
| 换融合算法 | 实现 `Fusion` trait + 注册 | ✅ | 重启 |
| 换去重/多样性策略 | 实现对应 trait | ✅ | 重启 |
| 开海外源 | `config/routing.json` 的 `intl` | ❌ | 热加载 |

---

## 7. 目录结构（落实版）

```
src/
├─ main.rs                启动、配置加载、信号处理、优雅退出
├─ config/
│   ├─ mod.rs             Config 聚合 + 校验 + 热加载（notify 8）
│   └─ spec.rs            声明式 spec 类型（serde + jsonschema）
├─ engines/
│   ├─ mod.rs             Engine trait、EngineMeta、Registry、native_engines()
│   ├─ declarative.rs     L1 执行器（http → rss/json/html → RawItem）
│   ├─ external.rs        L3 客户端（reqwest → 外挂协议）
│   ├─ native/
│   │   ├─ so360.rs  bing.rs  quote_cn.rs
├─ core/
│   ├─ router.rs          信号表（routing.json）→ EnginePlan
│   ├─ orchestrator.rs    JoinSet + 软截止 + tier_c 兜底
│   ├─ gates.rs           结构断言 + 相关性闸门
│   ├─ dedupe.rs          exact_url / simhash
│   ├─ fusion.rs          rrf / weighted_sum / borda
│   ├─ features/          rrf.rs term_coverage.rs authority.rs freshness.rs length.rs ...
│   ├─ score.rs           profile 加权 + explain
│   ├─ diversity.rs       mmr / domain_cap
│   ├─ cache.rs  ratelimit.rs  health.rs
├─ extract/               fetch / readabl / md
├─ admin/                 /admin/reload /admin/engines /admin/conformance（仅回环）
├─ api/                   /v1/search /v1/sub-domains /v1/extract
├─ mcp/  cli.rs
config/
├─ engines/*.json  routing.json  scoring.json
tests/
├─ fixtures/<engine>/response.bin + expected.json
├─ golden/ranking_*.json   固定候选集 → 期望排序
└─ eval/queries.json
```

---

## 8. 测试策略（保证"扩展了不会悄悄坏"）

| 层次 | 做法 | 跑在哪 |
| --- | --- | --- |
| 渠道契约 | 每个 L1 spec 配 `tests/fixtures/<id>/response.bin` + `expected.json`（前 3 条 url/title） | 离线，CI |
| 结构断言 | 解析结果数 ≥ `assert.min_items`，字段缺失率 ≤ 阈值 | 离线 |
| 打分金测 | `tests/golden/ranking_*.json`：固定候选集 + profile → 断言顺序 | 离线，CI |
| 热加载 | 改配置 → reload → 断言 revision+1 且新渠道可命中；坏配置 → 断言保留旧版本 | 离线 |
| 实网体检 | `samplesearch engine check <id> [--spec x.json]`：字段覆盖率、条数、延迟、闸门结果 | 手动 |
| 外挂契约 | `POST /admin/conformance`：10 条固定查询，校验 schema/超时/错误语义 | 手动/CI（mock） |

`engine check` 是新渠道的准入门槛：覆盖率不到 90% 或经常 0 条，就先别进配置。

---

## 9. 风险与取舍

| 风险 | 取舍 |
| --- | --- |
| 抽象过度、包了一层没收益 | 固定骨架只有 7 个环节，且默认实现先写死；L1 spec 只覆盖"取数 + 映射"，逻辑一律不塞进配置 |
| L1 表达力不够 | 明确边界（多步/签名/分页/二进制 → L2/L3），不做"配置里写脚本"这种歪路 |
| 热加载出现半更新状态 | 一次性构造新 `Registry` 再 `arc_swap` 原子替换；失败回滚到上一版并保留 revision |
| 权重越调越玄学 | 权重必须能被 `eval` 量化；每次调参附前后对比，进版本库 |
| 外挂渠道成为攻击面 | 默认仅回环、限定超时/大小、失败自动下线、`hosts` 白名单 |
| dyn async 性能 | 每请求 clone 一次 `Arc<dyn Engine>`，开销在纳秒级；实测上线后看 `sources[].ms` 再说 |

---

## 10. 落地顺序（与里程碑对齐）

| 阶段 | 交付 | 验证 |
| --- | --- | --- |
| M0 | `Engine` trait + Registry（先用 Vec + arc-swap）+ 两个原生适配器（so360/bing） | 能搜到结果，trait 边界够用 |
| M1 | **L1 声明式引擎** + `config/engines/*.json` + 热加载 + 融合/特征/打分骨架 + `/admin/reload` | 新增一个 RSS 源只改 JSON，revision 增加且可命中 |
| M2 | 垂直源全部用 L1 写（GitHub/IT之家/arXiv/Crossref/政府网…），仅行情与 Bing 留 L2 | 8 个渠道全部来自配置，零核心改动 |
| M3 | L3 外挂协议 + `/admin/conformance` + MCP/CLI | 一个非 Rust 外挂渠道能参与融合与排序 |
| M4 | 权重校准（`eval --grid`）、`engine check` 体检 | 调参前后命中率有数字 |
