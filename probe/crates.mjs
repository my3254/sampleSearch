// Check crates.io reachability + availability/versions of the crates a Rust port would need.
const UA = 'mysearch-probe/0.1 (local feasibility check)';
const crates = [
  'axum', 'tokio', 'reqwest', 'serde', 'serde_json', 'scraper', 'readability',
  'html2md', 'htmd', 'moka', 'governor', 'rusqlite', 'sqlx', 'rmcp', 'tower-http',
  'tracing', 'tracing-subscriber', 'regex', 'url', 'clap', 'anyhow', 'thiserror',
  'quick-xml', 'serde_yaml', 'notify',
];
const out = [];
for (const c of crates) {
  try {
    const r = await fetch(`https://crates.io/api/v1/crates/${c}`, {
      headers: { 'user-agent': UA, accept: 'application/json' },
      signal: AbortSignal.timeout(12000),
    });
    if (r.status !== 200) { out.push(`${c.padEnd(20)} ${r.status}`); continue; }
    const j = await r.json();
    out.push(`${c.padEnd(20)} ${String(j.crate.max_stable_version || j.crate.max_version).padEnd(12)} dl=${j.crate.downloads}`);
  } catch (e) {
    out.push(`${c.padEnd(20)} ERR ${e.cause?.code || e.message}`);
  }
}
console.log(out.join('\n'));
