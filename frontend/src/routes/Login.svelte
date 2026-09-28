<script lang="ts">
  import { onMount } from 'svelte';
  import { login, authLoading } from '../lib/authStore';

  export let onRegister: () => void;
  export let onSuccess: () => void;

  let email = '';
  let password = '';
  let error = '';
  let infoMessage = '';

  onMount(() => {
    if (sessionStorage.getItem('imw_session_expired')) {
      infoMessage = 'Phiên đăng nhập đã hết hạn. Vui lòng đăng nhập lại để tiếp tục.';
      sessionStorage.removeItem('imw_session_expired');
    }
  });

  async function submit() {
    error = '';
    infoMessage = '';
    if (!email || !password) {
      error = 'Vui lòng nhập email và mật khẩu.';
      return;
    }
    try {
      await login(email, password);
      onSuccess();
    } catch (e: any) {
      error = e.message || 'Đăng nhập thất bại';
    }
  }
</script>

<div class="auth-wrap">
  <div class="card auth-card">
    <div class="brand">👁 IMWatching</div>
    <h1>Đăng nhập</h1>
    <p class="sub">Hệ thống phân tích hành vi từ video bằng AI</p>

    {#if infoMessage}<div class="info-banner">{infoMessage}</div>{/if}
    {#if error}<div class="error">{error}</div>{/if}

    <form on:submit|preventDefault={submit}>
      <label>
        Email
        <input type="email" bind:value={email} placeholder="you@example.com" autocomplete="email" />
      </label>
      <label>
        Mật khẩu
        <input type="password" bind:value={password} placeholder="••••••" autocomplete="current-password" />
      </label>
      <button type="submit" class="primary" disabled={$authLoading}>
        {$authLoading ? 'Đang đăng nhập…' : 'Đăng nhập'}
      </button>
    </form>

    <p class="switch">
      Chưa có tài khoản?
      <a href="#" on:click|preventDefault={onRegister}>Đăng ký</a>
    </p>
  </div>
</div>

<style>
  .auth-wrap {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 20px;
  }
  .auth-card {
    width: 100%;
    max-width: 380px;
  }
  .brand {
    font-size: 20px;
    font-weight: 700;
    color: var(--primary);
    margin-bottom: 8px;
  }
  .sub {
    color: var(--text-dim);
    margin-top: 0;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 16px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-dim);
  }
  .info-banner {
    background: rgba(212, 160, 23, 0.15);
    border: 1px solid rgba(212, 160, 23, 0.35);
    color: #f7c96a;
    padding: 10px 12px;
    border-radius: var(--radius);
    margin-top: 12px;
    font-size: 13px;
    line-height: 1.4;
  }
  .error {
    background: rgba(231, 76, 60, 0.15);
    color: #ff8a7a;
    padding: 10px;
    border-radius: var(--radius);
    margin-top: 12px;
    font-size: 14px;
  }
  .switch {
    margin-top: 16px;
    text-align: center;
    color: var(--text-dim);
    font-size: 14px;
  }
</style>
