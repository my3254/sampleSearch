// Extract DSH's MCP client docs / config shape from the app.asar bundle.
import { readFileSync } from 'node:fs';

const buf = readFileSync('D:\\software\\deepseek\\resources\\app.asar');
const text = buf.toString('utf8');

function around(needle, opts = {}) {
  const { before = 400, after = 1200, limit = 4, filter = () => true } = opts;
  const out = [];
  let idx = 0;
  while (out.length < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    const win = text.slice(Math.max(0, at - before), at + after);
    if (filter(win)) out.push(win.replace(/\u0000/g, ' ').replace(/\s+/g, ' '));
  }
  return out;
}

console.log('=== package names mentioning mcp ===');
for (const name of ['dsh-mcp', '@deepseek-ai/dsh-mcp', 'mcp-client', 'dsh-tools-mcp']) {
  const n = text.split(name).length - 1;
  if (n) console.log(`${name}: ${n}`);
}

console.log('\n=== docs window: mcpServers with transport/command ===');
for (const w of around('mcpServers', {
  limit: 3,
  before: 800,
  after: 2000,
  filter: (x) => /transport|command|stdio|url/i.test(x),
})) {
  console.log('---\n' + w.slice(0, 2600) + '\n');
}

console.log('\n=== yaml/json example shapes ===');
for (const needle of ['type: "stdio"', "type: 'stdio'", '"type": "stdio"', 'transport: stdio', 'command:']) {
  const samples = around(needle, { limit: 2, before: 300, after: 700, filter: (x) => /mcp/i.test(x) });
  if (samples.length) {
    console.log(`\n### needle: ${needle}`);
    for (const s of samples) console.log('…' + s + '…\n');
  }
}
