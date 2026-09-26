<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../lib/api';

  interface Preset {
    id: string;
    label: string;
  }

  let visionPresets: Preset[] = [];
  let chatPresets: Preset[] = [];
  let visionModel: string | null = null;
  let chatModel: string | null = null;

  // Live model discovery
  let liveGemini: string[] = [];
  let liveCommandCode: string[] = [];
  let discoveryDone = false;
  let discovering = false;

  let loading = true;
  let saving = false;
  let error = '';
  let okMsg = '';

  onMount(async () => {
    await load();
    discover();
  });

  async function load() {
    loading = true;
    try {
      const res = await api.get<{
        vision_model: string | null;
        chat_model: string | null;
        vision_presets: Preset[];
        chat_presets: Preset[];
      }>('/ai/config');
      visionModel = res.vision_model;
      chatModel = res.chat_model;
      visionPresets = res.vision_presets;
      chatPresets = res.chat_presets;
    } catch (e: any) {
      error = e.message || 'Không tải được cấu hình';
    } finally {
      loading = false;
    }
  }

  async function discover() {
    discovering = true;
    try {
      const res = await api.get<{
        gemini: string[];
        commandcode: string[];
      }>('/ai/models');
      liveGemini = res.gemini ?? [];
      liveCommandCode = res.commandcode ?? [];
    } catch {
      // Discovery is best-effort; presets still work.
    } finally {
      discovering = false;
      discoveryDone = true;
    }
  }

  async function save() {
    saving = true;
    error = '';
    okMsg = '';
    try {
      await api.post('/ai/config', {
        vision_model: visionModel,
        chat_model: chatModel,
      });
      okMsg = 'Đã lưu cấu hình model.';
      setTimeout(() => (okMsg = ''), 3000);
    } catch (e: any) {
      error = e.message || 'Lưu thất bại';
    } finally {
      saving = false;
    }
  }

  /** Presets curated for image analysis (multimodal). */
  function visionOptions(): Preset[] {
    return visionPresets;
  }

  /** Presets curated for text chat. */
  function chatOptions(): Preset[] {
    return chatPresets;
  }

  /** Heuristic: does this live model id look multimodal (image-capable)? */
  function isImageModel(id: string): boolean {
    const s = id.toLowerCase();
    const textOnly = [
      'embedding',
      'tts',
      'transcribe',
      'lyria',
      'veo',
      'robotics',
      'audio',
      'live-translate',
      'codex',
      'kimi-k',
      'glm-5.3-flashx',
    ];
    if (textOnly.some((t) => s.includes(t))) return false;
    const imageish = [
      'flash',
      'vision',
      'omni',
      'vl',
      'gpt-4o',
      'gpt-5',
      'gemini',
      'mimo',
      'qwen',
      'minimax',
      'claude',
      'grok',
      'glm',
    ];
    return imageish.some((t) => s.includes(t));
  }

  /** Filter live models: keep budget models (Qwen, DeepSeek, Ling) to avoid expensive models */
  $: liveFiltered = liveCommandCode.filter((m) => {
    const s = m.toLowerCase();
    return s.includes('qwen') || s.includes('deepseek') || s.includes('ling');
  });
  $: liveFilteredVision = liveFiltered.filter((m) => isImageModel(m));
</script>

<div class="settings">
  <h1>⚙️ Cài đặt AI</h1>
  <p class="sub">
    Chọn model cho từng tác vụ (Hỗ trợ Gemini, Qwen, DeepSeek & Ling giá rẻ, tốc độ cao). Thay đổi có hiệu lực ngay.
  </p>

  {#if error}<div class="error">{error}</div>{/if}
  {#if okMsg}<div class="ok">{okMsg}</div>{/if}

  {#if loading}
    <div class="empty">Đang tải…</div>
  {:else}
    <div class="card group">
      <h2>👁 Model phân tích hình ảnh (Vision)</h2>
      <p class="hint">
        Nhận diện hành vi + đối tượng từ frame video.
        <strong>Bắt buộc là model đa phương thức (nhận ảnh).</strong>
      </p>
      <select bind:value={visionModel}>
        <optgroup label="⭐ Đề xuất (Gemini, Qwen, DeepSeek)">
          {#each visionOptions() as p (p.id)}
            <option value={p.id}>{p.label}</option>
          {/each}
        </optgroup>
        {#if liveGemini.length > 0}
          <optgroup label={`Gemini trực tiếp khả dụng (${liveGemini.length})`}>
            {#each liveGemini as m (m)}
              <option value={m}>{m}</option>
            {/each}
          </optgroup>
        {/if}
        {#if liveFilteredVision.length > 0}
          <optgroup label={`Model mở rộng khả dụng (${liveFilteredVision.length})`}>
            {#each liveFilteredVision as m (m)}
              <option value={m}>{m} 👁</option>
            {/each}
          </optgroup>
        {/if}
      </select>
    </div>

    <div class="card group">
      <h2>💬 Model chatbot (Text)</h2>
      <p class="hint">Xử lý tổng hợp văn bản và trả lời câu hỏi — ưu tiên model siêu rẻ, tốc độ cao.</p>
      <select bind:value={chatModel}>
        <optgroup label="⭐ Đề xuất (Gemini, Qwen, DeepSeek, Ling)">
          {#each chatOptions() as p (p.id)}
            <option value={p.id}>{p.label}</option>
          {/each}
        </optgroup>
        {#if liveGemini.length > 0}
          <optgroup label={`Gemini trực tiếp khả dụng (${liveGemini.length})`}>
            {#each liveGemini as m (m)}
              <option value={m}>{m}</option>
            {/each}
          </optgroup>
        {/if}
        {#if liveFiltered.length > 0}
          <optgroup label={`Model mở rộng khả dụng (${liveFiltered.length})`}>
            {#each liveFiltered as m (m)}
              <option value={m}>{m}</option>
            {/each}
          </optgroup>
        {/if}
      </select>
    </div>

    <div class="actions">
      <button class="primary" on:click={save} disabled={saving}>
        {saving ? 'Đang lưu…' : '💾 Lưu cấu hình'}
      </button>
      <button on:click={discover} disabled={discovering}>
        {discovering ? 'Đang dò…' : '🔄 Dò lại model khả dụng'}
      </button>
    </div>

    {#if discoveryDone && liveGemini.length === 0 && liveCommandCode.length === 0}
      <div class="empty small">
        Không dò được model nào. Kiểm tra lại API key trong <code>backend/.env</code>.
      </div>
    {/if}
  {/if}
</div>

<style>
  .sub {
    color: var(--text-dim);
    margin-top: -8px;
    margin-bottom: 16px;
  }
  .error {
    background: rgba(231, 76, 60, 0.15);
    color: #ff8a7a;
    padding: 10px 14px;
    border-radius: var(--radius);
    margin-bottom: 12px;
  }
  .ok {
    background: rgba(46, 204, 113, 0.15);
    color: #6ff0a2;
    padding: 10px 14px;
    border-radius: var(--radius);
    margin-bottom: 12px;
  }
  .group {
    margin-bottom: 16px;
  }
  .group h2 {
    margin: 0 0 6px;
    font-size: 17px;
  }
  .hint {
    color: var(--text-dim);
    font-size: 13px;
    margin: 0 0 10px;
  }
  select {
    max-width: 520px;
  }
  .actions {
    display: flex;
    gap: 10px;
  }
  .empty.small {
    padding: 16px;
    font-size: 14px;
  }
  code {
    background: var(--surface-2);
    padding: 1px 6px;
    border-radius: 4px;
  }
</style>
