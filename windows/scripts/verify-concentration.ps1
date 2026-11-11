# Native messages to project-owned HWNDs only. This does NOT prove physical click-through.
param([int]$Cycles=100)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe=Join-Path $root 'target/release/dynamic-island.exe'
if(Get-Process | Where-Object {$_.Path -like "$root\*"}) {throw 'Close existing project processes before the test.'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ConcentrationChecks {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom; public override string ToString(){return $"{Left},{Top},{Right},{Bottom}";}}
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] static extern int GetWindowLong(IntPtr h,int n);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr h,uint f);
 [DllImport("user32.dll")] static extern bool RegisterHotKey(IntPtr h,int id,uint mods,uint key);
 [DllImport("user32.dll")] static extern bool UnregisterHotKey(IntPtr h,int id);
 public static bool Available(){bool ok=RegisterHotKey(IntPtr.Zero,0x6135,0x4006,0x78);if(ok)UnregisterHotKey(IntPtr.Zero,0x6135);return ok;}
 public static IntPtr Own(string cls,int pid){var h=FindWindow(cls,null);uint p;GetWindowThreadProcessId(h,out p);return p==pid?h:IntPtr.Zero;}
 public static bool Transparent(IntPtr h){return (GetWindowLong(h,-20)&0x20)!=0;}
 public static Rect Bounds(IntPtr h){Rect r;if(!GetWindowRect(h,out r))throw new Exception("GetWindowRect");return r;}
 public static void Command(IntPtr h,int id){if(!PostMessage(h,0x111,new IntPtr(id),IntPtr.Zero))throw new Exception("PostMessage");}
}
'@
[void][ConcentrationChecks]::SetThreadDpiAwarenessContext([IntPtr](-4))
$dir=Join-Path $root ('artifacts/concentration-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
[void](New-Item -ItemType Directory -Path $dir)
$env:ISLAND_SETTINGS_PATH=Join-Path $dir 'settings.ini'
$env:ISLAND_DIAGNOSTICS_PATH=Join-Path $dir 'diagnostics.txt'
[IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,"version=2`nmonitor=`nx=321000`ny=240000`nhotkey=none`nconcentration_hotkey=Ctrl+Shift+F9`n")
$shortcutFree=[ConcentrationChecks]::Available()
$checks=[Collections.Generic.List[object]]::new()
function Check($name,$ok,$detail='') {$checks.Add([pscustomobject]@{Test=$name;Passed=[bool]$ok;Detail=$detail});if(-not $ok){Write-Warning $name}}
function Pause {Start-Sleep -Milliseconds 300}
function Own($cls,$p) {
 for($i=0;$i -lt 50;$i++){
  $h=[ConcentrationChecks]::Own($cls,$p.Id)
  if($h -ne [IntPtr]::Zero){return $h}
  if($p.HasExited){throw "Project process exited ($($p.ExitCode))"}
  Start-Sleep -Milliseconds 100
 };throw 'Owned test HWND missing'
}
function IsLine($h) {
 $r=[ConcentrationChecks]::Bounds($h);$dpi=[ConcentrationChecks]::GetDpiForWindow($h)
 return [ConcentrationChecks]::Transparent($h) -and ($r.Right-$r.Left) -eq [math]::Round(96*$dpi/96) -and ($r.Bottom-$r.Top) -eq [math]::Round(4*$dpi/96)
}
function Resources($p){$p.Refresh();return @{Gdi=[ConcentrationChecks]::GetGuiResources($p.Handle,0);User=[ConcentrationChecks]::GetGuiResources($p.Handle,1);Handles=$p.HandleCount;Private=$p.PrivateMemorySize64}}
$p=$null;$fixture=$null
try {
 $p=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
 $h=Own 'DynamicIsland.Native.Prototype' $p;Pause
 $initial=[ConcentrationChecks]::Bounds($h).ToString()
 Check 'Starts interactive and visible' (-not [ConcentrationChecks]::Transparent($h) -and [ConcentrationChecks]::IsWindowVisible($h))
 $foreground=[ConcentrationChecks]::GetForegroundWindow()
 [ConcentrationChecks]::Command($h,9);Pause
 Check 'Line is 96x4 DIP with layered pass-through style' (IsLine $h)
 Check 'Mode activation does not acquire foreground' ([ConcentrationChecks]::GetForegroundWindow() -eq $foreground)
 [ConcentrationChecks]::Command($h,9);Pause
 Check 'Normal position and input style restored' (-not [ConcentrationChecks]::Transparent($h) -and [ConcentrationChecks]::Bounds($h).ToString() -eq $initial)
 # Open then immediately interrupt. No synthetic global input.
 [ConcentrationChecks]::Command($h,1)
 Start-Sleep -Milliseconds 30
 [ConcentrationChecks]::Command($h,9);Pause
 Check 'Concentration interrupts opening and remains settled' (IsLine $h)
 [ConcentrationChecks]::Command($h,6);Pause
 Check 'Manual hide hides line' (-not [ConcentrationChecks]::IsWindowVisible($h))
 [ConcentrationChecks]::Command($h,9);Pause
 Check 'Mode toggle preserves manual hide' (-not [ConcentrationChecks]::IsWindowVisible($h))
 [ConcentrationChecks]::Command($h,5);Pause
 Check 'Show restores interactive compact' ([ConcentrationChecks]::IsWindowVisible($h) -and -not [ConcentrationChecks]::Transparent($h) -and [ConcentrationChecks]::Bounds($h).ToString() -eq $initial)
 [ConcentrationChecks]::Command($h,9);Pause
 [void][ConcentrationChecks]::PostMessage($h,0x8001,[IntPtr]::Zero,[IntPtr]0x202);Pause
 Check 'Tray left click message restores normal' (-not [ConcentrationChecks]::Transparent($h) -and [ConcentrationChecks]::Bounds($h).ToString() -eq $initial)
 if($shortcutFree){
  Check 'Concentration shortcut registered' (-not [ConcentrationChecks]::Available())
  [void][ConcentrationChecks]::PostMessage($h,0x312,[IntPtr]2,[IntPtr]::Zero);Pause
  Check 'Shortcut dispatch enters concentration' (IsLine $h)
  [void][ConcentrationChecks]::PostMessage($h,0x312,[IntPtr]2,[IntPtr]::Zero);Pause
  Check 'Shortcut dispatch restores compact' (-not [ConcentrationChecks]::Transparent($h))
 }
 [ConcentrationChecks]::Command($h,9);Pause
 $fixture=Start-Process -FilePath (Join-Path $root 'target/release/examples/desktop_fixture.exe') -ArgumentList '--fullscreen' -WindowStyle Hidden -PassThru
 $f=Own 'IslandTestFixture' $fixture;Pause
 Check 'Fullscreen hides concentration' (-not [ConcentrationChecks]::IsWindowVisible($h))
 [void][ConcentrationChecks]::PostMessage($f,0x100,[IntPtr]0x7a,[IntPtr]::Zero);Pause
 Check 'Fullscreen return preserves concentration' ([ConcentrationChecks]::IsWindowVisible($h) -and (IsLine $h))
 [void][ConcentrationChecks]::PostMessage($f,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
 if(-not $fixture.WaitForExit(5000)){throw 'Fixture did not close normally'}
 for($i=0;$i -lt 10;$i++){[ConcentrationChecks]::Command($h,9);Start-Sleep -Milliseconds 30}
 $before=Resources $p
 for($i=0;$i -lt $Cycles;$i++){
  [ConcentrationChecks]::Command($h,9);Start-Sleep -Milliseconds 30
  [ConcentrationChecks]::Command($h,9);Start-Sleep -Milliseconds 30
 };Pause
 $after=Resources $p
 Check '100 roundtrips preserve concentration' (IsLine $h) $Cycles
 Check 'GDI and USER stable after roundtrips' ($before.Gdi -eq $after.Gdi -and $before.User -eq $after.User) "$($before.Gdi)/$($before.User) -> $($after.Gdi)/$($after.User)"
 [ConcentrationChecks]::Command($h,9);Pause
 Check 'Roundtrips preserve saved compact placement' ([ConcentrationChecks]::Bounds($h).ToString() -eq $initial)
 [ConcentrationChecks]::Command($h,9);Pause
 [ConcentrationChecks]::Command($h,2)
 Check 'Normal exit from concentration' ($p.WaitForExit(5000))
 if($shortcutFree){Check 'Shortcut released on exit' ([ConcentrationChecks]::Available())}
 $saved=Get-Content -LiteralPath $env:ISLAND_SETTINGS_PATH -Raw
 Check 'Position preferences unchanged' ($saved -match '(?m)^x=321000\r?$' -and $saved -match '(?m)^y=240000\r?$')
 $p=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
 $h=Own 'DynamicIsland.Native.Prototype' $p;Pause
 Check 'Restart is normal at saved placement' (-not [ConcentrationChecks]::Transparent($h) -and [ConcentrationChecks]::Bounds($h).ToString() -eq $initial)
 [ConcentrationChecks]::Command($h,9);Pause
 [ConcentrationChecks]::Command($h,8);Pause
 Check 'Move command restores normal input' (-not [ConcentrationChecks]::Transparent($h))
 [ConcentrationChecks]::Command($h,9);Pause
 [ConcentrationChecks]::Command($h,7);Pause
 Check 'Emergency reset restores visible normal' ([ConcentrationChecks]::IsWindowVisible($h) -and -not [ConcentrationChecks]::Transparent($h))
} finally {
 foreach($proc in @($p,$fixture)){
  if($proc -and -not $proc.HasExited){[void]$proc.CloseMainWindow();if(-not $proc.WaitForExit(5000)){Write-Warning "Project process $($proc.Id) failed normal shutdown"}}
 }
 @{Checks=$checks;Before=$before;After=$after;Cycles=$Cycles;ShortcutInitiallyFree=$shortcutFree;Sha256=(Get-FileHash $exe).Hash;Note='Native messages/style tests only. Physical input routing and Opera tabs require manual validation.'} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $dir 'report.json')
 $checks | Format-Table -AutoSize
 Write-Output "Evidence: $dir"
}
if(@($checks | Where-Object {-not $_.Passed}).Count){exit 1}
