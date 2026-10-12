import { Archive, Sparkles } from 'lucide-react'
import { useState } from 'react'
import type { Card } from '../../entities/card/model'
import { getSourceTypeLabel } from '../../entities/card/source'
import { Button } from '../../shared/ui/Button'
import { StatusBadge } from '../../shared/ui/StatusBadge'

interface CardDetailProps {
  card: Card | null
  onSaveNote: (note: string) => void
  onMarkPendingAnalysis: () => void
  onArchive: () => void
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

export function CardDetail({
  card,
  onSaveNote,
  onMarkPendingAnalysis,
  onArchive,
}: CardDetailProps) {
  if (!card) {
    return (
      <div className="card-detail-empty">
        <p>选择一张卡片开始处理</p>
      </div>
    )
  }

  return (
    <CardDetailForm
      card={card}
      onSaveNote={onSaveNote}
      onMarkPendingAnalysis={onMarkPendingAnalysis}
      onArchive={onArchive}
    />
  )
}

function CardDetailForm({
  card,
  onSaveNote,
  onMarkPendingAnalysis,
  onArchive,
}: CardDetailProps & { card: Card }) {
  const [note, setNote] = useState(card.note)

  return (
    <div className="card-detail">
      <header className="card-detail-header">
        <div>
          <p className="card-detail-kicker">卡片详情</p>
          <h2>{card.title}</h2>
        </div>
        <StatusBadge tone={statusTones[card.status]}>
          {statusLabels[card.status]}
        </StatusBadge>
      </header>

      <section className="card-detail-section">
        <h3>原始内容</h3>
        <p className="card-detail-content">{card.content}</p>
      </section>

      <section className="card-detail-section">
        <h3>来源信息</h3>
        <dl className="card-source-info">
          <div>
            <dt>类型</dt>
            <dd>{getSourceTypeLabel(card.sourceType)}</dd>
          </div>
          <div>
            <dt>来源</dt>
            <dd>{card.sourceName}</dd>
          </div>
          <div>
            <dt>地址</dt>
            <dd>
              {card.sourceUrl.startsWith('http') ? (
                <a
                  href={card.sourceUrl}
                  rel="noreferrer"
                  target="_blank"
                >
                  {card.sourceUrl}
                </a>
              ) : (
                card.sourceUrl || '无'
              )}
            </dd>
          </div>
        </dl>
      </section>

      <section className="card-detail-section">
        <label className="card-detail-field">
          <span>我的想法</span>
          <textarea
            rows={5}
            value={note}
            onBlur={() => {
              if (note !== card.note) {
                onSaveNote(note)
              }
            }}
            onChange={(event) => setNote(event.target.value)}
          />
        </label>
      </section>

      <div className="card-detail-actions">
        <Button
          disabled={card.status === 'pending-analysis'}
          variant="secondary"
          onClick={onMarkPendingAnalysis}
        >
          <Sparkles aria-hidden="true" size={15} />
          标记待分析
        </Button>
        <Button variant="ghost" onClick={onArchive}>
          <Archive aria-hidden="true" size={15} />
          归档
        </Button>
      </div>
    </div>
  )
}
