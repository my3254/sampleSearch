// Inspect the AnySearch reference product surface: repo metadata, README, docs.
const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36';
const H = { 'user-agent': UA, 'accept': 'application/vnd.github+json' };
const get = async (u, h = H) => {
  const r = await fetch(u, { headers: h, redirect: 'follow', signal: AbortSignal.timeout(20000) });
  return { status: r.status, body: await r.text() };
};
const strip = (s) => s.replace(/<script[\s\S]*?<\/script>/gi, ' ').replace(/<style[\s\S]*?<\/style>/gi, ' ')
  .replace(/<[^>]+>/g, ' ').replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

const repo = await get('https://api.github.com/repos/anysearch-ai/anysearch-skill');
if (repo.status === 200) {
  const j = JSON.parse(repo.body);
  console.log(`repo: ${j.full_name} stars=${j.stargazers_count} updated=${j.pushed_at}\n desc: ${j.description}\n homepage: ${j.homepage}\n`);
} else console.log('repo meta status', repo.status);

for (const f of ['README.md', 'README_zh.md']) {
  const r = await get(`https://api.github.com/repos/anysearch-ai/anysearch-skill/contents/${f}`);
  if (r.status !== 200) { console.log(`${f}: status ${r.status}`); continue; }
  const txt = Buffer.from(JSON.parse(r.body).content, 'base64').toString('utf8');
  console.log(`\n===== ${f} (${txt.length} chars, first 5000) =====\n` + txt.slice(0, 5000));
  break;
}

const home = await get('https://www.anysearch.com/', { 'user-agent': UA });
console.log(`\n===== anysearch.com home (status ${home.status}) =====\n` + strip(home.body).slice(0, 2500));
