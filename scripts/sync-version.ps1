#Requires -Version 5.1
<#
.SYNOPSIS
  版本号统一：根 Cargo.toml -> 商业插件 Cargo.toml + 商店四段号派生。
.DESCRIPTION
  唯一来源：E:\2026soft\RarZap\Cargo.toml [package] version。
  - 若 plugins/rarzap-batch/Cargo.toml 存在（本地商业目录），对其 version 对齐。
  - 输出微软商店四段号：cargo 版本已是 4 段则直接用，否则追加 .0。
  外网不通时请先设置代理：
    $env:HTTP_PROXY="http://127.0.0.1:3128"; $env:HTTPS_PROXY="http://127.0.0.1:3128"
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cargoToml = Join-Path $root "Cargo.toml"

$ver = Select-String -LiteralPath $cargoToml -Pattern '^version\s*=\s*"([^"]+)"' |
  Select-Object -First 1 | ForEach-Object { $_.Matches[0].Groups[1].Value }
if (-not $ver) { throw "cannot parse version from $cargoToml" }

$pluginToml = Join-Path $root "plugins\rarzap-pro\Cargo.toml"
if (Test-Path -LiteralPath $pluginToml) {
  $text = Get-Content -LiteralPath $pluginToml -Raw
  $text = [regex]::Replace($text, '(?m)^version\s*=\s*"[^"]+"', "version = `"$ver`"", 1)
  Set-Content -LiteralPath $pluginToml -Value $text -NoNewline
  Write-Output "plugin version synced -> $ver"
} else {
  Write-Output "no local commercial plugin; skip plugin sync"
}

$parts = $ver.Split(".")
$storeVer = if ($parts.Count -ge 4) { $ver } else { "$ver.0" }
Write-Output "cargo version : $ver"
Write-Output "msstore version: $storeVer"
