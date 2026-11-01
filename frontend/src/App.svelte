<script lang="ts">
  import { currentUser, logout } from './lib/authStore';
  import Login from './routes/Login.svelte';
  import Register from './routes/Register.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Recorder from './routes/Recorder.svelte';
  import VideoLibrary from './routes/VideoLibrary.svelte';
  import VideoPlayer from './routes/VideoPlayer.svelte';
  import Settings from './routes/Settings.svelte';
  import SystemMonitor from './routes/SystemMonitor.svelte';

  type Route =
    | { name: 'login' }
    | { name: 'register' }
    | { name: 'dashboard' }
    | { name: 'recorder' }
    | { name: 'library' }
    | { name: 'monitor' }
    | { name: 'settings' }
    | { name: 'player'; videoId: string };

  let route: Route = $currentUser ? { name: 'library' } : { name: 'login' };
  let isDrawerOpen = false;

  $: isAuthed = $currentUser != null;

  function toggleDrawer() {
    isDrawerOpen = !isDrawerOpen;
  }

  function closeDrawer() {
    isDrawerOpen = false;
  }

  function navigate(r: Route) {
    route = r;
    closeDrawer();
  }

  function handleLogout() {
    closeDrawer();
    logout();
    route = { name: 'login' };
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && isDrawerOpen) {
      closeDrawer();
    }
  }

  function getPageTitle(r: Route): string {
    switch (r.name) {
      case 'library':
        return 'Thư viện video';
      case 'recorder':
        return 'Thêm video';
      case 'dashboard':
        return 'Thống kê';
      case 'monitor':
        return 'Tài nguyên và hệ điều hành';
      case 'settings':
        return 'Cài đặt';
      case 'player':
        return 'Chi tiết video';
      default:
        return '';
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<main>
  {#if !isAuthed}
    {#if route.name === 'register'}
      <Register
        onLogin={() => navigate({ name: 'login' })}
        onSuccess={() => navigate({ name: 'library' })}
      />
    {:else}
      <Login
        onRegister={() => navigate({ name: 'register' })}
        onSuccess={() => navigate({ name: 'library' })}
      />
    {/if}
  {:else}
    <!-- Thanh điều hướng trên cùng kèm nút 3 dấu gạch ngang -->
    <header class="topbar">
      <div class="topbar-left">
        <button
          type="button"
          class="hamburger-btn"
          on:click={toggleDrawer}
          aria-label="Mở trình đơn điều hướng"
        >
          <span class="bar"></span>
          <span class="bar"></span>
          <span class="bar"></span>
        </button>
        <button type="button" class="brand-link" on:click={() => navigate({ name: 'library' })}>
          IMWatching
        </button>
        <span class="current-section-name">{getPageTitle(route)}</span>
      </div>

      <div class="userbox">
        <span class="user-email">{$currentUser?.email}</span>
        <button type="button" class="ghost" on:click={handleLogout}>Đăng xuất</button>
      </div>
    </header>

    <!-- Lớp phủ mờ (Backdrop overlay) -->
    {#if isDrawerOpen}
      <div
        class="drawer-backdrop"
        on:click={closeDrawer}
        role="presentation"
      ></div>
    {/if}

    <!-- Menu trượt ra từ bên trái (Slide-out Drawer Menu) -->
    <aside class="drawer" class:open={isDrawerOpen} aria-label="Trình đơn điều hướng chính">
      <div class="drawer-header">
        <div class="drawer-brand">IMWatching</div>
        <button
          type="button"
          class="drawer-close-btn"
          on:click={closeDrawer}
          aria-label="Đóng trình đơn"
        >
          Đóng
        </button>
      </div>

      <div class="drawer-divider"></div>

      <nav class="drawer-nav">
        <button
          type="button"
          class:active={route.name === 'library'}
          on:click={() => navigate({ name: 'library' })}
        >
          Thư viện video
        </button>
        <button
          type="button"
          class:active={route.name === 'recorder'}
          on:click={() => navigate({ name: 'recorder' })}
        >
          Thêm video
        </button>
        <button
          type="button"
          class:active={route.name === 'dashboard'}
          on:click={() => navigate({ name: 'dashboard' })}
        >
          Thống kê
        </button>
        <button
          type="button"
          class:active={route.name === 'monitor'}
          on:click={() => navigate({ name: 'monitor' })}
        >
          Tài nguyên và hệ điều hành
        </button>
        <button
          type="button"
          class:active={route.name === 'settings'}
          on:click={() => navigate({ name: 'settings' })}
        >
          Cài đặt
        </button>
      </nav>

      <div class="drawer-footer">
        <div class="drawer-user-info">
          <span class="drawer-user-label">Tài khoản</span>
          <span class="drawer-user-email">{$currentUser?.email}</span>
        </div>
        <button type="button" class="drawer-logout-btn" on:click={handleLogout}>
          Đăng xuất
        </button>
      </div>
    </aside>

    <!-- Vùng hiển thị nội dung trang -->
    <div class="content">
      {#if route.name === 'library'}
        <VideoLibrary
          onOpen={(videoId) => navigate({ name: 'player', videoId })}
          onUploadClick={() => navigate({ name: 'recorder' })}
        />
      {:else if route.name === 'recorder'}
        <Recorder
          onUploaded={(videoId) => navigate({ name: 'player', videoId })}
          onNavigateMonitor={() => navigate({ name: 'monitor' })}
        />
      {:else if route.name === 'dashboard'}
        <Dashboard />
      {:else if route.name === 'monitor'}
        <SystemMonitor />
      {:else if route.name === 'settings'}
        <Settings />
      {:else if route.name === 'player'}
        <VideoPlayer videoId={route.videoId} />
      {/if}
    </div>
  {/if}
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100%;
    position: relative;
  }

  /* Topbar */
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    background: #0f172a;
    border-bottom: 1px solid #1e293b;
    z-index: 40;
  }

  .topbar-left {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  /* Hamburger Button (3 horizontal bars) */
  .hamburger-btn {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 5px;
    width: 38px;
    height: 38px;
    background: #1e293b;
    border: 1px solid #334155;
    border-radius: 8px;
    cursor: pointer;
    padding: 0;
    transition: background 0.15s, border-color 0.15s;
  }

  .hamburger-btn:hover {
    background: #334155;
    border-color: #64748b;
  }

  .hamburger-btn .bar {
    display: block;
    width: 20px;
    height: 2px;
    background-color: #f1f5f9;
    border-radius: 2px;
    transition: all 0.2s ease;
  }

  .brand-link {
    font-weight: 800;
    font-size: 19px;
    color: #38bdf8;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 0;
    letter-spacing: 0.5px;
  }

  .brand-link:hover {
    color: #7dd3fc;
  }

  .current-section-name {
    font-size: 14px;
    color: #94a3b8;
    padding-left: 12px;
    border-left: 1px solid #334155;
  }

  .userbox {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .user-email {
    color: #94a3b8;
    font-size: 13px;
  }

  button.ghost {
    background: transparent;
    border: 1px solid #334155;
    color: #cbd5e1;
    padding: 6px 14px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    transition: all 0.15s;
  }

  button.ghost:hover {
    border-color: #64748b;
    color: #ffffff;
    background: rgba(255, 255, 255, 0.05);
  }

  /* Backdrop Overlay */
  .drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(2px);
    z-index: 90;
    animation: fadeIn 0.2s ease;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* Slide-out Drawer */
  .drawer {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    width: 280px;
    max-width: 85vw;
    background: #0f172a;
    border-right: 1px solid #1e293b;
    box-shadow: 4px 0 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
    transform: translateX(-100%);
    transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    display: flex;
    flex-direction: column;
    padding: 20px;
  }

  .drawer.open {
    transform: translateX(0);
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .drawer-brand {
    font-size: 20px;
    font-weight: 800;
    color: #38bdf8;
    letter-spacing: 0.5px;
  }

  .drawer-close-btn {
    background: #1e293b;
    border: 1px solid #334155;
    color: #94a3b8;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .drawer-close-btn:hover {
    color: #ffffff;
    background: #334155;
  }

  .drawer-divider {
    height: 1px;
    background: #1e293b;
    margin-bottom: 16px;
  }

  .drawer-nav {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    overflow-y: auto;
  }

  .drawer-nav button {
    text-align: left;
    padding: 12px 16px;
    border-radius: 8px;
    background: transparent;
    border: 1px solid transparent;
    color: #94a3b8;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .drawer-nav button:hover {
    color: #f1f5f9;
    background: #1e293b;
  }

  .drawer-nav button.active {
    background: #2563eb;
    color: #ffffff;
    border-color: #3b82f6;
    box-shadow: 0 2px 8px rgba(37, 99, 235, 0.35);
  }

  .drawer-footer {
    padding-top: 16px;
    border-top: 1px solid #1e293b;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .drawer-user-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .drawer-user-label {
    font-size: 11px;
    color: #64748b;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .drawer-user-email {
    font-size: 13px;
    color: #cbd5e1;
    word-break: break-all;
  }

  .drawer-logout-btn {
    background: #1e293b;
    border: 1px solid #334155;
    color: #ff8a7a;
    padding: 9px 14px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    text-align: center;
    transition: all 0.15s;
  }

  .drawer-logout-btn:hover {
    background: rgba(231, 76, 60, 0.15);
    border-color: #ef4444;
    color: #ffffff;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }
</style>
