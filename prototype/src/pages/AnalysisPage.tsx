import { AlertTriangle, Play, RotateCcw, Trash2, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import type { Card } from '../entities/card/model'
import { AnalysisResult } from '../features/analysis/AnalysisResult'
import { runMockAnalysis } from '../features/analysis/mockAnalysisService'
import { Button } from '../shared/ui/Button'
import { useIdeaCardStore } from '../store/useIdeaCardStore'

export function AnalysisPage() {
  const { analysisId } = useParams()
  const navigate = useNavigate()
  const cards = useIdeaCardStore((state) => state.cards)
  const analyses = useIdeaCardStore((state) => state.analyses)
  const updateAnalysis = useIdeaCardStore((state) => state.updateAnalysis)
  const setAnalysisOutput = useIdeaCardStore(
    (state) => state.setAnalysisOutput,
  )
  const addFollowUp = useIdeaCardStore((state) => state.addFollowUp)
  const saveInsight = useIdeaCardStore((state) => state.saveInsight)
  const analysis = analyses.find((item) => item.id === analysisId)
  const abortControllerRef = useRef<AbortController | null>(null)
  const followUpRequestRef = useRef(0)
  const isMountedRef = useRef(true)
  const materialRefs = useRef<Record<string, HTMLLIElement | null>>({})
  const [highlightedCardId, setHighlightedCardId] = useState<string | null>(
    null,
  )
  const [errorMessage, setErrorMessage] = useState('AI 延伸服务暂时不可用')
  const [followUpQuestion, setFollowUpQuestion] = useState('')
  const [followUpCardId, setFollowUpCardId] = useState('')
  const [isSubmittingFollowUp, setIsSubmittingFollowUp] = useState(false)

  const materialCards = useMemo(() => {
    if (!analysis) {
      return []
    }

    const cardsById = new Map(cards.map((card) => [card.id, card]))
    return analysis.cardIds
      .map((cardId) => cardsById.get(cardId))
      .filter((card): card is Card => Boolean(card))
  }, [analysis, cards])

  useEffect(
    () => {
      isMountedRef.current = true

      return () => {
        isMountedRef.current = false
        followUpRequestRef.current += 1
        abortControllerRef.current?.abort()
      }
    },
    [],
  )

  if (!analysis) {
    return (
      <section className="page-section not-found" aria-labelledby="analysis-not-found-title">
        <h1 id="analysis-not-found-title">未找到分析集</h1>
        <p className="not-found-description">
          这个分析集可能已经不存在，返回 Inbox 后可以重新选择材料创建。
        </p>
        <Button onClick={() => navigate('/')}>返回 Inbox</Button>
      </section>
    )
  }

  const currentAnalysis = analysis
  const isRunning = currentAnalysis.status === 'running'
  const isInteractionLocked = isRunning || isSubmittingFollowUp
  const canStart = Boolean(currentAnalysis.goal.trim()) && !isInteractionLocked

  function setGoal(goal: string) {
    updateAnalysis(currentAnalysis.id, { goal })
  }

  function removeMaterial(cardId: string) {
    if (isInteractionLocked) {
      return
    }

    updateAnalysis(currentAnalysis.id, {
      cardIds: currentAnalysis.cardIds.filter((item) => item !== cardId),
    })
    if (highlightedCardId === cardId) {
      setHighlightedCardId(null)
    }
  }

  async function startAnalysis(outcome: 'success' | 'failure') {
    if (!currentAnalysis.goal.trim()) {
      return
    }

    const controller = new AbortController()
    abortControllerRef.current = controller
    setErrorMessage('AI 延伸服务暂时不可用')
    updateAnalysis(currentAnalysis.id, {
      goal: currentAnalysis.goal,
      status: 'running',
      output: undefined,
    })

    try {
      const output = await runMockAnalysis({
        cards: materialCards,
        goal: currentAnalysis.goal.trim(),
        outcome,
        signal: controller.signal,
      })

      setAnalysisOutput(currentAnalysis.id, output)
    } catch (error) {
      if (error instanceof Error && error.name === 'AbortError') {
        updateAnalysis(currentAnalysis.id, { status: 'cancelled' })
        return
      }

      setErrorMessage(
        error instanceof Error ? error.message : 'AI 延伸服务暂时不可用',
      )
      updateAnalysis(currentAnalysis.id, { status: 'failed' })
    } finally {
      if (abortControllerRef.current === controller) {
        abortControllerRef.current = null
      }
    }
  }

  function cancelAnalysis() {
    abortControllerRef.current?.abort()
    updateAnalysis(currentAnalysis.id, { status: 'cancelled' })
  }

  function focusReference(cardId: string) {
    setHighlightedCardId(cardId)
    const element = materialRefs.current[cardId]
    if (element && typeof element.scrollIntoView === 'function') {
      element.scrollIntoView({ block: 'nearest' })
    }
  }

  async function submitFollowUp() {
    const question = followUpQuestion.trim()

    if (!question || isSubmittingFollowUp) {
      return
    }

    const requestId = followUpRequestRef.current + 1
    const cardId = followUpCardId || undefined

    followUpRequestRef.current = requestId
    setIsSubmittingFollowUp(true)
    try {
      await new Promise((resolve) => window.setTimeout(resolve, 500))

      if (!isMountedRef.current || followUpRequestRef.current !== requestId) {
        return
      }

      addFollowUp(currentAnalysis.id, {
        question,
        answer: '优先验证快速记录后的回访处理率。',
        cardId,
      })
      setFollowUpQuestion('')
      setFollowUpCardId('')
    } finally {
      if (isMountedRef.current && followUpRequestRef.current === requestId) {
        setIsSubmittingFollowUp(false)
      }
    }
  }

  function saveCurrentInsight() {
    if (isSubmittingFollowUp) {
      return
    }

    const insightId = saveInsight(currentAnalysis.id)
    navigate(`/insights/${insightId}`)
  }

  return (
    <section className="analysis-page" aria-labelledby="analysis-title">
      <aside className="analysis-materials" aria-labelledby="analysis-materials-title">
        <header className="analysis-materials-header">
          <p className="page-eyebrow">本次上下文</p>
          <h1 id="analysis-materials-title">参考材料（{materialCards.length}）</h1>
        </header>

        <ul className="analysis-material-list" aria-label="参考材料列表">
          {materialCards.map((card) => {
            const isHighlighted = highlightedCardId === card.id

            return (
              <li
                className={[
                  'analysis-material-item',
                  isHighlighted ? 'is-highlighted' : '',
                ]
                  .filter(Boolean)
                  .join(' ')}
                key={card.id}
                ref={(node) => {
                  materialRefs.current[card.id] = node
                }}
              >
                <button
                  aria-label={`查看材料：${card.title}`}
                  aria-current={isHighlighted ? 'true' : 'false'}
                  className="analysis-material-open"
                  onClick={() => setHighlightedCardId(card.id)}
                  type="button"
                >
                  <span className="analysis-material-title">
                    查看材料：{card.title}
                  </span>
                  <span className="analysis-material-content">{card.content}</span>
                  {card.note ? (
                    <span className="analysis-material-note">
                      我的想法：{card.note}
                    </span>
                  ) : null}
                </button>
                <button
                  aria-label={`移除材料：${card.title}`}
                  className="analysis-material-remove"
                  disabled={isInteractionLocked}
                  onClick={() => removeMaterial(card.id)}
                  title="移除材料"
                  type="button"
                >
                  <Trash2 size={15} />
                </button>
              </li>
            )
          })}
        </ul>
      </aside>

      <div className="analysis-workspace">
        <header className="analysis-header">
          <div>
            <p className="page-eyebrow">AI 延伸与查询</p>
            <h1 id="analysis-title">{currentAnalysis.title}</h1>
          </div>
          <StatusLabel status={currentAnalysis.status} />
        </header>

        <div className="analysis-controls">
          <label className="analysis-field">
            <span>想延伸或查询的问题</span>
            <textarea
              disabled={isInteractionLocked}
              onChange={(event) => setGoal(event.target.value)}
              placeholder="例如：这个想法还能往哪几个方向展开？有哪些概念需要查清楚？"
              rows={3}
              value={currentAnalysis.goal}
            />
          </label>

          <div className="analysis-actions">
            <Button disabled={!canStart} onClick={() => void startAnalysis('success')}>
              <Play size={15} />
              开始延伸
            </Button>
            <Button
              disabled={!canStart}
              onClick={() => void startAnalysis('failure')}
              variant="secondary"
            >
              <AlertTriangle size={15} />
              演示查询失败
            </Button>
          </div>
        </div>

        {currentAnalysis.status === 'running' ? (
          <section className="analysis-status-panel" aria-live="polite">
            <div>
              <h2>正在延伸想法</h2>
              <p>正在基于当前上下文补充方向、查询线索和可追溯引用，稍等片刻。</p>
            </div>
            <div className="analysis-progress" aria-hidden="true">
              <span />
            </div>
            <Button onClick={cancelAnalysis} variant="secondary">
              <X size={15} />
              取消延伸
            </Button>
          </section>
        ) : null}

        {currentAnalysis.status === 'failed' ? (
          <section className="analysis-alert" role="alert">
            <div>
              <h2>延伸失败</h2>
              <p>{errorMessage}</p>
            </div>
            <Button
              disabled={!canStart}
              onClick={() => void startAnalysis('success')}
              variant="secondary"
            >
              <RotateCcw size={15} />
              重试
            </Button>
          </section>
        ) : null}

        {currentAnalysis.status === 'cancelled' ? (
          <section className="analysis-alert analysis-alert-muted" role="status">
            <h2>已取消，本次材料和目标已保留</h2>
            <p>可以调整问题或材料后重新发起延伸。</p>
          </section>
        ) : null}

        {currentAnalysis.status === 'completed' && currentAnalysis.output ? (
          <>
            <AnalysisResult
              cards={materialCards}
              onReferenceClick={focusReference}
              output={currentAnalysis.output}
            />

            <section className="analysis-follow-up" aria-labelledby="follow-up-title">
              <div className="analysis-follow-up-header">
                <div>
                  <h2 id="follow-up-title">继续追问</h2>
                  <p>追问只基于当前上下文和已有生成结果。</p>
                </div>
                <Button disabled={isSubmittingFollowUp} onClick={saveCurrentInsight}>
                  保存为结论
                </Button>
              </div>

              {currentAnalysis.followUps.length > 0 ? (
                <div className="analysis-follow-up-list">
                  {currentAnalysis.followUps.map((followUp) => (
                    <article className="analysis-follow-up-item" key={followUp.id}>
                      <h3>{followUp.question}</h3>
                      <p>{followUp.answer}</p>
                    </article>
                  ))}
                </div>
              ) : null}

              <label className="analysis-field">
                <span>追问内容</span>
                <textarea
                  onChange={(event) => setFollowUpQuestion(event.target.value)}
                  placeholder="围绕当前材料继续问一个具体问题"
                  rows={3}
                  value={followUpQuestion}
                />
              </label>

              <label className="analysis-field">
                <span>引用材料</span>
                <select
                  onChange={(event) => setFollowUpCardId(event.target.value)}
                  value={followUpCardId}
                >
                  <option value="">不指定材料</option>
                  {materialCards.map((card) => (
                    <option key={card.id} value={card.id}>
                      {card.title}
                    </option>
                  ))}
                </select>
              </label>

              <Button
                disabled={!followUpQuestion.trim() || isSubmittingFollowUp}
                onClick={() => void submitFollowUp()}
                variant="secondary"
              >
                {isSubmittingFollowUp ? '生成追问回答中' : '提交追问'}
              </Button>
            </section>
          </>
        ) : null}
      </div>
    </section>
  )
}

function StatusLabel({ status }: { status: string }) {
  const labels: Record<string, string> = {
    draft: '草稿',
    running: '延伸中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  }

  return <span className="analysis-status-label">{labels[status] ?? status}</span>
}
