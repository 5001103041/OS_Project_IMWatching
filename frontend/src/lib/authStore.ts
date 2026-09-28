import { writable } from 'svelte/store';
import { api, setToken, setUser, getUser, type AuthUser } from './api';

export const currentUser = writable<AuthUser | null>(getUser());
export const authLoading = writable<boolean>(false);

export async function login(email: string, password: string): Promise<AuthUser> {
  authLoading.set(true);
  try {
    const res = await api.post<{ token: string; user: AuthUser }>('/auth/login', {
      email,
      password,
    });
    setToken(res.token);
    setUser(res.user);
    currentUser.set(res.user);
    return res.user;
  } finally {
    authLoading.set(false);
  }
}

export async function register(
  email: string,
  password: string,
  name?: string
): Promise<AuthUser> {
  authLoading.set(true);
  try {
    const res = await api.post<{ token: string; user: AuthUser }>('/auth/register', {
      email,
      password,
      name,
    });
    setToken(res.token);
    setUser(res.user);
    currentUser.set(res.user);
    return res.user;
  } finally {
    authLoading.set(false);
  }
}

export function logout() {
  setToken(null);
  setUser(null);
  currentUser.set(null);
}

if (typeof window !== 'undefined') {
  window.addEventListener('imw_unauthorized', () => {
    logout();
  });
}
