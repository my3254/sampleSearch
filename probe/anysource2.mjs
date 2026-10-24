// Print the rest of the AnySearch README (API surface) and the repo tree.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/131.0.0.0 Safari/537.36';
const get = async (u) => {
  const r = await fetch(u, { headers: { 'user-agent': UA, accept: 'application/vnd.github+json' }, signal: AbortSignal.timeout(20000) });
  return { status: r.status, body: await r.text() };
};
const readme = await get('https://api.github.com/repos/anysearch-ai/anysearch-skill/contents/README.md');
const txt = Buffer.from(JSON.parse(readme.body).content, 'base64').toString('utf8');
console.log('===== README tail =====\n' + txt.slice(5000, 11500));

const tree = await get('https://api.github.com/repos/anysearch-ai/anysearch-skill/git/trees/main?recursive=1');
if (tree.status === 200) {
  const paths = JSON.parse(tree.body).tree.map((t) => `${t.type === 'tree' ? 'd' : 'f'} ${t.path}`);
  console.log('\n===== repo tree (first 60) =====\n' + paths.slice(0, 60).join('\n'));
}
