<#
.SYNOPSIS
  Install mysearch (self-hosted search for AI agents) on Windows.

.DESCRIPTION
  Downloads the release binary, installs it under %LOCALAPPDATA%\Programs\mysearch,
  adds it to the user PATH, installs the agent skill into ~/.agents/skills/mysearch
  (and ~/.claude/skills when present), and prints the MCP configuration.

  One-liner:
    irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1 | iex

  With DeepSeek Harness registration:
    & ([scriptblock]::Create((irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1))) -InstallDshBundle

  Offline / from an extracted release directory (also used by the tests):
    .\install.ps1 -FromLocal C:\path\to\mysearch-v0.1.0-win-x64 -SkipPath

.PARAMETER Version
  Release tag to install, or "latest".

.PARAMETER Prefix
  Install root. Defaults to %LOCALAPPDATA%\Programs\mysearch.

.PARAMETER FromLocal
  Install from an already-extracted release directory instead of downloading.

.PARAMETER SkipPath
  Do not modify the user PATH.

.PARAMETER SkipSkill
  Do not install the agent skill.

.PARAMETER InstallDshBundle
  Register the bundled MCP client patch with DeepSeek Harness (desktop profile).
#>
[CmdletBinding()]
param(
  [string]$Version = 'latest',
  [string]$Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\mysearch'),
  [string]$FromLocal,
  [string]$Repo = 'my3254/sampleSearch',
  [switch]$SkipPath,
  [switch]$SkipSkill,
  [switch]$InstallDshBundle
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg) { Write-Host "    $msg" -ForegroundColor Green }

$asset = 'win-x64.zip'
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("mysearch-install-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

try {
  if ($FromLocal) {
    Write-Step "using local package: $FromLocal"
    if (-not (Test-Path $FromLocal)) { throw "path not found: $FromLocal" }
    $pkgRoot = $FromLocal
  }
  else {
    $api = if ($Version -eq 'latest') {
      "https://api.github.com/repos/$Repo/releases/latest"
    }
    else {
      "https://api.github.com/repos/$Repo/releases/tags/$Version"
    }
    Write-Step "resolving release ($Version)"
    $release = Invoke-RestMethod -Uri $api -Headers @{ 'user-agent' = 'mysearch-installer' }
    $file = $release.assets | Where-Object { $_.name -like "*$asset" } | Select-Object -First 1
    if (-not $file) { throw "release $($release.tag_name) has no asset matching *$asset" }

    Write-Step "downloading $($file.name)"
    $zip = Join-Path $tmp $file.name
    Invoke-WebRequest -Uri $file.browser_download_url -OutFile $zip

    $shaAsset = $release.assets | Where-Object { $_.name -eq "$($file.name).sha256" } | Select-Object -First 1
    if ($shaAsset) {
      $expected = (Invoke-WebRequest -Uri $shaAsset.browser_download_url).Content.Trim().Split(' ')[0]
      $actual = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLower()
      if ($expected.ToLower() -ne $actual) { throw "sha256 mismatch for $($file.name)" }
      Write-Ok "sha256 verified"
    }

    Expand-Archive -Path $zip -DestinationPath (Join-Path $tmp 'pkg') -Force
    $pkgRoot = Join-Path $tmp 'pkg'
    if (-not (Test-Path (Join-Path $pkgRoot 'bin\mysearch.exe'))) {
      $nested = Get-ChildItem $pkgRoot -Directory | Select-Object -First 1
      if ($nested) { $pkgRoot = $nested.FullName }
    }
  }

  $exe = Join-Path $pkgRoot 'bin\mysearch.exe'
  if (-not (Test-Path $exe)) { throw "package does not contain bin\mysearch.exe" }

  Write-Step "installing to $Prefix"
  $binDir = Join-Path $Prefix 'bin'
  New-Item -ItemType Directory -Path $binDir -Force | Out-Null
  Copy-Item $exe (Join-Path $binDir 'mysearch.exe') -Force
  foreach ($item in 'README.md', 'SKILL.md', 'install.ps1', 'install.sh') {
    $src = Join-Path $pkgRoot $item
    if (Test-Path $src) { Copy-Item $src (Join-Path $Prefix $item) -Force }
  }
  if (Test-Path (Join-Path $pkgRoot 'integrations')) {
    Copy-Item (Join-Path $pkgRoot 'integrations') (Join-Path $Prefix 'integrations') -Recurse -Force
  }
  Write-Ok "binary: $(Join-Path $binDir 'mysearch.exe')"

  if (-not $SkipPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $binDir) {
      $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $binDir } else { "$userPath;$binDir" }
      [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
      Write-Ok "added to user PATH (open a new terminal to pick it up)"
    }
    else { Write-Ok "already on user PATH" }
    $env:Path = "$binDir;$env:Path"
  }

  if (-not $SkipSkill) {
    $skillSrc = Join-Path $Prefix 'SKILL.md'
    if (Test-Path $skillSrc) {
      foreach ($dir in (Join-Path $HOME '.agents\skills\mysearch'), (Join-Path $HOME '.claude\skills\mysearch')) {
        if ($dir -like '*\.claude\*' -and -not (Test-Path (Join-Path $HOME '.claude'))) { continue }
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        Copy-Item $skillSrc (Join-Path $dir 'SKILL.md') -Force
        Write-Ok "skill: $dir"
      }
    }
  }

  if ($InstallDshBundle) {
    $dsh = Get-Command dsh.cmd -ErrorAction SilentlyContinue
    if (-not $dsh) { $dsh = Get-Command dsh -ErrorAction SilentlyContinue }
    $bundle = Join-Path $Prefix 'integrations\dsh-bundle'
    if (-not $dsh) {
      Write-Host "    ! dsh not found on PATH — skipped DeepSeek Harness registration" -ForegroundColor Yellow
    }
    elseif (-not (Test-Path $bundle)) {
      Write-Host "    ! bundle missing from package at $bundle" -ForegroundColor Yellow
    }
    else {
      $patch = Join-Path $bundle 'cordis.patch.yml'
      $text = Get-Content $patch -Raw
      $text = $text -replace "(?m)^(\s*)command: '.*'$", "`$1command: '$(Join-Path $binDir 'mysearch.exe')'"
      Set-Content -Path $patch -Value $text -Encoding utf8NoBOM
      & $dsh.Source plugin --profile desktop add "file:$bundle" | Out-Host
      Write-Ok "registered with DeepSeek Harness — restart DSH, then start a new conversation"
    }
  }

  Write-Step "verify"
  & (Join-Path $binDir 'mysearch.exe') health | Out-Host

  Write-Step "MCP configuration (paste into your client)"
  Write-Host @"
{
  "mcpServers": {
    "mysearch": {
      "command": "$((Join-Path $binDir 'mysearch.exe') -replace '\\','\\')",
      "args": ["mcp"]
    }
  }
}
"@ -ForegroundColor DarkGray

  Write-Host ""
  Write-Host "Done. Try:  mysearch search `"大模型 推理 优化`" -m 5" -ForegroundColor Green
}
finally {
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
