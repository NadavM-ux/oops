# End-to-end test on a real Windows desktop (runs in CI with Windows PowerShell 5.1):
#   1. type "akuo" in a text box, press Alt+O   -> "שלום"  (fix what was just typed)
#   2. press Alt+O again                         -> "akuo"  (undo)
#   3. select the text, press Alt+O              -> "שלום"  (fix a selection, via the clipboard)
# The text box lives in this script's own window, so its text is read directly.
param([Parameter(Mandatory)] [string] $Installer)
$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
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
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  public static void Key(ushort vk, bool up) {
    var input = new INPUT[1];
    input[0].type = 1;
    input[0].ki.wVk = vk;
    input[0].ki.wScan = (ushort)MapVirtualKey(vk, 0);
    input[0].ki.dwFlags = up ? 2u : 0u;
    if (SendInput(1, input, Marshal.SizeOf(typeof(INPUT))) != 1) throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
  }
}
"@

# Keystrokes are processed by this thread's message loop, so keep it pumping while we wait.
function Pump([int] $ms) {
  $end = (Get-Date).AddMilliseconds($ms)
  while ((Get-Date) -lt $end) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 10 }
}
function Tap([int] $vk) { [Kbd]::Key($vk, $false); Pump 30; [Kbd]::Key($vk, $true); Pump 30 }
function Chord([int] $mod, [int] $vk) { [Kbd]::Key($mod, $false); Pump 30; Tap $vk; [Kbd]::Key($mod, $true); Pump 30 }
function Expect([string] $want, [string] $what) {
  Pump 2500
  $got = $script:box.Text.Trim()
  Write-Host "   text box: '$got'"
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
Write-Host '   Oops is running'

Write-Host '3) Opening a test window with a text box'
$form = New-Object System.Windows.Forms.Form
$form.Text = 'Oops end-to-end test'
$form.Width = 640; $form.Height = 220
$form.StartPosition = 'CenterScreen'
$form.TopMost = $true
$script:box = New-Object System.Windows.Forms.TextBox
$box.Multiline = $true
$box.Dock = 'Fill'
$box.Font = New-Object System.Drawing.Font('Segoe UI', 18)
$form.Controls.Add($box)
$form.Show()
$form.Activate()
$null = [Kbd]::SetForegroundWindow($form.Handle)
$box.Focus() | Out-Null
Pump 1500
if ([Kbd]::GetForegroundWindow() -ne $form.Handle) { Write-Host '   warning: test window is not in front' }

$hebrew = -join [char[]](0x05E9, 0x05DC, 0x05D5, 0x05DD) # שלום

Write-Host '4) Typing "akuo" and pressing Alt+O'
foreach ($vk in 0x41, 0x4B, 0x55, 0x4F) { Tap $vk } # a k u o
Pump 300
if ($box.Text -ne 'akuo') { throw "Typing didn't reach the text box (got '$($box.Text)')" }
Chord 0x12 0x4F # Alt+O
Expect $hebrew 'fix typed text'

Write-Host '5) Pressing Alt+O again'
Chord 0x12 0x4F
Expect 'akuo' 'undo'

Write-Host '6) Selecting the text and pressing Alt+O'
Tap 0x23 # End: a caret move, so Oops forgets what was typed
$box.SelectAll()
Pump 300
Chord 0x12 0x4F
Expect $hebrew 'fix selected text'

Write-Host 'PASS: typed fix, undo and selection fix all work on Windows.'
$form.Close()
Stop-Process -Id $oops.Id -Force -ErrorAction SilentlyContinue
