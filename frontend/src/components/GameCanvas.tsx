import { useEffect, useMemo, useRef, useState } from 'react'

type Props = {
  mode: 'platformer' | 'shooter'
  onTrigger: (reason: 'death' | 'level_up') => void
  paused?: boolean
  fullscreen?: boolean
}

const SRC_BY_MODE: Record<Props['mode'], string> = {
  platformer: '/games/onoff_game/index.html',
  shooter: '/games/underrun_src/index.html',
}

export function GameCanvas({ mode, onTrigger, paused = false, fullscreen = false }: Props) {
  const lastEmitRef = useRef(0)
  const frameRef = useRef<HTMLIFrameElement | null>(null)
  const pressedRef = useRef<Set<string>>(new Set())
  const joystickRef = useRef<HTMLDivElement | null>(null)
  const [showTouchControls, setShowTouchControls] = useState(false)
  const [stick, setStick] = useState({ x: 0, y: 0, active: false })
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

  const emitGameMessage = (payload: Record<string, unknown>) => {
    frameRef.current?.contentWindow?.postMessage(payload, '*')
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
    const detectMobile = () => {
      const coarse = window.matchMedia('(pointer: coarse)').matches
      const touch = (navigator.maxTouchPoints ?? 0) > 0
      const ua = /Android|iPhone|iPad|iPod|Mobile/i.test(navigator.userAgent)
      return coarse || touch || ua || window.innerWidth < 1024
    }
    const updateTouchMode = () => {
      setShowTouchControls(detectMobile())
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
      setStick({ x: 0, y: 0, active: false })
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

  const updateJoystick = (nx: number, ny: number, active: boolean) => {
    if (mode !== 'shooter' && mode !== 'platformer') return
    const threshold = 0.34
    if (!active || paused) {
      release('ArrowLeft', 'ArrowLeft')
      release('ArrowRight', 'ArrowRight')
      release('ArrowUp', 'ArrowUp')
      if (mode === 'shooter') release('ArrowDown', 'ArrowDown')
      if (mode === 'shooter') emitGameMessage({ type: 'quiz_aim', nx: 0, ny: 0 })
      setStick({ x: 0, y: 0, active: false })
      return
    }

    if (nx <= -threshold) press('ArrowLeft', 'ArrowLeft')
    else release('ArrowLeft', 'ArrowLeft')
    if (nx >= threshold) press('ArrowRight', 'ArrowRight')
    else release('ArrowRight', 'ArrowRight')
    if (ny <= -threshold) press('ArrowUp', 'ArrowUp')
    else release('ArrowUp', 'ArrowUp')
    if (mode === 'shooter') {
      if (ny >= threshold) press('ArrowDown', 'ArrowDown')
      else release('ArrowDown', 'ArrowDown')
    }

    if (mode === 'shooter') emitGameMessage({ type: 'quiz_aim', nx, ny })
    setStick({ x: nx * 24, y: ny * 24, active: true })
  }

  const processJoystickTouch = (touch: { clientX: number; clientY: number }) => {
    const base = joystickRef.current
    if (!base) return
    const rect = base.getBoundingClientRect()
    const cx = rect.left + rect.width / 2
    const cy = rect.top + rect.height / 2
    let dx = touch.clientX - cx
    let dy = touch.clientY - cy
    const radius = Math.min(rect.width, rect.height) / 2 - 8
    const distance = Math.hypot(dx, dy)
    if (distance > radius) {
      const k = radius / distance
      dx *= k
      dy *= k
    }
    updateJoystick(dx / radius, dy / radius, true)
  }

  return (
    <div className={fullscreen ? 'h-full w-full overflow-hidden bg-black' : 'w-full overflow-hidden rounded-xl border border-emerald-950/20 bg-black/90'}>
      <div className={fullscreen ? 'relative h-full w-full bg-black flex items-center justify-center' : 'relative aspect-[16/9] w-full'}>
        <div className={fullscreen ? 'relative h-full w-auto max-w-full aspect-[16/9] overflow-hidden' : 'relative h-full w-full overflow-hidden'}>
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
              {(mode === 'shooter' || mode === 'platformer') && (
                <div
                  ref={joystickRef}
                  className="relative h-28 w-28 rounded-full border border-white/45 bg-slate-900/45 backdrop-blur"
                  onTouchStart={(e) => { e.preventDefault(); processJoystickTouch(e.touches[0]) }}
                  onTouchMove={(e) => { e.preventDefault(); processJoystickTouch(e.touches[0]) }}
                  onTouchEnd={(e) => { e.preventDefault(); updateJoystick(0, 0, false) }}
                  onTouchCancel={(e) => { e.preventDefault(); updateJoystick(0, 0, false) }}
                >
                  <div className="pointer-events-none absolute inset-0 m-auto h-4 w-4 rounded-full bg-white/35" />
                  <div
                    className="pointer-events-none absolute h-9 w-9 rounded-full bg-white/85 shadow-md"
                    style={{ left: `calc(50% + ${stick.x}px - 18px)`, top: `calc(50% + ${stick.y}px - 18px)` }}
                  />
                </div>
              )}
            </div>
            <div className="absolute bottom-3 right-3 flex gap-2 pointer-events-auto">
              {mode === 'platformer' ? (
                <button
                  className="relative h-14 w-14 rounded-full ring-2 ring-white/75 shadow-lg active:scale-95"
                  style={{ background: 'conic-gradient(#f8fafc 0deg 180deg, #0f172a 180deg 360deg)' }}
                  onTouchStart={(e) => { e.preventDefault(); emitGameMessage({ type: 'quiz_toggle_color' }) }}
                >
                  <span className="absolute inset-0 flex items-center justify-center text-lg font-black text-sky-700">↔</span>
                </button>
              ) : (
                <button
                  className="relative h-14 w-14 rounded-full border-2 border-rose-100/90 bg-rose-500/90 text-xs font-black uppercase tracking-wide text-white shadow-xl active:scale-95"
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
    </div>
  )
}
