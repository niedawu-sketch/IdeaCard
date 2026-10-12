type TextControl = HTMLInputElement | HTMLTextAreaElement

export interface VoiceInputTargetSession {
  applyDraft(text: string): void
  commit(): void
  rollback(): void
}

export interface VoiceInputTarget {
  beginSession(): VoiceInputTargetSession | null
  refreshSelection?(): void
}

export interface VoiceInputTargetRect {
  x: number
  y: number
  width: number
  height: number
}

interface TargetEntry {
  element: HTMLElement
  target: VoiceInputTarget
}

interface ActiveSession {
  id: number
  targetToken: string
  session: VoiceInputTargetSession
}

const supportedInputTypes = new Set(['email', 'search', 'tel', 'text', 'url'])

let nextTargetId = 0

function createTargetToken() {
  nextTargetId += 1
  return `voice-target-${nextTargetId}`
}

function isEditableTextControl(target: EventTarget | null): target is TextControl {
  if (target instanceof HTMLTextAreaElement) {
    return !target.disabled && !target.readOnly
  }
  return target instanceof HTMLInputElement
    && supportedInputTypes.has(target.type)
    && !target.disabled
    && !target.readOnly
}

function normalizeDraft(target: TextControl, text: string) {
  return target instanceof HTMLInputElement ? text.replace(/[\r\n]+/g, ' ') : text
}

function dispatchInput(target: TextControl, inputType: string, data: string | null) {
  const event = typeof InputEvent === 'function'
    ? new InputEvent('input', { bubbles: true, data, inputType })
    : new Event('input', { bubbles: true })
  target.dispatchEvent(event)
}

function createTextControlTarget(target: TextControl): VoiceInputTarget {
  let cachedStart = target.selectionStart ?? target.value.length
  let cachedEnd = target.selectionEnd ?? cachedStart

  function refreshSelection() {
    if (!target.isConnected || !isEditableTextControl(target)) return
    cachedStart = target.selectionStart ?? target.value.length
    cachedEnd = target.selectionEnd ?? cachedStart
  }

  return {
    refreshSelection,
    beginSession() {
      if (!target.isConnected || !isEditableTextControl(target)) return null

      const start = cachedStart
      const originalEnd = cachedEnd
      const originalSelection = target.value.slice(start, originalEnd)
      let currentEnd = originalEnd
      let currentDraft = ''

      function replaceCurrent(text: string, inputType: string) {
        const nextDraft = normalizeDraft(target, text)
        target.setRangeText(nextDraft, start, currentEnd, 'end')
        currentEnd = start + nextDraft.length
        currentDraft = nextDraft
        dispatchInput(target, inputType, nextDraft)
      }

      return {
        applyDraft(text) {
          if (!target.isConnected || !isEditableTextControl(target)) return
          replaceCurrent(text, 'insertCompositionText')
        },
        commit() {
          if (!target.isConnected || !isEditableTextControl(target)) return
          target.focus({ preventScroll: true })
          target.setSelectionRange(start + currentDraft.length, start + currentDraft.length)
          refreshSelection()
        },
        rollback() {
          if (!target.isConnected || !isEditableTextControl(target)) return
          target.setRangeText(originalSelection, start, currentEnd, 'end')
          dispatchInput(target, 'insertCompositionText', originalSelection)
          target.focus({ preventScroll: true })
          target.setSelectionRange(originalEnd, originalEnd)
          refreshSelection()
        },
      }
    },
  }
}

export class VoiceInputTargetRegistry {
  private readonly targets = new Map<string, TargetEntry>()
  private readonly controlTokens = new WeakMap<TextControl, string>()
  private readonly composingElements = new WeakSet<HTMLElement>()
  private readonly subscribers = new Set<(
    targetToken: string | null,
    targetRect: VoiceInputTargetRect | null,
  ) => void>()
  private activeToken: string | null = null
  private activeSession: ActiveSession | null = null

  registerCustomTarget(element: HTMLElement, target: VoiceInputTarget) {
    const token = createTargetToken()
    this.targets.set(token, { element, target })
    if (element.contains(document.activeElement)) {
      this.setActiveToken(token)
    }
    return {
      token,
      unregister: () => {
        this.targets.delete(token)
        if (this.activeToken === token) this.setActiveToken(null)
      },
    }
  }

  trackDomTargets(onTargetChange: (
    targetToken: string | null,
    targetRect: VoiceInputTargetRect | null,
  ) => void) {
    const unsubscribe = this.subscribe(onTargetChange)

    const activateFromEvent = (event: Event) => {
      const eventTarget = event.target
      if (isEditableTextControl(eventTarget)) {
        if (this.composingElements.has(eventTarget)) {
          this.setActiveToken(null)
          return
        }
        const token = this.ensureControlTarget(eventTarget)
        this.targets.get(token)?.target.refreshSelection?.()
        this.setActiveToken(token)
        return
      }
      if (eventTarget instanceof Node) {
        const custom = this.findCustomTarget(eventTarget)
        if (custom) {
          const element = this.targets.get(custom)?.element
          if (element && this.composingElements.has(element)) {
            this.setActiveToken(null)
            return
          }
          this.setActiveToken(custom)
          return
        }
      }
      this.setActiveToken(null)
    }

    document.addEventListener('focusin', activateFromEvent, true)
    document.addEventListener('pointerdown', activateFromEvent, true)
    const reactivateFromEvent = (event: Event) => {
      const previousToken = this.activeToken
      activateFromEvent(event)
      if (this.activeToken === previousToken) this.notifyActiveToken()
    }
    document.addEventListener('keyup', reactivateFromEvent, true)
    document.addEventListener('pointerup', reactivateFromEvent, true)
    const handleCompositionStart = (event: CompositionEvent) => {
      const element = this.findTargetElement(event.target)
      if (!element) return
      this.composingElements.add(element)
      this.setActiveToken(null)
    }
    const handleCompositionEnd = (event: CompositionEvent) => {
      const element = this.findTargetElement(event.target)
      if (!element) return
      this.composingElements.delete(element)
      activateFromEvent(event)
    }
    document.addEventListener('compositionstart', handleCompositionStart, true)
    document.addEventListener('compositionend', handleCompositionEnd, true)

    const activeElement = document.activeElement
    if (isEditableTextControl(activeElement)) {
      this.setActiveToken(this.ensureControlTarget(activeElement))
    } else if (activeElement instanceof Node) {
      this.setActiveToken(this.findCustomTarget(activeElement))
    }

    return () => {
      unsubscribe()
      document.removeEventListener('focusin', activateFromEvent, true)
      document.removeEventListener('pointerdown', activateFromEvent, true)
      document.removeEventListener('keyup', reactivateFromEvent, true)
      document.removeEventListener('pointerup', reactivateFromEvent, true)
      document.removeEventListener('compositionstart', handleCompositionStart, true)
      document.removeEventListener('compositionend', handleCompositionEnd, true)
    }
  }

  beginSession(id: number, targetToken: string) {
    if (this.activeSession) this.rollbackSession(this.activeSession.id)
    const entry = this.targets.get(targetToken)
    if (!entry || this.composingElements.has(entry.element)) return false
    const target = entry.target
    const session = target?.beginSession()
    if (!session) return false
    this.activeSession = { id, targetToken, session }
    return true
  }

  applyDraft(id: number, text: string) {
    if (this.activeSession?.id !== id) return false
    this.activeSession.session.applyDraft(text)
    return true
  }

  commitSession(id: number) {
    if (this.activeSession?.id !== id) return false
    const activeSession = this.activeSession
    this.activeSession = null
    activeSession.session.commit()
    this.setActiveToken(activeSession.targetToken)
    return true
  }

  rollbackSession(id: number) {
    if (this.activeSession?.id !== id) return false
    const activeSession = this.activeSession
    this.activeSession = null
    activeSession.session.rollback()
    this.setActiveToken(activeSession.targetToken)
    return true
  }

  resetForTests() {
    if (this.activeSession) this.activeSession.session.rollback()
    this.activeSession = null
    this.targets.clear()
    this.activeToken = null
    this.subscribers.clear()
  }

  private ensureControlTarget(target: TextControl) {
    const existing = this.controlTokens.get(target)
    if (existing) return existing
    const token = createTargetToken()
    this.controlTokens.set(target, token)
    this.targets.set(token, {
      element: target,
      target: createTextControlTarget(target),
    })
    return token
  }

  private findCustomTarget(node: Node) {
    for (const [token, entry] of this.targets) {
      if (!(entry.element instanceof HTMLInputElement)
        && !(entry.element instanceof HTMLTextAreaElement)
        && entry.element.contains(node)) {
        return token
      }
    }
    return null
  }

  private findTargetElement(target: EventTarget | null) {
    if (isEditableTextControl(target)) return target
    if (!(target instanceof Node)) return null
    const token = this.findCustomTarget(target)
    return token ? this.targets.get(token)?.element ?? null : null
  }

  private subscribe(subscriber: (
    targetToken: string | null,
    targetRect: VoiceInputTargetRect | null,
  ) => void) {
    this.subscribers.add(subscriber)
    subscriber(
      this.activeToken,
      this.activeToken
        ? this.targetRect(this.targets.get(this.activeToken)?.element)
        : null,
    )
    return () => this.subscribers.delete(subscriber)
  }

  private setActiveToken(targetToken: string | null) {
    if (this.activeToken === targetToken) return
    this.activeToken = targetToken
    this.notifyActiveToken()
  }

  private notifyActiveToken() {
    const targetRect = this.activeToken
      ? this.targetRect(this.targets.get(this.activeToken)?.element)
      : null
    for (const subscriber of this.subscribers) subscriber(this.activeToken, targetRect)
  }

  private targetRect(element: HTMLElement | undefined): VoiceInputTargetRect | null {
    if (!element?.isConnected) return null
    const rect = element.getBoundingClientRect()
    return {
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
    }
  }
}

export const voiceInputTargets = new VoiceInputTargetRegistry()
