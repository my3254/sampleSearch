// Fetch AnySearch's AI-facing interface spec + constants (endpoint + domain list).
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/131.0.0.0 Safari/537.36';
const getRaw = async (p) => {
  const u = `https://api.github.com/repos/anysearch-ai/anysearch-skill/contents/${p}`;
  const r = await fetch(u, { headers: { 'user-agent': UA, accept: 'application/vnd.github+json' }, signal: AbortSignal.timeout(20000) });
  if (r.status !== 200) return `[${p}] status ${r.status}`;
  const j = await r.json();
  return Buffer.from(j.content, 'base64').toString('utf8');
};
const doc = await getRaw('scripts/shared/doc_spec.md');
console.log('===== doc_spec.md (' + doc.length + ' chars, first 7000) =====\n' + doc.slice(0, 7000));
const c = await getRaw('scripts/shared/constants.json');
console.log('\n===== constants.json =====\n' + c.slice(0, 3000));
