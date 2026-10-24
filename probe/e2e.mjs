// End-to-end check against the local service (Node fetch works under the sandbox).
const base = process.env.SAMPLESEARCH_BASE || 'http://127.0.0.1:8787';

async function post(path, body) {
  const t0 = Date.now();
  const r = await fetch(base + path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(30000),
  });
  const text = await r.text();
  return { status: r.status, ms: Date.now() - t0, text };
}

const health = await fetch(base + '/health', { signal: AbortSignal.timeout(5000) })
  .then(async (r) => ({ status: r.status, body: await r.text() }))
  .catch((e) => ({ status: 'ERR', body: e.message }));
console.log('GET /health ->', health.status, health.body);

for (const q of ['大模型 推理 优化', 'tokio select usage', '600519 财报']) {
  const res = await post('/v1/search', { query: q, max_results: 5 });
  console.log(`\n=== "${q}" -> HTTP ${res.status} in ${res.ms}ms ===`);
  let json;
  try {
    json = JSON.parse(res.text);
  } catch {
    console.log(res.text.slice(0, 400));
    continue;
  }
  if (json.code !== 0) {
    console.log('code', json.code, 'message', json.message);
    continue;
  }
  const { results, metadata } = json.data;
  console.log('routing:', JSON.stringify(metadata.routing));
  console.log('sources:', JSON.stringify(metadata.sources));
  console.log(`total=${metadata.total_results} search_ms=${metadata.search_time_ms}`);
  results.slice(0, 5).forEach((r, i) => {
    console.log(`  ${i + 1}. [${r.source} ${r.score}] ${r.title.slice(0, 60)}`);
    console.log(`     ${r.url.slice(0, 100)}`);
    console.log(`     ${(r.snippet || '').slice(0, 90)}`);
  });
}

const bad = await post('/v1/search', { query: '', max_results: 5 });
console.log('\n=== empty query ===');
console.log('HTTP', bad.status, bad.text.slice(0, 200));
