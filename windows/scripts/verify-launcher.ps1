# Project-owned HWND messages only; never launch user applications.
param([int]$MeasureSeconds=60)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe=Join-Path $root 'target/release/dynamic-island.exe'
$fixtureExe=Join-Path $root 'target/release/examples/launcher_fixture.exe'
if(Get-Process | Where-Object {$_.Path -like "$root\*"}){throw 'Project process already active'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class LaunchChecks {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string c,string t);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] static extern IntPtr ReadMessage(IntPtr h,uint m,IntPtr w,StringBuilder text);
 public static string Text(IntPtr h) { var b=new StringBuilder(4096);ReadMessage(h,13,new IntPtr(b.Capacity),b);return b.ToString(); }
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h,int id);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] static extern IntPtr SetTextMessage(IntPtr h,uint m,IntPtr w,string value);
 public static void SetText(IntPtr h,string value) { if(h==IntPtr.Zero || SetTextMessage(h,12,IntPtr.Zero,value)==IntPtr.Zero)throw new Exception("Owned control text could not be set"); }
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr h,uint flags);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr h);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtr(IntPtr h,int index);
 [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h,uint command);
 public static IntPtr Own(string c,int pid) {var h=FindWindow(c,null);uint p;GetWindowThreadProcessId(h,out p);return p==pid?h:IntPtr.Zero;}
 public static Rect Bounds(IntPtr h){Rect r;if(!GetWindowRect(h,out r))throw new Exception("Bounds");return r;}
 public static void Send(IntPtr h,uint m,int w=0,int l=0){if(h==IntPtr.Zero||!PostMessage(h,m,new IntPtr(w),new IntPtr(l)))throw new Exception("Invalid owned HWND or PostMessage");}
}
'@
[void][LaunchChecks]::SetThreadDpiAwarenessContext([IntPtr](-4))
$dir=Join-Path $root ('artifacts/launcher-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
[void](New-Item -ItemType Directory -Path $dir)
$env:ISLAND_SETTINGS_PATH=Join-Path $dir 'settings.ini'
$env:ISLAND_SHORTCUTS_PATH=Join-Path $dir 'shortcuts.ini'
$env:ISLAND_DIAGNOSTICS_PATH=Join-Path $dir 'diagnostics.txt'
$env:ISLAND_LAUNCH_FIXTURE_LOG=Join-Path $dir 'launch.txt'
$n=[Environment]::NewLine
[IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,(@('version=1','monitor=','x=500000','y=180000','hotkey=none') -join $n)+$n)
function Hex([string]$s){[Convert]::ToHexString([Text.Encoding]::UTF8.GetBytes($s)).ToLowerInvariant()}
[IO.File]::WriteAllText($env:ISLAND_SHORTCUTS_PATH,(@('version=1',"1|exe|0|$(Hex 'Test local')|$(Hex $fixtureExe)","2|exe|0|$(Hex 'Application absente')|$(Hex 'C:\DynamicIslandMissingTest\absent.exe')") -join $n)+$n)
$checks=[Collections.Generic.List[object]]::new();$samples=[Collections.Generic.List[object]]::new()
function Pause {Start-Sleep -Milliseconds 350}
function Check($name,$valid,$detail=''){$checks.Add([pscustomobject]@{Test=$name;Passed=[bool]$valid;Detail=$detail});Write-Output "$name : $valid"}
function Command([int]$id){[LaunchChecks]::Send($h,0x111,$id);Pause}
function Click([int]$x,[int]$y){$scale=[LaunchChecks]::GetDpiForWindow($h)/96.0;$xy=([int]($y*$scale) -shl 16) -bor [int]($x*$scale);[LaunchChecks]::Send($h,0x201,0,$xy);[LaunchChecks]::Send($h,0x202,0,$xy);Pause}
function Editor { [LaunchChecks]::Own('DynamicIsland.ShortcutEditor',$p.Id) }
function Resources {$p.Refresh();return @{Gdi=[LaunchChecks]::GetGuiResources($p.Handle,0);User=[LaunchChecks]::GetGuiResources($p.Handle,1);Handles=$p.HandleCount}}
function Collect($mode,$width,$height) {
 Write-Output "START MEASUREMENT $mode ($MeasureSeconds seconds)";Start-Sleep -Seconds 3
 $p.Refresh();$cpu=$p.TotalProcessorTime.TotalSeconds;$watch=[Diagnostics.Stopwatch]::StartNew();$local=[Collections.Generic.List[object]]::new()
 while($watch.Elapsed.TotalSeconds -lt $MeasureSeconds){
  Start-Sleep -Seconds 1;$p.Refresh();if($p.HasExited){throw 'Unexpected shutdown'}
  $r=[LaunchChecks]::Bounds($h);$scale=[LaunchChecks]::GetDpiForWindow($h)/96.0
  $row=[pscustomobject]@{Mode=$mode;Seconds=$watch.Elapsed.TotalSeconds;Cpu=$p.TotalProcessorTime.TotalSeconds;WorkingSet=$p.WorkingSet64;PrivateBytes=$p.PrivateMemorySize64;Handles=$p.HandleCount;Gdi=[LaunchChecks]::GetGuiResources($p.Handle,0);User=[LaunchChecks]::GetGuiResources($p.Handle,1);Valid=([LaunchChecks]::IsWindowVisible($h) -and ($r.Right-$r.Left) -eq [math]::Round($width*$scale) -and ($r.Bottom-$r.Top) -eq [math]::Round($height*$scale) -and (Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=launcher')}
  $samples.Add($row);$local.Add($row)
 }
 $duration=$watch.Elapsed.TotalSeconds
 $summary=[ordered]@{Mode=$mode;Duration=$duration;Samples=$local.Count;Valid=@($local|Where-Object Valid).Count;CpuSeconds=($local[-1].Cpu-$cpu);CpuOneCorePercent=(100*($local[-1].Cpu-$cpu)/$duration)}
 foreach($metric in 'WorkingSet','PrivateBytes','Handles','Gdi','User'){$v=$local|Measure-Object $metric -Minimum -Maximum -Average;$summary[$metric]=@{Min=$v.Minimum;Max=$v.Maximum;Mean=$v.Average}}
 $summary|ConvertTo-Json -Depth 4|Set-Content (Join-Path $dir "$mode.json")
 Write-Output "END MEASUREMENT $mode valid=$($summary.Valid)/$($summary.Samples)"
}
$p=$null
try {
 $p=Start-Process $exe -WindowStyle Hidden -PassThru
 for($i=0;$i -lt 50;$i++){$h=[LaunchChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($h -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
 if($h -eq [IntPtr]::Zero){throw 'Prototype HWND missing'}
 Command 16;Start-Sleep -Seconds 1
 Check 'Launcher page stored in v5' ((Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=launcher')
 Check 'Catalog loading does not execute entries' (-not (Test-Path $env:ISLAND_LAUNCH_FIXTURE_LOG))
 Click 90 94;Start-Sleep -Seconds 2
 Check 'Explicit click launches own fixture; it exits normally' ((Test-Path $env:ISLAND_LAUNCH_FIXTURE_LOG) -and (Get-Content $env:ISLAND_LAUNCH_FIXTURE_LOG -Raw) -match 'started-visible[\r\n]+normal-exit')
 Command 16
 Click 90 118;Start-Sleep -Seconds 1
 Check 'Missing application reported without launch' ((Get-Content $env:ISLAND_DIAGNOSTICS_PATH -Raw) -match 'launcher operation failed: Cible introuvable')
 Click 290 94;Start-Sleep -Seconds 1
 Check 'Favorite saved asynchronously' ((Get-Content $env:ISLAND_SHORTCUTS_PATH -Raw) -match '1\|exe\|1\|')
 Click 160 66
 Click 65 66;$edit=Editor
 Check 'Add opens native editor' ($edit -ne [IntPtr]::Zero)
 if($edit -eq [IntPtr]::Zero){throw 'Editor unavailable'}
 Check 'Editor is independent and not topmost' ([LaunchChecks]::GetWindow($edit,4) -eq [IntPtr]::Zero -and ([LaunchChecks]::GetWindowLongPtr($edit,-20).ToInt64() -band 8) -eq 0)
 [LaunchChecks]::SetText([LaunchChecks]::GetDlgItem($edit,100),'Projet de test')
 [void][LaunchChecks]::SendMessage([LaunchChecks]::GetDlgItem($edit,101),0x14e,[IntPtr]1,[IntPtr]::Zero)
 [LaunchChecks]::SetText([LaunchChecks]::GetDlgItem($edit,102),$dir)
 [LaunchChecks]::Send($edit,0x111,1)
 for($i=0;$i -lt 40 -and (Editor) -ne [IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100}
 $saved=(Editor) -eq [IntPtr]::Zero -and (Get-Content $env:ISLAND_SHORTCUTS_PATH -Raw) -match (Hex 'Projet de test')
 $detail=if((Editor) -ne [IntPtr]::Zero){[LaunchChecks]::Text([LaunchChecks]::GetDlgItem((Editor),105))}else{''}
 Check 'Folder saved; editor closes without opening it' $saved $detail
 if(-not $saved){throw "Editor save failed: $detail"}
 Command 16
 $before=Resources
 for($i=0;$i -lt 10;$i++){Click 65 66;$edit=Editor;[LaunchChecks]::Send($edit,0x10);Pause;Command 16}
 Start-Sleep -Seconds 2;$after=Resources
 Check 'GDI stable after ten editor cycles' ($before.Gdi -eq $after.Gdi) "$($before.Gdi)/$($before.User)/$($before.Handles) -> $($after.Gdi)/$($after.User)/$($after.Handles)"
 Command 1
 if($MeasureSeconds -gt 0){Collect 'launcher-compact' 172 32}
 Command 16
 if($MeasureSeconds -gt 0){Collect 'launcher-expanded' 384 168}
 Command 17;Command 16
 Check 'Disabled launcher cannot navigate' ((Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'launcher_enabled=false' -and (Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=home')
 Command 17;Command 16
 Command 2;Check 'Normal shutdown' ($p.WaitForExit(5000))
 $p=Start-Process $exe -WindowStyle Hidden -PassThru
 for($i=0;$i -lt 50;$i++){$h=[LaunchChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($h -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
 Start-Sleep -Seconds 1
 Check 'Catalog survives restart without auto-execution' ((Get-Content $env:ISLAND_SHORTCUTS_PATH -Raw) -match (Hex 'Projet de test') -and @(Get-Content $env:ISLAND_LAUNCH_FIXTURE_LOG|Where-Object {$_ -eq 'started-visible'}).Count -eq 1)
 Command 2;Check 'Restarted prototype shuts down normally' ($p.WaitForExit(5000))
} finally {
 if($p -and -not $p.HasExited){$owned=[LaunchChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($owned -ne [IntPtr]::Zero){[LaunchChecks]::Send($owned,0x10);if(-not $p.WaitForExit(5000)){Write-Warning 'Normal shutdown timed out'}}}
 $samples|Export-Csv (Join-Path $dir 'samples.csv') -NoTypeInformation
 @{Checks=$checks;Before=$before;After=$after;Sha256=(Get-FileHash $exe).Hash;Note='Own HWND messages and own fixture only; no physical-input certification or third-party launch.'}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $dir 'report.json')
 Write-Output "Evidence: $dir"
}
if(@($checks|Where-Object {-not $_.Passed}).Count){exit 1}

