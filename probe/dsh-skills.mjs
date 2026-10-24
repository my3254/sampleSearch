// Find how DSH expects user skills to be installed (path + frontmatter), and
// whether the mcp-client bundle is shipped by default.
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

console.log('=== skill 安装目录相关文档 ===');
for (const w of windows('skills', 300, 900, 6, (x) => /DSH_HOME|\.dsh|install|directory|folder/i.test(x))) {
  console.log('---\n' + w.slice(0, 1200) + '\n');
}

console.log('=== mcp-client 是否为内置 bundle ===');
for (const needle of ['@deepseek-ai/dsh-mcp-client', 'dsh-mcp-client']) {
  const n = text.split(needle).length - 1;
  console.log(`${needle}: ${n}`);
}
for (const w of windows('@deepseek-ai/dsh-mcp-client', 500, 800, 3)) {
  console.log('---\n' + w.slice(0, 1300) + '\n');
}
