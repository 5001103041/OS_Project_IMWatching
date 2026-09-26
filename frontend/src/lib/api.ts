const API_BASE = '/api';

export interface AuthUser {
  id: string;
  email: string;
  name?: string;
  role: string;
}

function getToken(): string | null {
  return localStorage.getItem('imw_token');
}

export { getToken };

export function setToken(token: string | null) {
  if (token) localStorage.setItem('imw_token', token);
  else localStorage.removeItem('imw_token');
}

export function getUser(): AuthUser | null {
  const raw = localStorage.getItem('imw_user');
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function setUser(user: AuthUser | null) {
  if (user) localStorage.setItem('imw_user', JSON.stringify(user));
  else localStorage.removeItem('imw_user');
}

async function request<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = {
    ...(opts.headers as Record<string, string>),
  };
  const token = getToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await fetch(`${API_BASE}${path}`, { ...opts, headers });
  const text = await res.text();
  let data: any = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = null;
  }

  if (!res.ok) {
    const msg = data?.error || res.statusText || 'Request failed';
    throw new Error(msg);
  }
  return data as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: any) =>
    request<T>(path, {
      method: 'POST',
      headers: body instanceof FormData ? {} : { 'Content-Type': 'application/json' },
      body: body instanceof FormData ? body : JSON.stringify(body ?? {}),
    }),
  del: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
};

/**
 * Build a media URL that carries the JWT as a query param, because
 * <video>/<img> tags cannot send an Authorization header.
 */
export function mediaUrl(path: string): string {
  const token = getToken();
  const sep = path.includes('?') ? '&' : '?';
  return `${API_BASE}${path}${sep}token=${encodeURIComponent(token ?? '')}`;
}

export function formatTimestamp(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}
