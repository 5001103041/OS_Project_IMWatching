<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api, mediaUrl, formatTimestamp } from '../lib/api';
  import Chatbot from './Chatbot.svelte';
  import { renderMarkdown, behaviorLabel, genderLabel, colorLabel, clothingTypeLabel } from '../lib/format';

  export let videoId: string;

  interface Subject {
    gender: string;
    clothing_color: string;
    clothing_type?: string;
    glasses?: boolean;
    hat?: boolean;
    bag?: string;
    age_group?: string;
    mask?: boolean;
    hair_color?: string;
    role?: string;
    confidence: number;
  }

  interface Event {
    id: string;
    timestamp_secs: number;
    end_timestamp_secs: number | null;
    behavior_name: string;
    severity: string;
    gender: string;
    clothing_color: string | null;
    confidence: number;
    ai_reason: string | null;
    subjects: Subject[];
    attr_summary?: Record<string, string>;
    frame_image_path: string;
  }

  interface Progress {
    total_frames: number;
    analyzed_frames: number;
    status: string;
    progress_pct: number;
  }

  let videoEl: HTMLVideoElement;
  let video: any = null;
  let events: Event[] = [];
  let filtered: Event[] = []
  let search = '';
  let error = '';
  let loading = true;
  let activeEventId: string | null = null;
  let pollTimer: number | null = null;
  let summary: string | null = null;
  let summaryLoading = false;
  let summaryOpen = false;
  let progress: Progress | null = null;
  let timelineEl: HTMLDivElement;
  let elementDuration = 0;

  function updateElementDuration() {
    if (videoEl && videoEl.duration && isFinite(videoEl.duration) && videoEl.duration > 0) {
      elementDuration = videoEl.duration;
    }
  }

  $: streamUrl = mediaUrl(`/media/videos/${videoId}/stream`);
  $: maxEventTs = events.reduce(
    (max, e) => Math.max(max, e.end_timestamp_secs ?? e.timestamp_secs, e.timestamp_secs),
    0
  );
  $: videoDuration = (video?.duration_secs && video.duration_secs > 0)
    ? video.duration_secs
    : (elementDuration > 0 && isFinite(elementDuration))
      ? elementDuration
      : (maxEventTs > 0 ? maxEventTs + 1.0 : 0);
  $: isProcessing = video && !['completed', 'failed'].includes(video.status);
  $: dangerousCount = events.filter((e) => e.severity === 'dangerous').length;
  $: dangerousRate = events.length > 0 ? Math.round((dangerousCount / events.length) * 100) : 0;

  onMount(async () => {
    await load();
    pollTimer = window.setInterval(async () => {
      if (isProcessing) {
        await load();
        await fetchProgress();
      }
    }, 3000);
    if (isProcessing) await fetchProgress();
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  async function fetchProgress() {
    try {
      progress = await api.get<Progress>(`/videos/${videoId}/progress`);
    } catch { /* ignore */ }
  }

  async function loadSummary() {
    if (summaryLoading) return;
    summaryLoading = true;
    summary = null;
    try {
      const res = await api.get<{ summary: string }>(`/chat/videos/${videoId}/summary`);
      summary = res.summary;
      summaryOpen = true;
    } catch (e: any) {
      summary = 'Không tạo được tóm tắt: ' + (e.message || 'lỗi');
      summaryOpen = true;
    } finally {
      summaryLoading = false;
    }
  }

  async function load() {
    try {
      video = await api.get(`/videos/${videoId}`);
      const res = await api.get<{ events: Event[] }>(`/events?video_id=${videoId}`);
      events = res.events;
      applySearch();
      loading = false;
    } catch (e: any) {
      error = e.message || 'Không tải được video';
      loading = false;
    }
  }

  function seekTo(e: Event) {
    if (videoEl) {
      videoEl.currentTime = e.timestamp_secs;
      videoEl.play();
      activeEventId = e.id;
      setTimeout(() => {
        if (activeEventId === e.id) activeEventId = null;
      }, 2000);
    }
  }

  function seekToTime(ts: number) {
    if (videoEl) {
      videoEl.currentTime = ts;
      videoEl.play();
    }
  }

  function applySearch() {
    const q = search.trim().toLowerCase();
    if (!q) {
      filtered = events;
      return;
    }
    filtered = events.filter((e) =>
      [
        e.behavior_name,
        e.severity,
        e.gender,
        e.clothing_color ?? '',
        e.ai_reason ?? '',
        // Also search in attr_summary values
        ...Object.values(e.attr_summary ?? {}),
      ]
        .join(' ')
        .toLowerCase()
        .includes(q)
    );
  }

  function frameUrl(e: Event): string {
    const match = e.frame_image_path.match(/frame_(\d+)\.jpg$/);
    if (match) return mediaUrl(`/media/frames/${videoId}/${match[1]}`);
    return mediaUrl(`/media/frames/${videoId}/0`);
  }

  function statusLabel(s: string): string {
    const map: Record<string, string> = {
      uploaded: 'Đã tải lên',
      transcoding: 'Đang chuyển đổi',
      extracting: 'Đang cắt frame',
      analyzing: 'Đang phân tích AI',
      completed: 'Hoàn tất',
      failed: 'Lỗi',
    };
    return map[s] ?? s;
  }

  function subjectLabel(s: Subject): string {
    const g = genderLabel(s.gender);
    const c = s.clothing_color ? colorLabel(s.clothing_color) : 'không rõ màu';
    const type_ = s.clothing_type && s.clothing_type !== 'unknown' ? ` (${clothingTypeLabel(s.clothing_type)})` : '';
    return `${g}, áo ${c}${type_}`;
  }

  type TagDef = { key: string; icon: string; label: (v: string) => string };
  const ATTR_TAGS: TagDef[] = [
    { key: 'glasses',       icon: '🕶️', label: v => v === 'yes' ? 'Đeo kính' : '' },
    { key: 'hat',           icon: '🧢', label: v => v === 'yes' ? 'Đội mũ' : '' },
    { key: 'mask',          icon: '😷', label: v => v === 'yes' ? 'Khẩu trang' : '' },
    { key: 'bag',           icon: '🎒', label: v => ({ backpack: 'Ba lô', handbag: 'Túi xách' }[v] ?? v) },
    { key: 'age_group',     icon: '👤', label: v => ({ child: 'Trẻ em', teen: 'Thiếu niên', adult: 'Người lớn', elderly: 'Lớn tuổi' }[v] ?? v) },
    { key: 'clothing_type', icon: '👕', label: v => v !== 'unknown' ? clothingTypeLabel(v) : '' },
    { key: 'hair_color',    icon: '💇', label: v => v !== 'unknown' ? `Tóc ${colorLabel(v)}` : '' },
    { key: 'interaction_type', icon: '🤝', label: v => ({
      physical_contact: 'Tiếp xúc', verbal: 'Nói chuyện',
      gesture: 'Cử chỉ', proximity: 'Gần nhau',
    }[v] ?? '') },
  ];

  function buildAttrTags(e: Event): { icon: string; label: string }[] {
    const summary = e.attr_summary ?? {};
    const tags: { icon: string; label: string }[] = [];
    for (const def of ATTR_TAGS) {
      const val = summary[def.key];
      if (!val) continue;
      const label = def.label(val);
      if (!label) continue;
      tags.push({ icon: def.icon, label });
    }
    return tags;
  }

  function rangeLabel(e: Event): string {
    const start = formatTimestamp(e.timestamp_secs);
    if (e.end_timestamp_secs && e.end_timestamp_secs > e.timestamp_secs) {
      return `${start} – ${formatTimestamp(e.end_timestamp_secs)}`;
    }
    return start;
  }

  // Timeline: compute position of event dots as % of video duration
  function timelinePos(ts: number): number {
    if (videoDuration <= 0) return 0;
    return Math.min(100, Math.max(0, (ts / videoDuration) * 100));
  }

  function handleTimelineClick(evt: MouseEvent) {
    if (!timelineEl || videoDuration <= 0) return;
    const rect = timelineEl.getBoundingClientRect();
    const pct = (evt.clientX - rect.left) / rect.width;
    const ts = pct * videoDuration;
    seekToTime(ts);
  }
</script>

<div class="player-page">
  <div class="head">
    <h1>▶ {video?.final_filename || video?.original_filename || 'Video'}</h1>
    {#if video}
      <span class="status-pill {video.status}">{statusLabel(video.status)}</span>
      {#if video.status === 'completed' && events.length > 0}
        {#if dangerousCount >= 2}
          <span class="badge danger">⚠ Nguy hiểm</span>
        {:else if dangerousCount === 1}
          <span class="badge warning">⚠ Nguy cơ cao</span>
        {:else}
          <span class="badge normal">✓ An toàn</span>
        {/if}
      {/if}
    {/if}
  </div>

  {#if loading}
    <div class="empty">Đang tải…</div>
  {:else if error}
    <div class="empty">{error}</div>
  {:else}
    <!-- Progress bar during analysis -->
    {#if isProcessing && progress}
      <div class="progress-section">
        <div class="progress-label">
          <span>⏳ {statusLabel(video.status)}</span>
          <span>{progress.progress_pct}%</span>
        </div>
        <div class="progress-track">
          <div
            class="progress-fill"
            style="width: {progress.progress_pct}%"
          ></div>
        </div>
      </div>
    {/if}

    <div class="layout">
      <div class="video-pane">
        <video
          bind:this={videoEl}
          controls
          src={streamUrl}
          on:loadedmetadata={updateElementDuration}
          on:durationchange={updateElementDuration}
          on:canplay={updateElementDuration}
          on:timeupdate={updateElementDuration}
        ></video>

        <!-- Visual timeline -->
        {#if videoDuration > 0 && events.length > 0}
          <div
            class="timeline"
            bind:this={timelineEl}
            on:click={handleTimelineClick}
            title="Click để seek"
          >
            <div class="timeline-track">
              {#each events as e (e.id)}
                <div
                  class="timeline-dot {e.severity}"
                  style="left: {timelinePos(e.timestamp_secs)}%"
                  title="{behaviorLabel(e.behavior_name)} ({rangeLabel(e)})"
                  on:click|stopPropagation={() => seekTo(e)}
                ></div>
                {#if e.end_timestamp_secs && e.end_timestamp_secs > e.timestamp_secs}
                  <div
                    class="timeline-bar {e.severity}"
                    style="left: {timelinePos(e.timestamp_secs)}%; width: {timelinePos(e.end_timestamp_secs) - timelinePos(e.timestamp_secs)}%"
                  ></div>
                {/if}
              {/each}
            </div>
          </div>
        {/if}

        <div class="video-meta">
          {video.duration_secs ? `⏱ ${Math.round(video.duration_secs)}s` : ''}
          {video.width ? ` · ${video.width}×${video.height}` : ''}
          {events.length > 0 ? ` · ${events.length} frames` : ''}
        </div>

        <!-- Collapsible summary -->
        <div class="card summary-card">
          <div class="summary-head">
            <h3>Tóm tắt thông tin</h3>
            <div class="summary-actions">
              {#if summary}
                <button class="sm" on:click={() => summaryOpen = !summaryOpen}>
                  {summaryOpen ? '▲ Thu gọn' : '▼ Mở rộng'}
                </button>
              {/if}
              <button class="sm primary" on:click={loadSummary} disabled={summaryLoading}>
                {summaryLoading ? '⏳ Đang tạo…' : summary ? '🔄 Tạo lại' : '✨ Tạo tóm tắt'}
              </button>
            </div>
          </div>
          {#if summary && summaryOpen}
            <div class="summary-text">{@html renderMarkdown(summary)}</div>
          {/if}
        </div>
      </div>

      <div class="events-pane">
        <div class="search">
          <input
            type="text"
            bind:value={search}
            on:input={applySearch}
            placeholder='Tìm: "đánh nhau", "kính", "ba lô"…'
          />
        </div>

        <div class="count">
          {filtered.length} / {events.length} frames
          {#if events.filter(e => e.severity === 'dangerous').length > 0}
            · <span class="danger-count">⚠ {events.filter(e => e.severity === 'dangerous').length} nguy hiểm</span>
          {/if}
        </div>

        <div class="event-list">
          {#if filtered.length === 0}
            <div class="empty">
              {video.status === 'completed'
                ? 'Không có frame khớp.'
                : 'Đang phân tích… frames sẽ xuất hiện tại đây.'}
            </div>
          {:else}
            {#each filtered as e (e.id)}
              <button
                class="event-card"
                class:active={activeEventId === e.id}
                on:click={() => seekTo(e)}
              >
                <img src={frameUrl(e)} alt={e.behavior_name} loading="lazy" />
                <div class="info">
                  <div class="row">
                    <span class="badge {e.severity}">{e.severity === 'dangerous' ? '⚠ Nguy hiểm' : '✓ Bình thường'}</span>
                    <span class="time">{rangeLabel(e)}</span>
                    {#if e.confidence}
                      <span class="conf">{Math.round(e.confidence * 100)}%</span>
                    {/if}
                  </div>
                  <div class="behavior">{behaviorLabel(e.behavior_name)}</div>
                  {#if e.subjects && e.subjects.length > 0}
                    <div class="subjects">
                      {#each e.subjects as s, i (i)}
                        <span class="subject-chip">{subjectLabel(s)}</span>
                      {/each}
                    </div>
                  {:else}
                    <div class="attrs">
                      {genderLabel(e.gender)}
                      {e.clothing_color ? ` · áo ${colorLabel(e.clothing_color)}` : ''}
                    </div>
                  {/if}
                  {#if buildAttrTags(e).length > 0}
                    <div class="attr-tags">
                      {#each buildAttrTags(e) as tag}
                        <span class="attr-tag">{tag.icon} {tag.label}</span>
                      {/each}
                    </div>
                  {/if}
                  {#if e.ai_reason}<div class="reason">{e.ai_reason}</div>{/if}
                </div>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    </div>

    <Chatbot {videoId} />
  {/if}
</div>

<style>
  /* ═══ Page container ═══ */
  .player-page {
    max-width: 100%;
    overflow-y: auto;
    padding-bottom: 50px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .head h1 {
    font-size: 18px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status-pill {
    padding: 3px 12px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    background: var(--surface-2);
    color: var(--text-dim);
    flex-shrink: 0;
  }
  .status-pill.completed { background: rgba(46, 204, 113, 0.18); color: #6ff0a2; }
  .status-pill.analyzing,
  .status-pill.extracting,
  .status-pill.transcoding { background: rgba(243, 156, 18, 0.18); color: #f7c96a; }
  .status-pill.failed { background: rgba(231, 76, 60, 0.18); color: #ff8a7a; }

  /* ═══ Progress bar ═══ */
  .progress-section {
    margin-bottom: 12px;
    padding: 12px 16px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .progress-label {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    color: var(--text-dim);
    margin-bottom: 8px;
  }
  .progress-track {
    height: 6px;
    background: var(--surface-2);
    border-radius: 999px;
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--primary), #f0c040);
    border-radius: 999px;
    transition: width 0.5s ease;
  }

  /* ═══ Layout grid ═══ */
  .layout {
    display: grid;
    grid-template-columns: 1fr 380px;
    gap: 16px;
    align-items: start;
    margin-bottom: 24px;
  }
  @media (max-width: 1000px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }

  /* ═══ Video pane ═══ */
  .video-pane {
    min-width: 0;        /* critical: prevent grid blowout */
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .video-pane video {
    width: 100%;
    border-radius: var(--radius-lg);
    background: #000;
    max-height: 420px;
    object-fit: contain;
  }
  .video-meta {
    color: var(--text-dim);
    font-size: 13px;
  }

  /* ═══ Visual timeline ═══ */
  .timeline {
    position: relative;
    height: 24px;
    cursor: pointer;
    padding: 8px 0;
  }
  .timeline-track {
    position: relative;
    height: 8px;
    background: var(--surface-2);
    border-radius: 999px;
    overflow: visible;
  }
  .timeline-dot {
    position: absolute;
    top: -2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    transform: translateX(-50%);
    z-index: 2;
    transition: transform 0.15s;
    cursor: pointer;
  }
  .timeline-dot:hover {
    transform: translateX(-50%) scale(1.4);
    z-index: 3;
  }
  .timeline-dot.normal {
    background: #2ecc71;
    box-shadow: 0 0 4px rgba(46, 204, 113, 0.5);
  }
  .timeline-dot.dangerous {
    background: #e74c3c;
    box-shadow: 0 0 6px rgba(231, 76, 60, 0.6);
  }
  .timeline-bar {
    position: absolute;
    top: 1px;
    height: 6px;
    border-radius: 999px;
    z-index: 1;
    opacity: 0.35;
  }
  .timeline-bar.normal { background: #2ecc71; }
  .timeline-bar.dangerous { background: #e74c3c; }

  /* ═══ Summary card ═══ */
  .summary-card {
    margin-top: 4px;
  }
  .summary-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }
  .summary-head h3 {
    margin: 0;
    font-size: 14px;
  }
  .summary-actions {
    display: flex;
    gap: 6px;
  }
  button.sm {
    padding: 4px 10px;
    font-size: 12px;
  }
  .summary-text {
    margin-top: 10px;
    font-size: 13.5px;
    line-height: 1.65;
    color: var(--text);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 360px;
    overflow-y: auto;
    padding: 12px 14px;
    background: var(--surface-2);
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  :global(.summary-text .section-title) {
    font-weight: 700;
    color: var(--primary);
    display: inline-block;
    margin-top: 6px;
  }
  :global(.summary-text .bullet) {
    color: var(--primary);
    font-weight: 700;
    margin-right: 4px;
  }
  :global(.summary-text strong) {
    font-weight: 700;
    color: #fff;
  }

  /* ═══ Events pane ═══ */
  .events-pane {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 680px;
    overflow: hidden;
  }
  .count {
    color: var(--text-dim);
    font-size: 13px;
  }
  .danger-count {
    color: #ff8a7a;
    font-weight: 600;
  }
  .event-list {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
    padding-right: 4px;
  }

  /* ═══ Event card ═══ */
  .event-card {
    display: flex;
    justify-content: flex-start;
    width: 100%;
    box-sizing: border-box;
    gap: 10px;
    align-items: flex-start;
    text-align: left;
    padding: 8px;
    background: var(--surface);
    border-color: var(--border);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .event-card:hover {
    border-color: var(--primary);
  }
  .event-card.active {
    border-color: var(--primary);
    box-shadow: 0 0 0 1px var(--primary);
  }
  .event-card:active {
    transform: none;
  }
  .event-card img {
    width: 60px;
    min-width: 60px;
    max-width: 60px;
    height: 40px;
    object-fit: cover;
    border-radius: 6px;
    background: var(--surface-2);
    flex-shrink: 0;
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
    min-width: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .time {
    color: var(--text-dim);
    font-size: 11px;
  }
  .behavior {
    font-weight: 600;
    font-size: 13px;
  }
  .attrs {
    color: var(--text-dim);
    font-size: 12px;
  }
  .conf {
    color: var(--info);
    font-size: 10px;
    font-weight: 600;
  }
  .subjects {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }
  .subject-chip {
    font-size: 10px;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--surface-2);
    color: var(--text-dim);
    border: 1px solid var(--border);
  }
  .reason {
    color: var(--text-dim);
    font-size: 11px;
    font-style: italic;
    line-height: 1.4;
    white-space: normal;
    word-break: break-word;
  }
  .attr-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-top: 1px;
  }
  .attr-tag {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    background: rgba(99, 179, 237, 0.12);
    color: #63b3ed;
    border: 1px solid rgba(99, 179, 237, 0.25);
    white-space: nowrap;
  }
</style>
