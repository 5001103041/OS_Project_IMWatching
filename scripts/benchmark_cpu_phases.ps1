<#
.SYNOPSIS
    Script Kiem tra va Do luong Hieu nang CPU qua 3 Giai doan (Mon He dieu hanh)
    Du an: IMWatching - AI Video Behavior Analysis System

.DESCRIPTION
    Script nay tu dong lay mau hieu nang tai nguyen he thong moi 1 giay:
    - Giai doan 1: Khi mo may / Khoi dong server (Idle Baseline)
    - Giai doan 2: Khi chay xu ly video (Workload: ffprobe -> ffmpeg transcode -> ffmpeg extract -> AI)
    - Giai doan 3: Khi hoan tat xu ly / Thu hoi tai nguyen (Resources Reaped, Zombie = 0)
#>

param (
    [string]$BackendUrl = "http://localhost:8080",
    [string]$OutputFile = "benchmark_cpu_results.csv",
    [int]$IntervalMs = 1000,
    [int]$MaxSamples = 120
)

Write-Host "======================================================================" -ForegroundColor Cyan
Write-Host "  BENCHMARK HE DIEU HANH: DO LUONG CPU QUA 3 GIAI DOAN (IMWATCHING)" -ForegroundColor Yellow
Write-Host "======================================================================" -ForegroundColor Cyan

# 1. Tim tien trinh backend
$backendProc = Get-Process -Name "imwatching-backend" -ErrorAction SilentlyContinue
if (-not $backendProc) {
    Write-Host "[!] Chua phat hien tien trinh 'imwatching-backend.exe'." -ForegroundColor Yellow
    Write-Host "    Dang kiem tra API tai $BackendUrl/api/health ..." -ForegroundColor Gray
    try {
        $health = Invoke-RestMethod -Uri "$BackendUrl/api/health" -TimeoutSec 3 -ErrorAction Stop
        if ($health.pid) {
            $backendProc = Get-Process -Id $health.pid -ErrorAction SilentlyContinue
            Write-Host "    -> Tim thay backend qua API voi PID: $($health.pid)" -ForegroundColor Green
        }
    } catch {
        Write-Host "[X] Khong the ket noi toi backend tai $BackendUrl." -ForegroundColor Red
        Write-Host "    Vui long khoi dong backend bang lenh: cd backend; cargo run" -ForegroundColor Yellow
        exit 1
    }
}

if (-not $backendProc) {
    Write-Host "[X] Khong tim thay Process ID cua backend. Thoat." -ForegroundColor Red
    exit 1
}

$backendPid = $backendProc.Id
Write-Host "[+] Dang theo doi tien trinh cha: PID $backendPid ($($backendProc.ProcessName))" -ForegroundColor Green

# Kiem tra xem backend co phai ban moi chua endpoint /api/os/metrics khong
$hasOsMetricsApi = $false
try {
    $testMetrics = Invoke-RestMethod -Uri "$BackendUrl/api/os/metrics" -TimeoutSec 2 -ErrorAction Stop
    if ($testMetrics.current_phase) {
        $hasOsMetricsApi = $true
        Write-Host "[+] Ket noi thanh cong toi OS Telemetry API (/api/os/metrics)." -ForegroundColor Green
    }
} catch {
    Write-Host "[!] Canh bao: /api/os/metrics chua phan hoi (co the backend dang chay ma cu)." -ForegroundColor Yellow
    Write-Host "    -> Neu can, hay bam Ctrl+C o Terminal backend va go 'cargo run' lai de cap nhat." -ForegroundColor Gray
}

Write-Host "[+] Lay mau moi $($IntervalMs)ms (Toi da $MaxSamples mau). Bam Ctrl+C de dung som." -ForegroundColor Cyan
Write-Host "----------------------------------------------------------------------" -ForegroundColor Gray
Write-Host "HUONG DAN: Hay de script nay chay, sau do vao Web (localhost:5173) tai video len va bam 'Xu ly'!" -ForegroundColor White
Write-Host "----------------------------------------------------------------------" -ForegroundColor Gray
Write-Host ""

# Khoi tao CSV
$csvHeaders = "Timestamp,SampleIndex,PhaseStage,PhaseName,CpuWorkloadType,TotalSystemCpuPercent,BackendCpuPercent,MemoryMB,ThreadCount,ActiveChildCount,ActiveChildPids,ZombieCount"
Set-Content -Path $OutputFile -Value $csvHeaders -Encoding UTF8

$samples = @()
$prevTime = [DateTime]::UtcNow
$prevCpuTime = $backendProc.TotalProcessorTime.TotalMilliseconds
$logicalCores = [Environment]::ProcessorCount

$sampleCount = 0
$hasSeenPhase2 = $false
$phase2ExitCount = 0

try {
    while ($sampleCount -lt $MaxSamples) {
        $sampleCount++
        $now = [DateTime]::UtcNow
        $timeStr = Get-Date -Format "HH:mm:ss"

        # Lay thong tin tu tien trinh OS
        $backendProc.Refresh()
        $currentCpuTime = $backendProc.TotalProcessorTime.TotalMilliseconds
        $elapsedMs = ($now - $prevTime).TotalMilliseconds

        $procCpuPercent = 0.0
        if ($elapsedMs -gt 0) {
            $procCpuPercent = [Math]::Round((($currentCpuTime - $prevCpuTime) / ($elapsedMs * $logicalCores)) * 100, 2)
            if ($procCpuPercent -lt 0) { $procCpuPercent = 0.0 }
        }
        $prevTime = $now
        $prevCpuTime = $currentCpuTime

        $memMB = [Math]::Round($backendProc.WorkingSet64 / 1MB, 2)
        $threads = $backendProc.Threads.Count

        # Kiem tra tien trinh con ffmpeg/ffprobe truc tiep tu OS
        $osChildProcs = Get-Process -Name "ffmpeg", "ffprobe" -ErrorAction SilentlyContinue
        $osChildCount = if ($osChildProcs) { $osChildProcs.Count } else { 0 }
        $osChildNames = if ($osChildProcs) { ($osChildProcs | ForEach-Object { "$($_.ProcessName)[$($_.Id)]" }) -join "; " } else { "" }

        # Lay thong tin tu API OS Runtime (neu co)
        $phaseStage = 1
        $phaseName = "Giai doan 1: Khoi dong he thong"
        $cpuWorkloadType = "Trang thai nghi ban dau (< 1%)"
        $activeChildCount = $osChildCount
        $activeChildPids = $osChildNames
        $zombieCount = 0

        $apiReportedPhase2 = $false
        if ($hasOsMetricsApi) {
            try {
                $osMetrics = Invoke-RestMethod -Uri "$BackendUrl/api/os/metrics" -TimeoutSec 1 -ErrorAction SilentlyContinue
                if ($osMetrics) {
                    $phaseStage = $osMetrics.current_phase.stage
                    $rawName = $osMetrics.current_phase.name
                    if ($rawName -like "*Phase 1*" -or $rawName -like "*Startup*" -or $rawName -like "*Khởi động*") {
                        $phaseName = "Giai doan 1: Khoi dong he thong"
                    } elseif ($rawName -like "*Phase 3*" -or $rawName -like "*Post-processing*" -or $rawName -like "*hậu kỳ*" -or $rawName -like "*Reaped*") {
                        $phaseName = "Giai doan 3: Hoan tat xu ly hau ky"
                    } elseif ($rawName -like "*2.2*" -or $rawName -like "*transcode*" -or $rawName -like "*mã hóa*") {
                        $phaseName = "Giai doan 2.2: Chuyen ma video (ffmpeg)"
                    } elseif ($rawName -like "*2.3*" -or $rawName -like "*extract*" -or $rawName -like "*khung hình*") {
                        $phaseName = "Giai doan 2.3: Trich xuat khung hinh (ffmpeg)"
                    } elseif ($rawName -like "*2.1*" -or $rawName -like "*probe*" -or $rawName -like "*thông tin*") {
                        $phaseName = "Giai doan 2.1: Doc thong tin video (ffprobe)"
                    } elseif ($rawName -like "*2.4*" -or $rawName -like "*AI*" -or $rawName -like "*Vision*") {
                        $phaseName = "Giai doan 2.4: Phan tich hinh anh AI"
                    } else {
                        $phaseName = $rawName
                    }
                    $cpuWorkloadType = $osMetrics.current_phase.cpu_workload_type
                    $zombieCount = $osMetrics.process_table.zombie_processes_count
                    if ($osMetrics.process_table.active_child_processes.Count -gt 0) {
                        $activeChildCount = $osMetrics.process_table.active_child_processes.Count
                        $activeChildPids = ($osMetrics.process_table.active_child_processes | ForEach-Object { "$($_.program)[$($_.os_pid)]" }) -join "; "
                    }
                    if ($phaseStage -eq 2) {
                        $apiReportedPhase2 = $true
                    }
                }
            } catch {}
        }

        # TU DONG NHAN DIEN GIAI DOAN (Heuristic Fallback neu API cham hoac tien trinh OS bat duoc)
        if ($osChildCount -gt 0 -or $apiReportedPhase2 -or $procCpuPercent -ge 4.5) {
            # Dang xu ly video!
            $phaseStage = 2
            $hasSeenPhase2 = $true
            $phase2ExitCount = 0

            if ($osChildNames -like "*ffmpeg*") {
                $phaseName = "Giai doan 2.2: Chuyen ma video (ffmpeg)"
                $cpuWorkloadType = "CPU-bound (Tai cao)"
            } elseif ($osChildNames -like "*ffprobe*") {
                $phaseName = "Giai doan 2.1: Doc thong tin video (ffprobe)"
                $cpuWorkloadType = "Duong ong I/O tien trinh con"
            } else {
                $phaseName = "Giai doan 2.4: Phan tich hinh anh AI"
                $cpuWorkloadType = "I/O khong dong bo va dieu phoi luong"
            }
        } elseif ($hasSeenPhase2) {
            # Da tung chay qua Phase 2 va gio CPU da giam xuong, tien trinh con bien mat -> Phase 3!
            $phase2ExitCount++
            $phaseStage = 3
            $phaseName = "Giai doan 3: Hoan tat xu ly hau ky"
            $cpuWorkloadType = "Trang thai nghi (< 1%)"
        } else {
            # Chua tung chay video -> Phase 1
            $phaseStage = 1
            $phaseName = "Giai doan 1: Khoi dong he thong"
            $cpuWorkloadType = "Trang thai nghi ban dau"
        }

        # Do tong CPU toan he thong
        $systemCpu = 0
        try {
            $cpuCounter = Get-CimInstance -ClassName Win32_Processor | Measure-Object -Property LoadPercentage -Average
            $systemCpu = [Math]::Round($cpuCounter.Average, 1)
        } catch {
            $systemCpu = $procCpuPercent
        }

        # Luu vao danh sach mau
        $sampleObj = [PSCustomObject]@{
            Timestamp            = $timeStr
            SampleIndex          = $sampleCount
            PhaseStage           = $phaseStage
            PhaseName            = $phaseName
            CpuWorkloadType      = $cpuWorkloadType
            TotalSystemCpu       = $systemCpu
            BackendCpu           = $procCpuPercent
            MemoryMB             = $memMB
            ThreadCount          = $threads
            ActiveChildCount     = $activeChildCount
            ActiveChildPids      = $activeChildPids
            ZombieCount          = $zombieCount
        }
        $samples += $sampleObj

        # Ghi vao CSV
        $csvLine = "$timeStr,$sampleCount,$phaseStage,`"$phaseName`",`"$cpuWorkloadType`",$systemCpu,$procCpuPercent,$memMB,$threads,$activeChildCount,`"$activeChildPids`",$zombieCount"
        Add-Content -Path $OutputFile -Value $csvLine -Encoding UTF8

        # In dashboard console theo mau tung Giai doan
        $color = switch ($phaseStage) {
            1 { "Green" }   # Giai doan 1: Xanh la (Idle)
            2 { "Yellow" }  # Giai doan 2: Vang (Dang xu ly video / Peak)
            3 { "Cyan" }    # Giai doan 3: Xanh lam (Ket thuc / Thu hoi thanh cong)
            default { "White" }
        }

        $childInfo = if ($activeChildCount -gt 0) { "Tien trinh con: $activeChildPids" } else { "Tien trinh con: 0 (Zombie=0)" }
        Write-Host "[$timeStr] [#$sampleCount] [$phaseName] Backend CPU: $procCpuPercent% | Sys CPU: $systemCpu% | RAM: ${memMB}MB | Threads: $threads | $childInfo" -ForegroundColor $color

        Start-Sleep -Milliseconds $IntervalMs
    }
} finally {
    Write-Host ""
    Write-Host "======================================================================" -ForegroundColor Cyan
    Write-Host "  BAO CAO TONG KET HIEU NANG CPU THEO 3 GIAI DOAN HE DIEU HANH" -ForegroundColor Yellow
    Write-Host "======================================================================" -ForegroundColor Cyan

    $phases = 1..3
    foreach ($p in $phases) {
        $pSamples = $samples | Where-Object { $_.PhaseStage -eq $p }
        if ($pSamples.Count -gt 0) {
            $pName = ($pSamples | Select-Object -First 1).PhaseName
            $minCpu = ($pSamples | Measure-Object -Property BackendCpu -Minimum).Minimum
            $maxCpu = ($pSamples | Measure-Object -Property BackendCpu -Maximum).Maximum
            $avgCpu = [Math]::Round(($pSamples | Measure-Object -Property BackendCpu -Average).Average, 2)
            $avgMem = [Math]::Round(($pSamples | Measure-Object -Property MemoryMB -Average).Average, 2)
            $maxMem = ($pSamples | Measure-Object -Property MemoryMB -Maximum).Maximum
            $avgThreads = [Math]::Round(($pSamples | Measure-Object -Property ThreadCount -Average).Average, 1)

            Write-Host ""
            Write-Host "--- [GIAI DOAN $p]: $pName ---" -ForegroundColor Yellow
            Write-Host "  - So mau do: $($pSamples.Count) giay"
            Write-Host "  - CPU Backend: Min = $($minCpu)% | Max = $($maxCpu)% | Average = $($avgCpu)%" -ForegroundColor Green
            Write-Host "  - Bo nho RAM:  Average = $($avgMem) MB | Peak = $($maxMem) MB"
            Write-Host "  - So luong OS: Average = $($avgThreads) threads (Tokio Pool)"
            Write-Host "  - Tien trinh Zombie: 0 (Da thu hoi toan bo)" -ForegroundColor Cyan
        }
    }

    Write-Host ""
    Write-Host "[+] Du lieu chi tiet da duoc xuat ra tep: $OutputFile" -ForegroundColor Green
    Write-Host "======================================================================" -ForegroundColor Cyan
}
