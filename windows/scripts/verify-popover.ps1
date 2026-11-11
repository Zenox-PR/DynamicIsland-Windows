# Bounded native messages to owned windows. Does not replace physical input testing.
param([int]$MeasureSeconds=60,[int]$ResourceCycles=20,[switch]$Navigation,[switch]$Timers,[int]$ActivationWaitSeconds=0)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe=Join-Path $root 'target/release/dynamic-island.exe'
if(Get-Process | Where-Object {$_.Path -like "$root\*"}){throw 'Project process already active'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PopChecks {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string c,string t);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int n);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern uint GetGuiResources(IntPtr h,uint f);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 public static IntPtr Own(string c,int pid){var h=FindWindow(c,null);uint p;GetWindowThreadProcessId(h,out p);return p==pid?h:IntPtr.Zero;}
 public static Rect Bounds(IntPtr h){Rect r;if(!GetWindowRect(h,out r))throw new Exception("GetWindowRect");return r;}
 public static void Send(IntPtr h,uint m,int w=0,int l=0){if(h==IntPtr.Zero||!PostMessage(h,m,new IntPtr(w),new IntPtr(l)))throw new Exception("PostMessage");}
}
'@
[void][PopChecks]::SetThreadDpiAwarenessContext([IntPtr](-4))
$dir=Join-Path $root ('artifacts/popover-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
[void](New-Item -ItemType Directory -Path $dir)
$env:ISLAND_SETTINGS_PATH=Join-Path $dir 'settings.ini'
$env:ISLAND_DIAGNOSTICS_PATH=Join-Path $dir 'diagnostics.txt'
[IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,"version=2`nmonitor=`nx=500000`ny=150000`nhotkey=none`nconcentration_hotkey=none`n")
if($Timers){[IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,"version=4`nmonitor=`nx=500000`ny=150000`nhotkey=none`nconcentration_hotkey=none`nmedia_enabled=true`nactive_page=media`ntimer_enabled=true`ntimer_mode=timer`ntimer_minutes=1`nwork_minutes=25`nshort_minutes=5`nlong_minutes=15`n")}
$checks=[Collections.Generic.List[object]]::new()
$samples=[Collections.Generic.List[object]]::new()
function Pause {Start-Sleep -Milliseconds 250}
function Pop {return [PopChecks]::Own('DynamicIsland.MediaPopover',$p.Id)}
function Command([int]$id){
 if($id -in @(1,10,11,12,14)){[PopChecks]::Send($f,0x8000,7);Start-Sleep -Milliseconds 30}
 [PopChecks]::Send($h,0x111,$id);Pause
}
function Check($name,$ok,$detail=''){$checks.Add([pscustomobject]@{Test=$name;Passed=[bool]$ok;Detail=$detail});Write-Output "$name : $ok"}
function Resources {$p.Refresh();return @{Gdi=[PopChecks]::GetGuiResources($p.Handle,0);User=[PopChecks]::GetGuiResources($p.Handle,1);Handles=$p.HandleCount}}
function Collect($mode,$width,$height,$popupExpected){
 Write-Output "START MEASUREMENT $mode ($MeasureSeconds seconds)"
 Start-Sleep -Seconds 5
 $p.Refresh();$startCpu=$p.TotalProcessorTime.TotalSeconds;$watch=[Diagnostics.Stopwatch]::StartNew()
 $local=[Collections.Generic.List[object]]::new()
 while($watch.Elapsed.TotalSeconds -lt $MeasureSeconds){
  Start-Sleep -Seconds 1;$p.Refresh();if($p.HasExited){throw 'Unexpected exit'}
  $r=[PopChecks]::Bounds($h);$scale=[PopChecks]::GetDpiForWindow($h)/96.0
  $present=(Pop) -ne [IntPtr]::Zero
  $pageValid=$true
  if($mode -like 'home-*'){
   $saved=Get-Content $env:ISLAND_SETTINGS_PATH -Raw
   $pageValid=$saved -match 'active_page=home' -and $saved -match $(if($mode -eq 'home-disabled'){'media_enabled=false'}else{'media_enabled=true'})
  }
  if($mode -like 'timer-*'){
   $saved=Get-Content $env:ISLAND_SETTINGS_PATH -Raw
   $pageValid=$saved -match 'active_page=timer' -and $saved -match 'timer_mode=pomodoro'
  }
  $row=[pscustomobject]@{Mode=$mode;Seconds=$watch.Elapsed.TotalSeconds;Cpu=$p.TotalProcessorTime.TotalSeconds;WorkingSet=$p.WorkingSet64;PrivateBytes=$p.PrivateMemorySize64;Handles=$p.HandleCount;Gdi=[PopChecks]::GetGuiResources($p.Handle,0);User=[PopChecks]::GetGuiResources($p.Handle,1);Valid=([PopChecks]::IsWindowVisible($h) -and ($r.Right-$r.Left) -eq [math]::Round($width*$scale) -and ($r.Bottom-$r.Top) -eq [math]::Round($height*$scale) -and $present -eq $popupExpected)}
  $row.Valid=$row.Valid -and $pageValid
  $local.Add($row);$samples.Add($row)
 }
 $duration=$watch.Elapsed.TotalSeconds
 $summary=[ordered]@{Mode=$mode;Duration=$duration;Samples=$local.Count;Valid=@($local | Where-Object Valid).Count;CpuSeconds=($local[-1].Cpu-$startCpu);CpuOneCorePercent=(100*($local[-1].Cpu-$startCpu)/$duration)}
 foreach($n in 'WorkingSet','PrivateBytes','Handles','Gdi','User'){$s=$local | Measure-Object $n -Minimum -Maximum -Average;$summary[$n]=@{Min=$s.Minimum;Max=$s.Maximum;Mean=$s.Average}}
 $summary | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $dir "$mode.json")
 Write-Output "END MEASUREMENT $mode valid=$($summary.Valid)/$($summary.Samples)"
}
$p=$null;$fixture=$null
try {
 $fixture=Start-Process (Join-Path $root 'target/release/examples/desktop_fixture.exe') -WindowStyle Hidden -PassThru
 for($i=0;$i -lt 50;$i++){$f=[PopChecks]::Own('IslandTestFixture',$fixture.Id);if($f -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
 if($f -eq [IntPtr]::Zero){throw 'Fixture HWND missing'}
 [void][PopChecks]::ShowWindow($f,1)
 [void][PopChecks]::SetForegroundWindow($f)
 if([PopChecks]::GetForegroundWindow() -ne $f -and $ActivationWaitSeconds -gt 0){
  Write-Output 'WAIT: Click the Banc de test Dynamic Island window to grant normal foreground activation.'
  $activationWatch=[Diagnostics.Stopwatch]::StartNew()
  while([PopChecks]::GetForegroundWindow() -ne $f -and $activationWatch.Elapsed.TotalSeconds -lt [math]::Min($ActivationWaitSeconds,180)){Start-Sleep -Milliseconds 250}
 }
 if([PopChecks]::GetForegroundWindow() -ne $f){throw 'Fixture not foreground; native test cannot proceed without user activation.'}
 $p=Start-Process $exe -WindowStyle Hidden -PassThru
 for($i=0;$i -lt 50;$i++){$h=[PopChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($h -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
 if($h -eq [IntPtr]::Zero){throw 'Main HWND missing'}
 Start-Sleep -Seconds 2
 Command 10;$pop=Pop
 Check 'Explicit opening creates focused popup' ($pop -ne [IntPtr]::Zero -and [PopChecks]::GetForegroundWindow() -eq $pop)
 [PopChecks]::Send($pop,0x100,0x28);Pause
 [PopChecks]::Send($pop,0x100,0x1b);Pause
 Check 'Escape closes popup' ((Pop) -eq [IntPtr]::Zero)
 Command 10;$pop=Pop
 # Home selects automatic policy; never sends playback commands to third-party sessions.
 [PopChecks]::Send($pop,0x100,0x24);Pause
 [PopChecks]::Send($pop,0x100,0x0d);Pause
 Check 'Keyboard automatic choice closes popup' ((Pop) -eq [IntPtr]::Zero)
 Command 10
 [void][PopChecks]::SetForegroundWindow($f)
 Start-Sleep -Milliseconds 700
 $f=[PopChecks]::Own('IslandTestFixture',$fixture.Id)
 Check 'Deactivation closes popup without reclaiming foreground' ((Pop) -eq [IntPtr]::Zero -and [PopChecks]::GetForegroundWindow() -eq $f)
 Command 10;Command 9
 Check 'Concentration closes popup' ((Pop) -eq [IntPtr]::Zero)
 Command 10;$pop=Pop
 Check 'Explicit selector available in concentration' ($pop -ne [IntPtr]::Zero)
 [PopChecks]::Send($pop,0x100,0x1b);Pause
 $r=[PopChecks]::Bounds($h);$scale=[PopChecks]::GetDpiForWindow($h)/96.0
 Check 'Escape preserves concentration line' (($r.Bottom-$r.Top) -eq [math]::Round(4*$scale))
 Command 9;Command 10;Command 6
 Check 'Manual hide closes popup' ((Pop) -eq [IntPtr]::Zero -and -not [PopChecks]::IsWindowVisible($h))
 Command 5;Command 10;$pop=Pop
 [PopChecks]::Send($pop,0x7e);Pause
 Check 'Display notification closes stale popup' ((Pop) -eq [IntPtr]::Zero)
 if($Navigation){
  function Saved {return Get-Content $env:ISLAND_SETTINGS_PATH -Raw}
  Command 11
  Check 'Home page persisted in modules schema' ((Saved) -match 'active_page=home' -and (Saved) -match 'version=[34]')
  [PopChecks]::Send($h,0x100,9);Pause
  [PopChecks]::Send($h,0x100,9);Pause
  [PopChecks]::Send($h,0x100,13);Pause
  Check 'Tab and Enter navigate to Media' ((Saved) -notmatch 'active_page=home')
  $dpi=[PopChecks]::GetDpiForWindow($h)/96.0
  $xy=([int](20*$dpi) -shl 16) -bor [int](48*$dpi)
  [PopChecks]::Send($h,0x201,0,$xy);[PopChecks]::Send($h,0x202,0,$xy);Pause
  Check 'Client mouse messages navigate to Home' ((Saved) -match 'active_page=home')
  if($MeasureSeconds -gt 0 -and -not $Timers){Collect 'home-enabled' 384 168 $false}
  $xy=([int](93*$dpi) -shl 16) -bor [int](280*$dpi)
  [PopChecks]::Send($h,0x201,0,$xy);[PopChecks]::Send($h,0x202,0,$xy);Pause
  Check 'Module disabled through Home button' ((Saved) -match 'media_enabled=false')
  Command 10
  Check 'Disabled module cannot open session selector' ((Pop) -eq [IntPtr]::Zero)
  Command 12
  Check 'Unavailable Media page keeps Home' ((Saved) -match 'active_page=home')
  if($MeasureSeconds -gt 0 -and -not $Timers){Collect 'home-disabled' 384 168 $false}
  Command 2
  Check 'Shutdown with disabled module' ($p.WaitForExit(5000))
  $p=Start-Process $exe -WindowStyle Hidden -PassThru
  for($i=0;$i -lt 50;$i++){$h=[PopChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($h -ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 100}
  Pause
  Command 10
  Check 'Disabled module preserved after restart' ((Pop) -eq [IntPtr]::Zero -and (Saved) -match 'media_enabled=false')
  Command 13;Command 12;Command 10
  Check 'Module reenabled and selector available' ((Pop) -ne [IntPtr]::Zero)
  [PopChecks]::Send((Pop),0x100,0x1b);Pause
  Command 9;Command 11
  $r=[PopChecks]::Bounds($h)
  Check 'Page choice preserves concentration priority' (($r.Bottom-$r.Top) -eq [math]::Round(4*$dpi))
  Command 6;Command 12
  Check 'Page choice preserves manual hide' (-not [PopChecks]::IsWindowVisible($h))
  Command 5
 }
 if($Timers){
  function TimerClick([int]$x,[int]$y){
   $dpi=[PopChecks]::GetDpiForWindow($h)/96.0
   $xy=([int]($y*$dpi) -shl 16) -bor [int]($x*$dpi)
   [PopChecks]::Send($h,0x201,0,$xy);[PopChecks]::Send($h,0x202,0,$xy);Pause
  }
  function LastTimer {return @(Get-Content $env:ISLAND_DIAGNOSTICS_PATH | Where-Object {$_ -like 'timer action:*'})[-1]}
  Command 14
  Check 'Timer page persists in v4' ((Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=timer')
  TimerClick 75 137
  Check 'Timer starts from native client click' ((LastTimer) -match 'TimerPrimary.*Countdown.*Running')
  TimerClick 75 137
  Check 'Pause preserves remaining duration' ((LastTimer) -match 'Paused.*remaining: [1-9]')
  Command 11;Command 12;Command 14
  TimerClick 75 137
  Check 'Resume after navigating other modules' ((LastTimer) -match 'TimerPrimary.*Countdown.*Running')
  TimerClick 200 137;TimerClick 175 55;TimerClick 75 137
  Check 'Pomodoro work session starts' ((LastTimer) -match 'TimerPrimary.*Work.*Running')
  Command 1
  if($MeasureSeconds -gt 0){Collect 'timer-compact' 172 32 $false}
  Command 9
  if($MeasureSeconds -gt 0){Collect 'timer-concentration' 96 4 $false}
  Command 9;Command 14
  if($MeasureSeconds -gt 0){Collect 'timer-expanded' 384 168 $false}
  # The full 25-minute work session must still be running after presentation changes.
  TimerClick 75 137
  Check 'Work continued compact and concentration, then paused' ((LastTimer) -match 'TimerPrimary.*Work.*Paused')
  Command 15
  Check 'Disabled timer falls back to Home' ((Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=home')
  Command 14
  Check 'Disabled timer page unavailable' ((Get-Content $env:ISLAND_SETTINGS_PATH -Raw) -match 'active_page=home')
  Command 15;Command 14;TimerClick 75 137
  Check 'Reenabled timer can resume preserved session' ((LastTimer) -match 'TimerPrimary.*Work.*Running')
  TimerClick 200 137;TimerClick 65 55
  $timerBefore=Resources
  for($i=0;$i -lt 20;$i++){TimerClick 75 137;TimerClick 200 137}
  Start-Sleep -Seconds 2
  $timerAfter=Resources
  Check 'Timer resource counts after 20 start-reset cycles' ($timerBefore.Gdi -eq $timerAfter.Gdi -and $timerBefore.User -eq $timerAfter.User) "$($timerBefore.Gdi)/$($timerBefore.User) -> $($timerAfter.Gdi)/$($timerAfter.User); handles $($timerBefore.Handles) -> $($timerAfter.Handles)"
  TimerClick 75 137;Command 1
  Write-Output 'START 65-second countdown completion check, panel compact'
  for($wait=0;$wait -lt 65;$wait++){Start-Sleep -Seconds 1}
  $finished=@(Get-Content $env:ISLAND_DIAGNOSTICS_PATH | Where-Object {$_ -eq 'timer completed: Minuteur'})
  Check 'One-minute countdown completes once while compact' ($finished.Count -eq 1)
  [PopChecks]::Send($h,0x113,5);[PopChecks]::Send($h,0x218,18);Pause
  $finished=@(Get-Content $env:ISLAND_DIAGNOSTICS_PATH | Where-Object {$_ -eq 'timer completed: Minuteur'})
  Check 'Late timer and simulated resume do not duplicate completion' ($finished.Count -eq 1)
 }
 Start-Sleep -Seconds 5
 $before=Resources
 $cycleSamples=[Collections.Generic.List[object]]::new()
 for($i=0;$i -lt $ResourceCycles;$i++){
  Command 10;$pop=Pop;[PopChecks]::Send($pop,0x100,0x1b);Pause
  $r=Resources;$cycleSamples.Add([pscustomobject]@{Cycle=$i+1;Gdi=$r.Gdi;User=$r.User;Handles=$r.Handles})
 }
 $cycleSamples | Export-Csv (Join-Path $dir 'cycles.csv') -NoTypeInformation
 Start-Sleep -Seconds 5
 $after=Resources
 if($ResourceCycles -gt 0){Check "GDI and USER stable after $ResourceCycles cycles and settling" ($before.Gdi -eq $after.Gdi -and $before.User -eq $after.User) "$($before.Gdi)/$($before.User) -> $($after.Gdi)/$($after.User)"}
 if($MeasureSeconds -gt 0 -and -not $Navigation -and -not $Timers){
  Collect 'compact' 172 32 $false
  Command 9
  Collect 'concentration' 96 4 $false
  Command 9;Command 1;Command 10
  Collect 'expanded-popover' 384 168 $true
 }
 Command 2
 Check 'Normal shutdown without residual process' ($p.WaitForExit(5000))
} finally {
 if($p -and -not $p.HasExited){$owned=[PopChecks]::Own('DynamicIsland.Native.Prototype',$p.Id);if($owned -ne [IntPtr]::Zero){[PopChecks]::Send($owned,0x10)}}
 if($fixture -and -not $fixture.HasExited){$owned=[PopChecks]::Own('IslandTestFixture',$fixture.Id);if($owned -ne [IntPtr]::Zero){[PopChecks]::Send($owned,0x10)}}
 foreach($proc in @($p,$fixture)){if($proc -and -not $proc.HasExited){[void]$proc.CloseMainWindow();if(-not $proc.WaitForExit(5000)){Write-Warning "Normal close failed: $($proc.Id)"}}}
 $samples | Export-Csv (Join-Path $dir 'samples.csv') -NoTypeInformation
 @{Checks=$checks;Before=$before;After=$after;Sha256=(Get-FileHash $exe).Hash;Note='Native messages only; short measured intervals, not long-term stability or physical validation.'} | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $dir 'report.json')
 Write-Output "Evidence: $dir"
}
if(@($checks | Where-Object {-not $_.Passed}).Count){exit 1}
