// Survey: which search sources do "search MCP" servers actually use?
// Data comes from npm + GitHub APIs (public metadata only).
const UA = 'samplesearch-survey/0.1';

async function json(url) {
  const r = await fetch(url, { headers: { 'user-agent': UA, accept: 'application/json' }, signal: AbortSignal.timeout(20000) });
  if (!r.ok) throw new Error(`${url} -> ${r.status}`);
  return r.json();
}

console.log('=== npm: "mcp search" (top hits) ===');
const npm = await json('https://registry.npmjs.org/-/v1/search?text=mcp%20search&size=25');
for (const o of npm.objects || []) {
  const p = o.package;
  console.log(`- ${p.name}@${p.version}  ${(p.description || '').slice(0, 110)}`);
}

console.log('\n=== GitHub: repos matching "search mcp server" (stars >= 300) ===');
const gh = await json('https://api.github.com/search/repositories?q=search+mcp+server+in:name,description&sort=stars&order=desc&per_page=25');
for (const r of gh.items || []) {
  if (r.stargazers_count < 300) continue;
  console.log(`- ${r.full_name}  ★${r.stargazers_count}  ${(r.description || '').slice(0, 100)}`);
}

console.log('\n=== GitHub: "web search mcp" (stars >= 300) ===');
const gh2 = await json('https://api.github.com/search/repositories?q=web+search+mcp+in:name,description&sort=stars&order=desc&per_page=25');
for (const r of gh2.items || []) {
  if (r.stargazers_count < 300) continue;
  console.log(`- ${r.full_name}  ★${r.stargazers_count}  ${(r.description || '').slice(0, 100)}`);
}
