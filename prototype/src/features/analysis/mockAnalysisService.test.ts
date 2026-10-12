import { describe, expect, it, vi } from 'vitest'
import { seedCards } from '../../store/seed'
import { runMockAnalysis } from './mockAnalysisService'

const cards = seedCards.slice(0, 3)

function createInput(
  outcome: 'success' | 'failure',
  signal = new AbortController().signal,
) {
  return {
    cards,
    goal: '归纳阅读沉淀的关键因素',
    outcome,
    signal,
  }
}

describe('runMockAnalysis', () => {
  it('returns structured output with references limited to input cards', async () => {
    vi.useFakeTimers()
    try {
      const result = runMockAnalysis(createInput('success'))

      await vi.advanceTimersByTimeAsync(1200)
      const output = await result

      expect(output.summary).toContain('归纳阅读沉淀的关键因素')
      expect(output.themes.length).toBeGreaterThan(0)
      expect(output.conflicts.length).toBeGreaterThan(0)
      expect(output.actions.length).toBeGreaterThan(0)
      expect(output.openQuestions.length).toBeGreaterThan(0)

      const allowedCardIds = new Set(cards.map((card) => card.id))
      for (const theme of output.themes) {
        expect(theme.references.length).toBeGreaterThan(0)
        for (const reference of theme.references) {
          expect(allowedCardIds.has(reference.cardId)).toBe(true)
        }
      }
    } finally {
      vi.useRealTimers()
    }
  })

  it('supports controlled failure', async () => {
    vi.useFakeTimers()
    try {
      const result = runMockAnalysis(createInput('failure'))
      const assertion = expect(result).rejects.toThrow('AI 延伸服务暂时不可用')

      await vi.advanceTimersByTimeAsync(1200)
      await assertion
    } finally {
      vi.useRealTimers()
    }
  })

  it('rejects with AbortError when cancelled during analysis', async () => {
    vi.useFakeTimers()
    try {
      const controller = new AbortController()
      const result = runMockAnalysis(createInput('success', controller.signal))
      const assertion = expect(result).rejects.toMatchObject({
        name: 'AbortError',
      })

      controller.abort()
      await vi.advanceTimersByTimeAsync(50)
      await assertion
    } finally {
      vi.useRealTimers()
    }
  })
})
