import { describe, expect, it, vi } from 'vitest'
import { createEmptyCaptureDraft, simulateCapture } from './captureAdapter'

describe('simulateCapture', () => {
  it('classifies an empty quick capture as a personal source', () => {
    const draft = createEmptyCaptureDraft('quick')

    expect(draft.sourceType).toBe('personal')
    expect(draft).not.toHaveProperty('tags')
  })

  it('returns the clipboard demo card after the simulated delay', async () => {
    vi.useFakeTimers()

    const capture = simulateCapture('clipboard', 'success')
    await vi.advanceTimersByTimeAsync(449)
    let settled = false
    capture.then(() => {
      settled = true
    })
    await Promise.resolve()

    expect(settled).toBe(false)

    await vi.advanceTimersByTimeAsync(1)

    await expect(capture).resolves.toMatchObject({
      title: '剪贴板摘录',
      content: 'AI 总结需要引用原始卡片，不能只给无来源结论。',
      note: '',
      type: 'clipboard',
      sourceType: 'personal',
      sourceName: '系统剪贴板',
    })
    await expect(capture).resolves.not.toHaveProperty('tags')

    vi.useRealTimers()
  })

  it('classifies an empty screenshot capture as an image source', () => {
    const draft = createEmptyCaptureDraft('screenshot')

    expect(draft.sourceType).toBe('image')
    expect(draft).not.toHaveProperty('tags')
  })

  it('rejects the screenshot failure demo with the OCR message', async () => {
    vi.useFakeTimers()

    const capture = simulateCapture('screenshot', 'failure')
    const expectation = expect(capture).rejects.toThrow('未识别到清晰文字')
    await vi.advanceTimersByTimeAsync(450)

    await expectation

    vi.useRealTimers()
  })
})
