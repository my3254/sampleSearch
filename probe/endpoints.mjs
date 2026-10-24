// Reachability + parseability probe for candidate free search sources.
// Run: node probe/endpoints.mjs [query]
// Node's fetch is used because PowerShell/curl schannel cannot create TLS
// credentials under the confined token; Node's OpenSSL stack works.

const q = process.argv[2] || 'anysearch';
const enc = encodeURIComponent(q);
const UA =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ' +
  '(KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';

const targets = [
  { n: 'bing-rss', u: `https://www.bing.com/search?q=${enc}&format=rss&count=20`, kind: 'rss' },
  { n: 'bing-cn-html', u: `https://cn.bing.com/search?q=${enc}`, kind: 'html' },
  { n: 'ddg-html', u: `https://html.duckduckgo.com/html/?q=${enc}`, kind: 'html' },
  { n: 'ddg-lite', u: `https://lite.duckduckgo.com/lite/?q=${enc}`, kind: 'html' },
  { n: 'baidu', u: `https://www.baidu.com/s?wd=${enc}`, kind: 'html' },
  { n: 'sogou', u: `https://www.sogou.com/web?query=${enc}`, kind: 'html' },
  { n: 'so360', u: `https://www.so.com/s?q=${enc}`, kind: 'html' },
  { n: 'mojeek', u: `https://www.mojeek.com/search?q=${enc}`, kind: 'html' },
  { n: 'brave-html', u: `https://search.brave.com/search?q=${enc}`, kind: 'html' },
  { n: 'startpage', u: `https://www.startpage.com/sp/search?query=${enc}`, kind: 'html' },
  { n: 'searx-be-json', u: `https://searx.be/search?q=${enc}&format=json`, kind: 'json' },
  { n: 'searxng-site', u: `https://searxng.site/search?q=${enc}&format=json`, kind: 'json' },
  { n: 'wiki-api', u: `https://zh.wikipedia.org/w/api.php?action=query&list=search&srsearch=${enc}&format=json&utf8=1`, kind: 'json' },
  { n: 'gh-api', u: `https://api.github.com/search/repositories?q=${enc}&per_page=5`, kind: 'json' },
  { n: 'hn-api', u: `https://hn.algolia.com/api/v1/search?query=${enc}&hitsPerPage=5`, kind: 'json' },
];

const CAPTCHA = [/验证码/, /安全验证/, /captcha/i, /unusual traffic/i, /robot check/i, /请完成下方验证/i];

function signals(kind, body) {
  const out = {};
  if (kind === 'rss') out.items = (body.match(/<item>/g) || []).length;
  if (kind === 'html') {
    out.h2 = (body.match(/<h2/g) || []).length;
    out.anchors = (body.match(/<a\s/gi) || []).length;
  }
  for (const re of CAPTCHA) if (re.test(body)) out.captcha = re.source;
  return out;
}

async function probe(t) {
  const started = Date.now();
  const rec = { name: t.n };
  try {
    const res = await fetch(t.u, {
      headers: {
        'user-agent': UA,
        'accept': t.kind === 'json' ? 'application/json' : 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
        'accept-language': 'zh-CN,zh;q=0.9,en;q=0.8',
      },
      redirect: 'follow',
      signal: AbortSignal.timeout(12000),
    });
    const body = await res.text();
    rec.status = res.status;
    rec.finalUrl = res.url !== t.u ? res.url.slice(0, 90) : undefined;
    rec.ctype = (res.headers.get('content-type') || '').split(';')[0];
    rec.bytes = body.length;
    rec.ms = Date.now() - started;
    rec.signals = signals(t.kind, body);
    if (t.kind === 'json') {
      try {
        const j = JSON.parse(body);
        rec.jsonKeys = Object.keys(j).slice(0, 6);
      } catch {
        rec.jsonKeys = 'PARSE_FAIL';
      }
    }
    if (t.kind === 'rss') {
      rec.titles = [...body.matchAll(/<title>(?:<!\[CDATA\[)?([^<\]]{1,90})/g)].slice(1, 4).map((m) => m[1]);
    }
  } catch (e) {
    rec.error = `${e.name}: ${e.message}${e.cause?.code ? ` (${e.cause.code})` : ''}`;
    rec.ms = Date.now() - started;
  }
  return rec;
}

const results = await Promise.all(targets.map(probe));
const pad = (s, n) => String(s ?? '').padEnd(n).slice(0, n);
console.log(`query="${q}"  ${new Date().toISOString()}\n`);
console.log(pad('source', 16), pad('status', 8), pad('ms', 7), pad('bytes', 8), pad('type', 20), 'signals');
console.log('-'.repeat(110));
for (const r of results) {
  const sig = r.error
    ? `ERROR ${r.error}`
    : JSON.stringify(r.signals) + (r.jsonKeys ? ` json=${JSON.stringify(r.jsonKeys)}` : '') + (r.finalUrl ? ` -> ${r.finalUrl}` : '');
  console.log(pad(r.name, 16), pad(r.status, 8), pad(r.ms, 7), pad(r.bytes, 8), pad(r.ctype, 20), sig);
}
const rss = results.find((r) => r.name === 'bing-rss');
if (rss?.titles?.length) console.log('\nbing-rss sample titles:\n - ' + rss.titles.join('\n - '));
