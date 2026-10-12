import { AiMarkdown } from '../ai/AiMarkdown'
import { X } from 'lucide-react'
import { useEffect, useState } from 'react'

import { useDesktopService } from '../../services/desktop/useDesktopService'
import { DesktopError } from '../../shared/errors/DesktopError'
import { captureTextSelection, type TextSelectionSnapshot } from './textSelection'

const menuWidth = 168
const menuItemHeight = 34
const viewportGap = 8
const conceptPreviewWidth = 320
const conceptPreviewHeight = 220
const conceptPreviewGap = 6

interface ConceptPreview {
  target: HTMLElement
  name: string
  explanation: string
}

interface MenuState {
  x: number
  y: number
  selection: TextSelectionSnapshot
  clipboardText: string | null
  clipboardReady: boolean
  aiExplainTarget: HTMLElement | null
  conceptPreview: ConceptPreview | null
}

interface ErrorState {
  x: number
  y: number
  error: DesktopError
}

function menuPosition(
  x: number,
  y: number,
  itemCount: number,
  hasConceptPreview: boolean,
) {
  const menuHeight = itemCount * menuItemHeight + 10
  const layerWidth = hasConceptPreview
    ? Math.min(conceptPreviewWidth, window.innerWidth - viewportGap * 2)
    : menuWidth
  const layerHeight = menuHeight + (hasConceptPreview
    ? conceptPreviewGap + conceptPreviewHeight
    : 0)
  return {
    x: Math.max(viewportGap, Math.min(x, window.innerWidth - layerWidth - viewportGap)),
    y: Math.max(viewportGap, Math.min(y, window.innerHeight - layerHeight - viewportGap)),
  }
}

function conceptPreviewFromTarget(target: Element | null): ConceptPreview | null {
  const conceptTarget = target?.closest<HTMLElement>('[data-concept-id]') ?? null
  const name = conceptTarget?.dataset.conceptName?.trim() ?? ''
  const explanation = conceptTarget?.dataset.conceptExplanation?.trim() ?? ''
  if (!conceptTarget || !name || !explanation) return null
  return { target: conceptTarget, name, explanation }
}

export function TextContextMenu() {
  const service = useDesktopService()
  const [menu, setMenu] = useState<MenuState | null>(null)
  const [operationError, setOperationError] = useState<ErrorState | null>(null)

  useEffect(() => {
    function closeMenu() {
      setMenu(null)
    }

    function handleContextMenu(event: MouseEvent) {
      event.preventDefault()
      setOperationError(null)
      const selection = captureTextSelection(event.target)
      if (!selection) {
        setMenu(null)
        return
      }

      const eventTarget = event.target instanceof Element ? event.target : null
      const conceptPreview = conceptPreviewFromTarget(eventTarget)
      const aiExplainTarget = selection.kind === 'document'
        ? eventTarget?.closest<HTMLElement>('[data-ai-explain-enabled="true"]') ?? null
        : null
      const itemCount = selection.kind === 'control' ? 4 : aiExplainTarget ? 2 : 1
      const position = menuPosition(
        event.clientX,
        event.clientY,
        itemCount,
        conceptPreview !== null,
      )
      setMenu({
        ...position,
        selection,
        clipboardText: null,
        clipboardReady: selection.kind === 'document',
        aiExplainTarget,
        conceptPreview,
      })

      if (selection.kind === 'control' && selection.canPaste) {
        void service.readClipboardText()
          .then((clipboardText) => {
            setMenu((current) => current?.selection === selection
              ? { ...current, clipboardText, clipboardReady: true }
              : current)
          })
          .catch((error) => {
            setMenu(null)
            setOperationError({
              ...position,
              error: DesktopError.fromUnknown(error),
            })
          })
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') closeMenu()
    }

    document.addEventListener('contextmenu', handleContextMenu)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('contextmenu', handleContextMenu)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [service])

  async function copySelection(cut: boolean) {
    if (!menu) return
    const { selection, x, y } = menu
    try {
      await service.writeClipboardText(selection.selectedText)
      if (cut) selection.cut()
      setMenu(null)
    } catch (error) {
      setMenu(null)
      setOperationError({ x, y, error: DesktopError.fromUnknown(error) })
    }
  }

  function pasteSelection() {
    if (!menu?.clipboardText) return
    menu.selection.paste(menu.clipboardText)
    setMenu(null)
  }

  function selectAll() {
    menu?.selection.selectAll()
    setMenu(null)
  }

  function explainConcept() {
    if (!menu?.aiExplainTarget || !menu.selection.selectedText) return
    const dispatchTarget = menu.conceptPreview?.target ?? menu.aiExplainTarget
    dispatchTarget.dispatchEvent(new CustomEvent('ideacard:explain-concept', {
      bubbles: menu.conceptPreview !== null,
      detail: { selectedText: menu.selection.selectedText },
    }))
    setMenu(null)
  }

  function openConcept() {
    if (!menu?.conceptPreview) return
    menu.conceptPreview.target.click()
    setMenu(null)
  }

  return (
    <>
      {menu ? (
        <>
          <div
            aria-hidden="true"
            className="text-context-dismiss-layer"
            onMouseDown={() => setMenu(null)}
          />
          <div
            className={`text-context-layer${menu.conceptPreview ? ' has-concept-preview' : ''}`}
            style={{ left: menu.x, top: menu.y }}
            onWheelCapture={(event) => event.stopPropagation()}
          >
            <div aria-label="文本操作" className="text-context-menu" role="menu">
            {menu.selection.kind === 'control' ? (
              <>
                <button
                  disabled={!menu.selection.canCut}
                  role="menuitem"
                  type="button"
                  onClick={() => void copySelection(true)}
                >
                  剪切
                </button>
                <button
                  disabled={!menu.selection.canCopy}
                  role="menuitem"
                  type="button"
                  onClick={() => void copySelection(false)}
                >
                  复制
                </button>
                <button
                  disabled={
                    !menu.selection.canPaste
                    || !menu.clipboardReady
                    || !menu.clipboardText
                  }
                  role="menuitem"
                  type="button"
                  onClick={pasteSelection}
                >
                  粘贴
                </button>
                <button
                  disabled={!menu.selection.canSelectAll}
                  role="menuitem"
                  type="button"
                  onClick={selectAll}
                >
                  全选
                </button>
              </>
            ) : (
              <>
                <button
                  disabled={!menu.selection.canCopy}
                  role="menuitem"
                  type="button"
                  onClick={() => void copySelection(false)}
                >
                  复制
                </button>
                {menu.aiExplainTarget ? (
                  <button
                    disabled={!menu.selection.selectedText}
                    role="menuitem"
                    type="button"
                    onClick={explainConcept}
                  >
                    AI 解释概念
                  </button>
                ) : null}
              </>
            )}
            </div>
            {menu.conceptPreview ? (
              <aside
                aria-label={`概念解释：${menu.conceptPreview.name}`}
                className="concept-context-preview"
              >
                <header className="concept-context-preview-header">
                  <strong>{menu.conceptPreview.name}</strong>
                  <button type="button" onClick={openConcept}>
                    查看概念
                  </button>
                </header>
                <AiMarkdown content={menu.conceptPreview.explanation} onOpenLink={(url) => void service.openExternalUrl(url)} />
              </aside>
            ) : null}
          </div>
        </>
      ) : null}
      {operationError ? (
        <div
          className="text-context-menu-error"
          role="alert"
          style={{ left: operationError.x, top: operationError.y }}
        >
          <div>
            <strong>{operationError.error.message}</strong>
            {operationError.error.recovery ? <span>{operationError.error.recovery}</span> : null}
          </div>
          <button
            aria-label="关闭剪贴板错误"
            className="icon-button subtle"
            title="关闭"
            type="button"
            onClick={() => setOperationError(null)}
          >
            <X aria-hidden="true" size={14} />
          </button>
        </div>
      ) : null}
    </>
  )
}
