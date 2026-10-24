// Fourth probe: RSSHub / aggregation proxies and remaining social+news sources.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8' };

const targets = [
  ['rsshub.app zhihu', 'https://rsshub.app/zhihu/hot'],
  ['rsshub.app v2ex', 'https://rsshub.app/v2ex/topics/hot'],
  ['rsshub.app weibo', 'https://rsshub.app/weibo/search/hot'],
  ['rsshub.rssforever', 'https://rsshub.rssforever.com/zhihu/hot'],
  ['cnbeta-rss', 'https://www.cnbeta.com.tw/backend.php'],
  ['36kr-newsflash', 'https://36kr.com/feed-newsflash'],
  ['sspai-feed', 'https://sspai.com/feed'],
  ['tieba', 'https://tieba.baidu.com/f?kw=ai'],
  ['douban-movie', 'https://movie.douban.com/'],
  ['juejin-post', 'https://api.juejin.cn/recommend_api/v1/article/recommend_cate_feed'],
  ['flk-post', 'https://flk.npc.gov.cn/api/detail?id=1'],
  ['gov-law', 'https://www.gov.cn/zhengce/zuixin/'],
];

const rows = [];
await Promise.all(targets.map(async ([name, url]) => {
  const t0 = Date.now();
  const opt = { headers: H, signal: AbortSignal.timeout(12000) };
  if (name === 'juejin-post') { opt.method = 'POST'; opt.body = '{"id_type":2,"client_type":2608,"sort_type":200,"cursor":"0","limit":5}'; opt.headers = { ...H, 'content-type': 'application/json' }; }
  try {
    const r = await fetch(url, opt);
    const buf = new Uint8Array(await r.arrayBuffer());
    const ct = (r.headers.get('content-type') || '').split(';')[0];
    let peek = '';
    try { peek = new TextDecoder('utf-8').decode(buf.slice(0, 180)).replace(/\s+/g, ' '); } catch {}
    rows.push({ name, status: r.status, ms: Date.now() - t0, bytes: buf.byteLength, ct, peek });
  } catch (e) {
    rows.push({ name, err: e.cause?.code || e.name, ms: Date.now() - t0 });
  }
}));
rows.sort((a, b) => a.name.localeCompare(b.name));
for (const r of rows) {
  console.log(
    r.name.padEnd(19),
    String(r.status ?? 'ERR').padEnd(5),
    String(r.ms).padStart(6) + 'ms',
    String(r.bytes ?? '-').padStart(8) + 'B',
    (r.ct || r.err || '').padEnd(18),
    (r.peek || '').slice(0, 80),
  );
}
