// Vertical / structured free data source feasibility matrix.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const targets = [
  ['openalex', 'https://api.openalex.org/works?search=llm&per_page=2'],
  ['crossref', 'https://api.crossref.org/works?query=llm&rows=2'],
  ['arxiv', 'http://export.arxiv.org/api/query?search_query=all:llm&max_results=2'],
  ['semanticscholar', 'https://api.semanticscholar.org/graph/v1/paper/search?query=llm&limit=2'],
  ['tencent-quote', 'https://qt.gtimg.cn/q=sh600519'],
  ['eastmoney-quote', 'https://push2.eastmoney.com/api/qt/stock/get?secid=1.600519&fields=f43,f57,f58'],
  ['pypi', 'https://pypi.org/pypi/requests/json'],
  ['npm', 'https://registry.npmjs.org/express/latest'],
  ['stackexchange', 'https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle=llm&site=stackoverflow'],
  ['bilibili', 'https://api.bilibili.com/x/web-interface/search/type?search_type=video&keyword=llm'],
  ['zhihu', 'https://www.zhihu.com/api/v4/search_v3?t=general&q=llm'],
  ['weibo', 'https://m.weibo.cn/api/container/getIndex?containerid=100103type%3D1%26q%3Dllm'],
  ['baidu-baike', 'https://baike.baidu.com/item/人工智能'],
  ['sogou-wx', 'https://weixin.sogou.com/weixin?type=2&query=大模型'],
];

const out = [];
await Promise.all(targets.map(async ([name, url]) => {
  const t0 = Date.now();
  try {
    const r = await fetch(url, { headers: { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8' }, signal: AbortSignal.timeout(12000) });
    const b = await r.arrayBuffer();
    const ct = (r.headers.get('content-type') || '').split(';')[0];
    out.push({ name, status: r.status, ms: Date.now() - t0, bytes: b.byteLength, ct });
  } catch (e) {
    out.push({ name, error: e.cause?.code || e.name, ms: Date.now() - t0 });
  }
}));
out.sort((a, b) => a.name.localeCompare(b.name));
for (const o of out) {
  console.log(
    o.name.padEnd(18),
    String(o.status ?? 'ERR').padEnd(6),
    String(o.ms).padStart(6) + 'ms',
    String(o.bytes ?? '-').padStart(9) + 'B',
    o.ct || o.error,
  );
}
