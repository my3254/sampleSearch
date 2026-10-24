// Binary bootstrap for the DeepSeek Harness plugin.
//
// The plugin must work the moment it is installed, on a machine that has never
// seen samplesearch. Resolution order:
//   1. explicit `binary` config
//   2. SAMPLESEARCH_BIN
//   3. `samplesearch` on PATH
//   4. the per-user install directory written by install.ps1 / install.sh
//   5. download the release asset for this platform, unpack it, and keep it
//
// Step 5 is what makes "install the plugin and it works" true. No dependencies
// beyond Node built-ins, so the package stays a two-file plugin.
import { existsSync, mkdirSync, rmSync, statSync } from 'node:fs';
import { createWriteStream } from 'node:fs';
import { join } from 'node:path';
import { homedir, platform, arch, tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const REPO = 'my3254/sampleSearch';

export function assetFor(platformName = platform(), archName = arch()) {
  if (platformName === 'win32' && archName === 'x64') return { suffix: 'win-x64.zip', binary: 'samplesearch.exe' };
  if (platformName === 'linux' && archName === 'x64') return { suffix: 'linux-x64.tar.gz', binary: 'samplesearch' };
  if (platformName === 'darwin' && archName === 'arm64') return { suffix: 'macos-arm64.tar.gz', binary: 'samplesearch' };
  return null;
}

export function installDir() {
  if (platform() === 'win32') {
    const base = process.env.LOCALAPPDATA || join(homedir(), 'AppData', 'Local');
    return join(base, 'Programs', 'samplesearch');
  }
  return join(homedir(), '.local', 'share', 'samplesearch');
}

function onPath(name) {
  const probe = spawnSync(platform() === 'win32' ? 'where' : 'which', [name], { encoding: 'utf8' });
  if (probe.status !== 0) return null;
  const first = String(probe.stdout || '').split(/\r?\n/).map((s) => s.trim()).filter(Boolean)[0];
  return first || null;
}

function usable(path) {
  try {
    if (!path || !existsSync(path) || !statSync(path).isFile()) return false;
    const run = spawnSync(path, ['help'], { encoding: 'utf8', windowsHide: true });
    return run.status === 0;
  } catch {
    return false;
  }
}

async function fetchJson(url) {
  const response = await fetch(url, {
    headers: { 'user-agent': 'samplesearch-dsh-plugin', accept: 'application/json' },
  });
  if (!response.ok) throw new Error(`GET ${url} -> HTTP ${response.status}`);
  return response.json();
}

async function download(url, destination) {
  const response = await fetch(url, { headers: { 'user-agent': 'samplesearch-dsh-plugin' } });
  if (!response.ok) throw new Error(`download ${url} -> HTTP ${response.status}`);
  await pipeline(Readable.fromWeb(response.body), createWriteStream(destination));
}

function unpack(archive, destination) {
  mkdirSync(destination, { recursive: true });
  if (archive.endsWith('.zip')) {
    const run = spawnSync(
      'powershell',
      ['-NoProfile', '-NonInteractive', '-Command', `Expand-Archive -LiteralPath '${archive}' -DestinationPath '${destination}' -Force`],
      { encoding: 'utf8', windowsHide: true },
    );
    if (run.status !== 0) throw new Error(`Expand-Archive failed: ${run.stderr || run.stdout}`);
    return;
  }
  const run = spawnSync('tar', ['-xzf', archive, '-C', destination], { encoding: 'utf8' });
  if (run.status !== 0) throw new Error(`tar failed: ${run.stderr || run.stdout}`);
}

/**
 * Return a runnable binary path, installing it first when necessary.
 * @param {{ binary?: string }} config plugin config
 */
export async function ensureBinary(config = {}) {
  for (const candidate of [config.binary, process.env.SAMPLESEARCH_BIN]) {
    if (candidate && usable(candidate)) return candidate;
  }
  const known = join(installDir(), 'bin', assetFor()?.binary || 'samplesearch');
  if (usable(known)) return known;
  const pathHit = onPath('samplesearch');
  if (pathHit && usable(pathHit)) return pathHit;

  const spec = assetFor();
  if (!spec) {
    throw new Error(
      `no prebuilt samplesearch binary for ${platform()}/${arch()}; build from source or set SAMPLESEARCH_BIN`,
    );
  }

  const release = await fetchJson(`https://api.github.com/repos/${REPO}/releases/latest`);
  const asset = (release.assets || []).find((item) => item.name.endsWith(spec.suffix));
  if (!asset) throw new Error(`release ${release.tag_name} has no asset matching *${spec.suffix}`);

  const staging = join(tmpdir(), `samplesearch-bootstrap-${process.pid}`);
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  const archive = join(staging, asset.name);
  await download(asset.browser_download_url, archive);
  unpack(archive, join(staging, 'pkg'));

  const extracted = join(staging, 'pkg', 'bin', spec.binary);
  if (!existsSync(extracted)) throw new Error(`asset ${asset.name} does not contain bin/${spec.binary}`);

  const targetDir = join(installDir(), 'bin');
  mkdirSync(targetDir, { recursive: true });
  const target = join(targetDir, spec.binary);
  const { copyFileSync, chmodSync } = await import('node:fs');
  copyFileSync(extracted, target);
  if (platform() !== 'win32') chmodSync(target, 0o755);
  rmSync(staging, { recursive: true, force: true });

  if (!usable(target)) throw new Error(`installed binary at ${target} does not run`);
  return target;
}
