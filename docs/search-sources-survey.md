# Search MCP servers: where their results actually come from

Compiled 2026-10-24 from public npm and GitHub metadata only (stars, descriptions, and
each project's own README), not from marketing pages. Numbers are snapshots.

## The five ways a search MCP gets its results

| Model | Who does it | What it means in practice |
| --- | --- | --- |
| **A. Own web index** | Exa, Tavily, Firecrawl, Brave, Kagi, Marginalia | They crawled the web, built an index, and sell query access. Capital-intensive; result quality is theirs to control. |
| **B. Licensed SERP APIs** | Serper, SerpAPI, Linkup, Bright Data, DataForSEO | Resell Google/Bing result pages under a quota. Cheap per query, still someone else's index. |
| **C. Meta-search / scraping** | SearXNG, open-webSearch, duckduckgo-mcp-server, search2ai, web-search-mcp | Query public result pages of Bing/Google/DuckDuckGo/Baidu and parse HTML. Free, unauthenticated, rate-limited by the target. |
| **D. The user's own browser** | Chrome MCP Server | Drives a real, logged-in browser session; sidesteps bot checks by being a human session. |
| **E. Vertical / site APIs** | arXiv MCP, apple-docs MCP, 12306 MCP, korean-law MCP, data.gouv MCP, GitHub/HN servers | Official stable APIs or RSS for one domain. Small scope, high reliability, documented quotas. |

## Projects that matter, and their sources

| Project | ★ | Source of results | Model | Key required |
| --- | --- | --- | --- | --- |
| xerj-org/xerj | 15.2k | Your local files (code, docs, logs) — not the web | local index | no |
| apify/apify-mcp-server | 12.9k | Apify Actors (Google/Bing/social scrapers run on their platform) | B/C, hosted | yes (Apify) |
| hangwin/mcp-chrome | 12.5k | The user's Chrome session | D | no |
| firecrawl/firecrawl-mcp-server | 7.7k | Firecrawl's crawler + search | A | yes |
| exa-labs/exa-mcp-server | 5.2k | Exa neural index | A | yes |
| KnockOutEZ/wigolo | 5.5k | Local-first fetch/crawl/search over the open web | C | no |
| tavily-ai/tavily-mcp | 2.5k | Tavily index + crawler | A | yes |
| **anysearch-ai/anysearch-mcp-server** | 2.1k | Hosted aggregator with 17 vertical domains | A/B/E mix, hosted | optional |
| Aas-ee/open-webSearch | 1.9k | Bing, Baidu, DuckDuckGo, Brave, GitHub, CSDN… parsed per engine | C | no |
| nickclyde/duckduckgo-mcp-server | 1.6k | DuckDuckGo HTML endpoints | C | no |
| brave/brave-search-mcp-server | 1.5k | Brave's independent index via official API | A/B | yes |
| ihor-sokoliuk/mcp-searxng | 1.3k | A SearXNG instance (70+ engines behind it) | C (meta) | no |
| fatwang2/search2ai | 1.3k | Self-hosted gateway with provider fallback (Bing/Google/Bocha…) | B/C | optional |
| mrkrsl/web-search-mcp | 1.2k | Bing / Brave / DuckDuckGo HTML | C | no |
| blazickjp/arxiv-mcp-server | 3.2k | arXiv API | E | no |

## What this says about our design

1. **We are category C, and that is a legitimate neighbourhood.** SearXNG, open-webSearch,
   duckduckgo-mcp-server and search2ai all live there; combined they hold more stars than any
   single paid provider's MCP. The trade-off is the one we hit in production: the upstream
   decides when you stop working.
2. **Nobody in category C sells accuracy.** The pitch of every project above is coverage
   ("70+ engines", "multi-engine", "provider fallback"). None documents *which* results it
   discards and why. Our filter chain and per-request filter report are the differentiator,
   not another engine.
3. **The reliability gap is fixable with category E.** Official APIs (GitHub 10 req/min
   unauthenticated, arXiv, Crossref, HN Algolia, npm, PyPI, StackExchange) do not hand out
   captchas. Every one of them added is throughput that 360/Bing cannot take away.
4. **Category A is out of scope** for a self-hosted, zero-cost tool: Exa/Tavily/Firecrawl
   prices are per-query, and building an index is a different project.

## Adopted from this survey (priority order)

| # | Change | Why | Source |
| --- | --- | --- | --- |
| 1 | Add official-API engines: GitHub, Hacker News, arXiv, Crossref, npm, PyPI, StackExchange | Quota-based, no captcha, stable schemas | E |
| 2 | Per-engine token bucket + suspend-on-error (SearXNG's pattern) | Stops burning quota after a block instead of retrying into a captcha | C |
| 3 | Chinese verticals: IT之家 / 新浪 / 少数派 RSS, 政府网政策页 | Chinese queries need non-engine supply | E |
| 4 | Engine health surfaced in `/health` with last error and quota state | Operators can see *why* a source is quiet | C |
| 5 | Optional keyed providers (Bocha / Tavily / Brave) behind config | Users who need volume can buy it without changing the client | A/B |

Not adopted: own index, browser automation, paid SERP resellers as a default dependency.
