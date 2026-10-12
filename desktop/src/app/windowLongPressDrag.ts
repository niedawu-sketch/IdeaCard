import { getCurrentWindow } from '@tauri-apps/api/window'

export const WINDOW_DRAG_LONG_PRESS_MS = 260
export const WINDOW_DRAG_MOVE_TOLERANCE_PX = 6

type StartDragging = () => Promise<void> | void

const BLOCKED_TARGET_SELECTOR = [
  'a',
  'button',
  'input',
  'textarea',
  'select',
  'option',
  'label',
  'summary',
  'details',
  'iframe',
  'object',
  'embed',
  '[contenteditable]:not([contenteditable="false"])',
  '[draggable="true"]',
  '[data-window-drag="off"]',
  '[role="button"]',
  '[role="link"]',
  '[role="textbox"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="slider"]',
  '[role="spinbutton"]',
  '[role="tab"]',
  '[role="menu"]',
  '[role="menuitem"]',
  '[role="listbox"]',
  '[role="option"]',
  '[role="dialog"]',
  '[aria-modal="true"]',
  '.rich-editor',
  '.text-context-menu',
  '.settings-entry-menu',
].join(',')

const TEXT_OR_MEDIA_SELECTOR = [
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'p',
  'span',
  'strong',
  'small',
  'em',
  'time',
  'img',
  'svg',
  'canvas',
  'video',
  'audio',
  'pre',
  'code',
  'blockquote',
  'td',
  'th',
].join(',')

function isBlankDragTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) return false
  if (target.closest(BLOCKED_TARGET_SELECTOR)) return false
  if (target.closest(TEXT_OR_MEDIA_SELECTOR)) return false
  return true
}

export function installWindowLongPressDrag(
  root: HTMLElement,
  startDragging: StartDragging,
) {
  let pending:
    | {
        pointerId: number
        startX: number
        startY: number
        timer: number
      }
    | undefined

  function cancelPending() {
    if (!pending) return
    window.clearTimeout(pending.timer)
    pending = undefined
  }

  function handlePointerDown(event: PointerEvent) {
    cancelPending()
    if (
      event.button !== 0
      || event.pointerType !== 'mouse'
      || event.isPrimary === false
      || event.altKey
      || event.ctrlKey
      || event.metaKey
      || event.shiftKey
      || !isBlankDragTarget(event.target)
    ) {
      return
    }

    const press = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      timer: 0,
    }
    press.timer = window.setTimeout(() => {
      if (pending !== press) return
      pending = undefined
      try {
        void Promise.resolve(startDragging()).catch(() => undefined)
      } catch {
        // The web prototype and a closing desktop window can reject native dragging.
      }
    }, WINDOW_DRAG_LONG_PRESS_MS)
    pending = press
  }

  function handlePointerMove(event: PointerEvent) {
    if (!pending || event.pointerId !== pending.pointerId) return
    const distance = Math.hypot(
      event.clientX - pending.startX,
      event.clientY - pending.startY,
    )
    if (distance > WINDOW_DRAG_MOVE_TOLERANCE_PX) cancelPending()
  }

  function handlePointerEnd(event: PointerEvent) {
    if (pending && event.pointerId === pending.pointerId) cancelPending()
  }

  function handleVisibilityChange() {
    if (document.visibilityState !== 'visible') cancelPending()
  }

  root.addEventListener('pointerdown', handlePointerDown, true)
  window.addEventListener('pointermove', handlePointerMove, true)
  window.addEventListener('pointerup', handlePointerEnd, true)
  window.addEventListener('pointercancel', handlePointerEnd, true)
  window.addEventListener('blur', cancelPending)
  document.addEventListener('visibilitychange', handleVisibilityChange)

  return () => {
    cancelPending()
    root.removeEventListener('pointerdown', handlePointerDown, true)
    window.removeEventListener('pointermove', handlePointerMove, true)
    window.removeEventListener('pointerup', handlePointerEnd, true)
    window.removeEventListener('pointercancel', handlePointerEnd, true)
    window.removeEventListener('blur', cancelPending)
    document.removeEventListener('visibilitychange', handleVisibilityChange)
  }
}

export function installTauriWindowLongPressDrag(root: HTMLElement) {
  if (!('__TAURI_INTERNALS__' in window)) return undefined
  return installWindowLongPressDrag(root, () => getCurrentWindow().startDragging())
}
