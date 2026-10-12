import { AiMarkdown } from '../ai/AiMarkdown'
import { Archive, BookOpen, ChevronRight, Trash2 } from 'lucide-react'
import { useState } from 'react'

import type { ConceptDetail, ConceptSummary } from '../../entities/concept/model'
import { DeleteConceptDialog } from './DeleteConceptDialog'

interface ConceptPageProps {
  onOpenLink(url: string): void
  concepts: ConceptSummary[]
  detail: ConceptDetail | null
  selectedConceptId: string | null
  onSelect(conceptId: string): void
  onDelete(conceptId: string): Promise<void>
  onOpenSource(source: {
    cardId: string
    mentionId: string
    cardStatus: 'archived' | 'unprocessed' | 'annotated' | 'pending_analysis' | 'concluded'
  }): void
}

export function ConceptPage({
  onOpenLink,
  concepts,
  detail,
  selectedConceptId,
  onSelect,
  onDelete,
  onOpenSource,
}: ConceptPageProps) {
  const [conceptToDelete, setConceptToDelete] = useState<ConceptDetail | null>(null)

  return (
    <>
      <section aria-label="概念集合" className="concept-workspace">
        <div className="concept-list-column">
          <header className="workspace-heading">
            <p>长期沉淀</p>
            <h1>概念集合</h1>
            <span>{concepts.length} 个</span>
          </header>
          {concepts.length > 0 ? (
            <ul className="concept-list">
              {concepts.map((concept) => (
                <li key={concept.id}>
                  <button
                    aria-current={concept.id === selectedConceptId ? 'true' : undefined}
                    className={concept.id === selectedConceptId ? 'is-selected' : ''}
                    type="button"
                    onClick={() => onSelect(concept.id)}
                  >
                    <span>
                      <strong>{concept.name}</strong>
                      <small>{concept.explanation}</small>
                    </span>
                    <em>{concept.mentionCount} 处</em>
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <div className="concept-empty">
              <BookOpen aria-hidden="true" size={19} />
              <strong>还没有保存的概念</strong>
              <span>在卡片原文中选中文字并使用 AI 解释后，会沉淀到这里。</span>
            </div>
          )}
        </div>

        <article className="concept-detail">
          {detail ? (
            <>
              <header>
                <div>
                  <p>概念</p>
                  <h2>{detail.name}</h2>
                </div>
                <div className="concept-detail-actions">
                  <span>{detail.mentionCount} 处引用</span>
                  <button
                    className="button compact is-danger"
                    type="button"
                    onClick={() => setConceptToDelete(detail)}
                  >
                    <Trash2 aria-hidden="true" size={14} />
                    删除概念
                  </button>
                </div>
              </header>
              <section className="concept-explanation">
                <h3>解释</h3>
                <AiMarkdown content={detail.explanation} onOpenLink={onOpenLink} />
              </section>
              <section className="concept-sources">
                <h3>来源卡片</h3>
                <ul>
                  {detail.mentions.map((mention) => (
                    <li key={mention.mentionId}>
                      <button
                        type="button"
                        onClick={() => onOpenSource({
                          cardId: mention.cardId,
                          mentionId: mention.mentionId,
                          cardStatus: mention.cardStatus,
                        })}
                      >
                        <span>
                          <strong>{mention.cardTitle}</strong>
                          <small>{mention.selectedText}</small>
                        </span>
                        {mention.cardStatus === 'archived' ? (
                          <em><Archive aria-hidden="true" size={12} />已归档</em>
                        ) : null}
                        <ChevronRight aria-hidden="true" size={15} />
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            </>
          ) : concepts.length > 0 ? (
            <div className="card-detail-empty"><p>选择一个概念查看解释与来源</p></div>
          ) : null}
        </article>
      </section>
      {conceptToDelete ? (
        <DeleteConceptDialog
          concept={conceptToDelete}
          onClose={() => setConceptToDelete(null)}
          onConfirm={() => onDelete(conceptToDelete.id)}
        />
      ) : null}
    </>
  )
}
