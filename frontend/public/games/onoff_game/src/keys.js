export const DOWN = new Set
export const PRESSED = new Set

const keyFromCode = (code) => (
  code === 'KeyW' ? 'w'
  : code === 'KeyA' ? 'a'
  : code === 'KeyS' ? 's'
  : code === 'KeyD' ? 'd'
  : code === 'ArrowUp' ? 'ArrowUp'
  : code === 'ArrowDown' ? 'ArrowDown'
  : code === 'ArrowLeft' ? 'ArrowLeft'
  : code === 'ArrowRight' ? 'ArrowRight'
  : code === 'Space' ? ' '
  : null
)

const NO_DEFAULT = new Set([
  'w',
  'a',
  's',
  'd',
  ' ',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight'
])

export const upKey = () => (
  DOWN.has('w') || DOWN.has('ArrowUp') || PRESSED.has(0) || PRESSED.has(12)
)

export const leftKey = () => (
  DOWN.has('a') || DOWN.has('ArrowLeft') || PRESSED.has(14)
)

export const rightKey = () => (
  DOWN.has('d') || DOWN.has('ArrowRight') || PRESSED.has(15)
)

document.addEventListener('keydown', (event) => {
  const mapped = keyFromCode(event.code) || event.key
  DOWN.add(mapped)
  if (NO_DEFAULT.has(mapped)) event.preventDefault()
})

document.addEventListener('keyup', ({key, code}) => {
  DOWN.delete(keyFromCode(code) || key)
})

const HANDLERS = new Map
export const onPress = (index, f) => {
  if (!HANDLERS.has(index)) HANDLERS.set(index, [])
  HANDLERS.get(index).push(f)
}

requestAnimationFrame(function tick (time) {
  const pad = navigator.getGamepads()[0]
  if (!pad) {
    PRESSED.clear()
    return
  }
  pad.buttons.forEach((button, index) => {
    if (button.pressed) {
      if (!PRESSED.has(index)) {
        const handlers = HANDLERS.get(index)
        if (handlers) handlers.forEach((f) => f())
      }
      PRESSED.add(index)
    } else {
      PRESSED.delete(index)
    }
  })
  requestAnimationFrame(tick)
})

window.addEventListener('message', ({data}) => {
  if (!data || data.type !== 'quiz_key') return
  const key = data.key || keyFromCode(data.code)
  if (!key) return
  if (data.kind === 'keydown') {
    DOWN.add(key)
  } else if (data.kind === 'keyup') {
    DOWN.delete(key)
  }
})
