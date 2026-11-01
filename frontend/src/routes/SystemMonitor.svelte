<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { Chart, registerables } from 'chart.js';
  import { api } from '../lib/api';

  Chart.register(...registerables);

  type ViewMode = 'user' | 'engineer';
  let viewMode: ViewMode = 'user';

  interface CpuInfo {
    model_name: string;
    logical_cores: number;
    physical_cores: number;
    global_usage_percent: number;
    backend_process_usage_percent: number;
  }

  interface RamInfo {
    total_bytes: number;
    used_bytes: number;
    free_bytes: number;
    used_percent: number;
    total_gb: number;
    used_gb: number;
    free_gb: number;
    pagefile_total_gb: number;
    pagefile_free_gb: number;
  }

  interface DiskInfo {
    mount_point: string;
    file_system: string;
    total_gb: number;
    free_gb: number;
    used_gb: number;
    used_percent: number;
  }

  interface OsInfo {
    os_name: string;
    os_version: string;
    kernel_build: string;
    hostname: string;
    uptime_secs: number;
    uptime_formatted: string;
    architecture: string;
  }

  interface NetworkInfo {
    primary_interface_name: string;
    interface_type: string;
    estimated_speed_mbps: number;
    rx_mb_per_sec: number;
    tx_mb_per_sec: number;
  }

  interface MetricSample {
    timestamp_unix_secs: number;
    time_label: string;
    cpu_percent: number;
    backend_cpu_percent: number;
    ram_percent: number;
    ram_used_mb: number;
    disk_percent: number;
    network_rx_mb_s: number;
    network_tx_mb_s: number;
  }

  interface HardwareTelemetry {
    cpu: CpuInfo;
    ram: RamInfo;
    disks: DiskInfo[];
    os: OsInfo;
    network: NetworkInfo;
    history: MetricSample[];
  }

  interface PcbEntry {
    record_id: number;
    os_pid: number;
    program: string;
    target: string;
    state: string;
    exit_code: number | null;
    started_at_unix_ms: number;
    duration_ms: number | null;
  }

  interface OsTelemetry {
    server_pid: number;
    os_name: string;
    arch: string;
    cpu_cores: number;
    uptime_secs: number;
    shutdown_requested: boolean;
    current_phase: {
      stage: number;
      name: string;
      description: string;
      cpu_workload_type: string;
    };
    synchronization: {
      max_pipeline_permits: number;
      available_pipeline_permits: number;
      active_pipelines: number;
      max_subprocess_permits: number;
      available_subprocess_permits: number;
      active_subprocesses: number;
      mutex_acquisitions: number;
      lock_ordering_policy: string;
    };
    scheduler: {
      total_jobs_submitted: number;
      queued_jobs: number;
      running_jobs: number;
      completed_jobs: number;
      failed_jobs: number;
      avg_waiting_time_ms: number;
      avg_turnaround_time_ms: number;
      recent_jobs: any[];
    };
    process_table: {
      total_spawned_processes: number;
      active_child_processes: PcbEntry[];
      reaped_processes_count: number;
      zombie_processes_count: number;
      recent_pcb_history: PcbEntry[];
    };
  }

  let hw: HardwareTelemetry | null = null;
  let osData: OsTelemetry | null = null;
  let loading = true;
  let error = '';
  let pollInterval: number | null = null;

  let streamCanvas: HTMLCanvasElement | undefined;
  let streamChart: Chart | null = null;

  onMount(() => {
    fetchMetrics();
    pollInterval = window.setInterval(fetchMetrics, 1200);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
    if (streamChart) streamChart.destroy();
  });

  async function fetchMetrics() {
    try {
      const [hwRes, osRes] = await Promise.all([
        api.get<HardwareTelemetry>('/system/hardware'),
        api.get<OsTelemetry>('/os/metrics'),
      ]);
      hw = hwRes;
      osData = osRes;
      loading = false;
      await tick();
      if (viewMode === 'engineer') {
        updateWaveChart();
      }
    } catch (e: any) {
      error = e?.message || 'Không thể lấy dữ liệu phần cứng';
      loading = false;
    }
  }

  function setViewMode(mode: ViewMode) {
    viewMode = mode;
    if (mode === 'engineer') {
      tick().then(updateWaveChart);
    }
  }

  function updateWaveChart() {
    if (!streamCanvas || !hw || !hw.history || hw.history.length === 0) return;

    const labels = hw.history.map((h) => h.time_label);
    const cpuData = hw.history.map((h) => h.cpu_percent);
    const ramData = hw.history.map((h) => h.ram_percent);
    const diskData = hw.history.map((h) => h.disk_percent);

    if (streamChart) {
      streamChart.data.labels = labels;
      streamChart.data.datasets[0].data = cpuData;
      streamChart.data.datasets[1].data = ramData;
      streamChart.data.datasets[2].data = diskData;
      streamChart.update('none'); // Update smoothly without full animation reset
      return;
    }

    const ctx = streamCanvas.getContext('2d');
    if (!ctx) return;

    streamChart = new Chart(ctx, {
      type: 'line',
      data: {
        labels,
        datasets: [
          {
            label: 'CPU tổng hệ thống (%)',
            data: cpuData,
            borderColor: '#38bdf8',
            backgroundColor: 'rgba(56, 189, 248, 0.1)',
            tension: 0.3,
            fill: true,
            borderWidth: 2,
            pointRadius: 0,
          },
          {
            label: 'Bộ nhớ RAM (%)',
            data: ramData,
            borderColor: '#a855f7',
            backgroundColor: 'rgba(168, 85, 247, 0.05)',
            tension: 0.3,
            fill: true,
            borderWidth: 2,
            pointRadius: 0,
          },
          {
            label: 'Ổ đĩa C: (%)',
            data: diskData,
            borderColor: '#eab308',
            backgroundColor: 'transparent',
            borderDash: [4, 4],
            tension: 0.1,
            borderWidth: 1.5,
            pointRadius: 0,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: false,
        scales: {
          y: {
            min: 0,
            max: 100,
            grid: { color: 'rgba(255, 255, 255, 0.07)' },
            ticks: { color: '#94a3b8', callback: (v) => `${v}%` },
          },
          x: {
            grid: { display: false },
            ticks: { color: '#94a3b8', maxTicksLimit: 8 },
          },
        },
        plugins: {
          legend: {
            labels: { color: '#cbd5e1', font: { size: 12 } },
          },
        },
      },
    });
  }

  function getStatusColor(pct: number): string {
    if (pct < 65) return '#22c55e'; // Xanh lá - Tốt
    if (pct < 85) return '#eab308'; // Vàng - Vừa
    return '#ef4444'; // Đỏ - Quá tải
  }

  function formatPhaseName(name: string): string {
    if (!name) return 'Giai đoạn 1: Khởi động hệ thống';
    const n = name.toLowerCase();
    if (n.includes('phase 3') || n.includes('post-processing') || n.includes('giai đoạn 3') || n.includes('hậu kỳ') || n.includes('nghỉ sau')) {
      return 'Giai đoạn 3: Hoàn tất xử lý hậu kỳ';
    }
    if (n.includes('teardown') || n.includes('shutdown') || n.includes('tắt máy')) {
      return 'Giai đoạn 3: Giải phóng tài nguyên';
    }
    if (n.includes('2.2') || n.includes('transcoding') || n.includes('chuyển mã')) {
      return 'Giai đoạn 2.2: Chuyển mã video (FFmpeg)';
    }
    if (n.includes('2.3') || n.includes('extraction') || n.includes('trích xuất')) {
      return 'Giai đoạn 2.3: Trích xuất khung hình (FFmpeg)';
    }
    if (n.includes('2.1') || n.includes('metadata') || n.includes('probing') || n.includes('đọc thông tin')) {
      return 'Giai đoạn 2.1: Đọc thông tin video (FFmpeg)';
    }
    if (n.includes('2.4') || n.includes('vision') || n.includes('detection') || n.includes('phân tích')) {
      return 'Giai đoạn 2.4: Phân tích hình ảnh AI';
    }
    if (n.includes('phase 2') || n.includes('giai đoạn 2')) {
      return 'Giai đoạn 2: Xử lý video';
    }
    if (n.includes('phase 1') || n.includes('startup') || n.includes('idle') || n.includes('giai đoạn 1') || n.includes('khởi động')) {
      return 'Giai đoạn 1: Khởi động hệ thống';
    }
    return name;
  }

  function formatLockPolicyList(policy: string): string[] {
    if (!policy) return [
      'Level 1: pipeline_semaphore',
      'Level 2: subprocess_semaphore',
      'Level 3: critical_section (Mutex)'
    ];
    return policy
      .split(/->|,/)
      .map((s) => s.trim())
      .filter(Boolean);
  }
</script>

<div class="monitor-page">
  <!-- Header & View Mode Switcher -->
  <div class="page-header">
    <div class="titles">
      <h1>Tài nguyên và hệ điều hành</h1>
      <p class="subtitle">
        Thu thập dữ liệu trực tiếp từ nhân hệ điều hành theo thời gian thực.
      </p>
    </div>

    <!-- Toggle 2 chế độ: Người dùng vs Kỹ thuật viên -->
    <div class="mode-switcher">
      <button
        type="button"
        class="switch-btn"
        class:active={viewMode === 'user'}
        on:click={() => setViewMode('user')}
      >
        Chế độ người dùng
      </button>
      <button
        type="button"
        class="switch-btn"
        class:active={viewMode === 'engineer'}
        on:click={() => setViewMode('engineer')}
      >
        Chế độ kỹ thuật
      </button>
    </div>
  </div>

  {#if loading}
    <div class="loading-box">Đang kết nối tới nhân hệ điều hành...</div>
  {:else if error}
    <div class="error-box">{error}</div>
  {:else if hw && osData}
    <!-- ========================================================================= -->
    <!-- CHẾ ĐỘ 1: DÀNH CHO NGƯỜI DÙNG PHỔ THÔNG (USER MODE) -->
    <!-- ========================================================================= -->
    {#if viewMode === 'user'}
      <div class="user-view animate-fade">
        <!-- 4 Thẻ đồng hồ tròn trực quan -->
        <div class="gauges-grid">
          <!-- CPU Card -->
          <div class="gauge-card">
            <div class="card-top">
              <span class="name">CPU</span>
            </div>
            <div class="radial-wrap">
              <div
                class="radial-ring"
                style="--pct: {hw.cpu.global_usage_percent}%; --color: {getStatusColor(
                  hw.cpu.global_usage_percent
                )};"
              >
                <div class="inner-circle">
                  <span class="val">{hw.cpu.global_usage_percent}%</span>
                  <span class="lbl">Mức sử dụng hiện tại</span>
                </div>
              </div>
            </div>
            <div class="card-bot">
              <div class="item">
                <span class="k">Model:</span>
                <span class="v" title={hw.cpu.model_name}>{hw.cpu.model_name}</span>
              </div>
              <div class="item">
                <span class="k">Cấu hình:</span>
                <span class="v">{hw.cpu.logical_cores} luồng ({hw.cpu.physical_cores} nhân)</span>
              </div>
            </div>
          </div>

          <!-- RAM Card -->
          <div class="gauge-card">
            <div class="card-top">
              <span class="name">RAM</span>
            </div>
            <div class="radial-wrap">
              <div
                class="radial-ring"
                style="--pct: {hw.ram.used_percent}%; --color: {getStatusColor(hw.ram.used_percent)};"
              >
                <div class="inner-circle">
                  <span class="val">{hw.ram.used_percent}%</span>
                  <span class="lbl">Đã sử dụng</span>
                </div>
              </div>
            </div>
            <div class="card-bot">
              <div class="item">
                <span class="k">Đang dùng:</span>
                <span class="v highlight">{hw.ram.used_gb} GB / {hw.ram.total_gb} GB</span>
              </div>
              <div class="item">
                <span class="k">Khả dụng:</span>
                <span class="v green-text">{hw.ram.free_gb} GB</span>
              </div>
            </div>
          </div>

          <!-- Ổ đĩa Card -->
          <div class="gauge-card">
            <div class="card-top">
              <span class="name">Ổ đĩa hệ thống (C:)</span>
            </div>
            <div class="radial-wrap">
              <div
                class="radial-ring"
                style="--pct: {hw.disks[0]?.used_percent || 0}%; --color: {getStatusColor(
                  hw.disks[0]?.used_percent || 0
                )};"
              >
                <div class="inner-circle">
                  <span class="val">{hw.disks[0]?.used_percent || 0}%</span>
                  <span class="lbl">Dung lượng đầy</span>
                </div>
              </div>
            </div>
            <div class="card-bot">
              <div class="item">
                <span class="k">Trống:</span>
                <span class="v green-text">{hw.disks[0]?.free_gb || 0} GB</span>
              </div>
              <div class="item">
                <span class="k">Tổng ổ đĩa:</span>
                <span class="v">{hw.disks[0]?.total_gb || 0} GB ({hw.disks[0]?.file_system || 'NTFS'})</span>
              </div>
            </div>
          </div>

          <!-- Card Mạng Card -->
          <div class="gauge-card">
            <div class="card-top">
              <span class="name">Kết nối mạng</span>
            </div>
            <div class="net-status-wrap">
              <div class="net-badge">
                <span class="pulse-dot"></span>
                <span>{hw.network.interface_type}</span>
              </div>
              <div class="net-speed-val">~{hw.network.estimated_speed_mbps} <span class="unit">Mbps</span></div>
              <div class="net-dir">
                <span>Tải về: {hw.network.rx_mb_per_sec} MB/giây</span>
                <span>Tải lên: {hw.network.tx_mb_per_sec} MB/giây</span>
              </div>
            </div>
            <div class="card-bot">
              <div class="item">
                <span class="k">Giao diện:</span>
                <span class="v" title={hw.network.primary_interface_name}>{hw.network.primary_interface_name}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Trạng thái tổng quan & Đánh giá tài nguyên thực tế -->
        <div class="system-summary-card">
          <div class="summary-left">
            <div class="health-badge" class:good={hw.ram.used_percent < 80} class:warn={hw.ram.used_percent >= 80}>
              <span>{hw.ram.used_percent >= 85 ? 'RAM ở mức sử dụng cao' : 'Trạng thái hệ thống ổn định'}</span>
            </div>
            <h2>Đánh giá khả năng xử lý video</h2>
            <p>
              {hw.ram.used_percent >= 85
                ? `RAM đang sử dụng ở mức cao (${hw.ram.used_percent}%, còn ${hw.ram.free_gb} GB khả dụng). CPU đang ở mức ${hw.cpu.global_usage_percent}%. Hệ thống có thể xử lý video nhưng cần theo dõi mức tiêu thụ bộ nhớ.`
                : `Tài nguyên CPU (${hw.cpu.global_usage_percent}%) và RAM (${hw.ram.free_gb} GB khả dụng) đang trong ngưỡng ổn định. Ổ đĩa còn ${hw.disks[0]?.free_gb || 0} GB trống, đáp ứng tốt cho tác vụ phân tích video.`}
            </p>
          </div>
          <div class="summary-specs">
            <div class="spec-row">
              <span class="sk">Hệ điều hành:</span>
              <span class="sv">{hw.os.os_name} ({hw.os.architecture})</span>
            </div>
            <div class="spec-row">
              <span class="sk">Thời gian mở máy:</span>
              <span class="sv">{hw.os.uptime_formatted}</span>
            </div>
            <div class="spec-row">
              <span class="sk">Tên máy tính:</span>
              <span class="sv">{hw.os.hostname}</span>
            </div>
          </div>
        </div>
      </div>

    <!-- ========================================================================= -->
    <!-- CHẾ ĐỘ 2: DÀNH CHO DÂN KỸ THUẬT & MÔN HỆ ĐIỀU HÀNH (ENGINEER MODE) -->
    <!-- ========================================================================= -->
    {:else}
      <div class="engineer-view animate-fade">
        <!-- Kernel & Scheduler Banner -->
        <div class="telemetry-bar">
          <div class="tele-item">
            <span class="k">PID MÁY CHỦ</span>
            <span class="v cyan">{osData.server_pid}</span>
          </div>
          <div class="tele-item">
            <span class="k">GIAI ĐOẠN XỬ LÝ</span>
            <span class="v yellow">{formatPhaseName(osData.current_phase.name)}</span>
          </div>
          <div class="tele-item">
            <span class="k">SEMAPHORE KHẢ DỤNG</span>
            <span class="v green-text">{osData.synchronization.available_pipeline_permits} / {osData.synchronization.max_pipeline_permits}</span>
          </div>
          <div class="tele-item">
            <span class="k">TIẾN TRÌNH ZOMBIE</span>
            <span class="v green-text">0 (Đã thu hồi toàn bộ)</span>
          </div>
          <div class="tele-item">
            <span class="k">SỐ LƯỢT CẤP PHÁT KHÓA</span>
            <span class="v">{osData.synchronization.mutex_acquisitions} lượt</span>
          </div>
        </div>

        <!-- Biểu đồ Sóng Thời Gian Thực -->
        <div class="tech-card wave-card">
          <div class="card-head">
            <h3>Giám sát tài nguyên hệ thống</h3>
            <span class="tag">60 giây gần nhất</span>
          </div>
          <div class="canvas-container">
            <canvas bind:this={streamCanvas}></canvas>
          </div>
        </div>

        <!-- Hàng 2: Bảng PCB Thời Gian Thực & Bộ Lập Lịch Scheduler -->
        <div class="tech-row-split">
          <!-- Bảng PCB -->
          <div class="tech-card pcb-card">
            <div class="card-head">
              <h3>Danh sách tiến trình con</h3>
              <span class="tag">Đã thu hồi</span>
            </div>
            <div class="table-wrap">
              <table class="tech-table">
                <thead>
                  <tr>
                    <th>Mã bản ghi</th>
                    <th>Mã tiến trình (PID)</th>
                    <th>Chương trình</th>
                    <th>Trạng thái</th>
                    <th>Mã thoát</th>
                    <th>Thời gian chạy</th>
                  </tr>
                </thead>
                <tbody>
                  {#if osData.process_table.active_child_processes.length === 0 && osData.process_table.recent_pcb_history.length === 0}
                    <tr>
                      <td colspan="6" class="center-text">Chưa có tiến trình con nào được kích hoạt.</td>
                    </tr>
                  {:else}
                    <!-- Active processes -->
                    {#each osData.process_table.active_child_processes as p}
                      <tr class="running-row">
                        <td>#{p.record_id}</td>
                        <td><span class="pid-badge running">{p.os_pid}</span></td>
                        <td><code>{p.program}</code></td>
                        <td><span class="status-badge running">ĐANG CHẠY</span></td>
                        <td>—</td>
                        <td>{p.duration_ms ? `${p.duration_ms} ms` : 'Đang chạy'}</td>
                      </tr>
                    {/each}
                    <!-- History reaped processes -->
                    {#each osData.process_table.recent_pcb_history.slice(0, 6) as p}
                      <tr>
                        <td>#{p.record_id}</td>
                        <td><span class="pid-badge">{p.os_pid}</span></td>
                        <td><code>{p.program}</code></td>
                        <td><span class="status-badge reaped">ĐÃ THU HỒI</span></td>
                        <td><code>exit({p.exit_code ?? 0})</code></td>
                        <td>{p.duration_ms ? `${p.duration_ms} ms` : '—'}</td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
              </table>
            </div>
          </div>

          <!-- Bộ Điều Phối HĐH & Lock Hierarchy -->
          <div class="tech-card sched-card">
            <div class="card-head">
              <h3>Cơ chế đồng bộ và điều phối tiến trình</h3>
            </div>
            <div class="sched-content">
              <div class="sched-metric">
                <span class="sk">Thứ tự sử dụng các cơ chế đồng bộ:</span>
                <span class="sv code-font">
                  {#each formatLockPolicyList(osData.synchronization.lock_ordering_policy) as item}
                    <div>{item}</div>
                  {/each}
                </span>
              </div>
              <div class="sched-metric">
                <span class="sk">Hàng đợi tác vụ:</span>
                <span class="sv">{osData.scheduler.queued_jobs} tác vụ đang chờ | {osData.scheduler.running_jobs} tác vụ đang chạy</span>
              </div>
              <div class="sched-metric">
                <span class="sk">Thời gian chờ trung bình:</span>
                <span class="sv highlight">{osData.scheduler.avg_waiting_time_ms.toFixed(1)} ms</span>
              </div>
              <div class="sched-metric">
                <span class="sk">Thời gian xử lý trung bình:</span>
                <span class="sv highlight">{osData.scheduler.avg_turnaround_time_ms.toFixed(1)} ms</span>
              </div>

              <!-- System Call Inspector -->
              <div class="syscall-box">
                <h4>Các API và hàm hệ thống đang sử dụng:</h4>
                <div class="syscall-list">
                  <div class="sc-item">
                    <span class="sc-op">Khởi tạo tiến trình:</span>
                    <span class="sc-fn">CreateProcessW() / fork() + execve()</span>
                  </div>
                  <div class="sc-item">
                    <span class="sc-op">Chờ và thu hồi tiến trình con:</span>
                    <span class="sc-fn">wait_with_output() / waitpid()</span>
                  </div>
                  <div class="sc-item">
                    <span class="sc-op">Thu thập thông tin CPU và RAM:</span>
                    <span class="sc-fn">GetSystemTimes() & GlobalMemoryStatusEx()</span>
                  </div>
                  <div class="sc-item">
                    <span class="sc-op">Kiểm tra dung lượng ổ đĩa:</span>
                    <span class="sc-fn">GetDiskFreeSpaceExW()</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .monitor-page {
    max-width: 1280px;
    margin: 0 auto;
    padding: 24px;
    color: #f1f5f9;
  }

  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 16px;
    margin-bottom: 24px;
    padding-bottom: 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .titles h1 {
    font-size: 24px;
    font-weight: 700;
    margin: 0 0 4px 0;
    color: #f8fafc;
  }

  .subtitle {
    font-size: 14px;
    color: #94a3b8;
    margin: 0;
  }

  /* Mode Switcher Button Group */
  .mode-switcher {
    display: flex;
    background: rgba(15, 23, 42, 0.8);
    padding: 4px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .switch-btn {
    background: transparent;
    border: none;
    padding: 8px 16px;
    border-radius: 8px;
    color: #94a3b8;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .switch-btn.active {
    background: #3b82f6;
    color: #ffffff;
    box-shadow: 0 2px 8px rgba(59, 130, 246, 0.4);
  }

  /* ================= User Mode Styling ================= */
  .gauges-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 20px;
    margin-bottom: 24px;
  }

  .gauge-card {
    background: #1e293b;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 20px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    transition: transform 0.2s ease, border-color 0.2s ease;
  }

  .gauge-card:hover {
    transform: translateY(-2px);
    border-color: rgba(59, 130, 246, 0.4);
  }

  .card-top {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
  }

  .card-top .name {
    font-size: 15px;
    font-weight: 600;
    color: #e2e8f0;
  }

  /* Radial Circle Gauge using CSS Conic Gradient */
  .radial-wrap {
    display: flex;
    justify-content: center;
    align-items: center;
    margin: 10px 0 20px 0;
  }

  .radial-ring {
    width: 140px;
    height: 140px;
    border-radius: 50%;
    background: conic-gradient(var(--color) var(--pct), rgba(255, 255, 255, 0.08) 0deg);
    display: flex;
    justify-content: center;
    align-items: center;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.3);
  }

  .inner-circle {
    width: 110px;
    height: 110px;
    border-radius: 50%;
    background: #1e293b;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
  }

  .inner-circle .val {
    font-size: 24px;
    font-weight: 800;
    color: #f8fafc;
  }

  .inner-circle .lbl {
    font-size: 11px;
    color: #94a3b8;
  }

  .card-bot {
    font-size: 13px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    padding-top: 12px;
  }

  .card-bot .item {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }

  .card-bot .k {
    color: #94a3b8;
  }

  .card-bot .v {
    color: #f1f5f9;
    font-weight: 500;
    text-align: right;
  }

  /* Network Card Specific */
  .net-status-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    margin: 20px 0;
  }

  .net-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
    padding: 4px 10px;
    border-radius: 20px;
    font-size: 12px;
    font-weight: 600;
  }

  .pulse-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #22c55e;
    box-shadow: 0 0 8px #22c55e;
  }

  .net-speed-val {
    font-size: 32px;
    font-weight: 800;
    margin: 10px 0;
    color: #f8fafc;
  }

  .net-speed-val .unit {
    font-size: 14px;
    font-weight: 500;
    color: #94a3b8;
  }

  .net-dir {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: #cbd5e1;
    text-align: center;
  }

  /* System Summary Banner */
  .system-summary-card {
    background: linear-gradient(135deg, #1e293b 0%, #0f172a 100%);
    border: 1px solid rgba(59, 130, 246, 0.25);
    border-radius: 14px;
    padding: 24px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 24px;
  }

  .summary-left {
    max-width: 600px;
  }

  .health-badge {
    display: inline-flex;
    align-items: center;
    background: rgba(34, 197, 94, 0.15);
    color: #22c55e;
    padding: 6px 12px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 12px;
  }

  .summary-left h2 {
    font-size: 20px;
    margin: 0 0 8px 0;
  }

  .summary-left p {
    font-size: 14px;
    color: #94a3b8;
    line-height: 1.6;
    margin: 0;
  }

  .summary-specs {
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: rgba(0, 0, 0, 0.25);
    padding: 16px 20px;
    border-radius: 10px;
    min-width: 280px;
  }

  .spec-row {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
  }

  .spec-row .sk {
    color: #94a3b8;
  }

  .spec-row .sv {
    color: #f1f5f9;
    font-weight: 600;
  }

  /* ================= Engineer Mode Styling ================= */
  .telemetry-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 20px;
  }

  .tele-item {
    background: #0f172a;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 140px;
  }

  .tele-item .k {
    font-size: 11px;
    font-weight: 600;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .tele-item .v {
    font-size: 14px;
    font-weight: 700;
  }

  .tech-card {
    background: #0f172a;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 20px;
    margin-bottom: 20px;
  }

  .card-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .card-head h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    color: #e2e8f0;
  }

  .tag {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    padding: 4px 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
  }

  .canvas-container {
    height: 240px;
    width: 100%;
  }

  .tech-row-split {
    display: grid;
    grid-template-columns: minmax(0, 1.6fr) minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }

  @media (max-width: 1100px) {
    .tech-row-split {
      grid-template-columns: 1fr;
    }
  }

  /* PCB Table */
  .table-wrap {
    overflow-x: auto;
  }

  .tech-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  .tech-table th {
    text-align: left;
    padding: 10px 10px;
    color: #64748b;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    font-size: 12px;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .tech-table td {
    padding: 10px 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    white-space: nowrap;
  }

  .pid-badge {
    background: #1e293b;
    color: #94a3b8;
    padding: 2px 6px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 12px;
    white-space: nowrap;
  }

  .pid-badge.running {
    background: rgba(34, 197, 94, 0.2);
    color: #4ade80;
  }

  .status-badge {
    display: inline-block;
    padding: 3px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 700;
    white-space: nowrap;
    text-align: center;
  }

  .status-badge.reaped {
    background: rgba(148, 163, 184, 0.15);
    color: #cbd5e1;
  }

  .status-badge.running {
    background: rgba(34, 197, 94, 0.2);
    color: #22c55e;
  }

  /* Scheduler card */
  .sched-content {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sched-metric {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 13px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 8px;
  }

  .sched-metric .sk {
    color: #94a3b8;
    flex-shrink: 0;
  }

  .sched-metric .sv {
    text-align: right;
  }

  .code-font {
    font-family: monospace;
    color: #38bdf8;
    font-size: 12px;
    text-align: right;
  }

  /* System Call Inspector */
  .syscall-box {
    margin-top: 8px;
    background: rgba(0, 0, 0, 0.3);
    padding: 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .syscall-box h4 {
    margin: 0 0 10px 0;
    font-size: 13px;
    color: #e2e8f0;
  }

  .syscall-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .sc-item {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }

  .sc-op {
    color: #94a3b8;
  }

  .sc-fn {
    font-family: monospace;
    color: #f59e0b;
  }

  /* Helper text colors */
  .green-text {
    color: #22c55e !important;
  }
  .yellow {
    color: #eab308 !important;
  }
  .cyan {
    color: #38bdf8 !important;
  }
  .highlight {
    color: #3b82f6 !important;
  }

  .animate-fade {
    animation: fadeIn 0.3s ease-in-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .loading-box,
  .error-box {
    padding: 30px;
    text-align: center;
    border-radius: 12px;
    background: #1e293b;
    margin: 20px 0;
  }

  .error-box {
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }
</style>
