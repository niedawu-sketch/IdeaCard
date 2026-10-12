import { Link, useNavigate, useParams } from 'react-router-dom'
import { AnalysisResult } from '../features/analysis/AnalysisResult'
import { Button } from '../shared/ui/Button'
import { useIdeaCardStore } from '../store/useIdeaCardStore'

export function InsightPage() {
  const { insightId } = useParams()
  const navigate = useNavigate()
  const insights = useIdeaCardStore((state) => state.insights)
  const cards = useIdeaCardStore((state) => state.cards)
  const insight = insightId
    ? insights.find((item) => item.id === insightId)
    : null

  if (insightId && !insight) {
    return (
      <section className="page-section insight-page" aria-labelledby="insight-title">
        <header className="page-header">
          <div>
            <p className="page-eyebrow">AI 沉淀</p>
            <h1 id="insight-title">未找到结论</h1>
          </div>
        </header>
        <p>这个结论可能已经不存在，返回结论列表查看已保存内容。</p>
        <Button onClick={() => navigate('/insights')}>返回结论</Button>
      </section>
    )
  }

  if (!insightId) {
    return (
      <section className="page-section insight-page" aria-labelledby="insight-title">
        <header className="page-header">
          <div>
            <p className="page-eyebrow">AI 沉淀</p>
            <h1 id="insight-title">结论</h1>
          </div>
        </header>

        {insights.length === 0 ? (
          <div className="insight-empty">
            <h2>还没有保存的结论</h2>
            <p>完成一次 AI 延伸后，可以把带来源引用的结果保存到这里。</p>
            <Button onClick={() => navigate('/')}>返回 Inbox</Button>
          </div>
        ) : (
          <div className="insight-list">
            {insights.map((item) => (
              <article className="insight-list-item" key={item.id}>
                <div>
                  <h2>{item.title}</h2>
                  <p>{item.goal}</p>
                  <div className="insight-list-meta">
                    <span>保存于 {formatInsightDate(item.createdAt)}</span>
                    <span>{item.cardIds.length} 张来源卡片</span>
                  </div>
                </div>
                <Link to={`/insights/${item.id}`} aria-label={`打开结论：${item.title}`}>
                  打开
                </Link>
              </article>
            ))}
          </div>
        )}
      </section>
    )
  }

  const sourceCards = insight
    ? insight.cardIds
        .map((cardId) => cards.find((card) => card.id === cardId))
        .filter((card): card is (typeof cards)[number] => Boolean(card))
    : []

  return (
    <section className="page-section insight-page" aria-labelledby="insight-title">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">AI 沉淀</p>
          <h1 id="insight-title">{insight!.title}</h1>
        </div>
        <Button onClick={() => navigate(`/analysis/${insight!.analysisId}`)}>
          基于原上下文继续延伸
        </Button>
      </header>

      <p className="insight-goal">{insight!.goal}</p>

      <AnalysisResult
        cards={sourceCards}
        onReferenceClick={(cardId) => navigate(`/?card=${cardId}`)}
        output={insight!.output}
      />

      {insight!.followUps.length > 0 ? (
        <section className="insight-section" aria-labelledby="insight-follow-ups">
          <h2 id="insight-follow-ups">追问记录</h2>
          <div className="insight-follow-up-list">
            {insight!.followUps.map((followUp) => (
              <article className="insight-follow-up" key={followUp.id}>
                <h3>{followUp.question}</h3>
                <p>{followUp.answer}</p>
              </article>
            ))}
          </div>
        </section>
      ) : null}

      <section className="insight-section" aria-labelledby="insight-source-cards">
        <h2 id="insight-source-cards">来源卡片</h2>
        <ul className="insight-source-list" aria-label="来源卡片">
          {sourceCards.map((card) => (
            <li key={card.id}>
              <button
                onClick={() => navigate(`/?card=${card.id}`)}
                type="button"
              >
                打开来源卡片：{card.title}
              </button>
            </li>
          ))}
        </ul>
      </section>
    </section>
  )
}

function formatInsightDate(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}
