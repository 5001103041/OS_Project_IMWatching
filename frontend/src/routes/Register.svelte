<script lang="ts">
  import { register, authLoading } from '../lib/authStore';

  export let onLogin: () => void;
  export let onSuccess: () => void;

  let name = '';
  let email = '';
  let password = '';
  let confirm = '';
  let error = '';

  async function submit() {
    error = '';
    if (!email || !password) {
      error = 'Vui lòng nhập email và mật khẩu.';
      return;
    }
    if (password.length < 6) {
      error = 'Mật khẩu phải có ít nhất 6 ký tự.';
      return;
    }
    if (password !== confirm) {
      error = 'Mật khẩu xác nhận không khớp.';
      return;
    }
    try {
      await register(email, password, name || undefined);
      onSuccess();
    } catch (e: any) {
      error = e.message || 'Đăng ký thất bại';
    }
  }
</script>

<div class="auth-wrap">
  <div class="card auth-card">
    <div class="brand">👁 IMWatching</div>
    <h1>Đăng ký</h1>
    <p class="sub">Tạo tài khoản để bắt đầu phân tích video</p>

    {#if error}<div class="error">{error}</div>{/if}

    <form on:submit|preventDefault={submit}>
      <label>
        Tên hiển thị
        <input type="text" bind:value={name} placeholder="Nguyễn Văn A" />
      </label>
      <label>
        Email
        <input type="email" bind:value={email} placeholder="you@example.com" autocomplete="email" />
      </label>
      <label>
        Mật khẩu
        <input type="password" bind:value={password} placeholder="Ít nhất 6 ký tự" autocomplete="new-password" />
      </label>
      <label>
        Xác nhận mật khẩu
        <input type="password" bind:value={confirm} placeholder="Nhập lại mật khẩu" autocomplete="new-password" />
      </label>
      <button type="submit" class="primary" disabled={$authLoading}>
        {$authLoading ? 'Đang đăng ký…' : 'Đăng ký'}
      </button>
    </form>

    <p class="switch">
      Đã có tài khoản?
      <a href="#" on:click|preventDefault={onLogin}>Đăng nhập</a>
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
