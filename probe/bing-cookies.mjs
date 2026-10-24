// Is the "degraded results for 3+ term queries" a cookie/session issue?
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();
const q = '大模型 推理 优化';

function titles(html) {
  const blocks = [...html.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].map((m) => m[0]);
  return { n: blocks.length, t: blocks.slice(0, 3).map((b) => strip((b.match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || '')) };
}

// cold
const cold = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
const c = titles(cold);
console.log(`COLD   blocks=${c.n}  ${c.t.join(' | ').slice(0, 100)}`);

// warm: visit homepage, keep cookies
const home = await fetch('https://cn.bing.com/', { headers: H, signal: AbortSignal.timeout(15000) });
const cookies = (home.headers.getSetCookie?.() || []).map((c) => c.split(';')[0]).join('; ');
console.log(`COOKIES ${cookies.slice(0, 160)}`);

const warm = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}`, {
  headers: { ...H, cookie: cookies, referer: 'https://cn.bing.com/' },
  signal: AbortSignal.timeout(15000),
}).then((r) => r.text());
const w = titles(warm);
console.log(`WARM   blocks=${w.n}  ${w.t.join(' | ').slice(0, 100)}`);

// warm + delay between two searches (same cookies)
await new Promise((s) => setTimeout(s, 2000));
const q2 = '大模型 推理 优化';
const warm2 = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q2)}`, {
  headers: { ...H, cookie: cookies, referer: 'https://cn.bing.com/search?q=' + encodeURIComponent(q) },
  signal: AbortSignal.timeout(15000),
}).then((r) => r.text());
const w2 = titles(warm2);
console.log(`WARM2  blocks=${w2.n}  ${w2.t.join(' | ').slice(0, 100)}`);

// also: what does the RSS give for the same query?
const rss = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}&format=rss`, { headers: H, signal: AbortSignal.timeout(15000) })
  .then((r) => (r.ok ? r.text() : Promise.reject(new Error('status ' + r.status))));
const items = [...rss.matchAll(/<item>([\s\S]*?)<\/item>/g)].map(([, x]) => strip((x.match(/<title>([\s\S]*?)<\/title>/) || [])[1] || ''));
console.log(`RSS    items=${items.length}  ${items.slice(1, 4).join(' | ').slice(0, 110)}`);
