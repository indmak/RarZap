# 真机截图（DPI-aware 进程，保证物理像素完整捕获）。
# 用法：powershell -ExecutionPolicy Bypass -File scripts/screenshot.ps1 [-Out path] [-Screen main|settings|privacy]
param(
    [string]$Out = "assets/screenshots/main-empty.png",
    [string]$Screen = "main",
    [int]$Delay = 2,
    [string]$Open = "",
    [string]$Exe = "target/debug/rarzap.exe"
)
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class DX {
    [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int v);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    public struct RECT { public int L; public int T; public int R; public int B; }
}
'@
[DX]::SetProcessDpiAwareness(2) | Out-Null

if ($Screen -ne "main") { $env:RARZAP_START_SCREEN = $Screen } else { Remove-Item Env:RARZAP_START_SCREEN -ErrorAction SilentlyContinue }
if ($Open -ne "") { $env:RARZAP_OPEN = $Open } else { Remove-Item Env:RARZAP_OPEN -ErrorAction SilentlyContinue }

$p = Start-Process -FilePath $Exe -PassThru
# 轮询等待窗口出现（首帧含资产加载，可能较慢）。
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 250
    if ($p.HasExited) { throw "process exited early (code $($p.ExitCode))" }
    $p.Refresh()
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $h = $p.MainWindowHandle; break }
}
if ($h -eq [IntPtr]::Zero) { Stop-Process -Id $p.Id -Force; throw "no main window handle" }
Start-Sleep -Seconds $Delay  # 等首帧稳定 / 让转换跑到想看的状态

$rect = New-Object DX+RECT
[DX]::GetWindowRect($h, [ref]$rect) | Out-Null
$w = $rect.R - $rect.L
$hh = $rect.B - $rect.T
Write-Output "true-rect=$w x $hh"
$bmp = New-Object System.Drawing.Bitmap($w, $hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[DX]::PrintWindow($h, $hdc, 2) | Out-Null
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Stop-Process -Id $p.Id -Force
Write-Output "SAVED $Out"
