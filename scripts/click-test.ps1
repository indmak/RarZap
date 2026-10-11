# 模拟鼠标点击（验证按钮可点 / 弹窗不穿透）。坐标以窗口左上角为原点，物理像素。
# 用法：powershell -ExecutionPolicy Bypass -File scripts/click-test.ps1 -X 1370 -Y 101 [-Shot out.png]
param(
    [int]$X,
    [int]$Y,
    [int]$Delay = 3,
    [string]$LogOut = "out.txt",
    [string]$Screen = "main",
    [string]$Exe = "target/debug/rarzap.exe",
    [string]$Shot = ""
)
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class CK {
    [DllImport("shcore.dll")] public static extern int SetProcessDpiAwareness(int v);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
    public struct RECT { public int L; public int T; public int R; public int B; }
}
'@
[CK]::SetProcessDpiAwareness(2) | Out-Null

if ($Screen -ne "main") { $env:RARZAP_START_SCREEN = $Screen } else { Remove-Item Env:RARZAP_START_SCREEN -ErrorAction SilentlyContinue }

$p = Start-Process -FilePath $Exe -PassThru -RedirectStandardOutput $LogOut -NoNewWindow
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 250
    if ($p.HasExited) { throw "exited early" }
    $p.Refresh()
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { $h = $p.MainWindowHandle; break }
}
if ($h -eq [IntPtr]::Zero) { Stop-Process -Id $p.Id -Force; throw "no window" }
Start-Sleep -Seconds $Delay
[CK]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 300

$rect = New-Object CK+RECT
[CK]::GetWindowRect($h, [ref]$rect) | Out-Null
$sx = $rect.L + $X
$sy = $rect.T + $Y
Write-Output "window=($($rect.L),$($rect.T)) click=($sx,$sy)"
[CK]::SetCursorPos($sx, $sy) | Out-Null
Start-Sleep -Milliseconds 200
[CK]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 60
[CK]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 400
[CK]::SetCursorPos($sx, $sy) | Out-Null
Start-Sleep -Milliseconds 150
[CK]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 60
[CK]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Seconds 2

if ($Shot -ne "") {
    $w = $rect.R - $rect.L
    $hh = $rect.B - $rect.T
    $bmp = New-Object System.Drawing.Bitmap($w, $hh)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    [CK]::PrintWindow($h, $hdc, 2) | Out-Null
    $g.ReleaseHdc($hdc)
    $g.Dispose()
    $bmp.Save($Shot, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Output "SAVED $Shot"
}

Stop-Process -Id $p.Id -Force
Write-Output "--- log tail ---"
Get-Content $LogOut -ErrorAction SilentlyContinue | Select-Object -Last 5
