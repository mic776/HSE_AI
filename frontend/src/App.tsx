import { useEffect, useMemo, useState } from 'react'
import type { FormEvent, ReactNode } from 'react'
import { Navigate, Route, Routes, useNavigate, useParams, useSearchParams, Link } from 'react-router-dom'
import { motion } from 'framer-motion'
import { QRCodeSVG } from 'qrcode.react'
import { api } from './lib/api'
import { connectRoom, sendWs } from './lib/ws'
import type { Question, Quiz, WsEnvelope } from './types'
import { GameCanvas } from './components/GameCanvas'
import { QuestionCard } from './components/QuestionCard'

function shell(title: string, body: ReactNode) {
  return (
    <div className="mx-auto min-h-screen w-full max-w-5xl px-4 py-6 md:px-8">
      <motion.header initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} className="mb-6 flex items-center justify-between rounded-2xl bg-white/70 p-4 shadow-sm backdrop-blur">
        <h1 className="text-2xl font-black tracking-tight">HoroQuiz</h1>
        <nav className="flex gap-3 text-sm">
          <Link to="/teacher/dashboard" className="rounded-md bg-white px-2 py-1">Панель</Link>
          <Link to="/teacher/quizzes/new" className="rounded-md bg-white px-2 py-1">Новый квиз</Link>
          <Link to="/teacher/library" className="rounded-md bg-white px-2 py-1">Библиотека</Link>
          <Link to="/join" className="rounded-md bg-white px-2 py-1">Вход ученика</Link>
        </nav>
      </motion.header>
      <h2 className="mb-3 text-xl font-bold">{title}</h2>
      {body}
    </div>
  )
}

function AuthPage({ mode }: { mode: 'login' | 'register' }) {
  const navigate = useNavigate()
  const [login, setLogin] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')

  async function onSubmit(e: FormEvent) {
    e.preventDefault()
    const trimmedLogin = login.trim()
    if (trimmedLogin.length < 3) {
      setError('Логин должен быть не короче 3 символов')
      return
    }
    if (password.length < 8) {
      setError('Пароль должен быть не короче 8 символов')
      return
    }
    try {
      setError('')
      if (mode === 'register') await api.register(trimmedLogin, password)
      await api.login(trimmedLogin, password)
      navigate('/teacher/dashboard')
    } catch (err) {
      setError(String(err))
    }
  }

  return shell(
    mode === 'login' ? 'Вход учителя' : 'Регистрация учителя',
    <form className="mx-auto max-w-md space-y-3 rounded-2xl bg-white/90 p-5 shadow" onSubmit={onSubmit}>
      <input className="w-full rounded-lg border px-3 py-2" value={login} onChange={(e) => setLogin(e.target.value)} placeholder="Логин" />
      <input className="w-full rounded-lg border px-3 py-2" type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Пароль" />
      <p className="text-xs text-emerald-950/70">Требования: логин от 3 символов, пароль от 8 символов.</p>
      {error && <p className="text-sm text-red-600">{error}</p>}
      <button className="rounded-lg bg-emerald-900 px-4 py-2 text-white" type="submit">
        {mode === 'login' ? 'Войти' : 'Зарегистрироваться'}
      </button>
      <p className="text-sm">
        {mode === 'login' ? <Link to="/register">Нет аккаунта</Link> : <Link to="/login">Уже есть аккаунт</Link>}
      </p>
    </form>,
  )
}

function DashboardPage() {
  const [quizzes, setQuizzes] = useState<Array<{ id: number; title: string; is_published: boolean }>>([])
  const [modeByQuiz, setModeByQuiz] = useState<Record<number, 'platformer' | 'shooter' | 'classic'>>({})
  const navigate = useNavigate()

  async function load() {
    try {
      const data = (await api.listQuizzes()) as { items: Array<{ id: number; title: string; is_published: boolean }> }
      setQuizzes(data.items)
    } catch {
      navigate('/login')
    }
  }

  useEffect(() => {
    load()
  }, [])

  async function startSession(quizId: number) {
    const mode = modeByQuiz[quizId] ?? 'classic'
    const response = (await api.createSession(quizId, mode)) as { sessionId: number; roomCode: string }
    navigate(`/teacher/sessions/${response.sessionId}/waiting?room=${response.roomCode}`)
  }

  return shell(
    'Панель учителя',
    <div className="space-y-3">
      {quizzes.length === 0 && (
        <div className="rounded-2xl bg-white/90 p-8 text-center shadow">
          <p className="mb-4 text-lg font-semibold">У вас ещё нет викторин</p>
          <button className="rounded-xl bg-emerald-900 px-5 py-2 text-white" onClick={() => navigate('/teacher/quizzes/new')}>
            Создать
          </button>
        </div>
      )}
      {quizzes.map((q) => (
        <motion.div key={q.id} initial={{ opacity: 0, y: 4 }} animate={{ opacity: 1, y: 0 }} className="rounded-xl bg-white/90 p-4 shadow-sm">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <p className="font-semibold">{q.title}</p>
              <p className="text-sm text-emerald-950/65">{q.is_published ? 'Опубликован' : 'Черновик'}</p>
            </div>
            <div className="flex gap-2">
              <button className="rounded bg-slate-100 px-3 py-1 text-emerald-900" onClick={() => navigate(`/teacher/quizzes/${q.id}/edit`)}>Редактировать</button>
              {!q.is_published && <button className="rounded bg-emerald-900 px-3 py-1 text-white" onClick={() => api.publishQuiz(q.id).then(load)}>Публиковать</button>}
              <div className="flex items-center gap-2 rounded-xl bg-slate-100 p-1.5">
                <select
                  className="rounded-lg border border-slate-200 bg-white px-2 py-1 text-sm text-emerald-900 outline-none focus:border-emerald-400"
                  value={modeByQuiz[q.id] ?? 'classic'}
                  onChange={(e) => setModeByQuiz((p) => ({ ...p, [q.id]: e.target.value as 'platformer' | 'shooter' | 'classic' }))}
                >
                  <option value="classic">Квиз</option>
                  <option value="platformer">Платформер</option>
                  <option value="shooter">Шутер</option>
                </select>
                <button className="rounded-lg bg-orange-600 px-3 py-1 text-sm font-semibold text-white transition hover:bg-orange-500 active:scale-[0.98]" onClick={() => startSession(q.id)}>
                  Запустить
                </button>
              </div>
            </div>
          </div>
        </motion.div>
      ))}
    </div>,
  )
}

function NewQuizPage() {
  type DraftQuestion = {
    id: string
    type: 'open' | 'single' | 'multi'
    prompt: string
    options: string[]
    openAnswer: string
    singleCorrect: number
    multiCorrect: boolean[]
  }
  type BuilderMode = 'pick' | 'manual' | 'ai_setup' | 'ai_edit'

  const createDraftQuestion = (type: 'open' | 'single' | 'multi', seed: number): DraftQuestion => ({
    id: `q${Date.now()}_${seed}`,
    type,
    prompt: '',
    options: type === 'open' ? [] : ['Вариант 1', 'Вариант 2'],
    openAnswer: '',
    singleCorrect: 0,
    multiCorrect: type === 'multi' ? [true, false] : [],
  })

  const navigate = useNavigate()
  const { id } = useParams()
  const isEdit = Boolean(id)
  const [mode, setMode] = useState<BuilderMode>('pick')
  const [title, setTitle] = useState('Новый квиз')
  const [description, setDescription] = useState('Описание')
  const [questions, setQuestions] = useState<DraftQuestion[]>([])
  const [topic, setTopic] = useState('История России')
  const [grade, setGrade] = useState('8')
  const [questionCount, setQuestionCount] = useState(5)
  const [isGenerating, setIsGenerating] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState('')

  function toQuizPayload(): Quiz {
    const payloadQuestions: Question[] = questions.map((q) => {
      if (q.type === 'open') {
        return {
          id: q.id,
          type: 'open',
          prompt: q.prompt.trim(),
          answer: { text: q.openAnswer.trim() },
        }
      }

      const options = q.options.map((text, index) => ({ id: `o${index + 1}`, text: text.trim() }))
      if (q.type === 'single') {
        return {
          id: q.id,
          type: 'single',
          prompt: q.prompt.trim(),
          options,
          answer: { optionId: options[Math.max(0, Math.min(q.singleCorrect, options.length - 1))]?.id ?? 'o1' },
        }
      }

      const selected = options
        .filter((_, idx) => q.multiCorrect[idx])
        .map((o) => o.id)
      return {
        id: q.id,
        type: 'multi',
        prompt: q.prompt.trim(),
        options,
        answer: { optionIds: selected.length > 0 ? selected : [options[0]?.id ?? 'o1'] },
      }
    })

    const normalizedDescription = description.trim()
    return {
      title: title.trim(),
      description: normalizedDescription.length > 0 ? normalizedDescription : undefined,
      questions: payloadQuestions,
    }
  }

  function applyQuizFromApi(quizApi: any) {
    setTitle(String(quizApi.title ?? 'Квиз от ИИ'))
    setDescription(String(quizApi.description ?? ''))
    const nextQuestions: DraftQuestion[] = (quizApi.questions ?? []).map((q: any, index: number) => {
      const qType = q.type as 'open' | 'single' | 'multi'
      if (qType === 'open') {
        return {
          id: String(q.id ?? `q${index + 1}`),
          type: 'open',
          prompt: String(q.prompt ?? ''),
          options: [],
          openAnswer: String(q.answer?.text ?? ''),
          singleCorrect: 0,
          multiCorrect: [],
        }
      }
      const options = (q.options ?? []).map((o: any) => String(o.text ?? ''))
      if (qType === 'single') {
        const optionId = String(q.answer?.optionId ?? '')
        const sourceOptions = q.options ?? []
        const selectedIndex = sourceOptions.findIndex((o: any) => String(o.id) === optionId)
        return {
          id: String(q.id ?? `q${index + 1}`),
          type: 'single',
          prompt: String(q.prompt ?? ''),
          options,
          openAnswer: '',
          singleCorrect: selectedIndex >= 0 ? selectedIndex : 0,
          multiCorrect: [],
        }
      }
      const sourceOptions = q.options ?? []
      const optionIds = new Set((q.answer?.optionIds ?? []).map((v: unknown) => String(v)))
      return {
        id: String(q.id ?? `q${index + 1}`),
        type: 'multi',
        prompt: String(q.prompt ?? ''),
        options,
        openAnswer: '',
        singleCorrect: 0,
        multiCorrect: sourceOptions.map((o: any) => optionIds.has(String(o.id))),
      }
    })
    setQuestions(nextQuestions)
  }

  async function save() {
    if (questions.length === 0) {
      setError('Добавьте хотя бы один вопрос')
      return
    }
    setIsSaving(true)
    try {
      const payload = toQuizPayload()
      if (isEdit && id) {
        await api.updateQuiz(Number(id), payload)
      } else {
        await api.createQuiz(payload)
      }
      navigate('/teacher/dashboard')
    } catch (err) {
      setError(String(err))
    } finally {
      setIsSaving(false)
    }
  }

  async function generateAi() {
    setIsGenerating(true)
    try {
      setError('')
      const created = (await api.aiGenerate(topic, grade, questionCount)) as { quizId: number }
      const quiz = await api.getQuiz(created.quizId)
      applyQuizFromApi(quiz)
      setMode('ai_edit')
    } catch (err) {
      setError(String(err))
    } finally {
      setIsGenerating(false)
    }
  }

  function addQuestion(type: 'open' | 'single' | 'multi') {
    setQuestions((prev) => [...prev, createDraftQuestion(type, prev.length + 1)])
  }

  useEffect(() => {
    if (!isEdit || !id) return
    setMode('manual')
    setError('')
    api.getQuiz(Number(id))
      .then((quiz) => applyQuizFromApi(quiz))
      .catch((err) => setError(String(err)))
  }, [isEdit, id])

  if (mode === 'pick' && !isEdit) {
    return shell(
      'Новый квиз',
      <div className="grid gap-4 md:grid-cols-3">
        <motion.button initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="rounded-2xl bg-white/95 p-5 text-left shadow" onClick={() => { setError(''); setTitle('Новый квиз'); setDescription(''); setQuestions([]); setMode('manual') }}>
          <p className="mb-2 text-sm uppercase tracking-[0.2em] text-emerald-950/60">Сценарий 1</p>
          <p className="text-lg font-bold">Ручное создание</p>
          <p className="mt-2 text-sm text-emerald-950/70">Полный визуальный конструктор с нуля.</p>
        </motion.button>
        <motion.button initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.05 }} className="rounded-2xl bg-white/95 p-5 text-left shadow" onClick={() => { setError(''); setMode('ai_setup') }}>
          <p className="mb-2 text-sm uppercase tracking-[0.2em] text-emerald-950/60">Сценарий 2</p>
          <p className="text-lg font-bold">ИИ генерация + доработка</p>
          <p className="mt-2 text-sm text-emerald-950/70">Сначала генерация от ИИ, потом правки в визуальном редакторе.</p>
        </motion.button>
        <motion.button initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.1 }} className="rounded-2xl bg-white/95 p-5 text-left shadow" onClick={() => navigate('/teacher/library')}>
          <p className="mb-2 text-sm uppercase tracking-[0.2em] text-emerald-950/60">Сценарий 3</p>
          <p className="text-lg font-bold">Выбрать из открытых</p>
          <p className="mt-2 text-sm text-emerald-950/70">Перейти в публичную библиотеку и клонировать готовый квиз.</p>
        </motion.button>
      </div>,
    )
  }

  if (mode === 'ai_setup') {
    return shell(
      'ИИ генерация викторины',
      <div className="mx-auto max-w-2xl rounded-2xl bg-white/95 p-5 shadow">
        <p className="mb-3 text-sm text-emerald-950/75">Заполните параметры. После генерации откроется экран доработки.</p>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-sm font-semibold">Тема</label>
            <input className="w-full rounded-lg border px-3 py-2" value={topic} onChange={(e) => setTopic(e.target.value)} placeholder="Например: История России XIX века" />
          </div>
          <div className="grid gap-3 md:grid-cols-2">
            <div>
              <label className="mb-1 block text-sm font-semibold">Класс</label>
              <input className="w-full rounded-lg border px-3 py-2" value={grade} onChange={(e) => setGrade(e.target.value)} placeholder="Например: 8" />
              <p className="mt-1 text-xs text-emerald-950/65">Используется ИИ для адаптации сложности формулировок.</p>
            </div>
            <div>
              <label className="mb-1 block text-sm font-semibold">Количество вопросов</label>
              <input className="w-full rounded-lg border px-3 py-2" type="number" min={1} max={20} value={questionCount} onChange={(e) => setQuestionCount(Number(e.target.value) || 1)} />
              <p className="mt-1 text-xs text-emerald-950/65">Сколько вопросов ИИ должен сгенерировать за один раз.</p>
            </div>
          </div>
          {error && <p className="rounded-lg bg-red-50 p-2 text-sm text-red-700">{error}</p>}
          <div className="flex gap-2">
            <button className="rounded-xl bg-slate-100 px-4 py-2" onClick={() => setMode('pick')}>Назад</button>
            <button className="rounded-xl bg-orange-600 px-4 py-2 font-semibold text-white" onClick={generateAi} disabled={isGenerating}>
              {isGenerating ? 'Генерируем...' : 'Сгенерировать'}
            </button>
          </div>
        </div>
      </div>,
    )
  }

  return shell(
    isEdit ? 'Редактирование викторины' : mode === 'manual' ? 'Ручной конструктор викторины' : 'Доработка викторины от ИИ',
    <div className="grid gap-4 xl:grid-cols-[360px_1fr]">
      <motion.aside initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} className="space-y-4 rounded-2xl bg-white/90 p-4 shadow">
        <div className="flex gap-2">
          {!isEdit && <button className="rounded-xl bg-slate-100 px-3 py-2 text-sm" onClick={() => setMode('pick')}>К выбору сценария</button>}
          {mode === 'ai_edit' && (
            <button className="rounded-xl bg-orange-100 px-3 py-2 text-sm text-orange-800" onClick={() => setMode('ai_setup')}>
              Перегенерировать через ИИ
            </button>
          )}
        </div>

        <div className="space-y-2 rounded-xl bg-emerald-50 p-3">
          <p className="text-xs uppercase tracking-[0.2em] text-emerald-950/60">Основное</p>
          <input className="w-full rounded-lg border px-3 py-2" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Название викторины" />
          <textarea className="h-24 w-full rounded-lg border px-3 py-2" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Описание" />
        </div>

        <div className="space-y-2">
          <p className="text-xs uppercase tracking-[0.2em] text-emerald-950/60">Добавить вопрос</p>
          <div className="grid grid-cols-3 gap-2 text-sm">
            <button className="rounded-lg bg-white px-2 py-2 shadow-sm" onClick={() => addQuestion('open')}>Ввод</button>
            <button className="rounded-lg bg-white px-2 py-2 shadow-sm" onClick={() => addQuestion('single')}>Один ответ</button>
            <button className="rounded-lg bg-white px-2 py-2 shadow-sm" onClick={() => addQuestion('multi')}>Мульти-ответ</button>
          </div>
        </div>

        <button className="w-full rounded-xl bg-emerald-900 px-4 py-3 font-semibold text-white transition hover:translate-y-[-1px]" onClick={save} disabled={isSaving}>
          {isSaving ? 'Сохраняем...' : isEdit ? 'Сохранить изменения' : 'Сохранить викторину'}
        </button>
        {error && <p className="rounded-lg bg-red-50 p-2 text-sm text-red-700">{error}</p>}
      </motion.aside>

      <div className="space-y-3">
        {questions.length === 0 && (
          <div className="rounded-2xl bg-white/90 p-8 text-center shadow">
            <p className="mb-3">Вопросов пока нет</p>
            <button className="rounded-xl bg-emerald-900 px-4 py-2 text-white" onClick={() => addQuestion('open')}>
              Добавить первый вопрос
            </button>
          </div>
        )}
        {questions.map((question, qIndex) => (
          <motion.div key={question.id} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} className="rounded-2xl bg-white/95 p-4 shadow">
            <div className="mb-3 flex items-center justify-between">
              <p className="font-semibold">Вопрос {qIndex + 1}</p>
              <div className="flex items-center gap-2">
                <select
                  className="rounded-lg border px-2 py-1 text-sm"
                  value={question.type}
                  onChange={(e) =>
                    setQuestions((prev) =>
                      prev.map((q, idx) => {
                        if (idx !== qIndex) return q
                        const nextType = e.target.value as 'open' | 'single' | 'multi'
                        return {
                          ...q,
                          type: nextType,
                          options: nextType === 'open' ? [] : q.options.length > 1 ? q.options : ['Вариант 1', 'Вариант 2'],
                          openAnswer: nextType === 'open' ? q.openAnswer : '',
                          singleCorrect: 0,
                          multiCorrect: nextType === 'multi' ? Array(Math.max(q.options.length, 2)).fill(false).map((_, i) => i === 0) : [],
                        }
                      }),
                    )
                  }
                >
                  <option value="open">Ввод</option>
                  <option value="single">Один ответ</option>
                  <option value="multi">Мульти-ответ</option>
                </select>
                <button className="rounded-lg bg-red-100 px-3 py-1 text-sm text-red-700" onClick={() => setQuestions((prev) => prev.filter((_, idx) => idx !== qIndex))}>
                  Удалить
                </button>
              </div>
            </div>

            <input
              className="mb-3 w-full rounded-lg border px-3 py-2"
              value={question.prompt}
              onChange={(e) => setQuestions((prev) => prev.map((q, idx) => idx === qIndex ? { ...q, prompt: e.target.value } : q))}
              placeholder="Текст вопроса"
            />

            {question.type === 'open' && (
              <div className="rounded-xl bg-slate-50 p-3">
                <p className="mb-2 text-sm text-emerald-950/70">Правильный ответ</p>
                <input
                  className="w-full rounded-lg border px-3 py-2"
                  value={question.openAnswer}
                  onChange={(e) => setQuestions((prev) => prev.map((q, idx) => idx === qIndex ? { ...q, openAnswer: e.target.value } : q))}
                  placeholder="Введите ответ"
                />
              </div>
            )}

            {(question.type === 'single' || question.type === 'multi') && (
              <div className="space-y-2 rounded-xl bg-slate-50 p-3">
                <p className="text-sm text-emerald-950/70">Варианты ответа</p>
                {question.options.map((option, optIndex) => (
                  <div key={`${question.id}_${optIndex}`} className="flex items-center gap-2">
                    {question.type === 'single' && (
                      <input
                        type="radio"
                        checked={question.singleCorrect === optIndex}
                        onChange={() => setQuestions((prev) => prev.map((q, idx) => idx === qIndex ? { ...q, singleCorrect: optIndex } : q))}
                      />
                    )}
                    {question.type === 'multi' && (
                      <input
                        type="checkbox"
                        checked={Boolean(question.multiCorrect[optIndex])}
                        onChange={(e) =>
                          setQuestions((prev) =>
                            prev.map((q, idx) => {
                              if (idx !== qIndex) return q
                              const next = [...q.multiCorrect]
                              next[optIndex] = e.target.checked
                              return { ...q, multiCorrect: next }
                            }),
                          )
                        }
                      />
                    )}
                    <input
                      className="flex-1 rounded-lg border px-3 py-2"
                      value={option}
                      onChange={(e) =>
                        setQuestions((prev) =>
                          prev.map((q, idx) => {
                            if (idx !== qIndex) return q
                            const nextOptions = [...q.options]
                            nextOptions[optIndex] = e.target.value
                            return { ...q, options: nextOptions }
                          }),
                        )
                      }
                    />
                    <button
                      className="rounded-lg bg-red-100 px-2 py-1 text-red-700"
                      onClick={() =>
                        setQuestions((prev) =>
                          prev.map((q, idx) => {
                            if (idx !== qIndex) return q
                            if (q.options.length <= 2) return q
                            const nextOptions = q.options.filter((_, i) => i !== optIndex)
                            const nextMulti = q.multiCorrect.filter((_, i) => i !== optIndex)
                            return {
                              ...q,
                              options: nextOptions,
                              multiCorrect: nextMulti,
                              singleCorrect: q.singleCorrect >= nextOptions.length ? nextOptions.length - 1 : q.singleCorrect,
                            }
                          }),
                        )
                      }
                    >
                      x
                    </button>
                  </div>
                ))}
                <button
                  className="rounded-lg bg-white px-3 py-2 text-sm shadow-sm"
                  onClick={() =>
                    setQuestions((prev) =>
                      prev.map((q, idx) => {
                        if (idx !== qIndex) return q
                        return {
                          ...q,
                          options: [...q.options, `Вариант ${q.options.length + 1}`],
                          multiCorrect: q.type === 'multi' ? [...q.multiCorrect, false] : q.multiCorrect,
                        }
                      }),
                    )
                  }
                >
                  + Добавить вариант
                </button>
              </div>
            )}
          </motion.div>
        ))}
      </div>
    </div>,
  )
}

function LibraryPage() {
  const [q, setQ] = useState('')
  const [items, setItems] = useState<Array<{ id: number; title: string; description?: string }>>([])

  async function search() {
    const data = (await api.searchLibrary(q)) as { items: Array<{ id: number; title: string; description?: string }> }
    setItems(data.items)
  }

  useEffect(() => {
    search()
  }, [])

  return shell(
    'Публичная библиотека',
    <div className="space-y-3">
      <div className="flex gap-2">
        <input className="w-full rounded border px-3 py-2" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Поиск" />
        <button className="rounded bg-emerald-900 px-4 py-2 text-white" onClick={search}>Найти</button>
      </div>
      {items.map((item) => (
        <div key={item.id} className="rounded-xl bg-white/90 p-3 shadow-sm">
          <p className="font-semibold">{item.title}</p>
          <p className="text-sm text-emerald-950/70">{item.description}</p>
          <button className="mt-2 rounded bg-orange-600 px-3 py-1 text-white" onClick={() => api.cloneQuiz(item.id)}>Добавить в мои</button>
        </div>
      ))}
    </div>,
  )
}

function TeacherWaitingPage() {
  const { id } = useParams()
  const [sp] = useSearchParams()
  const room = sp.get('room') ?? ''
  const [participants, setParticipants] = useState<string[]>([])
  const navigate = useNavigate()

  useEffect(() => {
    if (!room) return
    const ws = connectRoom(room, (msg: WsEnvelope) => {
      if (msg.event === 'waiting_room_update') {
        const payload = msg.payload as { participants: Array<{ nickname: string }> }
        setParticipants(payload.participants.map((p) => p.nickname))
      }
    })
    return () => ws.close()
  }, [room])

  async function start() {
    await api.startSession(Number(id))
    navigate(`/teacher/sessions/${id}/live?room=${room}`)
  }

  return shell(
    'Waiting room',
    <div className="grid gap-4 md:grid-cols-2">
      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <p className="text-sm">Комната: <b>{room}</b></p>
        <QRCodeSVG value={`${window.location.origin}/join?room=${room}`} className="mt-3" />
        <button className="mt-4 rounded bg-emerald-900 px-4 py-2 text-white" onClick={start}>Запустить квиз</button>
      </div>
      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <h3 className="mb-2 font-semibold">Подключившиеся ученики</h3>
        <ul className="space-y-1">
          {participants.map((p) => <li key={p}>{p}</li>)}
        </ul>
      </div>
    </div>,
  )
}

function TeacherLivePage() {
  const { id } = useParams()
  const [sp] = useSearchParams()
  const room = sp.get('room') ?? ''
  const [classStats, setClassStats] = useState({ correctPct: 0, wrongPct: 0 })
  const [students, setStudents] = useState<Array<{ nickname: string; correct: number; wrong: number; correctPct: number }>>([])
  const navigate = useNavigate()

  useEffect(() => {
    if (!room) return
    const ws = connectRoom(room, (msg) => {
      if (msg.event === 'stats_update') {
        const payload = msg.payload as {
          class?: { correctPct?: number; wrongPct?: number }
          students?: Array<{ nickname: string; correct: number; wrong: number; correctPct: number }>
        }
        setClassStats({
          correctPct: Number(payload.class?.correctPct ?? 0),
          wrongPct: Number(payload.class?.wrongPct ?? 0),
        })
        setStudents(payload.students ?? [])
      }
    })
    return () => ws.close()
  }, [room])

  async function finish() {
    await api.endSession(Number(id))
    navigate(`/teacher/sessions/${id}/results`)
  }

  const StatBar = ({ correctPct, wrongPct }: { correctPct: number; wrongPct: number }) => (
    <div className="h-3 w-full overflow-hidden rounded-full bg-slate-200">
      <div className="flex h-full w-full">
        <div className="h-full bg-emerald-500" style={{ width: `${Math.max(0, Math.min(100, correctPct))}%` }} />
        <div className="h-full bg-red-500" style={{ width: `${Math.max(0, Math.min(100, wrongPct))}%` }} />
      </div>
    </div>
  )

  return shell(
    'Лайв статистика',
    <div className="space-y-4">
      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <p className="mb-2 font-semibold">Класс</p>
        <StatBar correctPct={classStats.correctPct} wrongPct={classStats.wrongPct} />
        <p className="mt-2 text-sm text-emerald-900/80">Верно: {classStats.correctPct.toFixed(1)}% | Ошибки: {classStats.wrongPct.toFixed(1)}%</p>
      </div>

      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <p className="mb-3 font-semibold">Ученики</p>
        <div className="space-y-3">
          {students.length === 0 && <p className="text-sm text-emerald-950/70">Пока нет ответов</p>}
          {students.map((s) => (
            <div key={s.nickname} className="rounded-xl bg-slate-50 p-3">
              <div className="mb-2 flex items-center justify-between text-sm">
                <span className="font-medium">{s.nickname}</span>
                <span>Верно {s.correct} / Ошибок {s.wrong}</span>
              </div>
              <StatBar correctPct={s.correctPct} wrongPct={100 - s.correctPct} />
            </div>
          ))}
        </div>
      </div>

      <button className="rounded bg-orange-600 px-4 py-2 text-white" onClick={finish}>Завершить квиз</button>
    </div>,
  )
}

function TeacherResultsPage() {
  const { id } = useParams()
  const [data, setData] = useState<null | {
    session: { id: number; roomCode: string; status: string; gameMode?: string }
    classStats: { correct: number; wrong: number; correctPct: number }
    studentStats: Array<{ nickname: string; correct: number; wrong: number; correctPct: number }>
    mistakesByStudent: Array<{ nickname: string; questions: string[] }>
  }>(null)
  const [error, setError] = useState('')

  useEffect(() => {
    api
      .sessionResults(Number(id))
      .then((res) => setData(res as any))
      .catch((e) => setError(String(e)))
  }, [id])

  const StatBar = ({ correctPct, wrongPct }: { correctPct: number; wrongPct: number }) => (
    <div className="h-3 w-full overflow-hidden rounded-full bg-slate-200">
      <div className="flex h-full w-full">
        <div className="h-full bg-emerald-500" style={{ width: `${Math.max(0, Math.min(100, correctPct))}%` }} />
        <div className="h-full bg-red-500" style={{ width: `${Math.max(0, Math.min(100, wrongPct))}%` }} />
      </div>
    </div>
  )

  if (error) {
    return shell('Результаты', <div className="rounded-2xl bg-white/90 p-4 text-red-600 shadow">{error}</div>)
  }
  if (!data) {
    return shell('Результаты', <div className="rounded-2xl bg-white/90 p-4 shadow">Загрузка результатов...</div>)
  }

  const mistakesMap = new Map(data.mistakesByStudent.map((m) => [m.nickname, m.questions]))
  const wrongPct = 100 - data.classStats.correctPct

  return shell(
    'Результаты',
    <div className="space-y-4">
      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <div className="mb-2 flex flex-wrap items-center justify-between gap-2">
          <p className="text-sm text-emerald-950/70">Комната: <b>{data.session.roomCode}</b></p>
          <p className="text-sm text-emerald-950/70">Режим: <b>{data.session.gameMode === 'platformer' ? 'Платформер' : data.session.gameMode === 'shooter' ? 'Шутер' : 'Квиз'}</b></p>
        </div>
        <p className="mb-2 font-semibold">Итог по классу</p>
        <StatBar correctPct={data.classStats.correctPct} wrongPct={wrongPct} />
        <p className="mt-2 text-sm text-emerald-900/80">
          Верно: {data.classStats.correct} | Ошибки: {data.classStats.wrong} | Точность: {data.classStats.correctPct.toFixed(1)}%
        </p>
      </div>

      <div className="rounded-2xl bg-white/90 p-4 shadow">
        <p className="mb-3 font-semibold">По ученикам</p>
        <div className="space-y-3">
          {data.studentStats.length === 0 && <p className="text-sm text-emerald-950/70">Нет данных по ответам.</p>}
          {data.studentStats.map((s) => (
            <div key={s.nickname} className="rounded-xl bg-slate-50 p-3">
              <div className="mb-2 flex flex-wrap items-center justify-between gap-2 text-sm">
                <span className="font-semibold">{s.nickname}</span>
                <span>Верно {s.correct} / Ошибок {s.wrong} ({s.correctPct.toFixed(1)}%)</span>
              </div>
              <StatBar correctPct={s.correctPct} wrongPct={100 - s.correctPct} />
              <div className="mt-2 flex flex-wrap gap-2 text-xs">
                {(mistakesMap.get(s.nickname) ?? []).length > 0
                  ? (mistakesMap.get(s.nickname) ?? []).map((q, i) => (
                    <span key={`${s.nickname}-m-${i}`} className="rounded-full bg-red-100 px-2 py-1 text-red-700">{q}</span>
                  ))
                  : <span className="rounded-full bg-emerald-100 px-2 py-1 text-emerald-700">Без ошибок</span>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>,
  )
}

function JoinPage() {
  const [room, setRoom] = useState('')
  const [nickname, setNickname] = useState('')
  const navigate = useNavigate()
  const [sp] = useSearchParams()

  useEffect(() => {
    const initial = sp.get('room')
    if (initial) setRoom(initial)
  }, [sp])

  function join() {
    if (room.trim().length < 3 || nickname.trim().length < 2) return
    localStorage.setItem('student_nickname', nickname.trim())
    navigate(`/wait/${room.trim().toUpperCase()}`)
  }

  return shell(
    'Подключение ученика',
    <div className="mx-auto max-w-md space-y-3 rounded-2xl bg-white/90 p-4 shadow">
      <input className="w-full rounded border px-3 py-2" value={room} onChange={(e) => setRoom(e.target.value)} placeholder="Код комнаты" />
      <input className="w-full rounded border px-3 py-2" value={nickname} onChange={(e) => setNickname(e.target.value)} placeholder="Ник" />
      <button className="rounded bg-emerald-900 px-4 py-2 text-white" onClick={join}>Войти</button>
    </div>,
  )
}

function StudentWaitPage() {
  const { roomCode } = useParams()
  const navigate = useNavigate()
  const nickname = useMemo(() => localStorage.getItem('student_nickname') ?? '', [])

  useEffect(() => {
    if (!roomCode || !nickname) return
    const ws = connectRoom(roomCode, (msg) => {
      if (msg.event === 'start_quiz') {
        const payload = msg.payload as { gameMode?: string }
        if (payload.gameMode) {
          localStorage.setItem('session_game_mode', payload.gameMode)
        }
        navigate(`/play/${roomCode}`)
      }
    })
    ws.onopen = () => sendWs(ws, 'join_room', { role: 'student', nickname })
    return () => ws.close()
  }, [roomCode, nickname, navigate])

  return shell('Ожидание старта', <div className="rounded-2xl bg-white/90 p-4 shadow">Вы подключены как <b>{nickname}</b>. Ждите запуск учителем.</div>)
}

function StudentPlayPage() {
  const { roomCode } = useParams()
  const nickname = useMemo(() => localStorage.getItem('student_nickname') ?? '', [])
  const [socket, setSocket] = useState<WebSocket | null>(null)
  const initialMode = (() => {
    const raw = localStorage.getItem('session_game_mode')
    if (raw === 'platformer' || raw === 'shooter' || raw === 'classic') return raw
    return 'classic'
  })()
  const [mode] = useState<'platformer' | 'shooter' | 'classic'>(
    initialMode,
  )
  const [question, setQuestion] = useState<Question | null>(null)
  const [awaitingNextQuestion, setAwaitingNextQuestion] = useState(false)
  const [mustGetCorrect, setMustGetCorrect] = useState(false)
  const [status, setStatus] = useState(mode === 'classic' ? 'Классический режим запущен' : 'Игра запущена')
  const detectMobile = () => {
    if (typeof window === 'undefined') return false
    const coarse = window.matchMedia('(pointer: coarse)').matches
    const touch = (navigator.maxTouchPoints ?? 0) > 0
    const ua = /Android|iPhone|iPad|iPod|Mobile/i.test(navigator.userAgent)
    return coarse || touch || ua || window.innerWidth < 1024
  }
  const [mobileView, setMobileView] = useState(() => {
    if (typeof window === 'undefined') return false
    return detectMobile()
  })
  const [portrait, setPortrait] = useState(() => {
    if (typeof window === 'undefined') return false
    return window.innerHeight > window.innerWidth
  })
  const navigate = useNavigate()

  useEffect(() => {
    const updateViewport = () => {
      const isMobile = detectMobile()
      setMobileView(isMobile)
      setPortrait(window.innerHeight > window.innerWidth)
    }
    updateViewport()
    window.addEventListener('resize', updateViewport)
    window.addEventListener('orientationchange', updateViewport)
    return () => {
      window.removeEventListener('resize', updateViewport)
      window.removeEventListener('orientationchange', updateViewport)
    }
  }, [])

  useEffect(() => {
    const fullscreenGame = mode !== 'classic' && mobileView
    if (!fullscreenGame) return
    const html = document.documentElement
    const body = document.body
    const prevHtmlOverflow = html.style.overflow
    const prevBodyOverflow = body.style.overflow
    const prevBodyTouch = body.style.touchAction
    const prevOverscroll = body.style.overscrollBehavior
    html.style.overflow = 'hidden'
    body.style.overflow = 'hidden'
    body.style.touchAction = 'none'
    body.style.overscrollBehavior = 'none'
    return () => {
      html.style.overflow = prevHtmlOverflow
      body.style.overflow = prevBodyOverflow
      body.style.touchAction = prevBodyTouch
      body.style.overscrollBehavior = prevOverscroll
    }
  }, [mode, mobileView])

  useEffect(() => {
    if (!roomCode || !nickname) return
    const ws = connectRoom(roomCode, (msg) => {
      if (msg.event === 'question_push') {
        setAwaitingNextQuestion(false)
        setQuestion((msg.payload as { question: Question }).question)
      }
      if (msg.event === 'answer_result') {
        const payload = msg.payload as { correct: boolean; nextAction: string }
        if (mode === 'classic') {
          setStatus(payload.correct ? 'Верно, идём дальше' : 'Неверно, идём к следующему вопросу')
        } else if (payload.correct) {
          setStatus('Верно, продолжаем игру')
          setMustGetCorrect(false)
          setAwaitingNextQuestion(false)
        } else {
          setStatus('Неверно, следующий вопрос')
          setAwaitingNextQuestion(true)
          setTimeout(() => sendWs(ws, 'request_question', { reason: 'level_up' }), 180)
        }
        if (mode === 'classic' && payload.nextAction === 'continue') {
          setTimeout(() => sendWs(ws, 'request_question', { reason: 'level_up' }), 200)
        }
      }
      if (msg.event === 'end_quiz') navigate(`/done/${roomCode}`)
    })
    ws.onopen = () => {
      sendWs(ws, 'join_room', { role: 'student', nickname })
      if (mode === 'classic') {
        setTimeout(() => sendWs(ws, 'request_question', { reason: 'level_up' }), 150)
      }
    }
    setSocket(ws)
    return () => ws.close()
  }, [roomCode, nickname, navigate, mode])

  const triggerQuestion = (reason: 'death' | 'level_up') => {
    if (mode !== 'classic' && mobileView && portrait) return
    if (!socket || question || awaitingNextQuestion || mustGetCorrect) return
    setMustGetCorrect(true)
    setAwaitingNextQuestion(true)
    setStatus('Ответьте правильно, чтобы продолжить')
    sendWs(socket, 'request_question', { reason })
  }

  const submitAnswer = (answer: Record<string, unknown>) => {
    if (!socket || !question) return
    sendWs(socket, 'answer_submit', { questionId: question.id, answer })
    setQuestion(null)
    if (mode !== 'classic') {
      setAwaitingNextQuestion(true)
    }
  }

  const overlayActive = mode !== 'classic' && (Boolean(question) || awaitingNextQuestion || mustGetCorrect)
  const needsLandscape = mode !== 'classic' && mobileView && portrait
  const gamePaused = overlayActive || needsLandscape
  const content = (
    <div className="space-y-3">
      {mode === 'classic' ? (
        <div className="rounded-2xl bg-white/90 p-4 shadow">
          <p className="text-sm text-emerald-950/70">Мини-игры отключены. Следующий вопрос показывается автоматически после правильного ответа.</p>
        </div>
      ) : (
        <div className={mobileView ? 'relative h-full w-full' : 'relative'}>
          <GameCanvas mode={mode} onTrigger={triggerQuestion} paused={gamePaused} fullscreen={mobileView} />
          {needsLandscape && (
            <div className="absolute inset-0 z-30 flex items-center justify-center bg-black/85 p-6 text-center">
              <div className="max-w-sm rounded-2xl bg-white/95 p-5 shadow-lg">
                <p className="text-sm uppercase tracking-wide text-emerald-900/70">Игровой режим</p>
                <p className="mt-2 text-lg font-semibold text-emerald-950">Поверните устройство горизонтально</p>
                <p className="mt-2 text-sm text-emerald-950/70">В портретной ориентации игра остановлена.</p>
              </div>
            </div>
          )}
          {overlayActive && !needsLandscape && (
            <div className="absolute inset-0 z-20 flex items-center justify-center bg-black/70 p-4 backdrop-blur-[2px]">
              {question ? (
                <div className="w-full max-w-2xl">
                  <QuestionCard question={question} onSubmit={submitAnswer} />
                </div>
              ) : (
                <div className="rounded-2xl bg-white/95 px-6 py-4 text-center shadow-lg">
                  <p className="text-sm uppercase tracking-wide text-emerald-900/70">Вопрос</p>
                  <p className="mt-2 text-lg font-semibold text-emerald-950">Загружаем следующий вопрос...</p>
                </div>
              )}
            </div>
          )}
        </div>
      )}
      {!mobileView && <div className="rounded-xl bg-white/90 p-3 shadow text-sm">{status}</div>}
      {mode === 'classic' && question && <QuestionCard question={question} onSubmit={submitAnswer} />}
    </div>
  )

  if (mode !== 'classic' && mobileView) {
    return (
      <div className="fixed inset-0 z-40 h-screen w-screen overflow-hidden bg-black" style={{ touchAction: 'none', overscrollBehavior: 'none' }}>
        {content}
        {!needsLandscape && <div className="absolute left-2 top-2 z-40 rounded bg-black/45 px-2 py-1 text-xs text-white/85">{status}</div>}
      </div>
    )
  }

  return shell(
    mode === 'classic' ? 'Классический режим' : 'Игровой режим',
    content,
  )
}

function StudentDonePage() {
  return shell('Квиз завершён', <div className="rounded-2xl bg-white/90 p-4 shadow">Спасибо за участие.</div>)
}

function NotFound() {
  return <Navigate to="/register" replace />
}

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<Navigate to="/register" replace />} />
      <Route path="/login" element={<AuthPage mode="login" />} />
      <Route path="/register" element={<AuthPage mode="register" />} />

      <Route path="/teacher/dashboard" element={<DashboardPage />} />
      <Route path="/teacher/quizzes/new" element={<NewQuizPage />} />
      <Route path="/teacher/quizzes/:id/edit" element={<NewQuizPage />} />
      <Route path="/teacher/library" element={<LibraryPage />} />
      <Route path="/teacher/sessions/:id/waiting" element={<TeacherWaitingPage />} />
      <Route path="/teacher/sessions/:id/live" element={<TeacherLivePage />} />
      <Route path="/teacher/sessions/:id/results" element={<TeacherResultsPage />} />

      <Route path="/join" element={<JoinPage />} />
      <Route path="/wait/:roomCode" element={<StudentWaitPage />} />
      <Route path="/play/:roomCode" element={<StudentPlayPage />} />
      <Route path="/answer/:roomCode" element={<StudentPlayPage />} />
      <Route path="/done/:roomCode" element={<StudentDonePage />} />

      <Route path="*" element={<NotFound />} />
    </Routes>
  )
}
