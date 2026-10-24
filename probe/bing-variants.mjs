// Why did bing return irrelevant results? Test query encoding variants + news RSS.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();
const qPlus = '大模型+推理+优化';
const qPct = encodeURIComponent('大模型 推理 优化');

const variants = [
  ['q-plus', `https://cn.bing.com/search?q=${qPlus}`],
  ['q-pct', `https://cn.bing.com/search?q=${qPct}`],
  ['q-plus+count', `https://cn.bing.com/search?q=${qPlus}&count=20`],
  ['q-plus+setlang', `https://cn.bing.com/search?q=${qPlus}&setlang=zh-CN`],
  ['q-plus+ensearch', `https://cn.bing.com/search?q=${qPlus}&ensearch=0`],
  ['www-bing-plus', `https://www.bing.com/search?q=${qPlus}`],
];

for (const [name, url] of variants) {
  try {
    const html = await fetch(url, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
    const blocks = [...html.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].map((m) => m[0]);
    const titles = blocks.slice(0, 3).map((b) => strip((b.match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || ''));
    console.log(`${name.padEnd(16)} blocks=${String(blocks.length).padStart(2)}  ${titles.join(' | ').slice(0, 110)}`);
  } catch (e) {
    console.log(`${name.padEnd(16)} ERR ${e.cause?.code || e.message}`);
  }
}

console.log('\n=== bing news RSS ===');
for (const [name, url] of [
  ['cn-news-rss', `https://cn.bing.com/news/search?q=${qPlus}&format=RSS`],
  ['www-news-rss', `https://www.bing.com/news/search?q=${qPlus}&format=RSS`],
  ['cn-news-rss-7d', `https://cn.bing.com/news/search?q=${qPlus}&format=RSS&qft=interval%3d%227%22`],
]) {
  try {
    const r = await fetch(url, { headers: H, signal: AbortSignal.timeout(15000) });
    const body = await r.text();
    const items = [...body.matchAll(/<item>([\s\S]*?)<\/item>/g)].map(([, x]) => strip((x.match(/<title>([\s\S]*?)<\/title>/) || [])[1] || ''));
    console.log(`${name.padEnd(16)} ${r.status} bytes=${String(body.length).padStart(6)} items=${items.length} ${items.slice(0, 2).join(' | ').slice(0, 90)}`);
  } catch (e) {
    console.log(`${name.padEnd(16)} ERR ${e.cause?.code || e.message}`);
  }
}
