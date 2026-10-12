import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  installWindowLongPressDrag,
  WINDOW_DRAG_LONG_PRESS_MS,
} from './windowLongPressDrag'

function dispatchPointer(
  target: EventTarget,
  type: string,
  options: {
    button?: number
    clientX?: number
    clientY?: number
    pointerId?: number
    pointerType?: string
  } = {},
) {
  const event = new MouseEvent(type, {
    bubbles: true,
    button: options.button ?? 0,
    clientX: options.clientX ?? 0,
    clientY: options.clientY ?? 0,
  })
  Object.defineProperties(event, {
    pointerId: { value: options.pointerId ?? 1 },
    pointerType: { value: options.pointerType ?? 'mouse' },
    isPrimary: { value: true },
  })
  target.dispatchEvent(event)
}

describe('window long-press drag', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
    document.body.replaceChildren()
  })

  it('starts native window dragging after a mouse long press on blank space', async () => {
    const root = document.createElement('div')
    document.body.append(root)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    dispatchPointer(root, 'pointerdown')
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS - 1)
    expect(startDragging).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(1)
    expect(startDragging).toHaveBeenCalledTimes(1)
    cleanup()
  })

  it('allows blank-space dragging even when text elsewhere remains selected', async () => {
    const root = document.createElement('div')
    const paragraph = document.createElement('p')
    paragraph.textContent = '仍被选中的文字'
    root.append(paragraph)
    document.body.append(root)
    const selection = window.getSelection()
    const range = document.createRange()
    range.selectNodeContents(paragraph)
    selection?.removeAllRanges()
    selection?.addRange(range)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    dispatchPointer(root, 'pointerdown')
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)

    expect(startDragging).toHaveBeenCalledTimes(1)
    cleanup()
  })

  it('does not take over buttons, inputs, text, or explicitly blocked regions', async () => {
    const root = document.createElement('div')
    root.innerHTML = `
      <button type="button">操作</button>
      <input value="文字">
      <p>可选择文字</p>
      <div data-window-drag="off"><div class="blocked-child"></div></div>
    `
    document.body.append(root)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    for (const target of [
      root.querySelector('button'),
      root.querySelector('input'),
      root.querySelector('p'),
      root.querySelector('.blocked-child'),
    ]) {
      dispatchPointer(target!, 'pointerdown')
      await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)
    }

    expect(startDragging).not.toHaveBeenCalled()
    cleanup()
  })

  it('cancels before activation when the pointer moves beyond the tolerance', async () => {
    const root = document.createElement('div')
    document.body.append(root)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    dispatchPointer(root, 'pointerdown', { clientX: 10, clientY: 10 })
    dispatchPointer(window, 'pointermove', { clientX: 17, clientY: 10 })
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)

    expect(startDragging).not.toHaveBeenCalled()
    cleanup()
  })

  it('cancels when the mouse is released before the long press completes', async () => {
    const root = document.createElement('div')
    document.body.append(root)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    dispatchPointer(root, 'pointerdown')
    dispatchPointer(window, 'pointerup')
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)

    expect(startDragging).not.toHaveBeenCalled()
    cleanup()
  })

  it('ignores touch and secondary mouse buttons', async () => {
    const root = document.createElement('div')
    document.body.append(root)
    const startDragging = vi.fn().mockResolvedValue(undefined)
    const cleanup = installWindowLongPressDrag(root, startDragging)

    dispatchPointer(root, 'pointerdown', { pointerType: 'touch' })
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)
    dispatchPointer(root, 'pointerdown', { button: 2 })
    await vi.advanceTimersByTimeAsync(WINDOW_DRAG_LONG_PRESS_MS)

    expect(startDragging).not.toHaveBeenCalled()
    cleanup()
  })
})
