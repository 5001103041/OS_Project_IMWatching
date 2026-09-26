<script lang="ts">
  import { api } from '../lib/api';
  import { renderMarkdown } from '../lib/format';

  export let videoId: string;

  interface Msg {
    role: 'user' | 'assistant';
    content: string;
  }

  let open = false;
  let input = '';
  let sending = false;
  let messages: Msg[] = [];

  async function send() {
    const q = input.trim();
    if (!q || sending) return;
    messages = [...messages, { role: 'user', content: q }];
    input = '';
    sending = true;
    try {
      const res = await api.post<{ answer: string }>(`/chat/videos/${videoId}`, {
        question: q,
      });
      messages = [...messages, { role: 'assistant', content: res.answer }];
    } catch (e: any) {
      messages = [
        ...messages,
        { role: 'assistant', content: 'Lỗi: ' + (e.message || 'không trả lời được') },
      ];
    } finally {
      sending = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }
</script>

{#if open}
  <div class="chatbox card">
    <div class="head">
      <span>💬 Trợ lý video</span>
      <button class="close" on:click={() => (open = false)}>✕</button>
    </div>
    <div class="messages">
      {#if messages.length === 0}
        <div class="hint">
          Hỏi về video này, ví dụ: "Có bao nhiêu vụ đánh nhau?", "Có ai mặc áo đỏ không?"
        </div>
      {/if}
      {#each messages as m (m)}
        <div class="msg {m.role}">{@html renderMarkdown(m.content)}</div>
      {/each}
      {#if sending}
        <div class="msg assistant">Đang trả lời…</div>
      {/if}
    </div>
    <div class="input-row">
      <textarea
        bind:value={input}
        rows="1"
        placeholder="Đặt câu hỏi…"
        on:keydown={onKeydown}
      ></textarea>
      <button class="primary" on:click={send} disabled={sending || !input.trim()}>Gửi</button>
    </div>
  </div>
{:else}
  <button class="fab" on:click={() => (open = true)}>💬</button>
{/if}

<style>
  .chatbox {
    position: fixed;
    bottom: 20px;
    right: 20px;
    width: 360px;
    max-width: calc(100vw - 40px);
    height: 440px;
    display: flex;
    flex-direction: column;
    z-index: 100;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.5);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-weight: 600;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
  }
  .close {
    border: none;
    background: transparent;
    color: var(--text-dim);
    font-size: 16px;
    padding: 4px 8px;
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 0;
  }
  .hint {
    color: var(--text-dim);
    font-size: 13px;
    text-align: center;
  }
  .msg {
    padding: 8px 12px;
    border-radius: var(--radius);
    font-size: 14px;
    max-width: 85%;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .msg.user {
    align-self: flex-end;
    background: var(--primary);
    color: #1a1205;
  }
  .msg.assistant {
    align-self: flex-start;
    background: var(--surface-2);
    color: var(--text);
  }
  :global(.msg .section-title) {
    font-weight: 700;
    color: var(--primary);
    display: inline-block;
    margin-top: 4px;
  }
  :global(.msg .bullet) {
    color: var(--primary);
    font-weight: 700;
    margin-right: 2px;
  }
  :global(.msg strong) {
    font-weight: 700;
    color: #fff;
  }
  .input-row {
    display: flex;
    gap: 8px;
    align-items: flex-end;
  }
  textarea {
    resize: none;
    min-height: 40px;
    max-height: 100px;
  }
  .input-row button {
    white-space: nowrap;
  }
  .fab {
    position: fixed;
    bottom: 20px;
    right: 20px;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    font-size: 24px;
    background: var(--primary);
    border-color: var(--primary);
    color: #1a1205;
    z-index: 100;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
</style>
