import { create } from 'zustand'

import type { Card } from '../entities/card/model'
import type { RichDocument } from '../entities/document/model'
import { DesktopError } from '../shared/errors/DesktopError'
import type {
  CardPatch,
  CreateCardResult,
  DesktopService,
} from '../services/desktop/contracts'

export interface QuickCardInput {
  title: string
  originalContent: string
  document: RichDocument
  note: string
}

type DesktopOperation = 'load' | 'create' | 'update' | 'archive' | 'restore' | 'delete'

interface DesktopStoreState {
  cards: Card[]
  selectedCardId: string | null
  duplicateCardIds: string[]
  isLoading: boolean
  error: DesktopError | null
  errorOperation: DesktopOperation | null
  aiDrawerOpen: boolean
  loadCards(service: DesktopService): Promise<void>
  selectCard(id: string | null): void
  setAiDrawerOpen(open: boolean): void
  createQuickCard(service: DesktopService, input: QuickCardInput): Promise<CreateCardResult>
  updateCard(service: DesktopService, id: string, patch: CardPatch): Promise<Card>
  archiveCard(service: DesktopService, id: string): Promise<Card>
  restoreCard(service: DesktopService, id: string): Promise<Card>
  deleteCard(service: DesktopService, id: string): Promise<void>
  clearDuplicateCardIds(): void
  clearError(): void
}

function selectedCardAfterLoad(cards: Card[], selectedCardId: string | null) {
  if (selectedCardId && cards.some((card) => card.id === selectedCardId)) {
    return selectedCardId
  }
  return cards[0]?.id ?? null
}

export const useDesktopStore = create<DesktopStoreState>((set, get) => ({
  cards: [],
  selectedCardId: null,
  duplicateCardIds: [],
  isLoading: false,
  error: null,
  errorOperation: null,
  aiDrawerOpen: false,

  setAiDrawerOpen(open) {
    set({ aiDrawerOpen: open })
  },

  async loadCards(service) {
    set({ isLoading: true, error: null, errorOperation: null })
    try {
      const cards = await service.listCards(true)
      set({
        cards,
        selectedCardId: selectedCardAfterLoad(cards, get().selectedCardId),
        isLoading: false,
      })
    } catch (error) {
      set({ error: DesktopError.fromUnknown(error), errorOperation: 'load', isLoading: false })
    }
  },

  selectCard(id) {
    set({ selectedCardId: id })
  },

  async createQuickCard(service, input) {
    set({ error: null, errorOperation: null, duplicateCardIds: [] })
    try {
      const result = await service.createRichCard({
        title: input.title.trim(),
        document: input.document,
        note: input.note,
        sourceType: 'personal',
        captureMethod: 'quick',
        sourceName: '',
        sourceUri: '',
      })
      set((state) => ({
        cards: [result.card, ...state.cards.filter((card) => card.id !== result.card.id)],
        selectedCardId: result.card.id,
        duplicateCardIds: result.duplicateCardIds,
      }))
      return result
    } catch (error) {
      const desktopError = DesktopError.fromUnknown(error)
      set({ error: desktopError, errorOperation: 'create' })
      throw desktopError
    }
  },

  async updateCard(service, id, patch) {
    set({ error: null, errorOperation: null })
    try {
      const updatedCard = await service.updateCard(id, patch)
      set((state) => ({
        cards: state.cards.map((card) => (card.id === updatedCard.id ? updatedCard : card)),
      }))
      return updatedCard
    } catch (error) {
      const desktopError = DesktopError.fromUnknown(error)
      set({ error: desktopError, errorOperation: 'update' })
      throw desktopError
    }
  },

  async archiveCard(service, id) {
    set({ error: null, errorOperation: null })
    try {
      const archivedCard = await service.archiveCard(id)
      set((state) => ({
        cards: state.cards.map((card) => (card.id === archivedCard.id ? archivedCard : card)),
      }))
      return archivedCard
    } catch (error) {
      const desktopError = DesktopError.fromUnknown(error)
      set({ error: desktopError, errorOperation: 'archive' })
      throw desktopError
    }
  },

  async restoreCard(service, id) {
    set({ error: null, errorOperation: null })
    try {
      const restoredCard = await service.restoreCard(id)
      set((state) => ({
        cards: state.cards.map((card) => (card.id === restoredCard.id ? restoredCard : card)),
      }))
      return restoredCard
    } catch (error) {
      const desktopError = DesktopError.fromUnknown(error)
      set({ error: desktopError, errorOperation: 'restore' })
      throw desktopError
    }
  },

  async deleteCard(service, id) {
    set({ error: null, errorOperation: null })
    try {
      await service.deleteCard(id)
      set((state) => {
        const target = state.cards.find((card) => card.id === id)
        const targetIsArchived = target?.status === 'archived'
        const workspaceCards = state.cards.filter(
          (card) => (card.status === 'archived') === targetIsArchived,
        )
        const workspaceIndex = workspaceCards.findIndex((card) => card.id === id)
        const nextCard = workspaceCards[workspaceIndex + 1] ?? workspaceCards[workspaceIndex - 1]
        return {
          cards: state.cards.filter((card) => card.id !== id),
          selectedCardId: state.selectedCardId === id
            ? nextCard?.id ?? null
            : state.selectedCardId,
        }
      })
    } catch (error) {
      const desktopError = DesktopError.fromUnknown(error)
      set({ error: desktopError, errorOperation: 'delete' })
      throw desktopError
    }
  },

  clearDuplicateCardIds() {
    set({ duplicateCardIds: [] })
  },

  clearError() {
    set({ error: null, errorOperation: null })
  },
}))
