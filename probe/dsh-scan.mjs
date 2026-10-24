// Scan the Electron app.asar for MCP / skill support strings.
// (The archive is not mountable here, so search the bytes directly.)
import { readFileSync } from 'node:fs';

const file = process.argv[2] || 'D:\\software\\deepseek\\resources\\app.asar';
const needles = ['mcpServers', 'mcp_servers', 'mcpServer', 'tools/call', 'protocolVersion', 'skillsDir', 'SKILL.md', 'mcp'];

const buf = readFileSync(file);
console.log(`file=${file} bytes=${buf.length}`);

for (const needle of needles) {
  const target = Buffer.from(needle, 'utf8');
  let idx = 0;
  let count = 0;
  const samples = [];
  while (true) {
    const at = buf.indexOf(target, idx);
    if (at === -1) break;
    count++;
    if (samples.length < 3) {
      const from = Math.max(0, at - 120);
      const to = Math.min(buf.length, at + 160);
      samples.push(buf.subarray(from, to).toString('utf8').replace(/\s+/g, ' '));
    }
    idx = at + target.length;
    if (count > 5000) break;
  }
  console.log(`\n=== "${needle}" found ${count} ===`);
  for (const s of samples) console.log('  …' + s + '…');
}
