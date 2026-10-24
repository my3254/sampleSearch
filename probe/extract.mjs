// Second probe: (a) what is AnySearch, (b) can we actually extract structured
// results from each reachable HTML/RSS endpoint?
// Run: node probe/extract.mjs

const UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8' };

const get = async (u, extra = {}) => {
  const res = await fetch(u, { headers: { ...H, ...extra }, redirect: 'follow', signal: AbortSignal.timeout(15000) });
  return { status: res.status, url: res.url, body: await res.text() };
};
const decode = (s) =>
  s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"')
   .replace(/&#39;/g, "'").replace(/&amp;/g, '&').replace(/<[^>]+>/g, '')
   .replace(/\s+/g, ' ').trim();

console.log('=== (a) AnySearch reference product ===');
const rss = await get('https://cn.bing.com/search?q=anysearch%20ai%20search%20infrastructure&format=rss&count=20');
const items = [...rss.body.matchAll(/<item>([\s\S]*?)<\/item>/g)].map(([, x]) => ({
  title: decode((x.match(/<title>([\s\S]*?)<\/title>/) || [])[1] || ''),
  link: ((x.match(/<link>([\s\S]*?)<\/link>/) || [])[1] || '').trim(),
  desc: decode((x.match(/<description>([\s\S]*?)<\/description>/) || [])[1] || '').slice(0, 160),
}));
for (const it of items.slice(0, 8)) console.log(`- ${it.title}\n  ${it.link}\n  ${it.desc}`);

console.log('\n=== (b) structure probe per engine, query "ai 搜索 api" ===');
const q = encodeURIComponent('ai 搜索 api');
const engines = [
  ['cn.bing+html', `https://cn.bing.com/search?q=${q}&count=20`, [['li.b_algo', /class="b_algo"/g], ['h2>a', /<h2><a /g]]],
  ['bing+rss', `https://cn.bing.com/search?q=${q}&format=rss&count=20`, [['<item>', /<item>/g]]],
  ['baidu', `https://www.baidu.com/s?wd=${q}&rn=20`, [['div.result', /class="result[ "]/g], ['h3>a', /<h3[^>]*>\s*<a /g]]],
  ['sogou', `https://www.sogou.com/web?query=${q}`, [['div.vrwrap', /class="vrwrap"/g], ['h3>a', /<h3[^>]*>\s*<a /g]]],
  ['so360', `https://www.so.com/s?q=${q}`, [['li.res-list', /class="res-list"/g], ['h3>a', /<h3[^>]*>\s*<a /g]]],
  ['cn.bing+news', `https://cn.bing.com/news/search?q=${q}&format=RSS`, [['<item>', /<item>/g]]],
];
for (const [name, url, probes] of engines) {
  try {
    const r = await get(url);
    const hits = probes.map(([label, re]) => `${label}=${(r.body.match(re) || []).length}`);
    console.log(`${name.padEnd(14)} ${r.status} ${String(r.body.length).padStart(8)}B  ${hits.join('  ')}`);
  } catch (e) {
    console.log(`${name.padEnd(14)} ERROR ${e.message}`);
  }
}

console.log('\n=== (c) keys/quota check: github + hn ===');
const gh = await get('https://api.github.com/rate_limit');
if (gh.status === 200) {
  const j = JSON.parse(gh.body);
  console.log('github search remaining =', j.resources.search.remaining, '/', j.resources.search.limit);
}
