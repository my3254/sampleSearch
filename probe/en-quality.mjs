// English query quality: bing vs 360, plus data-mdurl coverage in 360 blocks.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&#(\d+);/g, (_, d) => String.fromCharCode(+d)).replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

const qs = ['tokio select usage', 'rust async runtime', 'nvidia earnings 2026', 'windows sandbox acl'];

for (const q of qs) {
  const bing = await fetch(`https://cn.bing.com/search?q=${encodeURIComponent(q)}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
  const bt = [...bing.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].map((m) => strip((m[0].match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || '')).filter(Boolean);

  const so = await fetch(`https://www.so.com/s?q=${encodeURIComponent(q)}`, { headers: H, signal: AbortSignal.timeout(15000) }).then((r) => r.text());
  const soBlocks = [...so.matchAll(/<li class="res-list[\s\S]*?<\/li>/g)].map((m) => m[0]);
  const st = soBlocks.map((b) => strip((b.match(/<h3[^>]*>([\s\S]*?)<\/h3>/) || [])[1] || '')).filter(Boolean);
  const mdurls = soBlocks.filter((b) => /data-mdurl="https?:/.test(b)).length;

  console.log(`\n[${q}]`);
  console.log(`  bing ${bt.length}n: ${bt.slice(0, 2).join(' | ').slice(0, 95)}`);
  console.log(`  so360 ${st.length}n (mdurl=${mdurls}/${soBlocks.length}): ${st.slice(0, 2).join(' | ').slice(0, 95)}`);
  await new Promise((r) => setTimeout(r, 800));
}
