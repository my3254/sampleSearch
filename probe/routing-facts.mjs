// Facts needed for the routing design:
// 1) does cn.bing.com HTML carry snippets (not just title/url)?
// 2) does count= work (can we get >10)?
// 3) does so.com HTML carry snippets?
// 4) is 澎湃 (thepaper) server-rendered enough to parse a list?
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

const q = encodeURIComponent('大模型 推理 优化');

// bing html
const bing = await fetch(`https://cn.bing.com/search?q=${q}&count=20&setlang=zh-CN`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
const blocks = [...bing.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].map((m) => m[0]);
console.log(`bing-html: blocks=${blocks.length} bytes=${bing.length}`);
for (const b of blocks.slice(0, 2)) {
  const title = strip((b.match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || '');
  const href = (b.match(/<h2><a[^>]+href="([^"]+)"/) || [])[1] || '';
  const cap = strip((b.match(/<p class="b_lineclamp[^"]*"[^>]*>([\s\S]*?)<\/p>/) || b.match(/<p[^>]*>([\s\S]*?)<\/p>/) || [])[1] || '');
  console.log('  T:', title.slice(0, 60));
  console.log('  U:', href.slice(0, 90));
  console.log('  S:', cap.slice(0, 110));
}

// so.com
const so = await fetch(`https://www.so.com/s?q=${q}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
const soBlocks = [...so.matchAll(/<li class="res-list[\s\S]*?<\/li>/g)].map((m) => m[0]);
console.log(`\nso360: blocks=${soBlocks.length} bytes=${so.length}`);
for (const b of soBlocks.slice(0, 2)) {
  const title = strip((b.match(/<h3[^>]*>([\s\S]*?)<\/h3>/) || [])[1] || '');
  const href = (b.match(/data-url="([^"]+)"/) || b.match(/<a[^>]+href="(https?:\/\/[^"]+)"/) || [])[1] || '';
  const snip = strip((b.match(/<p class="res-desc"[^>]*>([\s\S]*?)<\/p>/) || b.match(/<div class="res-comm-con[^"]*"[^>]*>([\s\S]*?)<\/div>/) || [])[1] || '');
  console.log('  T:', title.slice(0, 60));
  console.log('  U:', href.slice(0, 90));
  console.log('  S:', snip.slice(0, 110));
}

// thepaper list
const tp = await fetch('https://www.thepaper.cn/', { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
const links = [...tp.matchAll(/href="(\/newsDetail_[^"]+)"/g)].map((m) => m[1]);
const titles = [...tp.matchAll(/<h2[^>]*>([\s\S]*?)<\/h2>/g)].map((m) => strip(m[1])).filter(Boolean);
console.log(`\nthepaper: newsDetailLinks=${new Set(links).size} h2=${titles.length} bytes=${tp.length}`);
console.log('  sample:', (titles[0] || '').slice(0, 60), new Set(links).size ? [...new Set(links)][0] : '');

// ithome rss structure
const it = await fetch('https://www.ithome.com/rss/', { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
console.log(`\nithome-rss: items=${(it.match(/<item>/g) || []).length} hasLink=${/<link>/g.test(it)} hasDesc=${/<description>/g.test(it)}`);
