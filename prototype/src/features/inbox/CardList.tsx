import type { Card } from '../../entities/card/model'
import { StatusBadge } from '../../shared/ui/StatusBadge'

interface CardListProps {
  cards: Card[]
  activeCardId: string | null
  selectedCardIds: string[]
  onOpenCard: (cardId: string) => void
  onToggleSelection: (cardId: string) => void
}

const statusLabels = {
  unprocessed: '未处理',
  annotated: '已标注',
  'pending-analysis': '待分析',
  concluded: '已生成结论',
  archived: '已归档',
} as const

const statusTones = {
  unprocessed: 'warning',
  annotated: 'success',
  'pending-analysis': 'warning',
  concluded: 'success',
  archived: 'neutral',
} as const

function formatCardTime(value: string) {
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(value))
}

export function CardList({
  cards,
  activeCardId,
  selectedCardIds,
  onOpenCard,
  onToggleSelection,
}: CardListProps) {
  if (cards.length === 0) {
    return <p className="inbox-no-results">没有匹配的卡片</p>
  }

  return (
    <div className="card-list" aria-label="卡片列表">
      {cards.map((card) => (
        <article
          className={`card-list-item${activeCardId === card.id ? ' is-active' : ''}`}
          key={card.id}
        >
          <input
            aria-label={`选择「${card.title}」`}
            checked={selectedCardIds.includes(card.id)}
            className="card-selection-checkbox"
            type="checkbox"
            onChange={() => onToggleSelection(card.id)}
          />
          <button
            className="card-list-open"
            type="button"
            onClick={() => onOpenCard(card.id)}
          >
            <span className="card-list-title">{card.title}</span>
            <span className="card-list-summary">{card.content}</span>
            <span className="card-list-meta">
              <span>{card.sourceName}</span>
              <StatusBadge tone={statusTones[card.status]}>
                {statusLabels[card.status]}
              </StatusBadge>
              <time dateTime={card.updatedAt}>{formatCardTime(card.updatedAt)}</time>
            </span>
          </button>
        </article>
      ))}
    </div>
  )
}
