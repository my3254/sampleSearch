// What makes DSH show "已安装，未生效" and how does it decide a plugin is loaded?
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');
const needles = ['未生效', '已安装，未生效', 'not-a-bundle', 'declares no dsh.bundle', '由某个 bundle patch 按名加载'];

for (const needle of needles) {
  const count = text.split(needle).length - 1;
  if (!count) { continue; }
  console.log(`\n=== ${needle} (${count}) ===`);
  let idx = 0;
  let shown = 0;
  while (shown < 2) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    const win = text.slice(Math.max(0, at - 700), at + 500).replace(/\s+/g, ' ');
    console.log('---\n' + win.slice(0, 1200) + '\n');
    shown++;
  }
}
