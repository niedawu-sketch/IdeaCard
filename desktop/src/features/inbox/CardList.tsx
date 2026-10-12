import {
  getCardOrganizationStatus,
  sourceTypeLabel,
  type Card,
} from '../../entities/card/model'

interface CardListProps {
  cards: Card[]
  emptyMessage?: string
  selectedCardId: string | null
  showOrganizationStatus?: boolean
  onSelect(cardId: string): void
}

function formatCardTime(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) {
    return ''
  }
  return new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date)
}

export function CardList({
  cards,
  emptyMessage = '没有匹配的卡片，试试清除搜索或切换来源',
  selectedCardId,
  showOrganizationStatus = true,
  onSelect,
}: CardListProps) {
  return (
    <div aria-label="卡片列表" className="card-list">
      {cards.length === 0 ? (
        <p className="card-list-empty">{emptyMessage}</p>
      ) : null}
      {cards.map((card) => {
        const organizationStatus = getCardOrganizationStatus(card.status)
        return (
          <article
            className={`card-list-item${selectedCardId === card.id ? ' is-selected' : ''}`}
            key={card.id}
          >
            <button type="button" onClick={() => onSelect(card.id)}>
              <span className="card-list-heading">
                <strong>{card.title}</strong>
                {showOrganizationStatus ? (
                  <span className={`status-dot is-${organizationStatus.value}`}>
                    {organizationStatus.label}
                  </span>
                ) : null}
              </span>
              <span className="card-list-summary">{card.originalContent || '暂无正文'}</span>
              <span className="card-list-meta">
                <span className="card-list-source-type">{sourceTypeLabel(card.sourceType)}</span>
                <span className="card-list-source-name">{card.sourceName || '未设置来源'}</span>
                <time dateTime={card.updatedAt}>{formatCardTime(card.updatedAt)}</time>
              </span>
            </button>
          </article>
        )
      })}
    </div>
  )
}
