import type { Card, NewCardInput } from '../../entities/card/model'

export type CaptureKind = 'quick' | 'clipboard' | 'screenshot'
export type DemoOutcome = 'success' | 'failure'

export type CaptureDraft = NewCardInput

const demoDrafts = {
  quick: {
    title: '',
    content: '',
    note: '',
    type: 'thought',
    sourceType: 'personal',
    sourceName: '快速记录',
    sourceUrl: '',
  },
  clipboard: {
    title: '剪贴板摘录',
    content: 'AI 总结需要引用原始卡片，不能只给无来源结论。',
    note: '',
    type: 'clipboard',
    sourceType: 'personal',
    sourceName: '系统剪贴板',
    sourceUrl: 'ideacard://demo/clipboard',
  },
  screenshot: {
    title: '截图中的产品观点',
    content: '捕获阶段不应要求用户立即决定内容最终归属。',
    note: '',
    type: 'screenshot',
    sourceType: 'image',
    sourceName: '屏幕截图 OCR',
    sourceUrl: 'ideacard://demo/screenshot',
  },
} satisfies Record<CaptureKind, CaptureDraft>

export function simulateCapture(
  kind: CaptureKind,
  outcome: DemoOutcome = 'success',
) {
  return new Promise<CaptureDraft>((resolve, reject) => {
    window.setTimeout(() => {
      if (kind === 'screenshot' && outcome === 'failure') {
        reject(new Error('未识别到清晰文字'))
        return
      }

      resolve({ ...demoDrafts[kind] })
    }, 450)
  })
}

export function createEmptyCaptureDraft(kind: CaptureKind): CaptureDraft {
  return { ...demoDrafts[kind] }
}

export function isDuplicateCard(cards: Card[], draft: CaptureDraft) {
  const sourceUrl = draft.sourceUrl.trim()
  const content = draft.content.trim()

  return cards.some((card) => {
    const hasSameSourceUrl =
      Boolean(sourceUrl) && card.sourceUrl.trim() === sourceUrl
    const hasSameContent =
      Boolean(content) && card.content.trim() === content

    return hasSameSourceUrl || hasSameContent
  })
}
