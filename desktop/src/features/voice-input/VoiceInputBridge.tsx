import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useEffect } from 'react'

import { useDesktopService } from '../../services/desktop/useDesktopService'
import { voiceInputTargets } from './voiceInputTargets'

interface StartedPayload {
  sessionId: number
  targetToken: string
}

interface DraftPayload {
  sessionId: number
  text: string
}

interface ReleasedPayload {
  sessionId: number
  heldMs: number
}

interface CancelledPayload {
  sessionId: number
}

interface ActiveVoiceSession {
  id: number
  draft: string
  released: boolean
}

const QUICK_TAP_THRESHOLD_MS = 180
const FINAL_DRAFT_STABILITY_MS = 450
const FINAL_DRAFT_TIMEOUT_MS = 8_000
const TARGET_PREARM_DELAY_MS = 16

function isTauriRuntime() {
  return '__TAURI_INTERNALS__' in window
}

export function VoiceInputBridge() {
  const service = useDesktopService()

  useEffect(() => {
    if (
      !isTauriRuntime()
      || !service.setVoiceInputTarget
      || !service.finishVoiceInputSession
    ) {
      return
    }

    let disposed = false
    let activeSession: ActiveVoiceSession | null = null
    let stabilityTimer: number | null = null
    let timeoutTimer: number | null = null
    let targetSyncTimer: number | null = null
    const unlisteners: UnlistenFn[] = []

    function clearTimers() {
      if (stabilityTimer !== null) window.clearTimeout(stabilityTimer)
      if (timeoutTimer !== null) window.clearTimeout(timeoutTimer)
      stabilityTimer = null
      timeoutTimer = null
    }

    function finishSession(sessionId: number, commit: boolean, notifyNative = true) {
      if (activeSession?.id !== sessionId) return
      clearTimers()
      if (commit) {
        voiceInputTargets.commitSession(sessionId)
      } else {
        voiceInputTargets.rollbackSession(sessionId)
      }
      activeSession = null
      if (notifyNative) {
        void service.finishVoiceInputSession?.(sessionId).catch(() => undefined)
      }
    }

    function scheduleStableFinish(sessionId: number) {
      if (stabilityTimer !== null) window.clearTimeout(stabilityTimer)
      stabilityTimer = window.setTimeout(() => {
        if (activeSession?.id === sessionId && activeSession.released) {
          finishSession(sessionId, activeSession.draft.length > 0)
        }
      }, FINAL_DRAFT_STABILITY_MS)
    }

    function handleStarted(payload: StartedPayload) {
      if (activeSession) finishSession(activeSession.id, false)
      if (!voiceInputTargets.beginSession(payload.sessionId, payload.targetToken)) {
        void service.finishVoiceInputSession?.(payload.sessionId).catch(() => undefined)
        return
      }
      activeSession = {
        id: payload.sessionId,
        draft: '',
        released: false,
      }
    }

    function handleDraft(payload: DraftPayload) {
      if (activeSession?.id !== payload.sessionId) return
      if (!voiceInputTargets.applyDraft(payload.sessionId, payload.text)) return
      activeSession.draft = payload.text
      if (activeSession.released) scheduleStableFinish(payload.sessionId)
    }

    function handleReleased(payload: ReleasedPayload) {
      if (activeSession?.id !== payload.sessionId) return
      activeSession.released = true
      if (payload.heldMs < QUICK_TAP_THRESHOLD_MS && activeSession.draft.length === 0) {
        finishSession(payload.sessionId, false)
        return
      }
      timeoutTimer = window.setTimeout(() => {
        if (activeSession?.id === payload.sessionId) {
          finishSession(payload.sessionId, activeSession.draft.length > 0)
        }
      }, FINAL_DRAFT_TIMEOUT_MS)
      if (activeSession.draft.length > 0) scheduleStableFinish(payload.sessionId)
    }

    function handleCancelled(payload: CancelledPayload) {
      finishSession(payload.sessionId, false, false)
    }

    const stopTracking = voiceInputTargets.trackDomTargets((targetToken, targetRect) => {
      if (targetSyncTimer !== null) window.clearTimeout(targetSyncTimer)
      targetSyncTimer = window.setTimeout(() => {
        targetSyncTimer = null
        void service.setVoiceInputTarget?.(targetToken, targetRect).catch(() => undefined)
      }, targetToken ? TARGET_PREARM_DELAY_MS : 0)
    })
    const listenerPromise = Promise.all([
      listen<StartedPayload>('voice-input://started', (event) => handleStarted(event.payload)),
      listen<DraftPayload>('voice-input://draft', (event) => handleDraft(event.payload)),
      listen<ReleasedPayload>('voice-input://released', (event) => handleReleased(event.payload)),
      listen<CancelledPayload>('voice-input://cancelled', (event) => handleCancelled(event.payload)),
    ]).then((registered) => {
      if (disposed) {
        for (const unlisten of registered) unlisten()
      } else {
        unlisteners.push(...registered)
      }
    }).catch(() => undefined)

    return () => {
      disposed = true
      clearTimers()
      if (targetSyncTimer !== null) window.clearTimeout(targetSyncTimer)
      stopTracking()
      if (activeSession) {
        const sessionId = activeSession.id
        voiceInputTargets.rollbackSession(sessionId)
        void service.finishVoiceInputSession?.(sessionId).catch(() => undefined)
        activeSession = null
      }
      for (const unlisten of unlisteners) unlisten()
      void listenerPromise
    }
  }, [service])

  return null
}
