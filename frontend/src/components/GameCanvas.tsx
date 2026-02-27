import { useEffect, useMemo, useRef, useState } from 'react'

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
  const pressedRef = useRef<Set<string>>(new Set())
  const [showTouchControls, setShowTouchControls] = useState(false)
  const src = useMemo(() => SRC_BY_MODE[mode], [mode])

  const emitBridgeKey = (kind: 'keydown' | 'keyup', key: string, code?: string) => {
    const frameWindow = frameRef.current?.contentWindow as (Window & { document: Document }) | null
    if (!frameWindow || !frameWindow.document) return

    frameWindow.postMessage({ type: 'quiz_key', kind, key, code }, '*')

    const handler = kind === 'keydown' ? frameWindow.document.onkeydown : frameWindow.document.onkeyup
    if (typeof handler === 'function') {
      const keyCode = key === ' ' ? 32 : key === 'w' ? 87 : key === 'a' ? 65 : key === 's' ? 83 : key === 'd' ? 68 :
        key === 'ArrowUp' ? 38 : key === 'ArrowDown' ? 40 : key === 'ArrowLeft' ? 37 : 39
      handler.call(frameWindow.document, {
        keyCode,
        preventDefault: () => {},
      } as any)
    }
  }

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
    const updateTouchMode = () => {
      const coarse = window.matchMedia('(pointer: coarse)').matches
      setShowTouchControls(coarse || window.innerWidth < 900)
    }
    updateTouchMode()
    window.addEventListener('resize', updateTouchMode)
    return () => window.removeEventListener('resize', updateTouchMode)
  }, [])

  useEffect(() => {
    const target = frameRef.current?.contentWindow
    if (!target) return
    target.postMessage({ type: 'quiz_pause', paused }, '*')
    if (paused) {
      for (const composite of pressedRef.current) {
        const [key, code] = composite.split('|')
        emitBridgeKey('keyup', key, code || undefined)
      }
      pressedRef.current.clear()
    }
  }, [paused, src])

  useEffect(() => {
    if (paused) return
    const frame = frameRef.current
    if (!frame) return
    const focusGame = () => {
      frame.focus()
      frame.contentWindow?.focus()
    }
    // Focus immediately and once more shortly after DOM updates.
    focusGame()
    const t = window.setTimeout(focusGame, 60)
    return () => window.clearTimeout(t)
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
      emitBridgeKey(kind, normalized, event.code)
      event.preventDefault()
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

  const press = (key: string, code: string) => {
    if (paused) return
    const composite = `${key}|${code}`
    if (pressedRef.current.has(composite)) return
    pressedRef.current.add(composite)
    emitBridgeKey('keydown', key, code)
  }

  const release = (key: string, code: string) => {
    const composite = `${key}|${code}`
    if (!pressedRef.current.has(composite)) return
    pressedRef.current.delete(composite)
    emitBridgeKey('keyup', key, code)
  }

  const tap = (key: string, code: string) => {
    if (paused) return
    emitBridgeKey('keydown', key, code)
    window.setTimeout(() => emitBridgeKey('keyup', key, code), 35)
  }

  return (
    <div className="w-full overflow-hidden rounded-xl border border-emerald-950/20 bg-black/90">
      <div className="relative aspect-[16/9] w-full">
        <iframe
          ref={frameRef}
          key={`${mode}:${src}`}
          title={`game-${mode}`}
          src={src}
          tabIndex={0}
          className="absolute inset-0 h-full w-full border-0"
          allow="autoplay"
          onLoad={() => frameRef.current?.contentWindow?.focus()}
        />
        {showTouchControls && (
          <div className="pointer-events-none absolute inset-0 z-10 select-none" style={{ touchAction: 'none' }}>
            <div className="absolute bottom-3 left-3 flex gap-2 pointer-events-auto">
              <button
                className="h-12 w-12 rounded-full bg-white/80 text-xl font-black text-emerald-950 shadow active:scale-95"
                onTouchStart={(e) => { e.preventDefault(); press('ArrowLeft', 'ArrowLeft') }}
                onTouchEnd={(e) => { e.preventDefault(); release('ArrowLeft', 'ArrowLeft') }}
                onTouchCancel={(e) => { e.preventDefault(); release('ArrowLeft', 'ArrowLeft') }}
              >
                ◀
              </button>
              {mode === 'shooter' && (
                <button
                  className="h-12 w-12 rounded-full bg-white/80 text-xl font-black text-emerald-950 shadow active:scale-95"
                  onTouchStart={(e) => { e.preventDefault(); press('ArrowUp', 'ArrowUp') }}
                  onTouchEnd={(e) => { e.preventDefault(); release('ArrowUp', 'ArrowUp') }}
                  onTouchCancel={(e) => { e.preventDefault(); release('ArrowUp', 'ArrowUp') }}
                >
                  ▲
                </button>
              )}
              <button
                className="h-12 w-12 rounded-full bg-white/80 text-xl font-black text-emerald-950 shadow active:scale-95"
                onTouchStart={(e) => { e.preventDefault(); press('ArrowRight', 'ArrowRight') }}
                onTouchEnd={(e) => { e.preventDefault(); release('ArrowRight', 'ArrowRight') }}
                onTouchCancel={(e) => { e.preventDefault(); release('ArrowRight', 'ArrowRight') }}
              >
                ▶
              </button>
              {mode === 'shooter' && (
                <button
                  className="h-12 w-12 rounded-full bg-white/80 text-xl font-black text-emerald-950 shadow active:scale-95"
                  onTouchStart={(e) => { e.preventDefault(); press('ArrowDown', 'ArrowDown') }}
                  onTouchEnd={(e) => { e.preventDefault(); release('ArrowDown', 'ArrowDown') }}
                  onTouchCancel={(e) => { e.preventDefault(); release('ArrowDown', 'ArrowDown') }}
                >
                  ▼
                </button>
              )}
            </div>
            <div className="absolute bottom-3 right-3 flex gap-2 pointer-events-auto">
              {mode === 'platformer' ? (
                <>
                  <button
                    className="h-12 rounded-full bg-amber-200/90 px-4 text-sm font-bold text-amber-900 shadow active:scale-95"
                    onTouchStart={(e) => { e.preventDefault(); tap(' ', 'Space') }}
                  >
                    Цвет
                  </button>
                  <button
                    className="h-12 rounded-full bg-emerald-200/90 px-4 text-sm font-bold text-emerald-900 shadow active:scale-95"
                    onTouchStart={(e) => { e.preventDefault(); press('ArrowUp', 'ArrowUp') }}
                    onTouchEnd={(e) => { e.preventDefault(); release('ArrowUp', 'ArrowUp') }}
                    onTouchCancel={(e) => { e.preventDefault(); release('ArrowUp', 'ArrowUp') }}
                  >
                    Прыжок
                  </button>
                </>
              ) : (
                <button
                  className="h-12 rounded-full bg-rose-200/90 px-5 text-sm font-bold text-rose-900 shadow active:scale-95"
                  onTouchStart={(e) => { e.preventDefault(); press(' ', 'Space') }}
                  onTouchEnd={(e) => { e.preventDefault(); release(' ', 'Space') }}
                  onTouchCancel={(e) => { e.preventDefault(); release(' ', 'Space') }}
                >
                  Огонь
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
