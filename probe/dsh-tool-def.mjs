// Exact shape of defineTool + tool registration, to write a native DSH plugin.
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

function windows(needle, before, after, limit = 3, filter = () => true) {
  const out = [];
  let idx = 0;
  while (out.length < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    const w = text.slice(Math.max(0, at - before), at + after);
    if (filter(w)) out.push(w.replace(/\s+/g, ' '));
  }
  return out;
}

console.log('=== defineTool definition ===');
for (const w of windows('function defineTool', 200, 1600, 2)) console.log('---\n' + w.slice(0, 1800) + '\n');

console.log('\n=== defineTool({ usage examples ===');
for (const w of windows('defineTool({', 500, 1500, 4)) console.log('---\n' + w.slice(0, 2000) + '\n');

console.log('\n=== ctx.tools.* verbs ===');
const verbs = new Set();
for (const m of text.matchAll(/ctx\.tools\.([a-zA-Z]+)\(/g)) verbs.add(m[1]);
console.log([...verbs].join(', '));
