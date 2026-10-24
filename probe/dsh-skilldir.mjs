// Where does DSH load user skills from?
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

function windows(needle, before, after, limit, filter = () => true) {
  const out = [];
  let idx = 0;
  while (out.length < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    const win = text.slice(Math.max(0, at - before), at + after);
    if (filter(win)) out.push(win.replace(/\s+/g, ' '));
  }
  return out;
}

for (const needle of ['skillDir', 'skillsDir', 'skillRoot', 'skills:', 'office-skills']) {
  const ws = windows(needle, 250, 650, 3, (x) => /\.dsh|DSH_HOME|userData|resource|dir|path/i.test(x));
  if (!ws.length) continue;
  console.log(`\n### ${needle} (${text.split(needle).length - 1} hits)`);
  for (const w of ws) console.log('---\n' + w.slice(0, 900) + '\n');
}
