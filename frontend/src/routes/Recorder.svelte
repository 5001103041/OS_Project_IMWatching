<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api';
  import UploadHardwareAdvisor from '../lib/UploadHardwareAdvisor.svelte';

  export let onUploaded: (videoId: string) => void;
  export let onNavigateMonitor: (() => void) | undefined = undefined;

  // Tabs: 'upload' (default) or 'record'
  let activeTab: 'upload' | 'record' = 'upload';

  // Camera Recording States
  let videoElement: HTMLVideoElement;
  let stream: MediaStream | null = null;
  let recorder: MediaRecorder | null = null;
  let chunks: Blob[] = [];
  let recording = false;
  let timerId: number | null = null;

  // Selected or Recorded Video State
  let selectedFile: File | Blob | null = null;
  let selectedFileName: string = '';
  let previewUrl: string | null = null;
  let durationSecs = 0;
  let resolution = '1080p';
  let uploading = false;
  let error = '';
  let isDragging = false;

  let hiddenFileInput: HTMLInputElement;

  onMount(() => {
    if (activeTab === 'record') {
      startPreview();
    }
  });

  onDestroy(() => {
    stopAllTracks();
    if (timerId) clearInterval(timerId);
    if (previewUrl) URL.revokeObjectURL(previewUrl);
  });

  function switchTab(tab: 'upload' | 'record') {
    if (tab === activeTab) return;
    activeTab = tab;
    discard();
    if (tab === 'record') {
      startPreview();
    } else {
      stopAllTracks();
    }
  }

  // Camera handling
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
        'Không thể truy cập máy ảnh. Vui lòng cấp quyền và kiểm tra thiết bị. ' +
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
    discard();
    durationSecs = 0;

    recorder = new MediaRecorder(stream, {
      mimeType: pickMimeType(),
    });

    recorder.ondataavailable = (e) => {
      if (e.data && e.data.size > 0) chunks.push(e.data);
    };

    recorder.onstop = () => {
      const blob = new Blob(chunks, { type: recorder?.mimeType || 'video/webm' });
      selectedFile = blob;
      selectedFileName = `video_quay_${Date.now()}.${blob.type.includes('mp4') ? 'mp4' : 'webm'}`;
      previewUrl = URL.createObjectURL(blob);
      resolution = '720p (Máy ảnh)';
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

  // File Upload Handling
  function onFileChange(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      handleSelectedFile(input.files[0]);
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragging = false;
    if (e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files[0]) {
      handleSelectedFile(e.dataTransfer.files[0]);
    }
  }

  function handleSelectedFile(file: File) {
    discard();
    selectedFile = file;
    selectedFileName = file.name;
    previewUrl = URL.createObjectURL(file);

    const tempVideo = document.createElement('video');
    tempVideo.preload = 'metadata';
    tempVideo.src = previewUrl;
    tempVideo.onloadedmetadata = () => {
      durationSecs = Math.round(tempVideo.duration) || 30;
      if (tempVideo.videoWidth && tempVideo.videoHeight) {
        resolution = `${tempVideo.videoWidth}x${tempVideo.videoHeight}`;
      } else {
        resolution = '1080p';
      }
    };
  }

  async function upload() {
    if (!selectedFile) return;
    uploading = true;
    error = '';
    try {
      const ext = selectedFileName.endsWith('.mp4') ? 'mp4' : 'webm';
      const fd = new FormData();
      if (selectedFile instanceof File) {
        fd.append('file', selectedFile);
      } else {
        fd.append(
          'file',
          new File([selectedFile], selectedFileName || `video_quay_${Date.now()}.${ext}`, {
            type: selectedFile.type,
          })
        );
      }
      const res = await api.post<{ id: string; status: string }>('/videos', fd);

      try {
        await api.post(`/videos/${res.id}/process`);
      } catch {
        // bỏ qua lỗi nếu kích hoạt ngầm
      }
      onUploaded(res.id);
    } catch (e: any) {
      error = e.message || 'Tải lên thất bại';
    } finally {
      uploading = false;
    }
  }

  function discard() {
    selectedFile = null;
    selectedFileName = '';
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = null;
    durationSecs = 0;
    resolution = '1080p';
    if (hiddenFileInput) hiddenFileInput.value = '';
  }

  function fmt(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  }
</script>

<div class="recorder-container">
  <div class="page-header">
    <div class="title-meta">
      <h1>Thêm Video</h1>
      <p class="sub">
        Đánh giá năng lực phần cứng máy tính và kết nối mạng trước khi tải lên và phân tích.
      </p>
    </div>

    <!-- Mode Tabs -->
    <div class="mode-tabs">
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'upload'}
        on:click={() => switchTab('upload')}
      >
        Chọn tệp từ máy tính
      </button>
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'record'}
        on:click={() => switchTab('record')}
      >
        Quay video từ máy ảnh
      </button>
    </div>
  </div>

  {#if error}<div class="error-msg">{error}</div>{/if}

  <!-- TAB 1: File Upload & Drag & Drop -->
  {#if activeTab === 'upload'}
    {#if !selectedFile}
      <div
        class="dropzone"
        class:dragging={isDragging}
        on:dragover|preventDefault={() => (isDragging = true)}
        on:dragleave|preventDefault={() => (isDragging = false)}
        on:drop={handleDrop}
        on:click={() => hiddenFileInput.click()}
        role="button"
        tabindex="0"
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') hiddenFileInput.click();
        }}
      >
        <input
          type="file"
          accept="video/mp4,video/webm,video/quicktime,video/x-msvideo,video/mkv"
          bind:this={hiddenFileInput}
          on:change={onFileChange}
          style="display: none;"
        />
        <div class="drop-title">Kéo và thả tệp video vào đây, hoặc nhấn để duyệt tệp</div>
        <p class="dz-sub">Định dạng hỗ trợ: MP4, WebM, MKV, AVI</p>
        <button type="button" class="browse-btn">Chọn video từ máy tính</button>
      </div>
    {/if}
  {/if}

  <!-- TAB 2: Camera Recording -->
  {#if activeTab === 'record'}
    <div class="stage">
      <video bind:this={videoElement} autoplay playsinline muted></video>

      {#if recording}
        <div class="rec-indicator">
          <span class="dot"></span> Đang quay: {fmt(durationSecs)}
        </div>
      {/if}
    </div>

    {#if !selectedFile}
      <div class="controls">
        {#if recording}
          <button type="button" class="danger-btn" on:click={stopRecording}>Dừng quay</button>
        {:else}
          <button type="button" class="primary-btn" on:click={startRecording} disabled={!stream}>
            Bắt đầu quay
          </button>
        {/if}
      </div>
    {/if}
  {/if}

  <!-- PREVIEW & HARDWARE ADVISOR SECTION -->
  {#if selectedFile && previewUrl}
    <div class="preview-section">
      <div class="preview-card">
        <h4>Xem trước video đã chọn</h4>
        <div class="video-wrapper">
          <video src={previewUrl} controls class="preview-video">
            <track kind="captions" />
          </video>
        </div>
      </div>

      <!-- Live Hardware Probing & Network Recommendation Chart Component -->
      <UploadHardwareAdvisor
        file={selectedFile}
        fileName={selectedFileName}
        durationSecs={durationSecs}
        resolution={resolution}
        uploading={uploading}
        onUploadConfirmed={upload}
        onCancel={discard}
        onNavigateMonitor={onNavigateMonitor}
      />
    </div>
  {/if}
</div>

<style>
  .recorder-container {
    max-width: 1060px;
    margin: 0 auto;
    padding-bottom: 40px;
  }

  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 16px;
    margin-bottom: 20px;
  }

  .page-header h1 {
    font-size: 1.6rem;
    margin: 0 0 4px 0;
    color: #f1f5f9;
  }

  .sub {
    color: #8b9bb4;
    margin: 0;
    font-size: 0.9rem;
  }

  .mode-tabs {
    display: flex;
    background: #141b27;
    padding: 4px;
    border-radius: 10px;
    border: 1px solid #233149;
  }

  .tab-btn {
    background: transparent;
    border: none;
    color: #8b9bb4;
    padding: 8px 16px;
    border-radius: 8px;
    font-size: 0.88rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .tab-btn:hover {
    color: #e2e8f0;
  }

  .tab-btn.active {
    background: #2563eb;
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(37, 99, 235, 0.4);
  }

  .error-msg {
    background: rgba(231, 76, 60, 0.15);
    color: #ff8a7a;
    padding: 12px 16px;
    border-radius: 8px;
    margin-bottom: 16px;
    border: 1px solid rgba(231, 76, 60, 0.3);
  }

  /* Dropzone */
  .dropzone {
    border: 2px dashed #2a3a55;
    background: #141b27;
    border-radius: 14px;
    padding: 60px 24px;
    text-align: center;
    cursor: pointer;
    transition: all 0.2s ease;
    margin-bottom: 24px;
    outline: none;
  }

  .dropzone:hover,
  .dropzone.dragging {
    border-color: #38bdf8;
    background: rgba(56, 189, 248, 0.05);
    transform: translateY(-2px);
  }

  .drop-title {
    margin: 0 0 8px 0;
    color: #f1f5f9;
    font-size: 1.15rem;
    font-weight: 600;
  }

  .dz-sub {
    color: #64748b;
    font-size: 0.85rem;
    margin: 0 0 18px 0;
  }

  .browse-btn {
    background: #1e293b;
    color: #38bdf8;
    border: 1px solid #334155;
    padding: 9px 20px;
    border-radius: 8px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
  }

  .browse-btn:hover {
    background: #334155;
  }

  /* Camera Stage */
  .stage {
    position: relative;
    background: #000;
    border-radius: 12px;
    overflow: hidden;
    min-height: 380px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 16px;
    border: 1px solid #1e293b;
  }

  video {
    width: 100%;
    max-height: 480px;
    object-fit: contain;
  }

  .rec-indicator {
    position: absolute;
    top: 14px;
    left: 14px;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    padding: 6px 14px;
    border-radius: 999px;
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid rgba(255, 255, 255, 0.2);
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #e74c3c;
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
    margin-bottom: 20px;
  }

  .primary-btn {
    background: #2563eb;
    color: #fff;
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    font-weight: 600;
    cursor: pointer;
  }

  .primary-btn:hover:not(:disabled) {
    background: #1d4ed8;
  }

  .danger-btn {
    background: #dc2626;
    color: #fff;
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    font-weight: 600;
    cursor: pointer;
  }

  /* Preview Section */
  .preview-section {
    margin-top: 20px;
  }

  .preview-card {
    background: #141b27;
    border: 1px solid #233149;
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 20px;
  }

  .preview-card h4 {
    margin: 0 0 10px 0;
    color: #f1f5f9;
  }

  .video-wrapper {
    background: #000;
    border-radius: 8px;
    overflow: hidden;
    max-height: 400px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .preview-video {
    width: 100%;
    max-height: 400px;
    object-fit: contain;
  }
</style>
