// What does cn.bing.com return right now for queries that used to work?
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';

for (const q of ['tokio select 用法', '贵州茅台 2025 年报']) {
  const t = Date.now();
  const r = await fetch('https://cn.bing.com/search?q=' + encodeURIComponent(q), {
    headers: { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' },
    signal: AbortSignal.timeout(20000),
  });
  const b = await r.text();
  const algo = b.split('class="b_algo"').length - 1;
  const h2a = b.split('<h2><a ').length - 1;
  const title = (b.match(/<title>([^<]*)<\/title>/) || [])[1] || '(none)';
  console.log(`${q} | ${r.status} ${Date.now() - t}ms ${b.length}B  b_algo=${algo} h2a=${h2a}`);
  console.log('  title:', title.slice(0, 70));
  console.log('  has consent/captcha:', /consent|验证码|captcha/i.test(b));
  await new Promise((s) => setTimeout(s, 1500));
}
