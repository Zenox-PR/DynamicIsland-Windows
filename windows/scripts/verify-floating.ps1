param([int]$Cycles = 30)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class FloatingChecks {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; public override string ToString(){return $"{Left},{Top},{Right},{Bottom}";} }
 [StructLayout(LayoutKind.Sequential)] struct Monitor {public uint Size; public Rect Screen,Work; public uint Flags;}
 [StructLayout(LayoutKind.Sequential)] struct Point {public int X,Y;}
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="FindWindowW")] static extern IntPtr FindWindowNative(string cls,string title);
 public static IntPtr FindWindow(string cls,string ignored){return FindWindowNative(cls,null);}
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int c);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr p,uint f);
 [DllImport("user32.dll")] static extern IntPtr MonitorFromPoint(Point p,uint f);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern bool GetMonitorInfo(IntPtr h,ref Monitor m);
 [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr h,uint a,out Rect r,int size);
 public static string Work(){var m=new Monitor();m.Size=(uint)Marshal.SizeOf<Monitor>();if(!GetMonitorInfo(MonitorFromPoint(new Point(),1),ref m))throw new Exception("monitor query");return m.Work.ToString();}
 public static Rect Bounds(IntPtr h){Rect r;if(!GetWindowRect(h,out r))throw new Exception("bounds query");return r;}
 public static string Frame(IntPtr h){Rect r;if(DwmGetWindowAttribute(h,9,out r,Marshal.SizeOf<Rect>())!=0)throw new Exception("DWM query");return r.ToString();}
 public static void Command(IntPtr h,int id){PostMessage(h,0x111,new IntPtr(id),IntPtr.Zero);}
}
'@
[void][FloatingChecks]::SetThreadDpiAwarenessContext([IntPtr](-4))
$exe=Join-Path $root 'target/release/dynamic-island.exe'
if(Get-Process | Where-Object {$_.Path -eq $exe}) {throw 'Prototype already running; no duplicate launched.'}
if([FloatingChecks]::FindWindow('DynamicIsland.WorkspaceReservation',$null) -ne [IntPtr]::Zero){throw 'An old AppBar is still present; close its owning project process normally first.'}
$evidence=Join-Path $root ('artifacts/floating-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
[void](New-Item -ItemType Directory -Path $evidence)
$env:ISLAND_SETTINGS_PATH=Join-Path $evidence 'settings.ini'
$env:ISLAND_DIAGNOSTICS_PATH=Join-Path $evidence 'diagnostics.txt'
$before=[FloatingChecks]::Work()
$checks=[Collections.Generic.List[object]]::new()
function Check($name,$ok,$detail){$checks.Add([pscustomobject]@{Test=$name;Passed=[bool]$ok;Detail=$detail});if(-not $ok){Write-Warning $name}}
function OwnWindow($class,$proc){
 for($attempt=0;$attempt -lt 50;$attempt++) {
  $h=[FloatingChecks]::FindWindow($class,$null);[uint32]$owner=0
  [void][FloatingChecks]::GetWindowThreadProcessId($h,[ref]$owner)
  if($h -ne [IntPtr]::Zero -and $owner -eq $proc.Id){return $h}
  if($proc.HasExited){throw "Project process exited: $class ($($proc.ExitCode))"}
  Start-Sleep -Milliseconds 100
 }
 throw "Window ownership mismatch: $class owner=$owner expected=$($proc.Id) hwnd=$h"
}
function Pause {Start-Sleep -Milliseconds 350}
$prototype=$null;$fixture=$null
try {
 $prototype=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
 Start-Sleep -Seconds 1
 $h=OwnWindow 'DynamicIsland.Native.Prototype' $prototype
 Check 'No AppBar at startup' ([FloatingChecks]::FindWindow('DynamicIsland.WorkspaceReservation',$null) -eq [IntPtr]::Zero) ''
 Check 'Work area unchanged at startup' ([FloatingChecks]::Work() -eq $before) ([FloatingChecks]::Work())
 $fixture=Start-Process -FilePath (Join-Path $root 'target/release/examples/desktop_fixture.exe') -ArgumentList '--maximized' -WindowStyle Hidden -PassThru
 Start-Sleep -Seconds 1
 $f=OwnWindow 'IslandTestFixture' $fixture
 Check 'Maximized fixture occupies ordinary work area' ([FloatingChecks]::Frame($f) -eq $before) ([FloatingChecks]::Frame($f))
 Check 'Maximized fixture does not hide notch' ([FloatingChecks]::IsWindowVisible($h)) ''
 [void][FloatingChecks]::PostMessage($f,0x100,[IntPtr]0x7a,[IntPtr]::Zero);Pause
 Check 'Fullscreen hides notch' (-not [FloatingChecks]::IsWindowVisible($h)) ''
 [void][FloatingChecks]::PostMessage($f,0x100,[IntPtr]0x7a,[IntPtr]::Zero);Pause
 Check 'Leaving fullscreen restores notch' ([FloatingChecks]::IsWindowVisible($h)) ''
 [FloatingChecks]::Command($h,6);Pause
 Check 'Manual hide' (-not [FloatingChecks]::IsWindowVisible($h)) ''
 [void][FloatingChecks]::PostMessage($f,0x100,[IntPtr]0x7a,[IntPtr]::Zero);Pause
 [void][FloatingChecks]::PostMessage($f,0x100,[IntPtr]0x7a,[IntPtr]::Zero);Pause
 Check 'Fullscreen roundtrip preserves manual hide' (-not [FloatingChecks]::IsWindowVisible($h)) ''
 [FloatingChecks]::Command($h,5);Pause
 Check 'Tray show command restores notch' ([FloatingChecks]::IsWindowVisible($h)) ''
 $initial=[FloatingChecks]::Bounds($h).ToString()
 # Warm the native text/composition paths before comparing repeated cycles.
 for($warm=0;$warm -lt 5;$warm++) {
  [FloatingChecks]::Command($h,1);Pause
  [FloatingChecks]::Command($h,1);Pause
  [FloatingChecks]::Command($h,6);Pause
  [FloatingChecks]::Command($h,5);Pause
 }
 $prototype.Refresh();$resourcesBefore=@{Gdi=[FloatingChecks]::GetGuiResources($prototype.Handle,0);User=[FloatingChecks]::GetGuiResources($prototype.Handle,1);Handles=$prototype.HandleCount;Private=$prototype.PrivateMemorySize64}
 for($i=0;$i -lt $Cycles;$i++) {
   [FloatingChecks]::Command($h,1);Pause
   [FloatingChecks]::Command($h,1);Pause
   [FloatingChecks]::Command($h,6);Pause
   [FloatingChecks]::Command($h,5);Pause
 }
 $prototype.Refresh();$resourcesAfter=@{Gdi=[FloatingChecks]::GetGuiResources($prototype.Handle,0);User=[FloatingChecks]::GetGuiResources($prototype.Handle,1);Handles=$prototype.HandleCount;Private=$prototype.PrivateMemorySize64}
 Check 'Cycles preserve position' ([FloatingChecks]::Bounds($h).ToString() -eq $initial) $Cycles
 Check 'GDI objects stable after cycles' ($resourcesAfter.Gdi -eq $resourcesBefore.Gdi) "$($resourcesBefore.Gdi) -> $($resourcesAfter.Gdi)"
 Check 'Work area unchanged after cycles' ([FloatingChecks]::Work() -eq $before) ([FloatingChecks]::Work())
 [FloatingChecks]::Command($h,2)
 Check 'Normal quit finishes within five seconds' ($prototype.WaitForExit(5000)) ''
 Check 'No AppBar after quit' ([FloatingChecks]::FindWindow('DynamicIsland.WorkspaceReservation',$null) -eq [IntPtr]::Zero) ''
 Check 'Work area unchanged after quit' ([FloatingChecks]::Work() -eq $before) ([FloatingChecks]::Work())
 # Seed a legal edge placement; verify startup persistence and reset recovery without moving the user's cursor.
 $settings=Get-Content -LiteralPath $env:ISLAND_SETTINGS_PATH -Raw
 $settings=$settings -replace '(?m)^x=\d+','x=1000000' -replace '(?m)^y=\d+','y=1000000'
 [IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,$settings)
 $prototype=Start-Process -FilePath $exe -ArgumentList '--integrated' -WindowStyle Hidden -PassThru
 Start-Sleep -Seconds 1
 $h=OwnWindow 'DynamicIsland.Native.Prototype' $prototype
 $edge=[FloatingChecks]::Bounds($h).ToString()
 Check 'Saved edge position loaded' ($edge -ne $initial) $edge
 Check 'Retired integrated switch reserves nothing' ([FloatingChecks]::Work() -eq $before -and [FloatingChecks]::FindWindow('DynamicIsland.WorkspaceReservation',$null) -eq [IntPtr]::Zero) ''
 [FloatingChecks]::Command($h,6);Pause
 [FloatingChecks]::Command($h,7);Pause
 Check 'Reset restores visible default position' ([FloatingChecks]::IsWindowVisible($h) -and [FloatingChecks]::Bounds($h).ToString() -eq $initial) ([FloatingChecks]::Bounds($h).ToString())
} finally {
 foreach($p in @($prototype,$fixture)) {
  if($null -ne $p -and -not $p.HasExited){[void]$p.CloseMainWindow();if(-not $p.WaitForExit(5000)){Write-Warning "Project process $($p.Id) did not close normally"}}
 }
 @{Checks=$checks;Before=$resourcesBefore;After=$resourcesAfter;WorkBefore=$before;WorkAfter=[FloatingChecks]::Work();Cycles=$Cycles;Sha256=(Get-FileHash $exe).Hash} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $evidence 'report.json')
 $checks | Format-Table -AutoSize
 Write-Output "Evidence: $evidence"
}
if(@($checks | Where-Object {-not $_.Passed}).Count){exit 1}
