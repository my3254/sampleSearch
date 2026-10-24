// How does a DSH cordis plugin register a tool? (AnySearch's plugin works without mcp-client,
// so a native tool-registration API must exist.)
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

function around(needle, before = 400, after = 900, limit = 3) {
  const out = [];
  let idx = 0;
  while (out.length < limit) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    out.push(text.slice(Math.max(0, at - before), at + after).replace(/\s+/g, ' '));
  }
  return out;
}

const needles = [
  'ctx.tool(',
  'defineTool',
  'registerTool',
  'tools.define',
  'toolProvider',
  'inject = ["tools"]',
  'ToolDefinition',
];

for (const n of needles) {
  const hits = around(n, 300, 700, 2);
  if (!hits.length) continue;
  console.log(`\n=== ${n} (${text.split(n).length - 1}) ===`);
  for (const h of hits) console.log('---\n' + h.slice(0, 900) + '\n');
}
