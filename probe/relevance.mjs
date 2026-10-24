// Relevance smoke test across engines: does the engine actually answer the query?
// Metric: top-3 titles contain at least one query token (CJK >=2 chars or ASCII word).
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').replace(/&#(\d+);/g, (_, d) => String.fromCharCode(+d)).replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

const queries = [
  '大模型 推理 优化',
  'vLLM 部署 显存',
  'Rust 异步 运行时',
  '2026 新能源 汽车 销量',
  '劳动法 试用期 规定',
  'tokio select 用法',
  'nvidia 财报 2026',
  '台风 路径 上海',
];

const tokens = (q) => {
  const parts = q.split(/\s+/).filter(Boolean);
  const ascii = parts.filter((p) => /^[A-Za-z0-9._-]+$/.test(p));
  const cjk = parts.filter((p) => /[\u4e00-\u9fa5]/.test(p));
  return [...ascii, ...cjk];
};

async function bingHtml(q) {
  const html = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
  return [...html.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)]
    .map((m) => strip((m[0].match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || ''))
    .filter(Boolean);
}

async function bingRss(q) {
  const x = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}&format=rss`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
  return [...x.matchAll(/<item>([\s\S]*?)<\/item>/g)].map(([, it]) => strip((it.match(/<title>([\s\S]*?)<\/title>/) || [])[1] || '')).filter(Boolean);
}

async function so360(q) {
  const html = await fetch(`https://www.so.com/s?q=${encodeURIComponent(q)}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
  return [...html.matchAll(/<li class="res-list[\s\S]*?<\/li>/g)]
    .map((m) => strip((m[0].match(/<h3[^>]*>([\s\S]*?)<\/h3>/) || [])[1] || ''))
    .filter(Boolean);
}

const score = (q, titles) => {
  const tk = tokens(q);
  const hit = titles.slice(0, 3).filter((t) => tk.some((x) => t.includes(x))).length;
  return { hit, top: titles.slice(0, 2).join(' | ').slice(0, 70) };
};

const engines = [['bing-html', bingHtml], ['bing-rss', bingRss], ['so360', so360]];
const rows = [];
for (const q of queries) {
  const row = { q };
  for (const [name, fn] of engines) {
    try {
      const titles = await fn(q);
      const s = score(q, titles);
      row[name] = `${s.hit}/3 ${titles.length}n`;
      row[name + '_top'] = s.top;
    } catch (e) {
      row[name] = 'ERR';
    }
  }
  rows.push(row);
  await new Promise((r) => setTimeout(r, 600));
}

console.log('query'.padEnd(22), 'bing-html'.padEnd(10), 'bing-rss'.padEnd(10), 'so360'.padEnd(8));
console.log('-'.repeat(55));
for (const r of rows) console.log(r.q.padEnd(22), String(r['bing-html']).padEnd(10), String(r['bing-rss']).padEnd(10), String(r['so360']).padEnd(8));
console.log('\n--- 每行 top2 标题（判断相关性） ---');
for (const r of rows) {
  console.log(`\n[${r.q}]`);
  console.log('  bing-html:', r['bing-html_top']);
  console.log('  bing-rss :', r['bing-rss_top']);
  console.log('  so360    :', r['so360_top']);
}
