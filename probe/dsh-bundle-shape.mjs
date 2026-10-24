// What does a dsh bundle manifest look like? (bundlePatchPaths + bundle schema)
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

for (const needle of ['function bundlePatchPaths', 'bundlePatchPaths =', 'dsh?.bundle', 'interface DshBundle', 'bundle:']) {
  const at = text.indexOf(needle);
  if (at === -1) continue;
  console.log(`\n=== ${needle} @${at} ===`);
  console.log(text.slice(Math.max(0, at - 300), at + 1400).replace(/\s+/g, ' '));
}
