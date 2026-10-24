// Check feature flags for reqwest 0.13 / axum 0.8 / tokio / scraper before writing Cargo.toml.
const UA = 'mysearch-probe/0.1';
const want = [
  ['reqwest', '0.13.5'],
  ['axum', '0.8.9'],
  ['scraper', '0.29.0'],
  ['quick-xml', '0.42.0'],
];
for (const [name, ver] of want) {
  const r = await fetch(`https://crates.io/api/v1/crates/${name}/${ver}`, { headers: { 'user-agent': UA, accept: 'application/json' }, signal: AbortSignal.timeout(15000) });
  if (r.status !== 200) { console.log(`${name} ${ver} -> HTTP ${r.status}`); continue; }
  const j = await r.json();
  const feats = j.version.features || {};
  const keys = Object.keys(feats);
  console.log(`\n=== ${name} ${ver} (${keys.length} features) ===`);
  const important = keys.filter((k) => /tls|json|gzip|charset|brotli|deflate|stream|blocking|cookies|http2|default|query|form/.test(k));
  for (const k of important.sort()) console.log(`  ${k.padEnd(28)} = ${JSON.stringify(feats[k]).slice(0, 120)}`);
}
