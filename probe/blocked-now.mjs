// One careful request per engine: are we still blocked, and what does the block look like?
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };

async function probe(name, url, marker) {
  const t0 = Date.now();
  try {
    const r = await fetch(url, { headers: H, signal: AbortSignal.timeout(15000) });
    const body = await r.text();
    const blocked = marker.test(body);
    console.log(
      `${name.padEnd(8)} status=${r.status} ${Date.now() - t0}ms bytes=${body.length} blocked=${blocked}` +
        (blocked ? `  ← ${(body.match(marker) || [''])[0].slice(0, 40)}` : ''),
    );
  } catch (e) {
    console.log(`${name.padEnd(8)} ERROR ${e.cause?.code || e.message}`);
  }
}

await probe('so360', 'https://www.so.com/s?q=' + encodeURIComponent('杭州 天气'), /验证码|安全验证/);
await new Promise((r) => setTimeout(r, 1500));
await probe('bing', 'https://cn.bing.com/search?q=' + encodeURIComponent('杭州 天气'), /验证码|captcha|unusual traffic/i);
