#!/usr/bin/env bash
# Install samplesearch (self-hosted search for AI agents) on macOS / Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/my3254/sampleSearch/main/install.sh | bash
#
# Options (as arguments or env):
#   --version <tag|latest>     release to install (default: latest)
#   --prefix <dir>             install root (default: ~/.local/share/samplesearch)
#   --from-local <dir>         install from an extracted release directory
#   --skip-skill               do not install the agent skill
#   --no-path                  do not touch shell profile / symlink
set -euo pipefail

REPO="${SAMPLESEARCH_REPO:-my3254/sampleSearch}"
VERSION="latest"
PREFIX="${SAMPLESEARCH_PREFIX:-$HOME/.local/share/samplesearch}"
FROM_LOCAL=""
SKIP_SKILL=0
NO_PATH=0

while [ $# -gt 0 ]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --prefix) PREFIX="$2"; shift 2 ;;
    --from-local) FROM_LOCAL="$2"; shift 2 ;;
    --skip-skill) SKIP_SKILL=1; shift ;;
    --no-path) NO_PATH=1; shift ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

say() { printf '\033[36m==> %s\033[0m\n' "$1"; }
ok() { printf '\033[32m    %s\033[0m\n' "$1"; }

case "$(uname -s)" in
  Darwin) OS="macos" ;;
  Linux) OS="linux" ;;
  *) echo "unsupported OS: $(uname -s) — Windows users: run install.ps1" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) ARCH="x64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) echo "unsupported arch: $(uname -m)" >&2; exit 1 ;;
esac
# Only the shipped asset names are supported; refuse anything else loudly.
ASSET="${OS}-${ARCH}.tar.gz"
case "$ASSET" in
  macos-arm64.tar.gz|linux-x64.tar.gz) ;;
  *) echo "no prebuilt asset for ${OS}/${ARCH} yet; build from source: cargo build --release" >&2; exit 1 ;;
esac

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

if [ -n "$FROM_LOCAL" ]; then
  say "using local package: $FROM_LOCAL"
  PKG="$FROM_LOCAL"
else
  if [ "$VERSION" = "latest" ]; then
    API="https://api.github.com/repos/$REPO/releases/latest"
  else
    API="https://api.github.com/repos/$REPO/releases/tags/$VERSION"
  fi
  say "resolving release ($VERSION)"
  REL_JSON="$TMP/release.json"
  curl -fsSL -H 'user-agent: samplesearch-installer' "$API" -o "$REL_JSON"
  URL="$(grep -o "\"browser_download_url\": *\"[^\"]*${ASSET}\"" "$REL_JSON" | head -n1 | sed 's/.*"\(https[^"]*\)"/\1/')"
  if [ -z "$URL" ]; then echo "release has no asset matching *${ASSET}" >&2; exit 1; fi
  say "downloading $(basename "$URL")"
  curl -fsSL "$URL" -o "$TMP/pkg.tar.gz"
  mkdir -p "$TMP/pkg"
  tar -xzf "$TMP/pkg.tar.gz" -C "$TMP/pkg"
  PKG="$TMP/pkg"
fi

if [ ! -x "$PKG/bin/samplesearch" ] && [ ! -f "$PKG/bin/samplesearch" ]; then
  echo "package does not contain bin/samplesearch" >&2
  exit 1
fi

say "installing to $PREFIX"
mkdir -p "$PREFIX/bin"
install -m 0755 "$PKG/bin/samplesearch" "$PREFIX/bin/samplesearch"
for f in README.md SKILL.md install.ps1 install.sh; do
  [ -f "$PKG/$f" ] && cp "$PKG/$f" "$PREFIX/$f"
done
[ -d "$PKG/integrations" ] && cp -R "$PKG/integrations" "$PREFIX/integrations"
ok "binary: $PREFIX/bin/samplesearch"

if [ "$SKIP_SKILL" -eq 0 ] && [ -f "$PREFIX/SKILL.md" ]; then
  mkdir -p "$HOME/.agents/skills/samplesearch"
  cp "$PREFIX/SKILL.md" "$HOME/.agents/skills/samplesearch/SKILL.md"
  ok "skill: $HOME/.agents/skills/samplesearch"
  if [ -d "$HOME/.claude" ]; then
    mkdir -p "$HOME/.claude/skills/samplesearch"
    cp "$PREFIX/SKILL.md" "$HOME/.claude/skills/samplesearch/SKILL.md"
    ok "skill: $HOME/.claude/skills/samplesearch"
  fi
fi

if [ "$NO_PATH" -eq 0 ]; then
  if [ -w /usr/local/bin ]; then
    ln -sf "$PREFIX/bin/samplesearch" /usr/local/bin/samplesearch 2>/dev/null && ok "linked /usr/local/bin/samplesearch"
  else
    case ":$PATH:" in
      *":$PREFIX/bin:"*) ok "already on PATH" ;;
      *) ok "add this to your shell profile: export PATH=\"$PREFIX/bin:\$PATH\"" ;;
    esac
  fi
fi

say "verify"
"$PREFIX/bin/samplesearch" health || true

say "MCP configuration (paste into your client)"
cat <<JSON
{
  "mcpServers": {
    "samplesearch": {
      "command": "$PREFIX/bin/samplesearch",
      "args": ["mcp"]
    }
  }
}
JSON

echo
echo "Done. Try:  $PREFIX/bin/samplesearch search \"tokio select usage\" -m 5"
