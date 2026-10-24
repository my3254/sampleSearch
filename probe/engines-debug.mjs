// Why did github/arxiv/npm/stackexchange fail in the live run?
const UA = 'samplesearch/0.1 (diagnostic)';

async function probe(name, url) {
  const t = Date.now();
  try {
    const r = await fetch(url, { headers: { 'user-agent': UA, accept: 'application/json' }, signal: AbortSignal.timeout(20000) });
    const body = await r.text();
    console.log(`${name.padEnd(12)} ${String(r.status).padEnd(4)} ${Date.now() - t}ms ${body.length}B  ${body.slice(0, 90).replace(/\s+/g, ' ')}`);
  } catch (e) {
    console.log(`${name.padEnd(12)} ERR  ${e.cause?.code || e.name}  ${e.message.slice(0, 60)}`);
  }
}

await probe('github', 'https://api.github.com/search/repositories?q=tokio&per_page=3');
await probe('npm', 'https://registry.npmjs.org/-/v1/search?text=requests&size=3');
await probe('arxiv', 'https://export.arxiv.org/api/query?search_query=all:transformer&max_results=2');
await probe('stackexchange', 'https://api.stackexchange.com/2.3/search/advanced?order=desc&sort=relevance&q=tokio&site=stackoverflow&pagesize=2');
