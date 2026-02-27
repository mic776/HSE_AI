import { useState } from 'react'
import type { Question } from '../types'

type Props = {
  question: Question
  onSubmit: (answer: Record<string, unknown>) => void
}

export function QuestionCard({ question, onSubmit }: Props) {
  const [open, setOpen] = useState('')
  const [single, setSingle] = useState('')
  const [multi, setMulti] = useState<string[]>([])

  return (
    <div className="rounded-2xl bg-white/95 p-4 shadow-lg">
      <p className="mb-2 text-sm uppercase tracking-wide text-emerald-900/70">Вопрос</p>
      <h3 className="mb-3 text-xl font-semibold">{question.prompt}</h3>

      {question.type === 'open' && (
        <div className="space-y-3">
          <input className="w-full rounded-lg border px-3 py-2" value={open} onChange={(e) => setOpen(e.target.value)} placeholder="Ваш ответ" />
          <button className="rounded-lg bg-emerald-900 px-4 py-2 text-white" onClick={() => onSubmit({ text: open })}>Ответить</button>
        </div>
      )}

      {question.type === 'single' && (
        <div className="space-y-2">
          {(question.options ?? []).map((o) => (
            <label key={o.id} className="flex items-center gap-2 rounded border px-3 py-2">
              <input type="radio" name="single" checked={single === o.id} onChange={() => setSingle(o.id)} />
              <span>{o.text}</span>
            </label>
          ))}
          <button className="rounded-lg bg-emerald-900 px-4 py-2 text-white" onClick={() => onSubmit({ optionId: single })}>Ответить</button>
        </div>
      )}

      {question.type === 'multi' && (
        <div className="space-y-2">
          {(question.options ?? []).map((o) => (
            <label key={o.id} className="flex items-center gap-2 rounded border px-3 py-2">
              <input
                type="checkbox"
                checked={multi.includes(o.id)}
                onChange={(e) => {
                  if (e.target.checked) setMulti((prev) => [...prev, o.id])
                  else setMulti((prev) => prev.filter((v) => v !== o.id))
                }}
              />
              <span>{o.text}</span>
            </label>
          ))}
          <button className="rounded-lg bg-emerald-900 px-4 py-2 text-white" onClick={() => onSubmit({ optionIds: multi })}>Отправить</button>
        </div>
      )}
    </div>
  )
}
