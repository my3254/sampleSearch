// Versions for the plugin/scoring architecture crates.
const UA = 'samplesearch-probe/0.1';
const crates = [
  'async-trait', 'arc-swap', 'notify', 'jmespath', 'jsonpath-rust', 'serde_json_path',
  'jsonschema', 'moka', 'redb', 'governor', 'rmcp', 'tower', 'tower-http', 'axum',
  'reqwest', 'scraper', 'quick-xml', 'encoding_rs', 'chrono', 'unicode-segmentation',
  'serde_yaml', 'toml', 'figment', 'arc_swap', 'dashmap', 'parking_lot', 'wasmtime',
  'libloading', 'pulldown-cmark', 'htmd',
];
const out = [];
for (const c of crates) {
  try {
    const r = await fetch(`https://crates.io/api/v1/crates/${c}`, { headers: { 'user-agent': UA, accept: 'application/json' }, signal: AbortSignal.timeout(12000) });
    if (r.status !== 200) { out.push(`${c.padEnd(22)} HTTP ${r.status}`); continue; }
    const j = await r.json();
    out.push(`${c.padEnd(22)} ${String(j.crate.max_stable_version || j.crate.max_version).padEnd(12)} dl=${j.crate.downloads}  ${(j.crate.description || '').slice(0, 60)}`);
  } catch (e) {
    out.push(`${c.padEnd(22)} ERR ${e.cause?.code || e.message}`);
  }
}
console.log(out.join('\n'));
