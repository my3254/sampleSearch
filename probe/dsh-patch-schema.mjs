// Find the patch-entry schema: what does a *new entry* (not an override) look like?
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

function show(needle, before = 600, after = 1600, limit = 2) {
  let idx = 0;
  let n = 0;
  while (n < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    console.log(`\n=== ${needle} @${at} ===`);
    console.log(text.slice(Math.max(0, at - before), at + after).replace(/\s+/g, ' '));
    n++;
  }
}

show('patch: entry', 500, 1200, 1);
show('insert', 200, 600, 3);
show('PatchEntry', 200, 900, 2);
