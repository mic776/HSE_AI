import type { Quiz } from '../types'

const API = '/api/v1'

function getCookie(name: string): string | undefined {
  const target = document.cookie
    .split(';')
    .map((v) => v.trim())
    .find((v) => v.startsWith(`${name}=`))
  return target?.split('=').slice(1).join('=')
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const csrf = getCookie('csrf_token')
  const headers = new Headers(init.headers)
  headers.set('Content-Type', 'application/json')
  if (csrf) headers.set('x-csrf-token', csrf)
  const res = await fetch(`${API}${path}`, {
    ...init,
    credentials: 'include',
    headers,
  })
  if (!res.ok) {
    const body = await res.text()
    throw new Error(body || `HTTP ${res.status}`)
  }
  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export const api = {
  register: (login: string, password: string) =>
    request('/auth/register', { method: 'POST', body: JSON.stringify({ login, password }) }),
  login: (login: string, password: string) =>
    request('/auth/login', { method: 'POST', body: JSON.stringify({ login, password }) }),
  logout: () => request('/auth/logout', { method: 'POST' }),
  me: () => request('/auth/me'),
  listQuizzes: () => request('/quizzes'),
  getQuiz: (id: number) => request(`/quizzes/${id}`),
  createQuiz: (quiz: Quiz) => request('/quizzes', { method: 'POST', body: JSON.stringify(quiz) }),
  updateQuiz: (id: number, quiz: Quiz) => request(`/quizzes/${id}`, { method: 'PUT', body: JSON.stringify(quiz) }),
  deleteQuiz: (id: number) => request(`/quizzes/${id}`, { method: 'DELETE' }),
  publishQuiz: (id: number) => request(`/quizzes/${id}/publish`, { method: 'POST' }),
  unpublishQuiz: (id: number) => request(`/quizzes/${id}/unpublish`, { method: 'POST' }),
  cloneQuiz: (id: number) => request(`/quizzes/${id}/clone`, { method: 'POST' }),
  searchLibrary: (q: string) => request(`/library/quizzes?q=${encodeURIComponent(q)}`),
  aiGenerate: (topic: string, grade: string, questionCount: number) =>
    request('/ai/generate-quiz', {
      method: 'POST',
      body: JSON.stringify({ topic, grade, questionCount }),
    }),
  createSession: (quizId: number, gameMode: 'platformer' | 'shooter' | 'classic') =>
    request('/sessions', { method: 'POST', body: JSON.stringify({ quizId, gameMode }) }),
  startSession: (id: number) => request(`/sessions/${id}/start`, { method: 'POST' }),
  endSession: (id: number) => request(`/sessions/${id}/end`, { method: 'POST' }),
  sessionResults: (id: number) => request(`/sessions/${id}/results`),
}
