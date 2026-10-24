// Third probe: legal / news / social vertical sources (M2 candidates).
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8' };

const targets = [
  // legal
  ['flk-npc(法规库)', 'https://flk.npc.gov.cn/api/?type=xzfg&searchType=title;vague&sortTr=f_bbrq_s;desc&sort=true&page=1&size=5'],
  ['wenshu(裁判文书)', 'https://wenshu.court.gov.cn/website/parse/rest.q4w'],
  ['chinacourt', 'https://www.chinacourt.org/index.shtml'],
  // news
  ['36kr-feed', 'https://36kr.com/feed'],
  ['ithome-rss', 'https://www.ithome.com/rss/'],
  ['sina-rss', 'https://rss.sina.com.cn/news/marquee/ddt.xml'],
  ['baidu-news-rss', 'https://news.baidu.com/ns?word=AI&tn=newsrss&sr=0&cl=2&rn=20&ct=0'],
  ['cls-telegraph', 'https://www.cls.cn/nodeapi/telegraphList?app=CailianpressWeb&os=web&sv=7.7.5'],
  ['thepaper', 'https://www.thepaper.cn/'],
  // social
  ['v2ex-hot', 'https://www.v2ex.com/api/topics/hot.json'],
  ['weibo-hot', 'https://weibo.com/ajax/side/hotSearch'],
  ['zhihu-hot', 'https://www.zhihu.com/api/v3/feed/topstory/hot-lists/total?limit=5'],
  ['reddit-json', 'https://www.reddit.com/r/programming/top.json?limit=3'],
  ['juejin-feed', 'https://api.juejin.cn/recommend_api/v1/article/recommend_all_feed?aid=2608&uuid=0'],
  ['douban-group', 'https://www.douban.com/group/explore'],
];

const rows = [];
await Promise.all(targets.map(async ([name, url]) => {
  const t0 = Date.now();
  try {
    const r = await fetch(url, { headers: H, signal: AbortSignal.timeout(12000) });
    const buf = new Uint8Array(await r.arrayBuffer());
    const ct = (r.headers.get('content-type') || '').split(';')[0];
    let peek = '';
    try { peek = new TextDecoder('utf-8').decode(buf.slice(0, 200)).replace(/\s+/g, ' '); } catch {}
    rows.push({ name, status: r.status, ms: Date.now() - t0, bytes: buf.byteLength, ct, peek });
  } catch (e) {
    rows.push({ name, err: e.cause?.code || e.name, ms: Date.now() - t0 });
  }
}));
rows.sort((a, b) => a.name.localeCompare(b.name));
for (const r of rows) {
  console.log(
    r.name.padEnd(18),
    String(r.status ?? 'ERR').padEnd(5),
    String(r.ms).padStart(6) + 'ms',
    String(r.bytes ?? '-').padStart(8) + 'B',
    (r.ct || r.err || '').padEnd(22),
    (r.peek || '').slice(0, 90),
  );
}
