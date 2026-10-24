// How does the DSH plugin market discover packages? Look for catalog/registry URLs,
// required keywords/metadata, and the "申请收录插件" target.
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

function around(needle, before = 500, after = 900, limit = 2) {
  const hits = [];
  let idx = 0;
  while (hits.length < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    hits.push(text.slice(Math.max(0, at - before), at + after).replace(/\s+/g, ' '));
  }
  return hits;
}

const needles = [
  '申请收录插件',
  'registry.npmjs.org/-/v1/search',
  'dsh-plugin',
  'dsh-bundle',
  'catalog',
  'marketplace',
  'keywords',
];

for (const needle of needles) {
  const hits = around(needle, 400, 700, 2);
  if (!hits.length) continue;
  console.log(`\n=== ${needle} (${text.split(needle).length - 1} hits) ===`);
  for (const h of hits) console.log('---\n' + h.slice(0, 1100) + '\n');
}
