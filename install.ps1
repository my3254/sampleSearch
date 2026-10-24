<#
.SYNOPSIS
  Install samplesearch (self-hosted search for AI agents) on Windows.

.DESCRIPTION
  Downloads the release binary, installs it under %LOCALAPPDATA%\Programs\samplesearch,
  adds it to the user PATH, installs the agent skill into ~/.agents/skills/samplesearch
  (and ~/.claude/skills when present), and prints the MCP configuration.

  One-liner:
    irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1 | iex

  With DeepSeek Harness registration (native plugin, no MCP bridge needed):
    & ([scriptblock]::Create((irm https://raw.githubusercontent.com/my3254/sampleSearch/main/install.ps1))) -InstallDshPlugin

  Offline / from an extracted release directory (also used by the tests):
    .\install.ps1 -FromLocal C:\path\to\samplesearch-v0.1.0-win-x64 -SkipPath

.PARAMETER Version
  Release tag to install, or "latest".

.PARAMETER Prefix
  Install root. Defaults to %LOCALAPPDATA%\Programs\samplesearch.

.PARAMETER FromLocal
  Install from an already-extracted release directory instead of downloading.

.PARAMETER SkipPath
  Do not modify the user PATH.

.PARAMETER SkipSkill
  Do not install the agent skill.

.PARAMETER InstallDshPlugin
  Register the native DeepSeek Harness plugin (desktop profile). This is a single
  self-contained package — it does not install or need dsh-mcp-client.

.PARAMETER RestartDsh
  After installing, close and relaunch DeepSeek Harness so the plugin layer loads.
  Run this from a terminal outside DSH: it ends the current DSH session window
  (the conversation is kept and can be resumed). Without this switch you restart
  DSH yourself.
#>
[CmdletBinding()]
param(
  [string]$Version = 'latest',
  [string]$Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\samplesearch'),
  [string]$FromLocal,
  [string]$Repo = 'my3254/sampleSearch',
  [switch]$SkipPath,
  [switch]$SkipSkill,
  [switch]$InstallDshPlugin,
  [switch]$RestartDsh
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg) { Write-Host "    $msg" -ForegroundColor Green }

$asset = 'win-x64.zip'
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("samplesearch-install-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
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
    # curl.exe ships with Windows 10+ and uses its own TLS stack; Invoke-WebRequest
    # goes through schannel, which some locked-down environments cannot use.
    $hasCurl = [bool](Get-Command curl.exe -ErrorAction SilentlyContinue)
    if ($hasCurl) {
      $json = & curl.exe -fsSL -H 'user-agent: samplesearch-installer' $api
      if ($LASTEXITCODE -ne 0) { throw "curl failed to read $api" }
      $release = $json | ConvertFrom-Json
    }
    else {
      $release = Invoke-RestMethod -Uri $api -Headers @{ 'user-agent' = 'samplesearch-installer' }
    }
    $file = $release.assets | Where-Object { $_.name -like "*$asset" } | Select-Object -First 1
    if (-not $file) { throw "release $($release.tag_name) has no asset matching *$asset" }

    Write-Step "downloading $($file.name)"
    $zip = Join-Path $tmp $file.name
    if ($hasCurl) {
      & curl.exe -fsSL -o $zip $file.browser_download_url
      if ($LASTEXITCODE -ne 0) { throw "curl failed to download $($file.name)" }
    }
    else {
      Invoke-WebRequest -Uri $file.browser_download_url -OutFile $zip
    }

    $shaAsset = $release.assets | Where-Object { $_.name -eq "$($file.name).sha256" } | Select-Object -First 1
    if ($shaAsset) {
      $expected = (Invoke-WebRequest -Uri $shaAsset.browser_download_url).Content.Trim().Split(' ')[0]
      $actual = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLower()
      if ($expected.ToLower() -ne $actual) { throw "sha256 mismatch for $($file.name)" }
      Write-Ok "sha256 verified"
    }

    Expand-Archive -Path $zip -DestinationPath (Join-Path $tmp 'pkg') -Force
    $pkgRoot = Join-Path $tmp 'pkg'
    if (-not (Test-Path (Join-Path $pkgRoot 'bin\samplesearch.exe'))) {
      $nested = Get-ChildItem $pkgRoot -Directory | Select-Object -First 1
      if ($nested) { $pkgRoot = $nested.FullName }
    }
  }

  $exe = Join-Path $pkgRoot 'bin\samplesearch.exe'
  if (-not (Test-Path $exe)) { throw "package does not contain bin\samplesearch.exe" }

  Write-Step "installing to $Prefix"
  $binDir = Join-Path $Prefix 'bin'
  New-Item -ItemType Directory -Path $binDir -Force | Out-Null
  Copy-Item $exe (Join-Path $binDir 'samplesearch.exe') -Force
  foreach ($item in 'README.md', 'SKILL.md', 'install.ps1', 'install.sh') {
    $src = Join-Path $pkgRoot $item
    if (Test-Path $src) { Copy-Item $src (Join-Path $Prefix $item) -Force }
  }
  if (Test-Path (Join-Path $pkgRoot 'integrations')) {
    Copy-Item (Join-Path $pkgRoot 'integrations') (Join-Path $Prefix 'integrations') -Recurse -Force
  }
  Write-Ok "binary: $(Join-Path $binDir 'samplesearch.exe')"

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
      foreach ($dir in (Join-Path $HOME '.agents\skills\samplesearch'), (Join-Path $HOME '.claude\skills\samplesearch')) {
        if ($dir -like '*\.claude\*' -and -not (Test-Path (Join-Path $HOME '.claude'))) { continue }
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
        Copy-Item $skillSrc (Join-Path $dir 'SKILL.md') -Force
        Write-Ok "skill: $dir"
      }
    }
  }

  if ($InstallDshPlugin) {
    $dsh = Get-Command dsh.cmd -ErrorAction SilentlyContinue
    if (-not $dsh) { $dsh = Get-Command dsh -ErrorAction SilentlyContinue }
    $plugin = Join-Path $Prefix 'integrations\dsh-plugin'
    if (-not $dsh) {
      Write-Host "    ! dsh not found on PATH — skipped DeepSeek Harness registration" -ForegroundColor Yellow
    }
    elseif (-not (Test-Path $plugin)) {
      Write-Host "    ! plugin missing from package at $plugin" -ForegroundColor Yellow
    }
    else {
      # The plugin shells out to the binary; point it at this installation explicitly
      # instead of relying on PATH having been refreshed in the running DSH process.
      $patch = Join-Path $plugin 'cordis.patch.yml'
      # Explicit UTF-8 on read and write: Windows PowerShell 5.1 reads BOM-less files as
      # ANSI (mangling non-ASCII comments), and it has no `utf8NoBOM` encoding name.
      $text = [System.IO.File]::ReadAllText($patch, [System.Text.Encoding]::UTF8)
      $text = $text -replace "(?m)^(\s*)binary: ''\s*$", "`${1}binary: '$(Join-Path $binDir 'samplesearch.exe')'"
      [System.IO.File]::WriteAllText($patch, $text, (New-Object System.Text.UTF8Encoding($false)))
      & $dsh.Source plugin --profile desktop add "file:$plugin" | Out-Host
      Write-Ok "registered with DeepSeek Harness — restart DSH, then start a new conversation"
    }
  }

  if ($RestartDsh) {
    Write-Step "restarting DeepSeek Harness"
    $running = Get-Process -Name 'DeepSeek Harness' -ErrorAction SilentlyContinue
    if (-not $running) {
      Write-Host "    ! DeepSeek Harness is not running; start it yourself" -ForegroundColor Yellow
    }
    else {
      $exePath = ($running | Where-Object { $_.Path } | Select-Object -First 1).Path
      if (-not $exePath) { $exePath = 'D:\software\deepseek\DeepSeek Harness.exe' }
      # Graceful first so the harness can flush session state, force after 8 s.
      $running | ForEach-Object { $_.CloseMainWindow() | Out-Null }
      Start-Sleep -Seconds 2
      Get-Process -Name 'DeepSeek Harness' -ErrorAction SilentlyContinue | Stop-Process -Force
      Start-Sleep -Milliseconds 700
      Start-Process -FilePath $exePath
      Write-Ok "restarted: $exePath"
      Write-Host "    give it ~10 s, then open a new conversation" -ForegroundColor DarkGray
    }
  }

  Write-Step "verify"
  & (Join-Path $binDir 'samplesearch.exe') health | Out-Host

  Write-Step "MCP configuration (paste into your client)"
  Write-Host @"
{
  "mcpServers": {
    "samplesearch": {
      "command": "$((Join-Path $binDir 'samplesearch.exe') -replace '\\','\\')",
      "args": ["mcp"]
    }
  }
}
"@ -ForegroundColor DarkGray

  Write-Host ""
  Write-Host "Done. Try:  samplesearch search `"大模型 推理 优化`" -m 5" -ForegroundColor Green
}
finally {
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
