import {
  Archive,
  Bot,
  ChevronRight,
  SlidersHorizontal,
  Type,
  BookOpen,
  ChevronsLeft,
  ChevronsRight,
  Inbox,
  KeyRound,
  Settings,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'

import {
  DEFAULT_CONCEPT_SYSTEM_PROMPT,
  DEFAULT_TITLE_SYSTEM_PROMPT,
  defaultAiSettings,
  type AiSettings,
} from '../entities/ai/model'
import type { CardWorkspace } from '../entities/card/model'
import type { ConceptDetail, ConceptSummary } from '../entities/concept/model'
import { ConceptPage } from '../features/concepts/ConceptPage'
import { TextContextMenu } from '../features/context-menu/TextContextMenu'
import { AiSettingsDialog } from '../features/settings/AiSettingsDialog'
import { PromptSettingsDialog } from '../features/settings/PromptSettingsDialog'
import { VoiceInputBridge } from '../features/voice-input/VoiceInputBridge'
import { InboxPage } from '../pages/InboxPage'
import type { DesktopService } from '../services/desktop/contracts'
import { DesktopServiceProvider } from '../services/desktop/context'
import { useDesktopService } from '../services/desktop/useDesktopService'
import { useDesktopStore } from '../store/useDesktopStore'
import { installTauriWindowLongPressDrag } from './windowLongPressDrag'

interface AppProps {
  service?: DesktopService
}

const RAIL_COLLAPSED_KEY = 'ideacard.rail-collapsed'
type SettingsPanel = 'model' | 'conceptAssistant' | 'titleGeneration'

export function App({ service }: AppProps) {
  return (
    <DesktopServiceProvider service={service}>
      <VoiceInputBridge />
      <AppWorkspace />
      <TextContextMenu />
    </DesktopServiceProvider>
  )
}

function AppWorkspace() {
  const service = useDesktopService()
  const shellRef = useRef<HTMLDivElement>(null)
  const [workspace, setWorkspace] = useState<CardWorkspace | 'concepts'>('inbox')
  const [settingsMenuOpen, setSettingsMenuOpen] = useState(false)
  const [settingsPanel, setSettingsPanel] = useState<SettingsPanel | null>(null)
  const [aiSettings, setAiSettings] = useState<AiSettings>(defaultAiSettings('openAi'))
  const [promptMenuOpen, setPromptMenuOpen] = useState(false)
  const promptCloseTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  useEffect(() => () => {
    if (promptCloseTimer.current) clearTimeout(promptCloseTimer.current)
  }, [])
  const settingsMenuRef = useRef<HTMLDivElement>(null)
  const settingsButtonRef = useRef<HTMLButtonElement>(null)
  const [concepts, setConcepts] = useState<ConceptSummary[]>([])
  const [selectedConceptId, setSelectedConceptId] = useState<string | null>(null)
  const [conceptDetail, setConceptDetail] = useState<ConceptDetail | null>(null)
  const [targetMentionId, setTargetMentionId] = useState<string | null>(null)
  const [isRailCollapsed, setIsRailCollapsed] = useState(
    () => window.localStorage.getItem(RAIL_COLLAPSED_KEY) === '1',
  )
  const aiDrawerOpen = useDesktopStore((state) => state.aiDrawerOpen)
  const [railRestoreAfterDrawer, setRailRestoreAfterDrawer] = useState(false)
  const cards = useDesktopStore((state) => state.cards)
  const selectCard = useDesktopStore((state) => state.selectCard)
  const inboxCount = cards.filter((card) => card.status !== 'archived').length
  const archivedCount = cards.length - inboxCount

  useEffect(() => {
    const shell = shellRef.current
    if (!shell) return
    return installTauriWindowLongPressDrag(shell)
  }, [])

  function toggleRail() {
    // A manual toggle cancels any pending drawer-driven restore.
    setRailRestoreAfterDrawer(false)
    setIsRailCollapsed((current) => {
      window.localStorage.setItem(RAIL_COLLAPSED_KEY, current ? '0' : '1')
      return !current
    })
  }

  // While the AI drawer is open the readable detail column gives it room, so
  // the left rail collapses automatically; it comes back when the drawer
  // closes, unless the user changed the rail themselves in the meantime.
  // Auto-collapse/restore never touches the persisted preference. Adjusting
  // state during render (rather than in an effect) keeps this in sync with
  // the drawer's open/close transition only.
  const [wasAiDrawerOpen, setWasAiDrawerOpen] = useState(false)
  if (aiDrawerOpen !== wasAiDrawerOpen) {
    setWasAiDrawerOpen(aiDrawerOpen)
    if (aiDrawerOpen) {
      if (!isRailCollapsed) {
        setRailRestoreAfterDrawer(true)
        setIsRailCollapsed(true)
      }
    } else if (railRestoreAfterDrawer) {
      setRailRestoreAfterDrawer(false)
      setIsRailCollapsed(false)
    }
  }

  useEffect(() => {
    if (workspace !== 'concepts') return
    let active = true
    void service.listConcepts().then(async (loaded) => {
      if (!active) return
      setConcepts(loaded)
      const nextId = selectedConceptId ?? loaded[0]?.id ?? null
      setSelectedConceptId(nextId)
      setConceptDetail(nextId ? await service.getConcept(nextId) : null)
    }).catch(() => {
      if (active) {
        setConcepts([])
        setConceptDetail(null)
      }
    })
    return () => {
      active = false
    }
  }, [service, selectedConceptId, workspace])

  function openConcept(conceptId: string) {
    setWorkspace('concepts')
    setSelectedConceptId(conceptId)
    void service.getConcept(conceptId).then(setConceptDetail).catch(() => undefined)
  }

  async function deleteConcept(conceptId: string) {
    await service.deleteConcept(conceptId)
    const deletedIndex = concepts.findIndex((concept) => concept.id === conceptId)
    const remaining = concepts.filter((concept) => concept.id !== conceptId)
    const nextConcept = remaining[deletedIndex] ?? remaining[deletedIndex - 1] ?? remaining[0]
    const nextId = nextConcept?.id ?? null
    setConcepts(remaining)
    setSelectedConceptId(nextId)
    setConceptDetail(null)
    if (nextId) {
      void service.getConcept(nextId).then(setConceptDetail).catch(() => undefined)
    }
  }

  function openModelSettings() {
    setSettingsMenuOpen(false)
    setSettingsPanel('model')
    void service.getAiSettings().then(setAiSettings).catch(() => undefined)
  }

  function openPromptSettings(panel: 'conceptAssistant' | 'titleGeneration') {
    setSettingsMenuOpen(false)
    setSettingsPanel(panel)
  }

  function closeSettings() {
    setSettingsPanel(null)
    settingsButtonRef.current?.focus()
  }

  useEffect(() => {
    if (!settingsMenuOpen) return
    function dismiss(event: PointerEvent) {
      if (!settingsMenuRef.current?.contains(event.target as Node)) setSettingsMenuOpen(false)
    }
    document.addEventListener('pointerdown', dismiss)
    return () => document.removeEventListener('pointerdown', dismiss)
  }, [settingsMenuOpen])

  return (
    <>
      <div
        ref={shellRef}
        className={isRailCollapsed ? 'app-shell is-rail-collapsed' : 'app-shell'}
      >
        <aside className="app-sidebar">
          <div className="app-brand">
            <span aria-hidden="true">I</span>
            <strong>IdeaCard</strong>
          </div>
          <nav aria-label="工作区" className="app-navigation">
            <button
              aria-label={`收集箱 ${inboxCount}`}
              aria-current={workspace === 'inbox' ? 'page' : undefined}
              className={workspace === 'inbox' ? 'is-selected' : ''}
              title={isRailCollapsed ? `收集箱 ${inboxCount}` : undefined}
              type="button"
              onClick={() => setWorkspace('inbox')}
            >
              <Inbox aria-hidden="true" size={16} />
              <span>收集箱</span>
              <small>{inboxCount}</small>
            </button>
            <button
              aria-label={`已归档 ${archivedCount}`}
              aria-current={workspace === 'archived' ? 'page' : undefined}
              className={workspace === 'archived' ? 'is-selected' : ''}
              title={isRailCollapsed ? `已归档 ${archivedCount}` : undefined}
              type="button"
              onClick={() => setWorkspace('archived')}
            >
              <Archive aria-hidden="true" size={16} />
              <span>已归档</span>
              <small>{archivedCount}</small>
            </button>
            <button
              aria-label="概念集合"
              aria-current={workspace === 'concepts' ? 'page' : undefined}
              className={workspace === 'concepts' ? 'is-selected' : ''}
              title={isRailCollapsed ? '概念集合' : undefined}
              type="button"
              onClick={() => setWorkspace('concepts')}
            >
              <BookOpen aria-hidden="true" size={16} />
              <span>概念集合</span>
              <small>{concepts.length}</small>
            </button>
          </nav>
          <div className="app-sidebar-footer">
            <button
              aria-label={isRailCollapsed ? '展开侧栏' : '收起侧栏'}
              className="icon-button subtle on-chrome"
              title={isRailCollapsed ? '展开侧栏' : '收起侧栏'}
              type="button"
              onClick={toggleRail}
            >
              {isRailCollapsed ? (
                <ChevronsRight aria-hidden="true" size={15} />
              ) : (
                <ChevronsLeft aria-hidden="true" size={15} />
              )}
            </button>
            <div className="settings-menu-control" ref={settingsMenuRef} onKeyDown={(event) => {
              if (event.key === 'Escape') {
                event.stopPropagation()
                setSettingsMenuOpen(false)
                settingsButtonRef.current?.focus()
              }
              if (settingsMenuOpen && ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
                event.preventDefault()
                const items = Array.from(settingsMenuRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])
                const index = items.indexOf(document.activeElement as HTMLButtonElement)
                const next = event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1
                  : (index + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length
                items[next]?.focus()
              }
            }}>
              {settingsMenuOpen ? (
                <div aria-label="设置" className="settings-entry-menu" role="menu">
                  <button role="menuitem" type="button" onClick={openModelSettings}>
                    <KeyRound aria-hidden="true" size={15} />
                    <span>模型 API 配置</span>
                  </button>
                  <div className="settings-prompt-parent"
                    onMouseEnter={() => {
                      if (promptCloseTimer.current) clearTimeout(promptCloseTimer.current)
                      setPromptMenuOpen(true)
                    }}
                    onMouseLeave={() => {
                      promptCloseTimer.current = setTimeout(() => setPromptMenuOpen(false), 150)
                    }}
                    onKeyDown={(event) => {
                      if (event.key === 'ArrowRight') {
                        event.preventDefault()
                        setPromptMenuOpen(true)
                        requestAnimationFrame(() => settingsMenuRef.current?.querySelector<HTMLButtonElement>('#prompt-settings-submenu button')?.focus())
                      } else if (event.key === 'ArrowLeft') {
                        event.preventDefault()
                        setPromptMenuOpen(false)
                        settingsMenuRef.current?.querySelector<HTMLButtonElement>('#prompt-settings-trigger')?.focus()
                      }
                    }}>
                  <button id="prompt-settings-trigger" role="menuitem" type="button" aria-expanded={promptMenuOpen}
                    aria-haspopup="menu"
                    aria-controls="prompt-settings-submenu"
                    onClick={() => setPromptMenuOpen(true)}>
                    <SlidersHorizontal aria-hidden="true" size={15} />
                    <span>提示词配置</span>
                    <ChevronRight className="settings-menu-chevron" aria-hidden="true" size={14} />
                  </button>
                  {promptMenuOpen ? (
                    <div id="prompt-settings-submenu" role="menu" aria-label="提示词配置" className="settings-prompt-submenu">
                      <button role="menuitem" type="button" onClick={() => openPromptSettings('conceptAssistant')}>
                        <Bot aria-hidden="true" size={15} /><span>AI 概念助手</span>
                      </button>
                      <button role="menuitem" type="button" onClick={() => openPromptSettings('titleGeneration')}>
                        <Type aria-hidden="true" size={15} /><span>标题生成</span>
                      </button>
                    </div>
                  ) : null}
                  </div>
                </div>
              ) : null}
              <button
                ref={settingsButtonRef}
                aria-expanded={settingsMenuOpen}
                aria-haspopup="menu"
                aria-label="设置"
                className="icon-button subtle on-chrome"
                title="设置"
                type="button"
                onClick={() => { setSettingsMenuOpen((current) => !current); setPromptMenuOpen(false) }}
              >
                <Settings aria-hidden="true" size={15} />
              </button>
            </div>
          </div>
        </aside>
        <main className="app-main">
          {workspace === 'concepts' ? (
            <ConceptPage
              onOpenLink={(url) => void service.openExternalUrl(url)}
              concepts={concepts}
              detail={conceptDetail}
              selectedConceptId={selectedConceptId}
              onSelect={(conceptId) => {
                setSelectedConceptId(conceptId)
                void service.getConcept(conceptId).then(setConceptDetail).catch(() => undefined)
              }}
              onDelete={deleteConcept}
              onOpenSource={(source) => {
                selectCard(source.cardId)
                setTargetMentionId(source.mentionId)
                setWorkspace(source.cardStatus === 'archived' ? 'archived' : 'inbox')
              }}
            />
          ) : (
            <InboxPage
              targetMentionId={targetMentionId}
              workspace={workspace}
              onOpenAiSettings={() => {
                openModelSettings()
              }}
              onOpenConcept={openConcept}
            />
          )}
        </main>
      </div>
      {settingsPanel === 'model' ? (
        <AiSettingsDialog
          settings={aiSettings}
          onClose={() => setSettingsPanel(null)}
          onRemoveKey={async () => {
            const next = await service.removeAiKey(aiSettings.provider)
            setAiSettings(next)
            return next
          }}
          onSave={async (input) => {
            const next = await service.saveAiSettings(input)
            setAiSettings(next)
            return next
          }}
          onTest={(input) => service.testAiConnection(input)}
        />
      ) : null}
      {settingsPanel === 'conceptAssistant' ? (
        <PromptSettingsDialog
          key="concept"
          title="AI 概念助手"
          description="解释选中概念时使用，自动附加卡片标题和概念附近的原文。"
          defaultPrompt={DEFAULT_CONCEPT_SYSTEM_PROMPT}
          onLoad={service.getConceptAssistantSettings}
          onSave={service.saveConceptAssistantSettings}
          onClose={closeSettings}
        />
      ) : null}
      {settingsPanel === 'titleGeneration' ? (
        <PromptSettingsDialog
          key="title"
          title="标题生成"
          description="仅在首次创建卡片时使用，修改后不影响已有卡片。标题需为单行、最多 40 个字符。"
          defaultPrompt={DEFAULT_TITLE_SYSTEM_PROMPT}
          onLoad={service.getTitleGenerationSettings}
          onSave={service.saveTitleGenerationSettings}
          onClose={closeSettings}
        />
      ) : null}
    </>
  )
}
