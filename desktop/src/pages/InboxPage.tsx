import { listen } from '@tauri-apps/api/event'
import { AlertCircle, Lightbulb, Search, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'

import {
  sourceTypeOptions,
  visibleSourceType,
  type Card,
  type CardWorkspace,
  type VisibleSourceType,
} from '../entities/card/model'
import type { CaptureDraft } from '../entities/capture/model'
import { CaptureDialog } from '../features/capture/CaptureDialog'
import { CaptureMenu } from '../features/capture/CaptureMenu'
import { QuickCapture } from '../features/capture/QuickCapture'
import { CardDetail } from '../features/inbox/CardDetail'
import { CardList } from '../features/inbox/CardList'
import { DeleteCardDialog } from '../features/inbox/DeleteCardDialog'
import { NativeTextField } from '../features/native-text-field/NativeTextField'
import { useDesktopService } from '../services/desktop/useDesktopService'
import { useDesktopStore } from '../store/useDesktopStore'

interface InboxPageProps {
  workspace?: CardWorkspace
  onOpenAiSettings?(): void
  onOpenConcept?(conceptId: string): void
  targetMentionId?: string | null
}

export function InboxPage({
  workspace = 'inbox',
  onOpenAiSettings = () => undefined,
  onOpenConcept = () => undefined,
  targetMentionId = null,
}: InboxPageProps) {
  const service = useDesktopService()
  const cards = useDesktopStore((state) => state.cards)
  const selectedCardId = useDesktopStore((state) => state.selectedCardId)
  const duplicateCardIds = useDesktopStore((state) => state.duplicateCardIds)
  const error = useDesktopStore((state) => state.error)
  const errorOperation = useDesktopStore((state) => state.errorOperation)
  const loadCards = useDesktopStore((state) => state.loadCards)
  const selectCard = useDesktopStore((state) => state.selectCard)
  const createQuickCard = useDesktopStore((state) => state.createQuickCard)
  const updateCard = useDesktopStore((state) => state.updateCard)
  const archiveCard = useDesktopStore((state) => state.archiveCard)
  const restoreCard = useDesktopStore((state) => state.restoreCard)
  const deleteCard = useDesktopStore((state) => state.deleteCard)
  const clearDuplicateCardIds = useDesktopStore((state) => state.clearDuplicateCardIds)
  const clearError = useDesktopStore((state) => state.clearError)
  const [query, setQuery] = useState('')
  const [sourceFilters, setSourceFilters] = useState<
    Record<CardWorkspace, VisibleSourceType | null>
  >({
    inbox: null,
    archived: null,
  })
  const [quickCaptureOpen, setQuickCaptureOpen] = useState(false)
  const [captureDraft, setCaptureDraft] = useState<CaptureDraft | null>(null)
  const [cardPendingDelete, setCardPendingDelete] = useState<Card | null>(null)
  const captureOpenRef = useRef(false)

  useEffect(() => {
    captureOpenRef.current = quickCaptureOpen || captureDraft !== null
  }, [captureDraft, quickCaptureOpen])

  useEffect(() => {
    void loadCards(service)
  }, [loadCards, service])

  useEffect(() => {
    if (!('__TAURI_INTERNALS__' in window)) return
    const unlisten = Promise.all([
      listen('capture://quick', () => {
        if (!captureOpenRef.current) setQuickCaptureOpen(true)
      }),
      listen('capture://clipboard', () => {
        if (!captureOpenRef.current) void service.readClipboardDraft().then(setCaptureDraft).catch(() => undefined)
      }),
      listen('capture://screenshot', () => {
        void service.captureScreenshot().then(async (result) => {
          if (result.status === 'saved') {
            await loadCards(service)
            selectCard(result.card.id)
          }
        }).catch(() => undefined)
      }),
    ])
    return () => {
      void unlisten.then((callbacks) => callbacks.forEach((callback) => callback())).catch(() => undefined)
    }
  }, [loadCards, selectCard, service])

  const workspaceCards = useMemo(
    () => cards.filter((card) => (
      workspace === 'archived' ? card.status === 'archived' : card.status !== 'archived'
    )),
    [cards, workspace],
  )
  const sourceFilter = sourceFilters[workspace]
  const visibleCards = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase()
    return workspaceCards.filter((card) => {
      if (sourceFilter && visibleSourceType(card.sourceType) !== sourceFilter) {
        return false
      }
      if (!normalizedQuery) {
        return true
      }
      return [card.title, card.originalContent, card.note, card.sourceName]
        .join(' ')
        .toLocaleLowerCase()
        .includes(normalizedQuery)
    })
  }, [query, sourceFilter, workspaceCards])
  const sourceCounts = useMemo(
    () =>
      workspaceCards.reduce<Record<VisibleSourceType, number>>(
        (counts, card) => {
          counts[visibleSourceType(card.sourceType)] += 1
          return counts
        },
        { personal: 0, document: 0 },
      ),
    [workspaceCards],
  )
  const selectedCard = visibleCards.find((card) => card.id === selectedCardId) ?? null
  const isFiltering = query.trim().length > 0 || sourceFilter !== null
  const emptyMessage = isFiltering
    ? '当前来源下没有卡片。'
    : workspace === 'archived'
      ? '还没有归档卡片。'
      : '还没有卡片，先快速记录一个想法。'

  useEffect(() => {
    if (visibleCards.some((card) => card.id === selectedCardId)) return
    selectCard(visibleCards[0]?.id ?? null)
  }, [selectCard, selectedCardId, visibleCards])

  return (
    <section aria-labelledby="inbox-title" className="inbox-page">
      <aside className="source-panel">
        <header>
          <h1 id="inbox-title">{workspace === 'archived' ? '已归档' : '收集箱'}</h1>
        </header>
        <section aria-label="来源" className="source-filter">
          <button
            aria-pressed={sourceFilter === null}
            className={sourceFilter === null ? 'is-selected' : ''}
            type="button"
            onClick={() => setSourceFilters((current) => ({ ...current, [workspace]: null }))}
          >
            <span>全部来源</span>
            <span aria-hidden="true" className="source-filter-count">{workspaceCards.length}</span>
          </button>
          {sourceTypeOptions.map((option) => (
            <button
              aria-pressed={sourceFilter === option.value}
              className={sourceFilter === option.value ? 'is-selected' : ''}
              data-source-type={option.value}
              key={option.value}
              type="button"
              onClick={() => setSourceFilters((current) => ({
                ...current,
                [workspace]: option.value,
              }))}
            >
              <span>{option.label}</span>
              <span aria-hidden="true" className="source-filter-count">
                {sourceCounts[option.value]}
              </span>
            </button>
          ))}
        </section>
      </aside>

      <div className="inbox-list-column">
        <header className="inbox-toolbar">
          <label className="inbox-search">
            <Search aria-hidden="true" size={16} />
            <span className="sr-only">搜索卡片</span>
            <NativeTextField
              fieldId={`inbox-search-${workspace}`}
              placeholder="搜索卡片"
              type="search"
              value={query}
              onChange={setQuery}
            />
          </label>
          <span className="result-count">{visibleCards.length} 张</span>
          {workspace === 'inbox' ? (
            <>
              <CaptureMenu
                onClipboard={async () => {
                  setCaptureDraft(await service.readClipboardDraft())
                }}
                onImport={async () => {
                  const results = await service.selectAndImportFiles()
                  const selected = results.find((result) => result.card)?.card
                  await loadCards(service)
                  if (selected) selectCard(selected.id)
                  return results
                }}
              />
              <button
                className="button is-primary compact"
                type="button"
                onClick={() => {
                  clearError()
                  setQuickCaptureOpen(true)
                }}
              >
                <Lightbulb aria-hidden="true" size={15} />
                快速记录
              </button>
            </>
          ) : null}
        </header>

        {error && errorOperation === 'load' ? (
          <div className="load-error" role="alert">
            <AlertCircle aria-hidden="true" size={16} />
            <div>
              <strong>{error.message}</strong>
              {error.recovery ? <span>{error.recovery}</span> : null}
            </div>
            <button className="button is-secondary compact" type="button" onClick={() => void loadCards(service)}>
              重试
            </button>
          </div>
        ) : null}

        {duplicateCardIds.length > 0 ? (
          <div className="duplicate-notice" role="status">
            <span>发现 {duplicateCardIds.length} 张可能重复的卡片，本次记录已保存。</span>
            <button
              aria-label="关闭重复提示"
              className="icon-button subtle"
              title="关闭"
              type="button"
              onClick={clearDuplicateCardIds}
            >
              <X aria-hidden="true" size={14} />
            </button>
          </div>
        ) : null}

        <CardList
          cards={visibleCards}
          emptyMessage={emptyMessage}
          selectedCardId={selectedCardId}
          showOrganizationStatus={workspace === 'inbox'}
          onSelect={selectCard}
        />
      </div>

      <aside className="inbox-detail-column">
        <CardDetail
          onGenerateTitle={(content) => service.generateCardTitle(selectedCard!.id, content)}
          key={selectedCard ? `${workspace}:${selectedCard.id}` : 'empty'}
          card={selectedCard}
          workspace={workspace}
          onSave={(patch) => {
            if (!selectedCard) return Promise.resolve()
            return updateCard(service, selectedCard.id, patch)
          }}
          onArchive={() => {
            if (!selectedCard) return Promise.resolve()
            return archiveCard(service, selectedCard.id)
          }}
          onRestore={() => {
            if (!selectedCard) return Promise.resolve()
            return restoreCard(service, selectedCard.id)
          }}
          onDeleteRequest={() => {
            if (selectedCard) setCardPendingDelete(selectedCard)
          }}
          onRevealAttachment={(id) => service.revealAttachment(id)}
          onLoadDocument={service.getCardDocument}
          onSaveDocument={service.saveCardDocument}
          onLoadInlineImage={service.loadInlineImage}
          onLoadConcepts={service.listCardConcepts}
          onExplainConcept={service.explainCardConcept}
          onSaveConceptAnnotation={service.saveConceptAnnotation}
          onOpenConcept={onOpenConcept}
          onLoadAiSettings={service.getAiSettings}
          onLoadAiMessages={service.listCardAiMessages}
          onSendAiMessage={service.sendCardAiMessage}
          onRegenerateAiMessage={service.regenerateCardAiMessage}
          onCopyText={service.writeClipboardText}
          onCancelAiRequest={service.cancelAiRequest}
          onOpenAiSettings={onOpenAiSettings}
          onOpenExternalUrl={service.openExternalUrl}
          targetMentionId={targetMentionId}
        />
      </aside>

      {quickCaptureOpen ? (
        <QuickCapture
          onClose={() => setQuickCaptureOpen(false)}
          onSave={(input) => createQuickCard(service, input)}
        />
      ) : null}
      {captureDraft ? (
        <CaptureDialog
          draft={captureDraft}
          onClose={() => setCaptureDraft(null)}
          onSave={async (draft) => {
            const card = await service.saveCaptureDraft(draft)
            await loadCards(service)
            selectCard(card.id)
          }}
        />
      ) : null}
      {cardPendingDelete ? (
        <DeleteCardDialog
          card={cardPendingDelete}
          onClose={() => setCardPendingDelete(null)}
          onConfirm={() => deleteCard(service, cardPendingDelete.id)}
        />
      ) : null}
    </section>
  )
}
