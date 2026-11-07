# Collects everything the GPUI spike report needs from a Windows machine into spike-report.txt,
# next to this script: the system and GPU, the adapter GPUI picks, startup, memory, idle CPU, the
# drag benchmark, the UI Automation tree (what Narrator reads) of the dashboard and of the
# approval dialog, and where keyboard focus goes on Tab. Nothing is sent anywhere.
#
# Put it beside dashboard-gpui-spike.exe and measure-windows.ps1 and double-click run-check.cmd,
# or: powershell -ExecutionPolicy Bypass -File windows-check.ps1
param([string]$Exe = (Join-Path $PSScriptRoot "dashboard-gpui-spike.exe"))
$ErrorActionPreference = "Continue"
$report = Join-Path $PSScriptRoot "spike-report.txt"
"GPUI spike, Windows check, $(Get-Date -Format s)" | Set-Content -Encoding utf8 $report
# Add-Content, not Tee-Object: Windows PowerShell 5.1's Tee-Object writes UTF-16
function Out-Report([string]$Text) { Write-Host $Text; Add-Content -Encoding utf8 -Path $report -Value $Text }
function Section([string]$Title) { Out-Report ""; Out-Report "== $Title" }

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
$A = [System.Windows.Automation.AutomationElement]

function New-DataDir([string]$Name) {
  $dir = Join-Path $env:TEMP "spike-check-$Name-$(Get-Random)"
  New-Item -ItemType Directory -Force $dir | Out-Null
  $dir
}

function Start-Spike([string]$DataDir, [hashtable]$Env = @{}) {
  $info = New-Object System.Diagnostics.ProcessStartInfo $Exe
  $info.UseShellExecute = $false
  $info.RedirectStandardError = $true
  $info.EnvironmentVariables["SPIKE_DATA_DIR"] = $DataDir
  $info.EnvironmentVariables["RUST_LOG"] = "warn,gpui_windows=info"
  foreach ($k in $Env.Keys) { $info.EnvironmentVariables[$k] = $Env[$k] }
  [System.Diagnostics.Process]::Start($info)
}

function Find-Window([System.Diagnostics.Process]$Process) {
  $condition = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $Process.Id)
  for ($i = 0; $i -lt 100; $i++) {
    $window = $A::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $condition)
    if ($window) { return $window }
    Start-Sleep -Milliseconds 100
  }
  $null
}

# Saves a picture of the window, next to the report, for whoever runs the check to look at
function Save-Picture($Window, [string]$Name) {
  try {
    $r = $Window.Current.BoundingRectangle
    $bitmap = New-Object System.Drawing.Bitmap ([int]$r.Width), ([int]$r.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bitmap.Size)
    $path = Join-Path $PSScriptRoot $Name
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    Out-Report "picture: $Name"
  } catch { Out-Report "picture failed: $($_.Exception.Message)" }
}

function Write-Tree($Element, [int]$Depth = 0, [int]$MaxDepth = 14, $Walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker) {
  if ($null -eq $Element -or $Depth -gt $MaxDepth) { return }
  try {
    $c = $Element.Current
    $type = $c.ControlType.ProgrammaticName -replace "^ControlType\.", ""
    $flags = @()
    if ($c.IsKeyboardFocusable) { $flags += "focusable" }
    if ($c.HasKeyboardFocus) { $flags += "FOCUSED" }
    if ($c.IsOffscreen) { $flags += "offscreen" }
    $extra = ""
    if ($c.HelpText) { $extra += " help='$($c.HelpText)'" }
    $value = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$value)) {
      if ($value.Current.Value -and $value.Current.Value -ne $c.Name) { $extra += " value='$($value.Current.Value)'" }
    }
    $window = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern, [ref]$window)) {
      $extra += " modal=$($window.Current.IsModal)"
    }
    Out-Report ("{0}[{1}] '{2}'{3} {4}" -f ("  " * $Depth), $type, $c.Name, $extra, ($flags -join ","))
  } catch { Out-Report ("{0}<error: {1}>" -f ("  " * $Depth), $_.Exception.Message); return }
  $child = $Walker.GetFirstChild($Element)
  if ($Depth -eq 0 -and $null -eq $child -and $Walker -ne [System.Windows.Automation.TreeWalker]::RawViewWalker) {
    Out-Report "  (no children in the control view; the raw view follows)"
    $Walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $child = $Walker.GetFirstChild($Element)
  }
  while ($child) {
    Write-Tree $child ($Depth + 1) $MaxDepth $Walker
    $child = $Walker.GetNextSibling($child)
  }
}

function Stop-Spike([System.Diagnostics.Process]$Process) {
  if (-not $Process.HasExited) { $Process.Kill(); $Process.WaitForExit(5000) | Out-Null }
  $Process.StandardError.ReadToEnd() -split "`n" |
    Where-Object { $_ -match "first frame|Using GPU|feature level|cannot start|panicked|ERROR" } |
    ForEach-Object { Out-Report ("  log: " + $_.Trim()) }
}

if (-not (Test-Path $Exe)) { Out-Report "dashboard-gpui-spike.exe not found next to this script: $Exe"; exit 1 }

Section "System"
$os = Get-CimInstance Win32_OperatingSystem
Out-Report "$($os.Caption) $($os.Version) $($os.OSArchitecture)"
Get-CimInstance Win32_VideoController | ForEach-Object { Out-Report "GPU: $($_.Name), driver $($_.DriverVersion), $($_.CurrentHorizontalResolution)x$($_.CurrentVerticalResolution)" }
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
Out-Report "CPU: $($cpu.Name), $($cpu.NumberOfLogicalProcessors) threads; RAM $([math]::Round($os.TotalVisibleMemorySize / 1MB, 1)) GiB"

Section "Startup, memory, idle CPU (4 widgets)"
$dir = New-DataDir "measure"
$env:SPIKE_DATA_DIR = $dir
& $Exe --dev-approve 2>$null
foreach ($i in 1..3) {
  & (Join-Path $PSScriptRoot "measure-windows.ps1") -Label "gpui-$i" -Title "Dashboard (GPUI spike)" -Exe $Exe | ForEach-Object { Out-Report $_ }
}

Section "Drag benchmark (5 s, one step per frame)"
foreach ($i in 1..2) {
  $p = Start-Spike $dir @{ SPIKE_DRAG_BENCH = "1" }
  $p.WaitForExit(60000) | Out-Null
  $p.StandardError.ReadToEnd() -split "`n" |
    Where-Object { $_ -match "drag bench|Using GPU|first frame" } | ForEach-Object { Out-Report ("  " + $_.Trim()) }
}

Section "UI Automation tree: dashboard (what Narrator reads)"
$p = Start-Spike $dir
Start-Sleep -Seconds 4
$window = Find-Window $p
if ($window) { Save-Picture $window "spike-dashboard.png"; Write-Tree $window } else { Out-Report "window not found by UI Automation" }

Section "Keyboard: where focus goes on Tab"
if ($window) {
  try { $window.SetFocus() } catch { Out-Report "SetFocus failed: $($_.Exception.Message)" }
  Start-Sleep -Milliseconds 500
  foreach ($i in 1..14) {
    [System.Windows.Forms.SendKeys]::SendWait("{TAB}")
    Start-Sleep -Milliseconds 300
    $f = $A::FocusedElement
    $name = if ($f) { "$($f.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') '$($f.Current.Name)'" } else { "(none)" }
    Out-Report "Tab $i -> $name"
  }
  Out-Report "Arrow Down on the focused element:"
  [System.Windows.Forms.SendKeys]::SendWait("{DOWN}")
  Start-Sleep -Milliseconds 300
  $f = $A::FocusedElement
  if ($f) { Out-Report "  -> '$($f.Current.Name)'" }
}
Stop-Spike $p

Section "UI Automation tree: approval dialog"
$dir2 = New-DataDir "dialog"
'{"mode":"System","secret":"Teal"}' | Set-Content -Encoding ascii (Join-Path $dir2 "settings.json")
$p = Start-Spike $dir2 @{ SPIKE_REQUEST_ADD = "sampler" }
Start-Sleep -Seconds 4
$window = Find-Window $p
if ($window) {
  Save-Picture $window "spike-dialog.png"
  Write-Tree $window
  $f = $A::FocusedElement
  if ($f) { Out-Report "focus when the dialog opened: '$($f.Current.Name)'" }
  Out-Report "Tab and Shift+Tab inside the dialog:"
  try { $window.SetFocus() } catch {}
  Start-Sleep -Milliseconds 500
  foreach ($key in "{TAB}", "{TAB}", "{TAB}", "+{TAB}", "+{TAB}") {
    [System.Windows.Forms.SendKeys]::SendWait($key)
    Start-Sleep -Milliseconds 300
    $f = $A::FocusedElement
    if ($f) { Out-Report "  $key -> '$($f.Current.Name)'" }
  }
  [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
  Start-Sleep -Milliseconds 500
  $dialog = $window.FindFirst([System.Windows.Automation.TreeScope]::Descendants,
    (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, "Add Release status?")))
  Out-Report ("Escape closed the dialog: " + ($null -eq $dialog))
} else { Out-Report "window not found by UI Automation" }
Stop-Spike $p

Remove-Item -Recurse -Force $dir, $dir2 -ErrorAction SilentlyContinue
Out-Report ""
Out-Report "Done. Send spike-report.txt from this folder: $report"
