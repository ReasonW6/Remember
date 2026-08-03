param(
    [string]$Executable = (Join-Path $PSScriptRoot "..\src-tauri\target\release\remember.exe"),
    [ValidateRange(1, 100)]
    [int]$Runs = 10,
    [ValidateRange(500, 30000)]
    [int]$TimeoutMs = 5000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
if (-not ("RememberStartupNative" -as [type])) {
    Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class RememberStartupNative
{
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect
    {
        public int Left;
        public int Top;
        public int Right;
        public int Bottom;
    }

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetWindowRect(IntPtr window, out Rect rect);
}
"@
}

function Get-StartupShellPixels {
    param([IntPtr]$WindowHandle)

    $rect = [RememberStartupNative+Rect]::new()
    if (![RememberStartupNative]::GetWindowRect($WindowHandle, [ref]$rect)) {
        return $null
    }

    $windowWidth = $rect.Right - $rect.Left
    $windowHeight = $rect.Bottom - $rect.Top
    if ($windowWidth -le 0 -or $windowHeight -le 0) {
        return $null
    }

    $captureWidth = [Math]::Min($windowWidth, [Math]::Max(1, [int][Math]::Round($windowWidth * 0.55)))
    $captureHeight = [Math]::Min($windowHeight, [Math]::Max(1, [int][Math]::Round($windowHeight * 0.28)))
    $bitmap = [System.Drawing.Bitmap]::new($captureWidth, $captureHeight)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(
            $rect.Left,
            $rect.Top,
            0,
            0,
            [System.Drawing.Size]::new($captureWidth, $captureHeight)
        )

        $darkPixels = 0
        $nearWhitePixels = 0
        $sampledPixels = 0
        for ($y = 2; $y -lt $captureHeight; $y += 3) {
            for ($x = 2; $x -lt $captureWidth; $x += 3) {
                $color = $bitmap.GetPixel($x, $y)
                $sampledPixels += 1
                if ($color.R -lt 150 -and $color.G -lt 160 -and $color.B -lt 175) {
                    $darkPixels += 1
                }
                if ($color.R -gt 250 -and $color.G -gt 250 -and $color.B -gt 250) {
                    $nearWhitePixels += 1
                }
            }
        }

        return [pscustomobject]@{
            HasShellContent = $darkPixels -ge 12
            NearWhiteRatio = if ($sampledPixels -eq 0) { 0 } else { $nearWhitePixels / $sampledPixels }
        }
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Find-AutomationElement {
    param(
        [System.Windows.Automation.AutomationElement]$Root,
        [string]$AutomationId
    )

    if ($AutomationId) {
        $idCondition = [System.Windows.Automation.PropertyCondition]::new(
            [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
            $AutomationId
        )
        $element = $Root.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants,
            $idCondition
        )
        if ($null -ne $element) {
            return $element
        }
    }

    return $null
}

function Get-DescendantProcesses {
    param([int]$RootProcessId)

    $allProcesses = @(Get-CimInstance Win32_Process)
    $pending = [System.Collections.Generic.Queue[int]]::new()
    $pending.Enqueue($RootProcessId)
    $processes = [System.Collections.Generic.List[object]]::new()

    while ($pending.Count -gt 0) {
        $parentId = $pending.Dequeue()
        foreach ($process in $allProcesses | Where-Object ParentProcessId -eq $parentId) {
            $processes.Add($process)
            $pending.Enqueue([int]$process.ProcessId)
        }
    }

    return @($processes)
}

function Get-Percentile {
    param(
        [double[]]$Values,
        [double]$Percentile
    )

    $sorted = @($Values | Sort-Object)
    if ($sorted.Count -eq 0) {
        return $null
    }
    $index = [Math]::Max(0, [Math]::Ceiling($Percentile * $sorted.Count) - 1)
    return [Math]::Round([double]$sorted[$index], 1)
}

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$workingDirectory = Split-Path -Parent $resolvedExecutable
$existing = @(Get-CimInstance Win32_Process -Filter "Name = 'remember.exe'" | Where-Object {
    $_.ExecutablePath -eq $resolvedExecutable
})
if ($existing.Count -gt 0) {
    throw "Remember is already running from the measured path: $resolvedExecutable"
}

$results = [System.Collections.Generic.List[object]]::new()

for ($run = 1; $run -le $Runs; $run++) {
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $resolvedExecutable -WorkingDirectory $workingDirectory -PassThru
    $windowMs = $null
    $shellMs = $null
    $interactiveMs = $null
    $windowHandle = [IntPtr]::Zero
    $sawWhiteFrame = $false
    $childProcessIds = @()
    $uiaConfirmed = $false
    $uiaProbeMs = $null
    $nextUiaProbeAtMs = 0

    try {
        while ($stopwatch.ElapsedMilliseconds -lt $TimeoutMs) {
            $process.Refresh()
            if ($process.HasExited) {
                throw "Remember exited before its main window became ready (run $run)."
            }

            $currentWindowHandle = $process.MainWindowHandle
            if (
                $currentWindowHandle -ne [IntPtr]::Zero -and
                $process.MainWindowTitle -eq "Remember" -and
                $currentWindowHandle -ne $windowHandle
            ) {
                $windowHandle = $currentWindowHandle
                if ($null -eq $windowMs) {
                    $windowMs = [double]$stopwatch.Elapsed.TotalMilliseconds
                }
            }

            if ($windowHandle -ne [IntPtr]::Zero) {
                if ($null -eq $shellMs) {
                    $shellPixels = Get-StartupShellPixels -WindowHandle $windowHandle
                    if ($null -ne $shellPixels) {
                        if ($shellPixels.NearWhiteRatio -ge 0.95) {
                            $sawWhiteFrame = $true
                        }
                        if ($shellPixels.HasShellContent) {
                            $shellMs = [double]$stopwatch.Elapsed.TotalMilliseconds
                        }
                    }
                }

                if ($null -ne $shellMs -and $stopwatch.ElapsedMilliseconds -ge $nextUiaProbeAtMs) {
                    $uiaStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
                    try {
                        $root = [System.Windows.Automation.AutomationElement]::FromHandle($windowHandle)
                        $recordButton = Find-AutomationElement -Root $root -AutomationId "compact-record-button"
                        if ($null -ne $recordButton -and $recordButton.Current.IsEnabled) {
                            $uiaConfirmed = $true
                            $interactiveMs = [double]$stopwatch.Elapsed.TotalMilliseconds
                            $uiaProbeMs = [double]$uiaStopwatch.Elapsed.TotalMilliseconds
                            break
                        }
                    } catch [System.Windows.Automation.ElementNotAvailableException] {
                        # The WebView accessibility tree can be replaced while React mounts.
                    }
                    $nextUiaProbeAtMs = $stopwatch.ElapsedMilliseconds + 50
                }
            }

            Start-Sleep -Milliseconds 10
        }

        if ($null -eq $interactiveMs) {
            throw "Remember did not expose an enabled compact record button within ${TimeoutMs}ms (run $run)."
        }

        $descendants = @(Get-DescendantProcesses -RootProcessId $process.Id)
        $childProcessIds = @($descendants | ForEach-Object { [int]$_.ProcessId })
        $rendererCount = @($descendants | Where-Object {
            $_.Name -eq "msedgewebview2.exe" -and $_.CommandLine -match "--type=renderer"
        }).Count
        $treeProcessIds = @($process.Id) + @($descendants | ForEach-Object { [int]$_.ProcessId })
        $workingSetBytes = 0L
        $privateBytes = 0L
        foreach ($processId in $treeProcessIds) {
            $treeProcess = Get-Process -Id $processId -ErrorAction SilentlyContinue
            if ($null -ne $treeProcess) {
                $workingSetBytes += [int64]$treeProcess.WorkingSet64
                $privateBytes += [int64]$treeProcess.PrivateMemorySize64
            }
        }

        $result = [pscustomobject]@{
            Run = $run
            WindowMs = [Math]::Round($windowMs, 1)
            ShellMs = if ($null -eq $shellMs) { $null } else { [Math]::Round($shellMs, 1) }
            InteractiveMs = [Math]::Round($interactiveMs, 1)
            NonInteractiveMs = [Math]::Round($interactiveMs - $windowMs, 1)
            UiaConfirmed = $uiaConfirmed
            UiaProbeMs = [Math]::Round($uiaProbeMs, 1)
            WhiteFrameDetected = $sawWhiteFrame
            RendererCount = $rendererCount
            ProcessCount = $treeProcessIds.Count
            WorkingSetMb = [Math]::Round($workingSetBytes / 1MB, 1)
            PrivateMb = [Math]::Round($privateBytes / 1MB, 1)
        }
        $results.Add($result)
        $result | Format-Table -AutoSize | Out-String | Write-Host
    } finally {
        if (!$process.HasExited) {
            [void]$process.CloseMainWindow()
            if (!$process.WaitForExit($TimeoutMs)) {
                throw "Remember did not exit after WM_CLOSE (PID $($process.Id)); measurement stopped without force-killing it."
            }
        }
        if ($childProcessIds.Count -gt 0) {
            $childExitWait = [System.Diagnostics.Stopwatch]::StartNew()
            do {
                $remainingChildren = @($childProcessIds | Where-Object {
                    $null -ne (Get-Process -Id $_ -ErrorAction SilentlyContinue)
                })
                if ($remainingChildren.Count -eq 0) {
                    break
                }
                Start-Sleep -Milliseconds 20
            } while ($childExitWait.ElapsedMilliseconds -lt $TimeoutMs)

            if ($remainingChildren.Count -gt 0) {
                throw "Remember child processes did not exit after WM_CLOSE: $($remainingChildren -join ', ')."
            }
        }
    }

    Start-Sleep -Milliseconds 250
}

$summary = [pscustomobject]@{
    Runs = $results.Count
    WindowP50Ms = Get-Percentile -Values @($results.WindowMs) -Percentile 0.50
    WindowP95Ms = Get-Percentile -Values @($results.WindowMs) -Percentile 0.95
    ShellP50Ms = Get-Percentile -Values @($results | Where-Object ShellMs -ne $null | ForEach-Object ShellMs) -Percentile 0.50
    ShellP95Ms = Get-Percentile -Values @($results | Where-Object ShellMs -ne $null | ForEach-Object ShellMs) -Percentile 0.95
    InteractiveP50Ms = Get-Percentile -Values @($results.InteractiveMs) -Percentile 0.50
    InteractiveP95Ms = Get-Percentile -Values @($results.InteractiveMs) -Percentile 0.95
    NonInteractiveP50Ms = Get-Percentile -Values @($results.NonInteractiveMs) -Percentile 0.50
    NonInteractiveP95Ms = Get-Percentile -Values @($results.NonInteractiveMs) -Percentile 0.95
    UiaProbeP50Ms = Get-Percentile -Values @($results.UiaProbeMs) -Percentile 0.50
    UiaProbeP95Ms = Get-Percentile -Values @($results.UiaProbeMs) -Percentile 0.95
    UiaConfirmationFailures = @($results | Where-Object { !$_.UiaConfirmed }).Count
    WhiteFrameRuns = @($results | Where-Object WhiteFrameDetected).Count
    MaxRendererCount = ($results.RendererCount | Measure-Object -Maximum).Maximum
    WorkingSetP50Mb = Get-Percentile -Values @($results.WorkingSetMb) -Percentile 0.50
    PrivateP50Mb = Get-Percentile -Values @($results.PrivateMb) -Percentile 0.50
}

"Startup summary" | Write-Host
$summary | Format-List
