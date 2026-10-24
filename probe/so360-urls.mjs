// 360 detail: can we recover the real target URL without following the /link?m= redirect?
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/\s+/g, ' ').trim();

const html = await fetch(`https://www.so.com/s?q=${encodeURIComponent('大模型 推理 优化')}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
const blocks = [...html.matchAll(/<li class="res-list[\s\S]*?<\/li>/g)].map((m) => m[0]);
console.log('blocks =', blocks.length);

const b = blocks[1] || blocks[0];
const attrs = [...b.matchAll(/(data-[a-z-]+)="([^"]{0,200})"/g)].map(([, k, v]) => `${k}=${v}`);
console.log('\nattributes:\n  ' + attrs.slice(0, 12).join('\n  '));

const hrefs = [...b.matchAll(/href="([^"]+)"/g)].map(([, h]) => h);
console.log('\nhrefs:\n  ' + hrefs.slice(0, 5).join('\n  '));

const cite = (b.match(/<cite[^>]*>([\s\S]*?)<\/cite>/) || [])[1];
console.log('\ncite:', cite ? strip(cite) : '(none)');

const shown = (b.match(/class="[^"]*mh-citem[^"]*"[^>]*>([\s\S]{0,120})/) || [])[1];
console.log('shown url-ish:', shown ? strip(shown).slice(0, 80) : '(none)');

// does following the /link redirect resolve quickly?
if (hrefs[0]?.startsWith('https://www.so.com/link')) {
  const r = await fetch(hrefs[0], { headers: H, redirect: 'manual', signal: AbortSignal.timeout(10000) });
  console.log('\nlink redirect status:', r.status, 'location:', (r.headers.get('location') || '').slice(0, 120));
}
