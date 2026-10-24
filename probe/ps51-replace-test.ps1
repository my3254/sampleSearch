# Mirrors the exact replacement expression used in install.ps1 (line 169).
$source = "line1`r`n        binary: ''`r`n        timeoutMs: 45000"
$bin = 'C:\x\samplesearch.exe'

# (a) what install.ps1 actually runs
$a = $source -replace "(?m)^(\s*)binary: ''\s*$", "`${1}binary: '$(Join-Path $bin '..')'"
Write-Host "(a) install.ps1 form:"
Write-Host $a

# (b) explicit .NET call with the same pattern
$re = [regex]::new("(?m)^(\s*)binary: ''\s*$")
Write-Host "(b) .NET form:"
Write-Host ($re.Replace($source, '${1}binary: ''X'''))

# (c) is group 1 captured?
$m = $re.Match($source)
Write-Host ("(c) success=$($m.Success) group1Length=$($m.Groups[1].Value.Length)")
