<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { Chart, registerables } from 'chart.js';
  import { api } from './api';

  Chart.register(...registerables);

  export let file: File | Blob | null = null;
  export let fileName: string = 'video.mp4';
  export let durationSecs: number = 30;
  export let resolution: string = '1080p';
  export let uploading: boolean = false;
  export let onUploadConfirmed: () => void;
  export let onCancel: () => void;
  export let onNavigateMonitor: (() => void) | undefined = undefined;

  interface HardwarePillarEvaluation {
    name: string;
    score: number;
    status: string; // 'excellent' | 'adequate' | 'warning'
    current_spec: string;
    minimum_required: string;
    recommended_spec: string;
    advice: string;
  }

  interface UploadHardwareReadiness {
    video_size_mb: number;
    overall_score: number;
    bottleneck_component: string;
    network_eval: HardwarePillarEvaluation;
    cpu_eval: HardwarePillarEvaluation;
    ram_eval: HardwarePillarEvaluation;
    disk_eval: HardwarePillarEvaluation;
    summary_recommendation: string;
  }

  let evaluation: UploadHardwareReadiness | null = null;
  let loading = true;
  let error = '';
  let chartCanvas: HTMLCanvasElement | undefined;
  let chartInstance: Chart | null = null;

  $: if (file) {
    evaluateHardware();
  }

  onMount(() => {
    evaluateHardware();
  });

  onDestroy(() => {
    if (chartInstance) {
      chartInstance.destroy();
      chartInstance = null;
    }
  });

  async function evaluateHardware() {
    if (!file) return;
    loading = true;
    error = '';
    try {
      const res = await api.post<UploadHardwareReadiness>('/system/evaluate-upload', {
        file_size_bytes: file.size,
        duration_secs: durationSecs || 30,
        resolution: resolution || '1080p',
      });
      evaluation = res;
      await tick();
      renderChart();
    } catch (e: any) {
      error = e.message || 'Không thể đánh giá tài nguyên máy tính';
    } finally {
      loading = false;
    }
  }

  function renderChart() {
    if (!chartCanvas || !evaluation) return;
    if (chartInstance) {
      chartInstance.destroy();
    }

    const labels = ['Mạng', 'CPU', 'RAM', 'Ổ đĩa'];
    const scores = [
      evaluation.network_eval.score,
      evaluation.cpu_eval.score,
      evaluation.ram_eval.score,
      evaluation.disk_eval.score,
    ];

    const colors = scores.map((s) => {
      if (s >= 80) return 'rgba(34, 197, 94, 0.85)';
      if (s >= 65) return 'rgba(234, 179, 8, 0.85)';
      return 'rgba(239, 68, 68, 0.85)';
    });

    const borderColors = scores.map((s) => {
      if (s >= 80) return '#22c55e';
      if (s >= 65) return '#eab308';
      return '#ef4444';
    });

    const ctx = chartCanvas.getContext('2d');
    if (!ctx) return;

    chartInstance = new Chart(ctx, {
      type: 'bar',
      data: {
        labels,
        datasets: [
          {
            label: 'Điểm đánh giá',
            data: scores,
            backgroundColor: colors,
            borderColor: borderColors,
            borderWidth: 1.5,
            borderRadius: 4,
            barThickness: 16,
          },
          {
            type: 'line',
            label: 'Ngưỡng an toàn (70)',
            data: [70, 70, 70, 70],
            borderColor: 'rgba(255, 255, 255, 0.35)',
            borderDash: [5, 4],
            borderWidth: 1.5,
            pointRadius: 0,
            fill: false,
          },
        ],
      },
      options: {
        indexAxis: 'y',
        responsive: true,
        maintainAspectRatio: false,
        scales: {
          x: {
            min: 0,
            max: 100,
            grid: { color: 'rgba(255,255,255,0.06)' },
            ticks: {
              color: '#94a3b8',
              callback: (val) => `${val}`,
            },
          },
          y: {
            grid: { display: false },
            ticks: {
              color: '#e2e8f0',
              font: { weight: '600', size: 12 },
            },
          },
        },
        plugins: {
          legend: {
            labels: { color: '#cbd5e1', font: { size: 12 }, boxWidth: 14 },
          },
          tooltip: {
            callbacks: {
              label: (item) => ` ${item.dataset.label}: ${item.parsed.x}/100`,
            },
          },
        },
      },
    });
  }

  function getPillarStatus(pillar: HardwarePillarEvaluation) {
    if (pillar.score >= 80) return { label: 'Đáp ứng tốt', class: 'status-good' };
    if (pillar.score >= 65) return { label: 'Cần theo dõi', class: 'status-ok' };
    return { label: 'Cần cải thiện', class: 'status-warn' };
  }
</script>

<div class="advisor-box">
  <!-- PHẦN 1: Tiêu đề và mô tả ngắn -->
  <div class="advisor-header">
    <div class="title-row">
      <div>
        <h3>Đánh giá tài nguyên máy tính</h3>
        <p class="subtitle">
          Phân tích cấu hình và tài nguyên hệ thống để đánh giá khả năng xử lý video.
        </p>
      </div>
    </div>
    {#if evaluation}
      <div class="overall-badge" class:good={evaluation.overall_score >= 80} class:warn={evaluation.overall_score < 80}>
        <span class="score-num">{evaluation.overall_score}</span>
        <span class="score-label">/ 100 điểm đánh giá</span>
      </div>
    {/if}
  </div>

  {#if loading}
    <div class="loading-state">
      <div class="spinner"></div>
      <p>Đang kiểm tra tài nguyên máy tính từ hệ điều hành...</p>
    </div>
  {:else if error}
    <div class="error-banner">{error}</div>
  {:else if evaluation}
    <!-- Thông tin tệp video gọn gàng -->
    <div class="file-summary">
      <div class="item">
        <span class="lbl">Tệp tải lên:</span>
        <span class="val bold">{fileName}</span>
      </div>
      <div class="item">
        <span class="lbl">Dung lượng:</span>
        <span class="val">{evaluation.video_size_mb} MB</span>
      </div>
      <div class="item">
        <span class="lbl">Độ phân giải:</span>
        <span class="val">{resolution}</span>
      </div>
      <div class="item">
        <span class="lbl">Thời lượng:</span>
        <span class="val">~{durationSecs} giây</span>
      </div>
    </div>

    <!-- Đánh giá tài nguyên hệ thống -->
    <div class="system-eval-banner" class:has-warn={evaluation.overall_score < 75 || evaluation.ram_eval.score < 65}>
      <div class="banner-text">
        <div class="banner-title">
          {evaluation.overall_score >= 80 && evaluation.ram_eval.score >= 65
            ? 'Đánh giá tài nguyên hệ thống: Cấu hình đáp ứng tốt'
            : `Đánh giá tài nguyên hệ thống: ${evaluation.bottleneck_component}`}
        </div>
        <div class="banner-desc">{evaluation.summary_recommendation}</div>
      </div>
    </div>

    <!-- PHẦN 2: Biểu đồ điểm đánh giá tài nguyên -->
    <div class="chart-container">
      <div class="chart-title">Điểm đánh giá tài nguyên</div>
      <div class="canvas-wrapper">
        <canvas bind:this={chartCanvas}></canvas>
      </div>
    </div>

    <!-- PHẦN 3: Khuyến nghị cấu hình (Grid 2x2 gọn gàng, 3 thành phần mỗi mục) -->
    <div class="specs-section">
      <h4 class="specs-title">Khuyến nghị cấu hình</h4>
      <div class="rec-grid">
        <!-- 1. CPU -->
        <div class="rec-card">
          <div class="rec-top">
            <span class="rec-name">CPU</span>
            <span class="rec-status {getPillarStatus(evaluation.cpu_eval).class}">
              {getPillarStatus(evaluation.cpu_eval).label}
            </span>
          </div>
          <div class="rec-spec">
            <span class="spec-val">{evaluation.cpu_eval.current_spec}</span>
          </div>
          <div class="rec-advice">
            {evaluation.cpu_eval.advice}
          </div>
        </div>

        <!-- 2. RAM -->
        <div class="rec-card">
          <div class="rec-top">
            <span class="rec-name">RAM</span>
            <span class="rec-status {getPillarStatus(evaluation.ram_eval).class}">
              {getPillarStatus(evaluation.ram_eval).label}
            </span>
          </div>
          <div class="rec-spec">
            <span class="spec-val">{evaluation.ram_eval.current_spec}</span>
          </div>
          <div class="rec-advice">
            {evaluation.ram_eval.advice}
          </div>
        </div>

        <!-- 3. Mạng -->
        <div class="rec-card">
          <div class="rec-top">
            <span class="rec-name">Mạng</span>
            <span class="rec-status {getPillarStatus(evaluation.network_eval).class}">
              {getPillarStatus(evaluation.network_eval).label}
            </span>
          </div>
          <div class="rec-spec">
            <span class="spec-val">{evaluation.network_eval.current_spec}</span>
          </div>
          <div class="rec-advice">
            {evaluation.network_eval.advice}
          </div>
        </div>

        <!-- 4. Ổ đĩa -->
        <div class="rec-card">
          <div class="rec-top">
            <span class="rec-name">Ổ đĩa</span>
            <span class="rec-status {getPillarStatus(evaluation.disk_eval).class}">
              {getPillarStatus(evaluation.disk_eval).label}
            </span>
          </div>
          <div class="rec-spec">
            <span class="spec-val">{evaluation.disk_eval.current_spec}</span>
          </div>
          <div class="rec-advice">
            {evaluation.disk_eval.advice}
          </div>
        </div>
      </div>
    </div>

    <!-- PHẦN 4: Nút hành động -->
    <div class="advisor-actions">
      <button type="button" class="primary-btn" on:click={onUploadConfirmed} disabled={uploading}>
        {uploading ? 'Đang tải lên máy chủ...' : 'Xác nhận tải lên và phân tích ngay'}
      </button>

      {#if onNavigateMonitor}
        <button type="button" class="monitor-btn" on:click={onNavigateMonitor}>
          Xem tài nguyên và hệ điều hành chi tiết
        </button>
      {/if}

      <button type="button" class="ghost-btn" on:click={onCancel} disabled={uploading}>
        Chọn video khác
      </button>
    </div>
  {/if}
</div>

<style>
  .advisor-box {
    background: #0f172a;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 20px;
    margin: 16px 0;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    color: #e2e8f0;
  }

  .advisor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
    padding-bottom: 14px;
    margin-bottom: 16px;
    flex-wrap: wrap;
    gap: 12px;
  }

  .title-row h3 {
    margin: 0;
    font-size: 1.15rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .subtitle {
    margin: 4px 0 0 0;
    font-size: 0.82rem;
    color: #94a3b8;
  }

  .overall-badge {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 6px 14px;
    border-radius: 8px;
    background: rgba(30, 41, 59, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .overall-badge.good {
    background: rgba(34, 197, 94, 0.12);
    border-color: rgba(34, 197, 94, 0.3);
  }

  .overall-badge.warn {
    background: rgba(234, 179, 8, 0.12);
    border-color: rgba(234, 179, 8, 0.3);
  }

  .score-num {
    font-size: 1.5rem;
    font-weight: 800;
    color: #22c55e;
    line-height: 1;
  }

  .overall-badge.warn .score-num {
    color: #eab308;
  }

  .score-label {
    font-size: 0.72rem;
    color: #94a3b8;
    margin-top: 2px;
  }

  .file-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    background: #1e293b;
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 0.85rem;
    margin-bottom: 14px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .file-summary .item {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .file-summary .lbl {
    color: #94a3b8;
  }

  .file-summary .val {
    color: #f1f5f9;
  }

  .file-summary .val.bold {
    font-weight: 600;
    color: #38bdf8;
  }

  .system-eval-banner {
    background: rgba(30, 41, 59, 0.5);
    border-left: 4px solid #22c55e;
    border-radius: 6px;
    padding: 10px 14px;
    margin-bottom: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .system-eval-banner.has-warn {
    border-left-color: #eab308;
    background: rgba(234, 179, 8, 0.06);
  }

  .banner-title {
    font-size: 0.88rem;
    font-weight: 600;
    color: #f8fafc;
    margin-bottom: 2px;
  }

  .banner-desc {
    font-size: 0.8rem;
    color: #94a3b8;
    line-height: 1.4;
  }

  /* Biểu đồ */
  .chart-container {
    background: #1e293b;
    border-radius: 8px;
    padding: 14px 16px;
    margin-bottom: 16px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .chart-title {
    font-size: 0.88rem;
    font-weight: 600;
    color: #cbd5e1;
    margin-bottom: 10px;
  }

  .canvas-wrapper {
    height: 145px;
    width: 100%;
  }

  /* Khuyến nghị cấu hình (Grid 2x2) */
  .specs-section {
    margin-bottom: 18px;
  }

  .specs-title {
    font-size: 0.95rem;
    font-weight: 600;
    color: #f8fafc;
    margin: 0 0 10px 0;
  }

  .rec-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  @media (max-width: 768px) {
    .rec-grid {
      grid-template-columns: 1fr;
    }
  }

  .rec-card {
    background: #1e293b;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .rec-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .rec-name {
    font-size: 0.92rem;
    font-weight: 700;
    color: #f1f5f9;
  }

  .rec-status {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 4px;
    white-space: nowrap;
  }

  .rec-status.status-good {
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.3);
  }

  .rec-status.status-ok {
    background: rgba(234, 179, 8, 0.15);
    color: #facc15;
    border: 1px solid rgba(234, 179, 8, 0.3);
  }

  .rec-status.status-warn {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .rec-spec {
    display: flex;
    gap: 6px;
    font-size: 0.8rem;
    line-height: 1.3;
  }

  .spec-lbl {
    color: #94a3b8;
    flex-shrink: 0;
  }

  .spec-val {
    color: #cbd5e1;
    font-weight: 500;
    word-break: break-word;
  }

  .rec-advice {
    font-size: 0.8rem;
    color: #94a3b8;
    line-height: 1.4;
    border-top: 1px dashed rgba(255, 255, 255, 0.06);
    padding-top: 6px;
  }

  /* Nút hành động */
  .advisor-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .primary-btn {
    background: #3b82f6;
    color: #ffffff;
    border: none;
    padding: 9px 18px;
    border-radius: 6px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s;
  }

  .primary-btn:hover:not(:disabled) {
    background: #2563eb;
  }

  .monitor-btn {
    background: transparent;
    color: #38bdf8;
    border: 1px solid rgba(56, 189, 248, 0.4);
    padding: 9px 16px;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .monitor-btn:hover {
    background: rgba(56, 189, 248, 0.08);
  }

  .ghost-btn {
    background: transparent;
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 9px 16px;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .ghost-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.05);
    color: #f1f5f9;
  }

  .primary-btn:disabled,
  .ghost-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .loading-state {
    text-align: center;
    padding: 30px;
    color: #94a3b8;
    font-size: 0.88rem;
  }

  .spinner {
    width: 28px;
    height: 28px;
    border: 3px solid rgba(255, 255, 255, 0.1);
    border-top-color: #38bdf8;
    border-radius: 50%;
    margin: 0 auto 10px auto;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .error-banner {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.3);
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 0.85rem;
  }
</style>
