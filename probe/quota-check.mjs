// Do these seven sources need payment? Measure their anonymous quota right now.
const UA = 'samplesearch-survey/0.1 (contact: local)';

async function probe(name, url, pick) {
  try {
    const r = await fetch(url, { headers: { 'user-agent': UA, accept: 'application/json' }, signal: AbortSignal.timeout(20000) });
    const text = await r.text();
    let info = '';
    try {
      info = pick ? pick(JSON.parse(text)) : `${text.length}B`;
    } catch {
      info = `${text.length}B (not json)`;
    }
    console.log(`${name.padEnd(16)} ${String(r.status).padEnd(4)} ${info}`);
  } catch (e) {
    console.log(`${name.padEnd(16)} ERR  ${e.cause?.code || e.message}`);
  }
}

await probe('github search', 'https://api.github.com/rate_limit', (j) => {
  const s = j.resources.search;
  return `search ${s.remaining}/${s.limit} per ${Math.round((s.reset - Date.now() / 1000) / 60)}min; core ${j.resources.core.remaining}/${j.resources.core.limit}`;
});

await probe('hn algolia', 'https://hn.algolia.com/api/v1/search?query=test&hitsPerPage=1', (j) => `hits ${j.hits?.length}, nbHits ${j.nbHits}`);

await probe('arxiv api', 'http://export.arxiv.org/api/query?search_query=all:electron&max_results=1', (t) => undefined);

await probe('crossref', 'https://api.crossref.org/works?query=test&rows=1', (j) => `total ${j.message?.['total-results']}`);

await probe('npm search', 'https://registry.npmjs.org/-/v1/search?text=test&size=1', (j) => `total ${j.total}`);

await probe('pypi json', 'https://pypi.org/pypi/requests/json', (j) => `name ${j.info?.name}, version ${j.info?.version}`);

await probe('stackexchange', 'https://api.stackexchange.com/2.3/info?site=stackoverflow', (j) => {
  const q = j.quota_remaining;
  return `quota ${q}/${j.quota_max} (anonymous)`;
});

await probe('se search', 'https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle=tokio&site=stackoverflow', (j) => {
  const q = j.quota_remaining;
  return `items ${j.items?.length}, quota left ${q}`;
});
