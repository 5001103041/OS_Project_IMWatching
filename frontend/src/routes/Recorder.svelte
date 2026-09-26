<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api';

  export let onUploaded: (videoId: string) => void;

  let videoElement: HTMLVideoElement;
  let stream: MediaStream | null = null;
  let recorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];
  let recording = false;
  let error = '';
  let uploading = false;
  let previewUrl: string | null = null;
  let recordedBlob: Blob | null = null;
  let durationSecs = 0;
  let timerId: number | null = null;

  onMount(() => {
    startPreview();
  });

  onDestroy(() => {
    stopAllTracks();
    if (timerId) clearInterval(timerId);
  });

  async function startPreview() {
    error = '';
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { width: 1280, height: 720 },
        audio: false,
      });
      if (videoElement) {
        videoElement.srcObject = stream;
        await videoElement.play();
      }
    } catch (e: any) {
      error =
        'Không thể truy cập camera. Vui lòng cấp quyền và kiểm tra thiết bị. ' +
        (e?.message || '');
    }
  }

  function stopAllTracks() {
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
  }

  function startRecording() {
    if (!stream || recorder?.state === 'recording') return;
    error = '';
    chunks = [];
    recordedBlob = null;
    previewUrl = null;
    durationSecs = 0;

    recorder = new MediaRecorder(stream, {
      mimeType: pickMimeType(),
    });

    recorder.ondataavailable = (e) => {
      if (e.data && e.data.size > 0) chunks.push(e.data);
    };

    recorder.onstop = () => {
      const blob = new Blob(chunks, { type: recorder?.mimeType || 'video/webm' });
      recordedBlob = blob;
      previewUrl = URL.createObjectURL(blob);
      if (timerId) clearInterval(timerId);
      timerId = null;
    };

    recorder.start(1000);
    recording = true;
    timerId = window.setInterval(() => {
      durationSecs += 1;
    }, 1000);
  }

  function pickMimeType(): string {
    const candidates = [
      'video/webm;codecs=vp9',
      'video/webm;codecs=vp8',
      'video/webm',
      'video/mp4',
    ];
    for (const c of candidates) {
      if (MediaRecorder.isTypeSupported(c)) return c;
    }
    return '';
  }

  function stopRecording() {
    recorder?.stop();
    recording = false;
  }

  async function upload() {
    if (!recordedBlob) return;
    uploading = true;
    error = '';
    try {
      const ext = recordedBlob.type.includes('mp4') ? 'mp4' : 'webm';
      const fd = new FormData();
      fd.append(
        'file',
        new File([recordedBlob], `recording_${Date.now()}.${ext}`, {
          type: recordedBlob.type,
        })
      );
      const res = await api.post<{ id: string; status: string }>('/videos', fd);
      // Auto-trigger analysis after upload.
      try {
        await api.post(`/videos/${res.id}/process`);
      } catch {
        // process trigger failure shouldn't block navigation
      }
      onUploaded(res.id);
    } catch (e: any) {
      error = e.message || 'Tải lên thất bại';
    } finally {
      uploading = false;
    }
  }

  function discard() {
    recordedBlob = null;
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = null;
    durationSecs = 0;
  }

  function fmt(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
</script>

<div class="recorder">
  <h1>🎥 Quay video từ camera</h1>
  <p class="sub">Bật webcam, quay video, sau đó tải lên để hệ thống phân tích.</p>

  {#if error}<div class="error">{error}</div>{/if}

  <div class="stage">
    <video bind:this={videoElement} autoplay playsinline muted></video>

    {#if !recording && previewUrl}
      <!-- After recording: show the recorded preview instead of live camera -->
      <video src={previewUrl} controls muted class="preview"></video>
    {/if}

    {#if recording}
      <div class="rec-indicator">
        <span class="dot"></span> Đang quay {fmt(durationSecs)}
      </div>
    {/if}
  </div>

  <div class="controls">
    {#if !previewUrl}
      {#if recording}
        <button class="danger" on:click={stopRecording}>⏹ Dừng quay</button>
      {:else}
        <button class="primary" on:click={startRecording} disabled={!stream}>
          ⏺ Bắt đầu quay
        </button>
      {/if}
    {:else}
      <button class="primary" on:click={upload} disabled={uploading}>
        {uploading ? 'Đang tải lên…' : '⬆ Tải lên & phân tích'}
      </button>
      <button on:click={discard}>🔄 Quay lại</button>
    {/if}
  </div>
</div>

<style>
  .sub {
    color: var(--text-dim);
    margin-top: -8px;
  }
  .error {
    background: rgba(231, 76, 60, 0.15);
    color: #ff8a7a;
    padding: 10px 14px;
    border-radius: var(--radius);
    margin-bottom: 12px;
  }
  .stage {
    position: relative;
    background: #000;
    border-radius: var(--radius-lg);
    overflow: hidden;
    min-height: 360px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 16px 0;
  }
  video {
    width: 100%;
    max-height: 480px;
    object-fit: contain;
  }
  video.preview {
    position: absolute;
    inset: 0;
  }
  .rec-indicator {
    position: absolute;
    top: 12px;
    left: 12px;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    padding: 6px 12px;
    border-radius: 999px;
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--danger);
    animation: pulse 1s infinite;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.3;
    }
  }
  .controls {
    display: flex;
    gap: 12px;
  }
</style>
