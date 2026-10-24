// (1) Real AnySearch response envelope (anonymous), (2) how Bing HTML links look,
// (3) stability of repeated Bing RSS calls.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';

console.log('=== (1) POST https://api.anysearch.com/v1/search (anonymous) ===');
try {
  const r = await fetch('https://api.anysearch.com/v1/search', {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'user-agent': UA },
    body: JSON.stringify({ query: 'microsoft earnings report', max_results: 2 }),
    signal: AbortSignal.timeout(25000),
  });
  const t = await r.text();
  console.log('status', r.status, 'bytes', t.length);
  console.log(t.slice(0, 2500));
} catch (e) {
  console.log('ERR', e.message, e.cause?.code);
}

console.log('\n=== (2) cn.bing.com HTML link shape ===');
try {
  const r = await fetch('https://cn.bing.com/search?q=' + encodeURIComponent('site:github.com anysearch'), {
    headers: { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' },
    signal: AbortSignal.timeout(20000),
  });
  const html = await r.text();
  const blocks = [...html.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].slice(0, 2).map((m) => m[0]);
  for (const b of blocks) {
    const href = (b.match(/<h2><a[^>]+href="([^"]+)"/) || [])[1] || '';
    const cite = (b.match(/<cite[^>]*>([\s\S]*?)<\/cite>/) || [])[1] || '';
    const title = ((b.match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || '').replace(/<[^>]+>/g, '');
    console.log('title:', title.slice(0, 60));
    console.log('href :', href.slice(0, 120));
    console.log('cite :', cite.replace(/<[^>]+>/g, '').slice(0, 80));
    console.log('---');
  }
} catch (e) {
  console.log('ERR', e.message);
}

console.log('=== (3) repeated bing RSS stability ===');
for (let i = 0; i < 3; i++) {
  const t0 = Date.now();
  try {
    const r = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent('test ' + i)}&format=rss&count=20`, {
      headers: { 'user-agent': UA },
      signal: AbortSignal.timeout(15000),
    });
    const b = await r.text();
    console.log(`#${i} ${r.status} ${Date.now() - t0}ms items=${(b.match(/<item>/g) || []).length}`);
  } catch (e) {
    console.log(`#${i} ERR ${e.message}`);
  }
  await new Promise((s) => setTimeout(s, 700));
}
