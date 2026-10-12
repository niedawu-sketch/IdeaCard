import { act, render, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { DesktopService } from '../../services/desktop/contracts'
import { DesktopServiceContext } from '../../services/desktop/useDesktopService'
import { NativeTextField } from './NativeTextField'

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => undefined),
}))

class TestResizeObserver {
  static callbacks: ResizeObserverCallback[] = []

  constructor(callback: ResizeObserverCallback) {
    TestResizeObserver.callbacks.push(callback)
  }

  observe() {}
  disconnect() {}

  static notifyLayoutChange() {
    for (const callback of TestResizeObserver.callbacks) {
      callback([], {} as ResizeObserver)
    }
  }
}

describe('NativeTextField', () => {
  beforeEach(() => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })
    vi.stubGlobal('ResizeObserver', TestResizeObserver)
    TestResizeObserver.callbacks = []
  })

  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    vi.unstubAllGlobals()
  })

  it('resyncs its native position when streaming text moves the dialog host', async () => {
    const syncNativeTextField = vi.fn().mockResolvedValue(undefined)
    const service = {
      syncNativeTextField,
      removeNativeTextField: vi.fn().mockResolvedValue(undefined),
      focusNativeTextField: vi.fn().mockResolvedValue(undefined),
    } as unknown as DesktopService
    const { getByRole, getByTestId } = render(
      <DesktopServiceContext.Provider value={service}>
        <section>
          <NativeTextField
            fieldId="concept-explain-name-field"
            value="TikTok"
            onChange={vi.fn()}
          />
          <p data-testid="streaming-explanation">解释开始</p>
        </section>
      </DesktopServiceContext.Provider>,
    )
    const input = getByRole('textbox')
    let y = 220
    vi.spyOn(input, 'getBoundingClientRect').mockImplementation(() => ({
      x: 36,
      y,
      top: y,
      right: 336,
      bottom: y + 36,
      left: 36,
      width: 300,
      height: 36,
      toJSON: () => ({}),
    }))
    vi.mocked(document.elementFromPoint).mockReturnValue(input)
    window.dispatchEvent(new Event('resize'))

    await waitFor(() => {
      expect(syncNativeTextField).toHaveBeenLastCalledWith(expect.objectContaining({ y: 220 }))
    })

    y = 96
    getByTestId('streaming-explanation').firstChild!.textContent = '持续追加后的完整解释正文'
    act(() => TestResizeObserver.notifyLayoutChange())

    await waitFor(() => {
      expect(syncNativeTextField).toHaveBeenLastCalledWith(expect.objectContaining({ y: 96 }))
    })
  })
  it('does not resync unchanged input when autosave status rerenders the parent', async () => {
    const sync = vi.fn().mockResolvedValue(undefined)
    const service = {
      syncNativeTextField: sync,
      removeNativeTextField: vi.fn().mockResolvedValue(undefined),
      focusNativeTextField: vi.fn().mockResolvedValue(undefined),
    } as unknown as DesktopService
    const view = (status: string, value = '正在输入') => (
      <DesktopServiceContext.Provider value={service}>
        <NativeTextField fieldId="draft" value={value} onChange={() => undefined} />
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
