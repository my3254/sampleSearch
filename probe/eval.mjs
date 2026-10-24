// Precision@5 harness: does the top-5 actually answer the query?
// A result counts as relevant when its title contains any expected keyword.
// Crude proxy, but stable and reproducible — good enough to compare filter changes.
const base = process.env.SAMPLESEARCH_BASE || 'http://127.0.0.1:8787';

const cases = [
  { q: '大模型 推理 优化', expect: ['推理', '优化'], forbid: ['汉语', '汉典', '部首', '翻译'] },
  { q: 'vLLM 显存 优化', expect: ['vLLM', 'vllm', '显存'] },
  { q: 'tokio select 用法', expect: ['tokio', 'select'] },
  { q: '劳动法 试用期 规定', expect: ['试用期'] },
  { q: '台风 路径 上海', expect: ['台风'] },
  { q: 'nvidia 财报 2026', expect: ['英伟达', 'NVIDIA', '财报'] },
  { q: '贵州茅台 2025 年报', expect: ['茅台', '年报'] },
  { q: 'rust async runtime', expect: ['async', 'runtime', 'tokio'] },
  { q: '中国政府网 最新政策', expect: ['政策'] },
];

let hitTotal = 0;
let returned = 0;
const rows = [];

for (const c of cases) {
  const r = await fetch(base + '/v1/search', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ query: c.q, max_results: 5 }),
    signal: AbortSignal.timeout(30000),
  });
  const j = await r.json();
  const results = j?.data?.results ?? [];
  let hits = 0;
  const flags = results.map((x) => {
    const t = (x.title || '').toLowerCase();
    const ok =
      c.expect.some((k) => t.includes(k.toLowerCase())) &&
      !(c.forbid || []).some((k) => t.includes(k.toLowerCase()));
    if (ok) hits++;
    return (ok ? '+' : '-') + (x.title || '').slice(0, 22);
  });
  hitTotal += hits;
  returned += results.length;
  rows.push({ q: c.q, n: results.length, hits, ms: j?.data?.metadata?.search_time_ms, flags, filters: j?.data?.metadata?.filters });
}

for (const r of rows) {
  console.log(`[${r.hits}/${r.n}] ${r.ms}ms  ${r.q}`);
  for (const f of r.flags) console.log('   ' + f);
  if (r.filters?.length) console.log('   filters: ' + r.filters.map((x) => `${x.reason}=${x.count}`).join(' '));
}
console.log(
  `\nprecision = ${(hitTotal / returned).toFixed(2)} (${hitTotal}/${returned} returned)  avg_returned = ${(returned / rows.length).toFixed(1)}/${5}`,
);
