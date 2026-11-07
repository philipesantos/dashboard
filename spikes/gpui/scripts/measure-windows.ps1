# Measures one desktop app on Windows: time until its main window has the given title, then,
# after it settles, memory summed over its whole process tree (so WebView2's processes count)
# and CPU while idle. Prints one line like scripts/measure-linux.sh.
#
#   measure-windows.ps1 -Label gpui -Title "Dashboard (GPUI spike)" -Exe path\to.exe [-Arguments ...]
param(
  [string]$Label, [string]$Title, [string]$Exe, [string[]]$Arguments = @(),
  [int]$SettleSeconds = 5, [int]$IdleSeconds = 10
)
$ErrorActionPreference = 'Stop'

function Get-Tree([int]$Id) {
  $all = Get-CimInstance Win32_Process
  $ids = [System.Collections.Generic.List[int]]::new()
  $ids.Add($Id)
  $i = 0
  while ($i -lt $ids.Count) {
    foreach ($p in $all | Where-Object { $_.ParentProcessId -eq $ids[$i] }) { $ids.Add([int]$p.ProcessId) }
    $i++
  }
  $ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
}

$watch = [System.Diagnostics.Stopwatch]::StartNew()
$p = if ($Arguments.Count) { Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru } else { Start-Process -FilePath $Exe -PassThru }
$windowMs = 'timeout'
while ($watch.ElapsedMilliseconds -lt 60000) {
  $p.Refresh()
  if ($p.HasExited) { $windowMs = "exited($($p.ExitCode))"; break }
  if ($p.MainWindowTitle -eq $Title) { $windowMs = $watch.ElapsedMilliseconds; break }
  Start-Sleep -Milliseconds 20
}
Start-Sleep -Seconds $SettleSeconds
$before = (Get-Tree $p.Id | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
Start-Sleep -Seconds $IdleSeconds
$tree = Get-Tree $p.Id
$after = ($tree | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
$ws = ($tree | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
$private = ($tree | Measure-Object -Property PrivateMemorySize64 -Sum).Sum / 1MB
$cpu = ($after - $before) / ($IdleSeconds * 1000) * 100
$count = @($tree).Count
$tree | Stop-Process -Force -ErrorAction SilentlyContinue
"{0} window_ms={1} working_set_mib={2:N1} private_mib={3:N1} idle_cpu={4:N1}% processes={5}" -f $Label, $windowMs, $ws, $private, $cpu, $count
