import { useEffect, useMemo, useRef } from 'react'

type Props = {
  mode: 'platformer' | 'shooter'
  onTrigger: (reason: 'death' | 'level_up') => void
  paused?: boolean
}

const SRC_BY_MODE: Record<Props['mode'], string> = {
  platformer: '/games/onoff_game/index.html',
  shooter: '/games/underrun_src/index.html',
}

export function GameCanvas({ mode, onTrigger, paused = false }: Props) {
  const lastEmitRef = useRef(0)
  const frameRef = useRef<HTMLIFrameElement | null>(null)
  const src = useMemo(() => SRC_BY_MODE[mode], [mode])

  useEffect(() => {
    const onMessage = (event: MessageEvent) => {
      const data = event.data as { type?: string; source?: string; reason?: string } | undefined
      if (!data || data.type !== 'quiz_game_event') return

      const now = Date.now()
      if (now - lastEmitRef.current < 900) return
      lastEmitRef.current = now

      if (mode === 'platformer' && data.reason === 'death') {
        onTrigger('death')
      } else if (mode === 'shooter' && data.reason === 'death') {
        onTrigger('death')
      }
    }

    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [mode, onTrigger])

  useEffect(() => {
    const target = frameRef.current?.contentWindow
    if (!target) return
    target.postMessage({ type: 'quiz_pause', paused }, '*')
  }, [paused, src])

  useEffect(() => {
    const fromCode = (code: string): string | null => {
      if (code === 'KeyW') return 'w'
      if (code === 'KeyA') return 'a'
      if (code === 'KeyS') return 's'
      if (code === 'KeyD') return 'd'
      if (code === 'ArrowUp') return 'ArrowUp'
      if (code === 'ArrowDown') return 'ArrowDown'
      if (code === 'ArrowLeft') return 'ArrowLeft'
      if (code === 'ArrowRight') return 'ArrowRight'
      if (code === 'Space') return ' '
      return null
    }

    const onKey = (kind: 'keydown' | 'keyup') => (event: KeyboardEvent) => {
      const normalized = fromCode(event.code)
      if (!normalized) return
      const frameWindow = frameRef.current?.contentWindow as (Window & { document: Document }) | null
      if (!frameWindow || !frameWindow.document) return

      // Universal bridge for embedded games.
      frameWindow.postMessage({ type: 'quiz_key', kind, key: normalized, code: event.code }, '*')

      // Backward compatibility for games that bind document.onkeydown directly.
      const handler = kind === 'keydown' ? frameWindow.document.onkeydown : frameWindow.document.onkeyup
      if (typeof handler === 'function') {
        handler.call(frameWindow.document, {
          keyCode: normalized === ' ' ? 32 : normalized === 'w' ? 87 : normalized === 'a' ? 65 : normalized === 's' ? 83 : normalized === 'd' ? 68 :
            normalized === 'ArrowUp' ? 38 : normalized === 'ArrowDown' ? 40 : normalized === 'ArrowLeft' ? 37 : 39,
          preventDefault: () => event.preventDefault(),
        } as any)
      }
    }

    const down = onKey('keydown')
    const up = onKey('keyup')
    window.addEventListener('keydown', down)
    window.addEventListener('keyup', up)
    return () => {
      window.removeEventListener('keydown', down)
      window.removeEventListener('keyup', up)
    }
  }, [mode])

  return (
    <div className="w-full overflow-hidden rounded-xl border border-emerald-950/20 bg-black/90">
      <div className="relative aspect-[16/9] w-full">
        <iframe
          ref={frameRef}
          key={`${mode}:${src}`}
          title={`game-${mode}`}
          src={src}
          className="absolute inset-0 h-full w-full border-0"
          allow="autoplay"
          onLoad={() => frameRef.current?.contentWindow?.focus()}
        />
      </div>
    </div>
  )
}
