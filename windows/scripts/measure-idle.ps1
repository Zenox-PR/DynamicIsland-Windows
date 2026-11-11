param(
    [Parameter(Mandatory = $true)][int]$PrototypeId,
    [ValidateRange(1, 3600)][int]$DurationSeconds = 300,
    [ValidateRange(0, 120)][int]$WarmupSeconds = 60,
    [ValidateSet('any','compact','expanded','hidden','concentration')][string]$ExpectedState = 'any',
    [ValidateSet('any','floating','integrated')][string]$ExpectedMode = 'any'
)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$expectedExe = Join-Path $projectRoot 'target\release\dynamic-island.exe'
$prototypeProcess = Get-Process -Id $PrototypeId
if ($prototypeProcess.Path -ne $expectedExe) { throw 'PID does not belong to this project Release executable.' }
$outputDirectory = Join-Path $projectRoot ('artifacts\validation-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $outputDirectory | Out-Null
Write-Output "Evidence directory: $outputDirectory"
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class IslandResourceCounters {
    [DllImport("user32.dll")]
    public static extern uint GetGuiResources(IntPtr process, uint flags);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out Rect bounds);
    [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr window);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [StructLayout(LayoutKind.Sequential)] public struct MonitorInfo { public uint Size; public Rect Monitor, Work; public uint Flags; }
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr window, uint flags);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
    public static int[] WindowState(int expectedProcess) {
        var window=FindWindow("DynamicIsland.Native.Prototype",null);
        uint process; GetWindowThreadProcessId(window,out process);
        if(window==IntPtr.Zero || process!=expectedProcess) return new int[9];
        var old=SetThreadDpiAwarenessContext(new IntPtr(-4));
        try { Rect r; if(!GetWindowRect(window,out r)) return new int[9];
            var bar=FindWindow("DynamicIsland.WorkspaceReservation",null);
            uint barProcess; GetWindowThreadProcessId(bar,out barProcess);
            Rect b=new Rect(); bool present=bar!=IntPtr.Zero && barProcess==expectedProcess && GetWindowRect(bar,out b);
            var info=new MonitorInfo(); info.Size=(uint)Marshal.SizeOf<MonitorInfo>();
            GetMonitorInfo(MonitorFromWindow(window,2),ref info);
            return new int[]{IsWindowVisible(window)?1:0,r.Right-r.Left,r.Bottom-r.Top,(int)GetDpiForWindow(window),
                present?1:0,b.Bottom-b.Top,info.Work.Top,b.Bottom,1};
        } finally { if(old!=IntPtr.Zero) SetThreadDpiAwarenessContext(old); }
    }
}
'@
$logicalProcessors = [int](Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
@{
    Started = (Get-Date).ToString('o')
    ProcessId = $PrototypeId
    ExpectedState = $ExpectedState
    ExpectedMode = $ExpectedMode
    Executable = $expectedExe
    Sha256 = (Get-FileHash -LiteralPath $expectedExe -Algorithm SHA256).Hash
    LogicalProcessors = $logicalProcessors
    CPU = (Get-CimInstance Win32_Processor | Select-Object Name,NumberOfLogicalProcessors)
    RAMBytes = (Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory
    OS = (Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,BuildNumber)
    GPU = (Get-CimInstance Win32_VideoController | Select-Object Name,CurrentHorizontalResolution,CurrentVerticalResolution,CurrentRefreshRate)
    Note = 'Read-only observation. Background applications retained. WorkingSet64 is NOT private working set. GPU counters exclude DWM attribution.'
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $outputDirectory 'environment.json')
function Save-ExtraCounters([string]$Label) {
    Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess = $PrototypeId" |
        Select-Object IDProcess,WorkingSet,WorkingSetPrivate,PrivateBytes,HandleCount,ThreadCount |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputDirectory "$Label-memory.json")
    $engines = @(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine |
        Where-Object Name -like "pid_${PrototypeId}_*" | Select-Object Name,UtilizationPercentage)
    @{ Samples = $engines; Note = 'An empty list is unavailable per-process attribution, not proof of zero GPU usage.' } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $outputDirectory "$Label-gpu.json")
}
for ($second = 0; $second -lt $WarmupSeconds; $second++) { Start-Sleep -Seconds 1 }
function Get-WindowSample {
    $window = [IslandResourceCounters]::WindowState($PrototypeId)
    if ($window[8] -ne 1) { throw "Prototype HWND missing or ownership mismatch; invalid sample." }
    $matches = $true
    if ($ExpectedState -eq 'hidden') { $matches = $window[0] -eq 0 }
    elseif ($ExpectedState -ne 'any') {
        $dipWidth = switch($ExpectedState){'compact'{172};'concentration'{96};default{384}}
        $dipHeight = switch($ExpectedState){'compact'{32};'concentration'{4};default{168}}
        $matches = $window[0] -eq 1 -and $window[1] -eq [math]::Round($dipWidth*$window[3]/96.0) -and $window[2] -eq [math]::Round($dipHeight*$window[3]/96.0)
    }
    if($ExpectedMode -eq 'integrated'){$matches=$matches -and $window[4] -eq 1 -and $window[5] -eq [math]::Round(48*$window[3]/96.0) -and $window[6] -ge $window[7]}
    if($ExpectedMode -eq 'floating'){$matches=$matches -and $window[4] -eq 0}
    return @{Visible=$window[0];Width=$window[1];Height=$window[2];Dpi=$window[3];Matches=$matches;AppBar=$window[4];BandHeight=$window[5];WorkTop=$window[6]}
}
if (-not (Get-WindowSample).Matches) { throw 'The prototype is not visible in the expected state; no idle measurement started.' }
Save-ExtraCounters 'before'
$samples = [Collections.Generic.List[object]]::new()
$prototypeProcess.Refresh()
$lastCpu = $prototypeProcess.TotalProcessorTime.TotalSeconds
$initialCpu = $lastCpu
$clock = [Diagnostics.Stopwatch]::StartNew()
$lastTime = 0.0
while ($clock.Elapsed.TotalSeconds -lt $DurationSeconds) {
    Start-Sleep -Seconds 1
    $prototypeProcess.Refresh()
    if ($prototypeProcess.HasExited) { throw 'Prototype exited during measurement.' }
    $elapsed = $clock.Elapsed.TotalSeconds
    $cpu = $prototypeProcess.TotalProcessorTime.TotalSeconds
    $window = Get-WindowSample
    $sample = [pscustomobject]@{
        ElapsedSeconds = $elapsed
        CpuSeconds = $cpu
        CpuMachinePercent = 100 * ($cpu - $lastCpu) / (($elapsed - $lastTime) * $logicalProcessors)
        WorkingSetBytes = $prototypeProcess.WorkingSet64
        PrivateBytes = $prototypeProcess.PrivateMemorySize64
        Handles = $prototypeProcess.HandleCount
        Threads = $prototypeProcess.Threads.Count
        GdiObjects = [IslandResourceCounters]::GetGuiResources($prototypeProcess.Handle, 0)
        UserObjects = [IslandResourceCounters]::GetGuiResources($prototypeProcess.Handle, 1)
        WindowVisible = $window.Visible
        WindowWidthPx = $window.Width
        WindowHeightPx = $window.Height
        WindowDpi = $window.Dpi
        AppBarPresent = $window.AppBar
        BandHeightPx = $window.BandHeight
        WorkTopPx = $window.WorkTop
        StateMatches = $window.Matches
    }
    $samples.Add($sample)
    $sample | Export-Csv -LiteralPath (Join-Path $outputDirectory 'idle.csv') -NoTypeInformation -Append
    $lastCpu = $cpu
    $lastTime = $elapsed
    if ($samples.Count % 30 -eq 0) { Write-Output "Collected $($samples.Count) samples; elapsed $([math]::Round($elapsed,1)) s" }
}
$summary = [ordered]@{
    DurationSeconds = $lastTime
    Samples = $samples.Count
    ExpectedState = $ExpectedState
    ExpectedMode = $ExpectedMode
    MatchingSamples = @($samples | Where-Object StateMatches).Count
    VisibleSamples = @($samples | Where-Object WindowVisible -eq 1).Count
    CpuSecondsDelta = $lastCpu - $initialCpu
    CpuMachinePercentMean = 100 * ($lastCpu - $initialCpu) / ($lastTime * $logicalProcessors)
    CpuMachinePercentPeak = ($samples | Measure-Object CpuMachinePercent -Maximum).Maximum
}
foreach ($name in 'WorkingSetBytes','PrivateBytes','Handles','Threads','GdiObjects','UserObjects') {
    $stat = $samples | Measure-Object -Property $name -Minimum -Maximum -Average
    $summary[$name] = @{ Min = $stat.Minimum; Max = $stat.Maximum; Mean = $stat.Average; First = $samples[0].$name; Last = $samples[-1].$name }
}
$summary | ConvertTo-Json -Depth 4 | Tee-Object -FilePath (Join-Path $outputDirectory 'summary.json')
Save-ExtraCounters 'after'
Write-Output "Completed. Evidence: $outputDirectory"
