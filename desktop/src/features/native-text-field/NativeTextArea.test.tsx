import { act, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { DesktopService } from '../../services/desktop/contracts'
import { DesktopServiceContext } from '../../services/desktop/useDesktopService'
import { NativeTextArea } from './NativeTextArea'

const tauriEvent = vi.hoisted(() => ({
  handlers: new Map<string, (event: { payload: unknown }) => void>(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (
    event: string,
    handler: (event: { payload: unknown }) => void,
  ) => {
    tauriEvent.handlers.set(event, handler)
    return () => tauriEvent.handlers.delete(event)
  }),
}))

class TestResizeObserver {
  observe() {}
  disconnect() {}
}

describe('NativeTextArea', () => {
  beforeEach(() => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    vi.stubGlobal('ResizeObserver', TestResizeObserver)
  })

  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    vi.unstubAllGlobals()
    tauriEvent.handlers.clear()
  })

  it('syncs the AI submit mode and forwards native focus and submit events', async () => {
    const syncNativeTextArea = vi.fn().mockResolvedValue(undefined)
    const removeNativeTextArea = vi.fn().mockResolvedValue(undefined)
    const focusNativeTextArea = vi.fn().mockResolvedValue(undefined)
    const onFocus = vi.fn()
    const onSubmit = vi.fn()
    const service = {
      syncNativeTextArea,
      removeNativeTextArea,
      focusNativeTextArea,
    } as unknown as DesktopService
    const { getByRole, unmount } = render(
      <DesktopServiceContext.Provider value={service}>
        <NativeTextArea
          fieldId="ai-question-field"
          nativeInsets={{ top: 5, right: 4, left: 4 }}
          submitOnEnter
          value="语音问题"
          onChange={vi.fn()}
          onFocus={onFocus}
          onSubmit={onSubmit}
        />
      </DesktopServiceContext.Provider>,
    )
    const textarea = getByRole('textbox')
    vi.spyOn(textarea, 'getBoundingClientRect').mockReturnValue({
      x: 20,
      y: 30,
      top: 30,
      right: 320,
      bottom: 74,
      left: 20,
      width: 300,
      height: 44,
      toJSON: () => ({}),
    })
    vi.mocked(document.elementFromPoint).mockReturnValue(textarea)
    window.dispatchEvent(new Event('resize'))

    await waitFor(() => {
      expect(syncNativeTextArea).toHaveBeenLastCalledWith({
        fieldId: 'ai-question-field',
        value: '语音问题',
        readOnly: false,
        submitOnEnter: true,
        visible: true,
        x: 24,
        y: 35,
        width: 292,
        height: 39,
      })
    })

    act(() => {
      tauriEvent.handlers.get('native-text-area://focus-changed')?.({
        payload: { fieldId: 'ai-question-field', focused: true },
      })
      tauriEvent.handlers.get('native-text-area://submitted')?.({
        payload: { fieldId: 'ai-question-field' },
      })
    })

    expect(onFocus).toHaveBeenCalledTimes(1)
    expect(onSubmit).toHaveBeenCalledTimes(1)

    unmount()
    expect(removeNativeTextArea).toHaveBeenCalledWith('ai-question-field')
  })
  it('does not resync unchanged input when autosave status rerenders the parent', async () => {
    const sync = vi.fn().mockResolvedValue(undefined)
    const service = {
      syncNativeTextArea: sync,
      removeNativeTextArea: vi.fn().mockResolvedValue(undefined),
      focusNativeTextArea: vi.fn().mockResolvedValue(undefined),
    } as unknown as DesktopService
    const view = (status: string, value = '正在输入') => (
      <DesktopServiceContext.Provider value={service}>
        <NativeTextArea fieldId="draft" value={value} onChange={() => undefined} />
        <span>{status}</span>
      </DesktopServiceContext.Provider>
    )
    const { rerender } = render(view('等待保存'))
    await waitFor(() => expect(sync).toHaveBeenCalledTimes(1))
    rerender(view('保存中'))
    window.dispatchEvent(new Event('resize'))
    await act(async () => {
      await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()))
    })
    expect(sync).toHaveBeenCalledTimes(1)
    rerender(view('已保存', '外部更新'))
    await waitFor(() => expect(sync).toHaveBeenCalledTimes(2))
    expect(sync).toHaveBeenLastCalledWith(expect.objectContaining({ value: '外部更新' }))
  })

})
