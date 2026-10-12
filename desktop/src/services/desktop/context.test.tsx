import type { PropsWithChildren } from 'react'
import { renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import type { DesktopService } from './contracts'
import { DesktopServiceProvider } from './context'
import { tauriDesktopService } from './tauriClient'
import { useDesktopService } from './useDesktopService'

function createService(): DesktopService {
  return {
    listCards: vi.fn().mockResolvedValue([]),
    getCard: vi.fn(),
    createCard: vi.fn(),
    createRichCard: vi.fn(),
    getCardDocument: vi.fn(),
    saveCardDocument: vi.fn(),
    loadInlineImage: vi.fn(),
    listConcepts: vi.fn(),
    getConcept: vi.fn(),
    deleteConcept: vi.fn(),
    listCardConcepts: vi.fn(),
    saveConceptAnnotation: vi.fn(),
    getAiSettings: vi.fn(),
    saveAiSettings: vi.fn(),
    getConceptAssistantSettings: vi.fn(),
    saveConceptAssistantSettings: vi.fn(),
    generateCardTitle: vi.fn(),
    getTitleGenerationSettings: vi.fn(),
    saveTitleGenerationSettings: vi.fn(),
    removeAiKey: vi.fn(),
    testAiConnection: vi.fn(),
    listCardAiMessages: vi.fn(),
    sendCardAiMessage: vi.fn(),
    regenerateCardAiMessage: vi.fn(),
    explainCardConcept: vi.fn(),
    cancelAiRequest: vi.fn(),
    openExternalUrl: vi.fn(),
    updateCard: vi.fn(),
    archiveCard: vi.fn(),
    restoreCard: vi.fn(),
    deleteCard: vi.fn(),
    readClipboardDraft: vi.fn(),
    saveCaptureDraft: vi.fn(),
    selectAndImportFiles: vi.fn().mockResolvedValue([]),
    captureScreenshot: vi.fn().mockResolvedValue({ status: 'cancelled' }),
    getCaptureStatus: vi.fn(),
    revealAttachment: vi.fn().mockResolvedValue(undefined),
    readClipboardText: vi.fn().mockResolvedValue(null),
    writeClipboardText: vi.fn().mockResolvedValue(undefined),
  }
}

describe('DesktopServiceProvider', () => {
  it('uses the Tauri service by default', () => {
    const wrapper = ({ children }: PropsWithChildren) => (
      <DesktopServiceProvider>{children}</DesktopServiceProvider>
    )

    const { result } = renderHook(() => useDesktopService(), { wrapper })

    expect(result.current).toBe(tauriDesktopService)
  })

  it('allows tests and previews to inject a replacement service', () => {
    const service = createService()
    const wrapper = ({ children }: PropsWithChildren) => (
      <DesktopServiceProvider service={service}>{children}</DesktopServiceProvider>
    )

    const { result } = renderHook(() => useDesktopService(), { wrapper })

    expect(result.current).toBe(service)
  })

  it('fails fast outside the provider', () => {
    expect(() => renderHook(() => useDesktopService())).toThrow(
      'useDesktopService 必须在 DesktopServiceProvider 内使用',
    )
  })
})
