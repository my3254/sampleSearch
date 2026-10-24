// Extract the market-index and plugin-manager READMEs straight out of the asar by offset.
import { readFileSync } from 'node:fs';

const buf = readFileSync('D:\\software\\deepseek\\resources\\app.asar');

function slice(offset, size) {
  return buf.subarray(Number(offset), Number(offset) + Number(size)).toString('utf8');
}

const files = [
  ['market-index README.md', 79725724, 3446],
  ['market-index package.json', 79719850, 3591],
  ['plugin-manager README.md (first 9000)', 79254341, 9000],
];

for (const [label, off, size] of files) {
  console.log(`\n===== ${label} @${off} =====\n`);
  console.log(slice(off, size));
}
