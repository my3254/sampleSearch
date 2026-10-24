// samplesearch — native DeepSeek Harness plugin.
//
// Registers search tools directly on `ctx.tools` (via @deepseek-ai/dsh-tools), so a
// DSH user installs this one package and nothing else: no MCP bridge, no separate
// dsh-mcp-client, no MCP config file. The work is done by the Rust binary, which the
// installer puts on PATH; `binary` overrides that path when it lives elsewhere.
import { spawn } from 'node:child_process';
import { ensureBinary } from './bootstrap.js';

export const name = 'dsh-samplesearch';
export const inject = ['tools'];

const DEFAULT_TIMEOUT_MS = 45_000;

/**
 * Local equivalent of `defineTool` from @deepseek-ai/dsh-tools.
 *
 * Importing the official helper would make this plugin depend on a package that is
 * resolved from the harness installation, not from the profile — one missing peer and
 * the plugin silently fails to load. The helper only converts a compact parameter spec
 * into JSON Schema and wraps the output renderer, so we do that here and stay
 * dependency-free.
 */
function defineTool(options) {
  const properties = {};
  const required = [];
  for (const [key, spec] of Object.entries(options.parameters || {})) {
    const { required: isRequired, ...rest } = spec;
    properties[key] = rest;
    if (isRequired) required.push(key);
  }
  return {
    name: options.name,
    description: options.description,
    parameters: {
      type: 'object',
      properties,
      ...(required.length > 0 ? { required } : {}),
    },
    output: {
      schema: options.output.schema,
      render: options.output.render,
    },
    execute: options.execute,
  };
}


/** Run the CLI once and parse its JSON envelope. */
function runCli(bin, args, timeoutMs) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = (fn, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      fn(value);
    };

    let child;
    try {
      child = spawn(bin, args, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (error) {
      reject(error);
      return;
    }

    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => {
      try {
        child.kill();
      } catch {
        /* already gone */
      }
      finish(reject, new Error(`samplesearch timed out after ${timeoutMs} ms`));
    }, timeoutMs);

    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', (error) => {
      if (error && error.code === 'ENOENT') {
        finish(
          reject,
          new Error(
            `samplesearch binary not found (${bin}). Install it, or set the plugin's "binary" option / SAMPLESEARCH_BIN.`,
          ),
        );
        return;
      }
      finish(reject, error);
    });
    child.on('close', (code) => {
      if (code !== 0) {
        finish(reject, new Error(`samplesearch exited with ${code}: ${(stderr || stdout).trim()}`));
        return;
      }
      try {
        finish(resolve, JSON.parse(stdout));
      } catch {
        finish(reject, new Error(`samplesearch returned invalid JSON: ${stdout.slice(0, 300)}`));
      }
    });
  });
}

/** Render one envelope as compact markdown the model can cite. */
function renderEnvelope(envelope) {
  const data = envelope && envelope.data;
  if (!data) {
    return `search failed (code ${envelope && envelope.code}): ${(envelope && envelope.message) || 'unknown error'}`;
  }
  const lines = [];
  const results = data.results || [];
  results.forEach((item, index) => {
    lines.push(`${index + 1}. ${item.title}`);
    lines.push(`   ${item.url}`);
    if (item.snippet) lines.push(`   ${String(item.snippet).slice(0, 220)}`);
  });
  if (lines.length === 0) {
    lines.push('No results survived the relevance filters. Try broader or rephrased keywords.');
  }
  const meta = data.metadata || {};
  const sources = (meta.sources || []).map((s) => `${s.name}=${s.status}`).join(' ');
  lines.push(
    `\n(${meta.total_results ?? 0} results, ${meta.search_time_ms ?? '?'} ms${sources ? `; sources: ${sources}` : ''})`,
  );
  return lines.join('\n');
}

const outputSpec = {
  schema: {
    type: 'object',
    properties: { text: { type: 'string' } },
    required: ['text'],
  },
  render: (_args, value) => [{ type: 'text', text: String(value && value.text ? value.text : '') }],
};

export function apply(ctx, config) {
  const timeoutMs = Number(config && config.timeoutMs) > 0 ? Number(config.timeoutMs) : DEFAULT_TIMEOUT_MS;
  // Resolve (and if necessary install) the binary once, on first tool call, so plugin
  // registration never blocks startup on a download.
  let binaryPromise = null;
  const binary = () => {
    binaryPromise ??= ensureBinary(config || {}).catch((error) => {
      binaryPromise = null;
      throw new Error(`samplesearch is not available: ${error.message}`);
    });
    return binaryPromise;
  };
  const runWith = async (args) => runCli(await binary(), args, timeoutMs);

  ctx.tools.register(
    defineTool({
      name: 'search',
      description:
        'Search the web through the local samplesearch service. Returns ranked results with title, url and snippet, plus engine routing metadata. Use for current facts, news, docs, papers or prices.',
      parameters: {
        query: { type: 'string', required: true, description: 'Natural-language query, Chinese or English.' },
        max_results: { type: 'integer', description: 'How many results, 1-10. Defaults to 10.' },
        explain: { type: 'boolean', description: 'Include per-feature score contributions (debugging).' },
      },
      output: outputSpec,
      async execute(args) {
        const cliArgs = ['search', args.query];
        if (args.max_results) cliArgs.push('--max', String(args.max_results));
        if (args.explain) cliArgs.push('--explain');
        const envelope = await runWith(cliArgs);
        return { text: renderEnvelope(envelope) };
      },
    }),
  );

  ctx.tools.register(
    defineTool({
      name: 'batch_search',
      description:
        'Run 2-5 independent search queries and return each result set. Use for multi-angle questions or when comparing several entities.',
      parameters: {
        queries: {
          type: 'array',
          required: true,
          description: 'Queries to run, 1-5 items.',
          items: { type: 'string' },
        },
        max_results: { type: 'integer', description: 'How many results per query, 1-10.' },
      },
      output: outputSpec,
      async execute(args) {
        const queries = Array.isArray(args.queries) ? args.queries.slice(0, 5) : [];
        if (queries.length === 0) return { text: 'batch_search needs at least one query.' };
        const sections = [];
        for (const query of queries) {
          const cliArgs = ['search', query];
          if (args.max_results) cliArgs.push('--max', String(args.max_results));
          try {
            const envelope = await runWith(cliArgs);
            sections.push(`## ${query}\n${renderEnvelope(envelope)}`);
          } catch (error) {
            sections.push(`## ${query}\nsearch failed: ${error.message}`);
          }
        }
        return { text: sections.join('\n\n') };
      },
    }),
  );

  ctx.tools.register(
    defineTool({
      name: 'search_health',
      description: 'Report which search engines samplesearch has enabled and whether overseas engines are switched on.',
      parameters: {},
      output: outputSpec,
      async execute() {
        const envelope = await runWith(['health']);
        const payload = envelope && envelope.data ? envelope.data : envelope;
        return { text: JSON.stringify(payload, null, 2) };
      },
    }),
  );
}
