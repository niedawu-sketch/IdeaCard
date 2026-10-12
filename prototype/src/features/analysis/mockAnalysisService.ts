import type { AnalysisOutput } from '../../entities/analysis/model'
import type { Card } from '../../entities/card/model'

export interface MockAnalysisInput {
  cards: Card[]
  goal: string
  outcome: 'success' | 'failure'
  signal: AbortSignal
}

export async function runMockAnalysis({
  cards,
  goal,
  outcome,
  signal,
}: MockAnalysisInput): Promise<AnalysisOutput> {
  await waitForMockDelay(signal)

  if (outcome === 'failure') {
    throw new Error('AI 延伸服务暂时不可用')
  }

  return createMockOutput(cards, goal)
}

async function waitForMockDelay(signal: AbortSignal) {
  const interval = 50
  const total = 1200

  for (let elapsed = 0; elapsed < total; elapsed += interval) {
    throwIfAborted(signal)
    await new Promise((resolve) => window.setTimeout(resolve, interval))
  }

  throwIfAborted(signal)
}

function throwIfAborted(signal: AbortSignal) {
  if (!signal.aborted) {
    return
  }

  const error = new Error('分析已取消')
  error.name = 'AbortError'
  throw error
}

function createMockOutput(cards: Card[], goal: string): AnalysisOutput {
  const [firstCard, secondCard = firstCard, thirdCard = secondCard] = cards
  const primaryReferences = createReferences([firstCard, secondCard])
  const secondaryReferences = createReferences([secondCard, thirdCard])

  return {
    summary: `围绕「${goal}」，可以先把它当成一个待延伸的问题：当前材料已经给出方向，但还需要补齐概念、使用场景和下一步验证线索。`,
    themes: [
      {
        title: '从记录走向判断',
        detail:
          '这组材料共同指向一个方向：记录本身不是终点，真正要延伸的是记录之后如何形成判断。',
        references: primaryReferences,
      },
      {
        title: '把不清楚处拆成可查询项',
        detail:
          '可以把模糊概念拆成定义、使用场景和验证指标三类查询项，避免直接跳到结论。',
        references: secondaryReferences,
      },
    ],
    conflicts: [
      '需要查询：这个想法依赖的关键概念是否已经有成熟定义。',
      '需要确认：用户在记录后是否真的会回访，还是只完成捕获动作就结束。',
    ],
    actions: [
      '把当前想法改写成一个可追问的问题，再继续让 AI 补充反例和边界。',
      '选择一张最关键的来源卡片，围绕它查询相关概念或竞品做法。',
      '保留每条延伸建议背后的来源卡片，避免想法脱离上下文。',
    ],
    openQuestions: [
      '哪些概念需要外部搜索确认，而不是只靠当前卡片推断？',
      '这个想法下一步更适合做用户访谈、竞品查询，还是写成产品假设？',
    ],
  }
}

function createReferences(cards: Array<Card | undefined>) {
  const references = []
  const seenCardIds = new Set<string>()

  for (const card of cards) {
    if (!card || seenCardIds.has(card.id)) {
      continue
    }

    references.push({
      cardId: card.id,
      label: card.title,
    })
    seenCardIds.add(card.id)
  }

  return references
}
