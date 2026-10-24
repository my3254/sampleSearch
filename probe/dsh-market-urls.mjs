// Which URLs back the DSH plugin market, and where does "申请收录插件" point?
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

const urls = new Set();
for (const m of text.matchAll(/https?:\/\/[A-Za-z0-9._~:/?#\[\]@!$&'()*+,;=%-]{6,120}/g)) {
  const u = m[0];
  if (/dsh|market|plugin|deepseek/i.test(u)) urls.add(u.replace(/[),.;]+$/, ''));
}
console.log('=== URL 候选 ===');
for (const u of [...urls].sort().slice(0, 60)) console.log('  ' + u);

console.log('\n=== 含"收录"的片段 ===');
let idx = 0;
let n = 0;
while (n < 4) {
  const at = text.indexOf('收录', idx);
  if (at === -1) break;
  idx = at + 2;
  console.log('---\n' + text.slice(Math.max(0, at - 400), at + 400).replace(/\s+/g, ' ') + '\n');
  n++;
}
