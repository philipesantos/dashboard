# The accessibility tree of a host window through UI Automation, and real input, for
# host/tests/window.rs. Usage: uia.ps1 dump TITLE | find TITLE NAME | activate TITLE |
# keys SENDKEYS | click X Y
param([string]$Command, [string]$A, [string]$B)
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Native {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
}
"@
$root = [System.Windows.Automation.AutomationElement]::RootElement
function Window($title) {
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $title)
  return $root.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
switch ($Command) {
  "dump" {
    $w = Window $A
    if ($w) {
      $all = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
      foreach ($e in $all) { $e.Current.ControlType.ProgrammaticName + "`t" + $e.Current.Name }
    }
  }
  "find" {
    $w = Window $A
    if ($w) {
      $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $B)
      $e = $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
      if ($e) { $r = $e.Current.BoundingRectangle; "$($r.X) $($r.Y) $($r.Width) $($r.Height)" }
    }
  }
  "activate" {
    $w = Window $A
    if ($w) { [Native]::SetForegroundWindow([IntPtr]$w.Current.NativeWindowHandle) | Out-Null }
    (New-Object -ComObject WScript.Shell).AppActivate($A) | Out-Null
  }
  "keys" { [System.Windows.Forms.SendKeys]::SendWait($A) }
  "click" {
    [Native]::SetCursorPos([int]$A, [int]$B) | Out-Null
    [Native]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
    [Native]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
  }
}
