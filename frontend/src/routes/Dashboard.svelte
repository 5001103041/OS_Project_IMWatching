<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Chart, registerables } from 'chart.js';
  import { api } from '../lib/api';
  import { behaviorLabel } from '../lib/format';

  Chart.register(...registerables);

  interface Stat {
    total: number;
    dangerous: number;
    normal: number;
    dangerous_rate: number;
    total_videos: number;
    dangerous_videos: number;
    dangerous_video_rate: number;
    dangerous_behaviors: { behavior: string; count: number }[];
    by_behavior: { behavior: string; count: number; dangerous: number }[];
    by_video: {
      id: string;
      name: string;
      count: number;
      dangerous: number;
      dangerous_rate: number;
      is_dangerous: boolean;
    }[];
  }

  let stats: Stat = {
    total: 0,
    dangerous: 0,
    normal: 0,
    dangerous_rate: 0,
    total_videos: 0,
    dangerous_videos: 0,
    dangerous_video_rate: 0,
    dangerous_behaviors: [],
    by_behavior: [],
    by_video: [],
  };

  let donutCanvas: HTMLCanvasElement | undefined;
  let dangerBarCanvas: HTMLCanvasElement | undefined;
  let videoDonutCanvas: HTMLCanvasElement | undefined;
  let donutChart: Chart | null = null;
  let dangerBarChart: Chart | null = null;
  let videoDonutChart: Chart | null = null;
  let error = '';
  let loading = true;

  onMount(async () => {
    try {
      stats = await api.get('/stats/overview');
      loading = false;
      await tick();
      renderCharts();
    } catch (e: any) {
      error = e.message || 'Không tải được thống kê';
      loading = false;
    }
  });

  function renderCharts() {
    if (donutChart) donutChart.destroy();
    if (dangerBarChart) dangerBarChart.destroy();
    if (videoDonutChart) videoDonutChart.destroy();

    // 1. Donut: Events danger ratio
    const ctx1 = donutCanvas?.getContext('2d');
    if (ctx1) {
      donutChart = new Chart(ctx1, {
        type: 'doughnut',
        data: {
          labels: ['Nguy hiểm', 'Bình thường'],
          datasets: [
            {
              data: [stats.dangerous, stats.normal],
              backgroundColor: ['#e74c3c', '#2ecc71'],
              borderWidth: 2,
              borderColor: '#1e2430',
              hoverOffset: 4,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          cutout: '66%',
          layout: {
            padding: { top: 4, bottom: 4, left: 4, right: 4 },
          },
          plugins: {
            legend: {
              position: 'bottom',
              align: 'center',
              labels: {
                color: '#cbd5e1',
                padding: 16,
                usePointStyle: true,
                pointStyle: 'circle',
                boxWidth: 10,
                boxHeight: 10,
                font: { size: 12, weight: '500' },
              },
            },
            tooltip: {
              callbacks: {
                label: (ctx) => ` Sự kiện ${ctx.label.toLowerCase()}: ${ctx.raw} (${stats.total > 0 ? Math.round(((Number(ctx.raw) / stats.total) * 100)) : 0}%)`,
              },
            },
          },
        },
      });
    }

    // 2. Donut: Video danger ratio
    const ctxVideo = videoDonutCanvas?.getContext('2d');
    if (ctxVideo) {
      const safeVideos = Math.max(0, stats.total_videos - stats.dangerous_videos);
      videoDonutChart = new Chart(ctxVideo, {
        type: 'doughnut',
        data: {
          labels: ['Có nguy hiểm', 'An toàn'],
          datasets: [
            {
              data: [stats.dangerous_videos, safeVideos],
              backgroundColor: ['#e67e22', '#3498db'],
              borderWidth: 2,
              borderColor: '#1e2430',
              hoverOffset: 4,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          cutout: '66%',
          layout: {
            padding: { top: 4, bottom: 4, left: 4, right: 4 },
          },
          plugins: {
            legend: {
              position: 'bottom',
              align: 'center',
              labels: {
                color: '#cbd5e1',
                padding: 16,
                usePointStyle: true,
                pointStyle: 'circle',
                boxWidth: 10,
                boxHeight: 10,
                font: { size: 12, weight: '500' },
              },
            },
            tooltip: {
              callbacks: {
                label: (ctx) => ` Video ${ctx.label.toLowerCase()}: ${ctx.raw} (${stats.total_videos > 0 ? Math.round(((Number(ctx.raw) / stats.total_videos) * 100)) : 0}%)`,
              },
            },
          },
        },
      });
    }

    // 3. Bar: Dangerous Behaviors (Translated & Consolidated)
    const ctx2 = dangerBarCanvas?.getContext('2d');
    if (ctx2) {
      // Group and sum counts by translated Vietnamese label to avoid duplicates
      const behaviorMap = new Map<string, number>();
      for (const b of stats.dangerous_behaviors) {
        const label = behaviorLabel(b.behavior);
        behaviorMap.set(label, (behaviorMap.get(label) || 0) + b.count);
      }
      const sorted = Array.from(behaviorMap.entries()).sort((a, b) => b[1] - a[1]);
      const labels = sorted.length > 0 ? sorted.map((item) => item[0]) : ['Không có hành vi nguy hiểm'];
      const data = sorted.length > 0 ? sorted.map((item) => item[1]) : [0];

      dangerBarChart = new Chart(ctx2, {
        type: 'bar',
        data: {
          labels,
          datasets: [
            {
              label: 'Số lần phát hiện',
              data,
              backgroundColor: '#e74c3c',
              borderRadius: 4,
              barThickness: 16,
            },
          ],
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          indexAxis: 'y',
          plugins: {
            legend: { display: false },
            tooltip: {
              callbacks: {
                label: (ctx) => ` ${ctx.dataset.label}: ${ctx.raw} lần`,
              },
            },
          },
          scales: {
            x: {
              grid: { color: 'rgba(255, 255, 255, 0.05)' },
              ticks: { color: '#9aa3b2', precision: 0 }
            },
            y: {
              grid: { display: false },
              ticks: { color: '#e6e9ef', font: { size: 12 } }
            },
          },
        },
      });
    }
  }
</script>

<div class="dashboard">
  <h1>Thống kê an ninh & Hành vi nguy hiểm</h1>
  <p class="subtitle">Báo cáo tổng hợp tập trung vào mức độ an toàn, tỷ lệ nguy hiểm và phân bổ sự kiện trên từng video.</p>

  {#if loading}
    <div class="empty">Đang tải dữ liệu thống kê…</div>
  {:else if error}
    <div class="empty">{error}</div>
  {:else}
    <!-- KPI cards -->
    <div class="cards">
      <div class="card kpi">
        <div class="num">{stats.total_videos}</div>
        <div class="label">Tổng số video</div>
      </div>
      <div class="card kpi danger">
        <div class="num">{stats.dangerous_videos} / {stats.total_videos}</div>
        <div class="label">Video có cảnh báo ({stats.dangerous_video_rate}%)</div>
      </div>
      <div class="card kpi">
        <div class="num">{stats.total}</div>
        <div class="label">Tổng sự kiện ghi nhận</div>
      </div>
      <div class="card kpi danger">
        <div class="num">{stats.dangerous}</div>
        <div class="label">Sự kiện nguy hiểm ({stats.dangerous_rate}%)</div>
      </div>
    </div>

    <!-- Charts row -->
    <div class="chart-row">
      <div class="card">
        <h3>Tỷ lệ sự kiện (Nguy hiểm / Bình thường)</h3>
        <p class="chart-sub">Độ nguy cơ: <strong>{stats.dangerous_rate}%</strong> trên tổng sự kiện</p>
        <div class="chart-wrap"><canvas bind:this={donutCanvas}></canvas></div>
      </div>
      <div class="card">
        <h3>Tỷ lệ video (Có nguy hiểm / An toàn)</h3>
        <p class="chart-sub">Độ nguy cơ: <strong>{stats.dangerous_video_rate}%</strong> trên tổng số video</p>
        <div class="chart-wrap"><canvas bind:this={videoDonutCanvas}></canvas></div>
      </div>
      <div class="card">
        <h3>Các hành vi nguy hiểm phát hiện</h3>
        <p class="chart-sub">Phân bố các loại hành vi cảnh báo</p>
        <div class="chart-wrap"><canvas bind:this={dangerBarCanvas}></canvas></div>
      </div>
    </div>

    <!-- Per-video danger details table -->
    <div class="table-card card">
      <h3>🎬 Phân tích mức độ rủi ro theo từng Video</h3>
      {#if stats.by_video.length === 0}
        <div class="empty small">Chưa có video nào được phân tích.</div>
      {:else}
        <table>
          <thead>
            <tr>
              <th>Video</th>
              <th>Tổng sự kiện</th>
              <th>Số sự kiện nguy hiểm</th>
              <th>Tỷ lệ nguy cơ</th>
              <th>Đánh giá trạng thái</th>
            </tr>
          </thead>
          <tbody>
            {#each stats.by_video as v (v.id)}
              <tr class={v.dangerous >= 2 ? 'row-danger' : v.dangerous === 1 ? 'row-warning' : 'row-safe'}>
                <td class="vname" title={v.name}>
                  <span class="vicon">{v.dangerous >= 2 ? '⛔' : v.dangerous === 1 ? '⚠' : '✓'}</span> {v.name}
                </td>
                <td><strong>{v.count}</strong> sự kiện</td>
                <td class={v.dangerous >= 2 ? 'text-danger' : v.dangerous === 1 ? 'text-warn' : 'text-safe'}>
                  {v.dangerous > 0 ? `${v.dangerous} cảnh báo` : '0'}
                </td>
                <td>
                  <div class="rate-bar-wrap">
                    <div class="rate-bar" class:critical={v.dangerous >= 2} style="width: {v.dangerous_rate}%;"></div>
                    <span>{v.dangerous_rate}%</span>
                  </div>
                </td>
                <td>
                  {#if v.dangerous >= 2}
                    <span class="badge danger">⚠ Nguy hiểm</span>
                  {:else if v.dangerous === 1}
                    <span class="badge warning">⚠ Nguy cơ cao</span>
                  {:else}
                    <span class="badge normal">✓ An toàn</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>

<style>
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 16px;
    margin-bottom: 20px;
  }
  .kpi .num {
    font-size: 30px;
    font-weight: 700;
  }
  .kpi .label {
    color: var(--text-dim);
    font-size: 13px;
  }
  .kpi.danger .num { color: #ff8a7a; }
  .kpi.normal .num { color: #6ff0a2; }

  .chart-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 16px;
    margin-bottom: 20px;
    align-items: stretch;
  }
  @media (max-width: 900px) {
    .chart-row { grid-template-columns: 1fr; }
  }
  .chart-row :global(.card) {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .chart-wrap {
    position: relative;
    height: 240px;
    width: 100%;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .subtitle {
    color: var(--text-dim);
    margin-top: -6px;
    margin-bottom: 20px;
    font-size: 14px;
  }
  .chart-sub {
    font-size: 12px;
    color: var(--text-dim);
    margin: -4px 0 12px 0;
    min-height: 18px;
  }
  .table-card {
    margin-top: 20px;
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 14px;
  }
  th, td {
    text-align: left;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  th { color: var(--text-dim); font-weight: 600; }
  .vname {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }
  .vicon {
    margin-right: 4px;
  }
  .rate-bar-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 100px;
  }
  .rate-bar {
    height: 8px;
    background: linear-gradient(90deg, #f39c12, #e74c3c);
    border-radius: 999px;
    max-width: 80px;
    min-width: 4px;
  }
  .rate-bar.critical {
    background: linear-gradient(90deg, #e74c3c, #ff5252);
  }
  .text-danger {
    color: #ff8a7a;
    font-weight: 600;
  }
  .text-warn {
    color: #f7c96a;
    font-weight: 600;
  }
  .text-safe {
    color: #6ff0a2;
  }
  .row-danger {
    background: rgba(231, 76, 60, 0.06);
  }
  .row-warning {
    background: rgba(243, 156, 18, 0.04);
  }
  .empty.small { padding: 16px; font-size: 13px; }
</style>
