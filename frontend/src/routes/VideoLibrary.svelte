<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api';

  export let onOpen: (videoId: string) => void;

  interface Video {
    id: string;
    original_filename: string;
    final_filename: string;
    mime_type: string;
    duration_secs: number | null;
    status: string;
    created_at: string;
    file_size_bytes: number | null;
  }

  let videos: Video[] = [];
  let loading = true;
  let error = '';
  let pollTimer: number | null = null;

  onMount(async () => {
    await load();
    pollTimer = window.setInterval(async () => {
      if (videos.some((v) => v.status !== 'completed' && v.status !== 'failed')) {
        await load();
      }
    }, 3000);
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  async function load() {
    try {
      const res = await api.get<{ videos: Video[] }>('/videos');
      videos = res.videos;
    } catch (e: any) {
      error = e.message || 'Không tải được danh sách video';
    } finally {
      loading = false;
    }
  }

  async function analyze(v: Video) {
    error = '';
    try {
      await api.post(`/videos/${v.id}/process`);
      await load();
    } catch (e: any) {
      error = e.message || 'Không kích hoạt được phân tích';
    }
  }

  async function remove(v: Video) {
    if (!confirm(`Xóa video "${v.final_filename || v.original_filename}"?`)) return;
    error = '';
    try {
      await api.del(`/videos/${v.id}`);
      await load();
    } catch (e: any) {
      error = e.message || 'Không xóa được video';
    }
  }

  async function importLibrary() {
    error = '';
    try {
      const res = await api.post<{ imported: number; message?: string }>(
        '/videos/import-library'
      );
      if (res.imported > 0) {
        await load();
        alert(`Đã nhập ${res.imported} video mẫu vào thư viện.`);
      } else {
        alert(res.message || 'Không có video mới để nhập.');
      }
    } catch (e: any) {
      error = e.message || 'Không nhập được thư viện';
    }
  }

  function fmtSize(bytes: number | null): string {
    if (!bytes) return '—';
    if (bytes > 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024).toFixed(1)} KB`;
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
</script>

<div class="library">
  <div class="head">
    <h1>Thư viện video</h1>
    <span class="count">{videos.length} video</span>
    <button class="import-btn" on:click={importLibrary}>🗃️ Nhập video mẫu</button>
  </div>

  {#if error}<div class="error">{error}</div>{/if}

  {#if loading}
    <div class="empty">Đang tải…</div>
  {:else if videos.length === 0}
    <div class="empty">
      Chưa có video nào. Bấm <strong>🗃️ Nhập video mẫu</strong> hoặc vào mục
      <strong>🎥 Quay video</strong> để quay và tải lên.
    </div>
  {:else}
    <div class="grid">
      {#each videos as v (v.id)}
        <div class="card video-card">
          <button class="thumb" on:click={() => onOpen(v.id)}>🎬</button>
          <div class="meta">
            <div class="name" title={v.final_filename || v.original_filename}>
              {v.final_filename || v.original_filename}
            </div>
            <div class="info">
              <span class="badge {v.status === 'completed' ? 'normal' : ''}">
                {statusLabel(v.status)}
              </span>
              <span>{fmtSize(v.file_size_bytes)}</span>
              <span>{v.duration_secs ? `${Math.round(v.duration_secs)}s` : ''}</span>
            </div>
            <div class="actions">
              <button on:click={() => onOpen(v.id)}>▶ Xem</button>
              {#if v.status === 'uploaded' || v.status === 'failed'}
                <button on:click={() => analyze(v)}>⚡ Phân tích</button>
              {/if}
              <button class="danger-text" on:click={() => remove(v)}>🗑 Xóa</button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 16px;
  }
  .count {
    color: var(--text-dim);
    font-size: 13px;
  }
  .import-btn {
    margin-left: auto;
    background: var(--primary);
    border-color: var(--primary);
    color: #1a1205;
    font-weight: 600;
  }
  .error {
    background: rgba(231, 76, 60, 0.15);
    color: #ff8a7a;
    padding: 10px 14px;
    border-radius: var(--radius);
    margin-bottom: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 16px;
  }
  .video-card {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--surface);
  }
  .thumb {
    height: 120px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 42px;
    background: var(--surface-2);
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
    width: 100%;
  }
  .thumb:hover {
    background: var(--surface-2);
  }
  .meta {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .name {
    font-weight: 600;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .info {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-dim);
    font-size: 12px;
    flex-wrap: wrap;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .actions button {
    font-size: 13px;
    padding: 5px 10px;
  }
  .danger-text {
    background: transparent;
    border-color: rgba(231, 76, 60, 0.4);
    color: #ff8a7a;
  }
  .danger-text:hover {
    border-color: var(--danger);
    background: rgba(231, 76, 60, 0.12);
  }
</style>
