# Bounded native registration/dispatch test; no physical/global input injection.
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$exe=Join-Path $root 'target/release/dynamic-island.exe'
if(Get-Process | Where-Object {$_.Path -eq $exe}){throw 'Close the project instance before shortcut test.'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class IslandShortcutCheck {
 [DllImport("user32.dll")] static extern bool RegisterHotKey(IntPtr h,int id,uint mods,uint key);
 [DllImport("user32.dll")] static extern bool UnregisterHotKey(IntPtr h,int id);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h,uint msg,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 public static bool Available(){bool ok=RegisterHotKey(IntPtr.Zero,0x6134,0x4000|2|4,0x79);if(ok)UnregisterHotKey(IntPtr.Zero,0x6134);return ok;}
 public static IntPtr Own(uint pid){var h=FindWindow("DynamicIsland.Native.Prototype",null);uint p;GetWindowThreadProcessId(h,out p);return p==pid?h:IntPtr.Zero;}
 public static void Dispatch(IntPtr h){PostMessage(h,0x312,new IntPtr(1),IntPtr.Zero);}
}
'@
if(-not [IslandShortcutCheck]::Available()){Write-Output 'SKIPPED: Ctrl+Shift+F10 already registered. No existing registration modified.';exit 0}
$dir=Join-Path $root ('artifacts/shortcut-'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
[void](New-Item -ItemType Directory -Path $dir)
$env:ISLAND_SETTINGS_PATH=Join-Path $dir 'settings.ini'
$env:ISLAND_DIAGNOSTICS_PATH=Join-Path $dir 'diagnostics.txt'
[IO.File]::WriteAllText($env:ISLAND_SETTINGS_PATH,"version=1`nmonitor=`nx=500000`ny=0`nhotkey=Ctrl+Shift+F10`n")
$p=$null;$results=[ordered]@{}
try {
 $p=Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
 $h=[IntPtr]::Zero
 for($i=0;$i -lt 50 -and $h -eq [IntPtr]::Zero;$i++){Start-Sleep -Milliseconds 100;$h=[IslandShortcutCheck]::Own($p.Id)}
 if($h -eq [IntPtr]::Zero){throw 'Project HWND unavailable'}
 Start-Sleep -Milliseconds 500
 $results.RegisteredByApp=-not [IslandShortcutCheck]::Available()
 [IslandShortcutCheck]::Dispatch($h);Start-Sleep -Milliseconds 350
 $results.MessageHides=-not [IslandShortcutCheck]::IsWindowVisible($h)
 [IslandShortcutCheck]::Dispatch($h);Start-Sleep -Milliseconds 350
 $results.MessageRestores=[IslandShortcutCheck]::IsWindowVisible($h)
} finally {
 if($p -and -not $p.HasExited){[void]$p.CloseMainWindow();$results.CleanExit=$p.WaitForExit(5000)}
 $results.ReleasedAfterExit=[IslandShortcutCheck]::Available()
 $results.Note='Native registration and WM_HOTKEY dispatch only; not a physical keyboard test.'
 $results | ConvertTo-Json | Tee-Object -FilePath (Join-Path $dir 'report.json')
 Write-Output "Evidence: $dir"
}
if($results.Values -contains $false){exit 1}
