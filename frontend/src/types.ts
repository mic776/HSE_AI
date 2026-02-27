export type QuestionType = 'open' | 'single' | 'multi'

export type QuizOption = {
  id: string
  text: string
}

export type AnswerKey =
  | { text: string }
  | { optionId: string }
  | { optionIds: string[] }

export type Question = {
  id: string
  type: QuestionType
  prompt: string
  options?: QuizOption[]
  answer: AnswerKey
}

export type Quiz = {
  title: string
  description?: string
  questions: Question[]
}

export type WsEnvelope = {
  event: string
  payload: Record<string, unknown>
  request_id?: string
  ts?: string
}
