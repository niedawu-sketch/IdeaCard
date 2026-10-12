import { useMemo, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import type { CardStatus, SourceType } from '../entities/card/model'
import { sourceTypeOptions } from '../entities/card/source'
import { CaptureDialog } from '../features/capture/CaptureDialog'
import type { CaptureKind } from '../features/capture/captureAdapter'
import { CardDetail } from '../features/inbox/CardDetail'
import { CardList } from '../features/inbox/CardList'
import { InboxToolbar } from '../features/inbox/InboxToolbar'
import { Button } from '../shared/ui/Button'
import { useIdeaCardStore } from '../store/useIdeaCardStore'

const analysisLimitError = '单次分析最多选择 20 张卡片'
const analysisGenericError = '创建分析集失败，请稍后再试'

export function InboxPage() {
  const cards = useIdeaCardStore((state) => state.cards)
  const selectedCardIds = useIdeaCardStore((state) => state.selectedCardIds)
  const toggleCardSelection = useIdeaCardStore(
    (state) => state.toggleCardSelection,
  )
  const clearSelection = useIdeaCardStore((state) => state.clearSelection)
  const createAnalysisSet = useIdeaCardStore(
    (state) => state.createAnalysisSet,
  )
  const createCard = useIdeaCardStore((state) => state.createCard)
  const updateCard = useIdeaCardStore((state) => state.updateCard)
  const navigate = useNavigate()
  const [searchParams, setSearchParams] = useSearchParams()
  const [query, setQuery] = useState('')
  const [sourceFilter, setSourceFilter] = useState<SourceType | null>(null)
  const [captureKind, setCaptureKind] = useState<CaptureKind | null>(null)
  const [analysisError, setAnalysisError] = useState<string | null>(null)
  const activeCardId = searchParams.get('card')
  const requestedStatus = searchParams.get('status')
  const statusFilter = isCardStatus(requestedStatus)
    ? requestedStatus
    : null
  const selectedCount = selectedCardIds.length

  const visibleCards = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()

    return cards.filter((card) => {
      if (statusFilter && card.status !== statusFilter) {
        return false
      }

      if (sourceFilter && card.sourceType !== sourceFilter) {
        return false
      }

      return !normalizedQuery || [card.title, card.content, card.note]
        .join(' ')
        .toLocaleLowerCase()
        .includes(normalizedQuery)
    })
  }, [cards, query, sourceFilter, statusFilter])

  const activeCard =
    cards.find((card) => card.id === activeCardId) ?? null

  function openCard(cardId: string) {
    const nextSearchParams = new URLSearchParams(searchParams)
    nextSearchParams.set('card', cardId)
    setSearchParams(nextSearchParams)
  }

  function updateActiveCard(
    patch: Parameters<typeof updateCard>[1],
  ) {
    if (activeCard) {
      updateCard(activeCard.id, patch)
    }
  }

  function archiveActiveCard() {
    if (!activeCard) {
      return
    }

    updateCard(activeCard.id, { status: 'archived' })
    const nextSearchParams = new URLSearchParams(searchParams)
    nextSearchParams.delete('card')
    setSearchParams(nextSearchParams)
  }

  function clearSelectedCards() {
    setAnalysisError(null)
    clearSelection()
  }

  function toggleSelectedCard(cardId: string) {
    const nextSelectedCount = selectedCardIds.includes(cardId)
      ? selectedCount - 1
      : selectedCount + 1

    if (
      nextSelectedCount <= 20 &&
      analysisError === analysisLimitError
    ) {
      setAnalysisError(null)
    }

    toggleCardSelection(cardId)
  }

  function createAnalysisFromSelection() {
    try {
      const analysisId = createAnalysisSet(selectedCardIds)
      setAnalysisError(null)
      navigate(`/analysis/${analysisId}`)
    } catch (error) {
      if (error instanceof Error && error.message === analysisLimitError) {
        setAnalysisError(analysisLimitError)
        return
      }

      setAnalysisError(analysisGenericError)
    }
  }

  return (
    <section className="inbox-page" aria-labelledby="inbox-title">
      <aside className="inbox-filters">
        <div className="inbox-filters-header">
          <p className="page-eyebrow">统一收集箱</p>
          <h1 id="inbox-title">收集箱</h1>
        </div>

        <FilterGroup
          label="来源"
          allLabel="全部来源"
          items={sourceTypeOptions}
          selectedItem={sourceFilter}
          onSelect={setSourceFilter}
        />
      </aside>

      <div className="inbox-list-column">
        <InboxToolbar
          query={query}
          resultCount={visibleCards.length}
          onQueryChange={setQuery}
          onOpenCapture={setCaptureKind}
        />
        <CardList
          cards={visibleCards}
          activeCardId={activeCardId}
          selectedCardIds={selectedCardIds}
          onOpenCard={openCard}
          onToggleSelection={toggleSelectedCard}
        />
        {selectedCount > 0 ? (
          <div className="inbox-bulk-bar" aria-label="批量操作">
            <div className="inbox-bulk-summary">
              <strong>已选择 {selectedCount} 张卡片</strong>
              {selectedCount === 1 ? (
                <span className="inbox-bulk-chip">单卡片分析</span>
              ) : null}
              {analysisError ? (
                <span className="inbox-bulk-error" role="alert">
                  {analysisError}
                </span>
              ) : null}
            </div>
            <div className="inbox-bulk-actions">
              <Button
                size="small"
                variant="ghost"
                onClick={clearSelectedCards}
              >
                清除选择
              </Button>
              <Button size="small" onClick={createAnalysisFromSelection}>
                创建分析集
              </Button>
            </div>
          </div>
        ) : null}
      </div>

      <aside className="inbox-detail-column">
        <CardDetail
          key={activeCard?.id ?? 'empty'}
          card={activeCard}
          onSaveNote={(note) => updateActiveCard({ note })}
          onMarkPendingAnalysis={() =>
            updateActiveCard({ status: 'pending-analysis' })
          }
          onArchive={archiveActiveCard}
        />
      </aside>

      {captureKind ? (
        <CaptureDialog
          cards={cards}
          kind={captureKind}
          key={captureKind}
          onClose={() => setCaptureKind(null)}
          onCreateCard={createCard}
        />
      ) : null}
    </section>
  )
}

const cardStatuses: CardStatus[] = [
  'unprocessed',
  'annotated',
  'pending-analysis',
  'concluded',
  'archived',
]

function isCardStatus(status: string | null): status is CardStatus {
  return cardStatuses.includes(status as CardStatus)
}

interface FilterGroupProps {
  label: string
  allLabel: string
  items: ReadonlyArray<{ value: SourceType; label: string }>
  selectedItem: SourceType | null
  onSelect: (item: SourceType | null) => void
}

function FilterGroup({
  label,
  allLabel,
  items,
  selectedItem,
  onSelect,
}: FilterGroupProps) {
  return (
    <section className="inbox-filter-group">
      <h2>{label}</h2>
      <div className="inbox-filter-list">
        <button
          aria-pressed={selectedItem === null}
          className={selectedItem === null ? 'is-selected' : ''}
          type="button"
          onClick={() => onSelect(null)}
        >
          {allLabel}
        </button>
        {items.map((item) => (
          <button
            aria-pressed={selectedItem === item.value}
            className={selectedItem === item.value ? 'is-selected' : ''}
            key={item.value}
            type="button"
            onClick={() => onSelect(item.value)}
          >
            {item.label}
          </button>
        ))}
      </div>
    </section>
  )
}
