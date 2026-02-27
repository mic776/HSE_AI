import type { WsEnvelope } from '../types'

export function connectRoom(roomCode: string, onMessage: (msg: WsEnvelope) => void): WebSocket {
  const proto = window.location.protocol === 'https:' ? 'wss' : 'ws'
  const socket = new WebSocket(`${proto}://${window.location.host}/ws/sessions/${roomCode}`)
  socket.onmessage = (event) => {
    try {
      onMessage(JSON.parse(event.data) as WsEnvelope)
    } catch {
      // ignore malformed
    }
  }
  return socket
}

export function sendWs(socket: WebSocket, event: string, payload: Record<string, unknown>) {
  if (socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ event, payload }))
  }
}
