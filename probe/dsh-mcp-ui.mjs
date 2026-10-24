// Does DSH have a dedicated MCP settings UI (not the plugin page)?
import { readFileSync } from 'node:fs';

const text = readFileSync('D:\\software\\deepseek\\resources\\app.asar').toString('utf8');

const needles = ['MCP 服务器', 'MCP 服务', '服务器配置', 'mcpServers', 'addMcpServer', 'mcp-server', 'MCP Server'];
for (const needle of needles) {
  const count = text.split(needle).length - 1;
  if (!count) continue;
  console.log(`\n### ${needle}  (${count})`);
  let idx = 0;
  let shown = 0;
  while (shown < 3) {
    const at = text.indexOf(needle, idx);
    if (at === -1) break;
    idx = at + needle.length;
    const win = text.slice(Math.max(0, at - 200), at + 300).replace(/\s+/g, ' ');
    console.log('  …' + win.slice(0, 480) + '…');
    shown++;
  }
}
