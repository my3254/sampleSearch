// Does Bing actually parse the Chinese query? Read back the search box value + try more shapes.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9' };
const strip = (s) => s.replace(/<[^>]+>/g, '').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

async function probe(label, url) {
  try {
    const r = await fetch(url, { headers: H, signal: AbortSignal.timeout(15000), redirect: 'follow' });
    const html = await r.text();
    const qbox = (html.match(/<input[^>]+name="q"[^>]*value="([^"]*)"/) || html.match(/<input[^>]+value="([^"]*)"[^>]*name="q"/) || [])[1] || '(none)';
    const blocks = [...html.matchAll(/<li class="b_algo"[\s\S]*?<\/li>/g)].map((m) => m[0]);
    const titles = blocks.slice(0, 2).map((b) => strip((b.match(/<h2><a[^>]*>([\s\S]*?)<\/a>/) || [])[1] || ''));
    console.log(`${label.padEnd(20)} q=${decodeURIComponent(qbox).slice(0, 24).padEnd(24)} blocks=${blocks.length} ${titles.join(' | ').slice(0, 90)}`);
  } catch (e) {
    console.log(`${label.padEnd(20)} ERR ${e.cause?.code || e.message}`);
  }
}

const e = (s) => encodeURIComponent(s);
await probe('llm英文', `https://cn.bing.com/search?q=${e('large language model inference optimization')}`);
await probe('大模型(单词)', `https://cn.bing.com/search?q=${e('大模型')}`);
await probe('大模型 推理', `https://cn.bing.com/search?q=${e('大模型 推理')}`);
await probe('无空格', `https://cn.bing.com/search?q=${e('大模型推理优化')}`);
await probe('+form=QBRE', `https://cn.bing.com/search?q=${e('大模型 推理 优化')}&form=QBRE&qs=n`);
await probe('qft+setmkt', `https://cn.bing.com/search?q=${e('大模型 推理 优化')}&setmkt=zh-CN&setlang=zh-CN&form=QBRE`);
await probe('双引号', `https://cn.bing.com/search?q=${e('"大模型" "推理优化"')}`);
await probe('混合', `https://cn.bing.com/search?q=${e('vLLM 推理 吞吐 优化')}`);
