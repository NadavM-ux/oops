# End-to-end test on a real Windows desktop (runs in CI):
#   1. type "akuo" in Notepad, press Alt+O          -> "שלום"  (fix what was just typed)
#   2. press Alt+O again                             -> "akuo"  (undo)
#   3. select all, press Alt+O                       -> "שלום"  (fix a selection, via the clipboard)
# Notepad's text is read through UI Automation, so reading never sends keystrokes.
param([Parameter(Mandatory)] [string] $Installer)
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Threading;
public static class Kbd {
  [StructLayout(LayoutKind.Sequential)]
  struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit, Size = 40)]
  struct INPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint mapType);
  public static void Key(ushort vk, bool up) {
    var input = new INPUT[1];
    input[0].type = 1;
    input[0].ki.wVk = vk;
    input[0].ki.wScan = (ushort)MapVirtualKey(vk, 0);
    input[0].ki.dwFlags = up ? 2u : 0u;
    if (SendInput(1, input, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
    Thread.Sleep(30);
  }
  public static void Tap(ushort vk) { Key(vk, false); Key(vk, true); }
  public static void Chord(ushort mod, ushort vk) { Key(mod, false); Tap(vk); Key(mod, true); }
}
"@

$A = [System.Windows.Automation.AutomationElement]
function Read-Notepad([IntPtr] $hwnd) {
  $root = $A::FromHandle($hwnd)
  $isText = New-Object System.Windows.Automation.OrCondition(
    (New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::Document)),
    (New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)))
  $box = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $isText)
  if ($null -eq $box) { throw 'Could not find Notepad''s text area' }
  $pattern = $null
  if ($box.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$pattern)) {
    return $pattern.DocumentRange.GetText(-1).Trim()
  }
  if ($box.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$pattern)) {
    return $pattern.Current.Value.Trim()
  }
  throw 'Notepad''s text area exposes no readable text'
}
function Expect([IntPtr] $hwnd, [string] $want, [string] $what) {
  Start-Sleep -Seconds 2
  $got = Read-Notepad $hwnd
  Write-Host "   Notepad: '$got'"
  if ($got -ne $want) { throw "$what failed: expected '$want', got '$got'" }
  Write-Host "   OK: $what"
}

Write-Host '1) Installing silently'
Start-Process -FilePath $Installer -ArgumentList '/S' -Wait
$exe = Join-Path $env:LOCALAPPDATA 'Oops\Oops.exe'
if (-not (Test-Path $exe)) { throw "Oops.exe was not installed at $exe" }

Write-Host '2) Starting Oops in the background (onboarding already done)'
$cfgDir = Join-Path $env:APPDATA 'tech.nadav.oops'
New-Item -ItemType Directory -Force -Path $cfgDir | Out-Null
'{"onboarded":true,"launchAtLogin":false,"checkUpdates":false,"sounds":false,"switchLayout":false}' |
  Set-Content -Encoding ascii (Join-Path $cfgDir 'settings.json')
$oops = Start-Process $exe -ArgumentList '--autostart' -PassThru
Start-Sleep -Seconds 5
if ($oops.HasExited) { throw "Oops exited right away (exit code $($oops.ExitCode))" }

Write-Host '3) Opening Notepad'
Start-Process notepad | Out-Null
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 30 -and $hwnd -eq [IntPtr]::Zero; $i++) {
  Start-Sleep -Milliseconds 500
  $p = Get-Process -Name notepad -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
  if ($p) { $hwnd = $p.MainWindowHandle }
}
if ($hwnd -eq [IntPtr]::Zero) { throw 'Notepad window did not appear' }
$wsh = New-Object -ComObject WScript.Shell
$null = $wsh.AppActivate($p.Id)
Start-Sleep -Seconds 1

$hebrew = -join [char[]](0x05E9, 0x05DC, 0x05D5, 0x05DD) # שלום

Write-Host '4) Typing "akuo" and pressing Alt+O'
foreach ($vk in 0x41, 0x4B, 0x55, 0x4F) { [Kbd]::Tap($vk) } # a k u o
Start-Sleep -Milliseconds 300
[Kbd]::Chord(0x12, 0x4F) # Alt+O
Expect $hwnd $hebrew 'fix typed text'

Write-Host '5) Pressing Alt+O again'
[Kbd]::Chord(0x12, 0x4F)
Expect $hwnd 'akuo' 'undo'

Write-Host '6) Selecting everything and pressing Alt+O'
[Kbd]::Chord(0x11, 0x41) # Ctrl+A
Start-Sleep -Milliseconds 300
[Kbd]::Chord(0x12, 0x4F)
Expect $hwnd $hebrew 'fix selected text'

Write-Host 'PASS: typed fix, undo and selection fix all work on Windows.'
Get-Process -Name notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Stop-Process -Id $oops.Id -Force -ErrorAction SilentlyContinue
