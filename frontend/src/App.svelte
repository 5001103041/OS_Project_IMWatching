<script lang="ts">
  import { currentUser, logout } from './lib/authStore';
  import Login from './routes/Login.svelte';
  import Register from './routes/Register.svelte';
  import Dashboard from './routes/Dashboard.svelte';
  import Recorder from './routes/Recorder.svelte';
  import VideoLibrary from './routes/VideoLibrary.svelte';
  import VideoPlayer from './routes/VideoPlayer.svelte';
  import Settings from './routes/Settings.svelte';

  type Route =
    | { name: 'login' }
    | { name: 'register' }
    | { name: 'dashboard' }
    | { name: 'recorder' }
    | { name: 'library' }
    | { name: 'settings' }
    | { name: 'player'; videoId: string };

  let route: Route = $currentUser ? { name: 'library' } : { name: 'login' };

  $: isAuthed = $currentUser != null;

  function navigate(r: Route) {
    route = r;
  }

  function handleLogout() {
    logout();
    route = { name: 'login' };
  }
</script>

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
    <header class="topbar">
      <div class="brand" on:click={() => navigate({ name: 'library' })}>
        <span class="logo">🧿</span>IMWatching
      </div>
      <nav>
        <button
          class:active={route.name === 'library'}
          on:click={() => navigate({ name: 'library' })}
        >
          Thư viện video
        </button>
        <button
          class:active={route.name === 'recorder'}
          on:click={() => navigate({ name: 'recorder' })}
        >
          🎥 Quay video
        </button>
        <button
          class:active={route.name === 'dashboard'}
          on:click={() => navigate({ name: 'dashboard' })}
        >
          Thống kê
        </button>
        <button
          class:active={route.name === 'settings'}
          on:click={() => navigate({ name: 'settings' })}
        >
          Cài đặt
        </button>
      </nav>
      <div class="userbox">
        <span class="user-email">{$currentUser?.email}</span>
        <button class="ghost" on:click={handleLogout}>Đăng xuất</button>
      </div>
    </header>

    <div class="content">
      {#if route.name === 'library'}
        <VideoLibrary onOpen={(videoId) => navigate({ name: 'player', videoId })} />
      {:else if route.name === 'recorder'}
        <Recorder onUploaded={(videoId) => navigate({ name: 'player', videoId })} />
      {:else if route.name === 'dashboard'}
        <Dashboard />
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
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 12px 20px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    font-weight: 700;
    font-size: 18px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--primary);
  }

  nav {
    display: flex;
    gap: 8px;
  }

  nav button {
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-dim);
    font-weight: 600;
    padding: 8px 14px;
    border-radius: var(--radius);
    transition: all 0.15s ease;
  }

  nav button:hover {
    color: var(--text);
    background: rgba(255, 255, 255, 0.05);
  }

  nav button.active {
    color: var(--primary);
    background: var(--surface-2);
    border-color: var(--border);
    font-weight: 700;
  }

  .userbox {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .user-email {
    color: var(--text-dim);
    font-size: 13px;
  }

  button.ghost {
    background: transparent;
    border-color: var(--border);
  }

  .content {
    flex: 1;
    overflow: auto;
    padding: 20px;
  }
</style>
