import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'

import type { DesktopService } from '../../services/desktop/contracts'
import { DesktopServiceProvider } from '../../services/desktop/context'
import { TextContextMenu } from './TextContextMenu'

function createService(overrides: Partial<DesktopService> = {}): DesktopService {
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
    revealAttachment: vi.fn(),
    readClipboardText: vi.fn().mockResolvedValue('剪贴板文字'),
    writeClipboardText: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

function renderMenu(service = createService()) {
  function ControlledTextarea() {
    const [value, setValue] = useState('保留删除保留')
    return (
      <textarea
        aria-label="测试文本"
        value={value}
        onChange={(event) => setValue(event.target.value)}
      />
    )
  }

  render(
    <DesktopServiceProvider service={service}>
      <ControlledTextarea />
      <p>普通页面文字</p>
      <button type="button">菜单外部</button>
      <TextContextMenu />
    </DesktopServiceProvider>,
  )
  return service
}

describe('TextContextMenu', () => {
  it('prevents the native menu and shows only four basic actions for editable text', async () => {
    renderMenu()
    const textarea = screen.getByLabelText('测试文本') as HTMLTextAreaElement
    textarea.setSelectionRange(2, 4)
    const contextEvent = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 80,
      clientY: 90,
    })

    textarea.dispatchEvent(contextEvent)

    expect(contextEvent.defaultPrevented).toBe(true)
    const menu = await screen.findByRole('menu', { name: '文本操作' })
    expect(menu).toBeInTheDocument()
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual([
      '剪切',
      '复制',
      '粘贴',
      '全选',
    ])
    for (const removedItem of ['查词', '翻译', '搜索', '检查元素', '服务']) {
      expect(screen.queryByText(removedItem)).not.toBeInTheDocument()
    }
  })

  it('cuts through the system clipboard and updates the controlled textarea', async () => {
    const user = userEvent.setup()
    const service = renderMenu()
    const textarea = screen.getByLabelText('测试文本') as HTMLTextAreaElement
    textarea.setSelectionRange(2, 4)
    fireEvent.contextMenu(textarea, { clientX: 80, clientY: 90 })

    await user.click(await screen.findByRole('menuitem', { name: '剪切' }))

    expect(service.writeClipboardText).toHaveBeenCalledWith('删除')
    expect(textarea).toHaveValue('保留保留')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('shows only copy for a normal document selection', async () => {
    renderMenu()
    const paragraph = screen.getByText('普通页面文字')
    const range = document.createRange()
    range.setStart(paragraph.firstChild!, 0)
    range.setEnd(paragraph.firstChild!, 2)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)

    fireEvent.contextMenu(paragraph, { clientX: 40, clientY: 50 })

    expect((await screen.findAllByRole('menuitem')).map((item) => item.textContent)).toEqual([
      '复制',
    ])
  })

  it('adds AI concept explanation only for an enabled rich-text selection', async () => {
    const user = userEvent.setup()
    const explain = vi.fn()
    render(
      <DesktopServiceProvider service={createService()}>
        <div data-ai-explain-enabled="true">
          <p onClick={() => undefined}>费曼学习法可以帮助理解</p>
        </div>
        <TextContextMenu />
      </DesktopServiceProvider>,
    )
    const paragraph = screen.getByText('费曼学习法可以帮助理解')
    const editor = paragraph.parentElement!
    editor.addEventListener('ideacard:explain-concept', (event) => {
      explain((event as CustomEvent).detail)
    })
    const range = document.createRange()
    range.setStart(paragraph.firstChild!, 0)
    range.setEnd(paragraph.firstChild!, 5)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)

    fireEvent.contextMenu(paragraph, { clientX: 40, clientY: 50 })

    expect((await screen.findAllByRole('menuitem')).map((item) => item.textContent)).toEqual([
      '复制',
      'AI 解释概念',
    ])
    await user.click(screen.getByRole('menuitem', { name: 'AI 解释概念' }))
    expect(explain).toHaveBeenCalledWith({ selectedText: '费曼学习法' })
  })

  it('shows a saved concept explanation below the menu without requiring a selection', async () => {
    const user = userEvent.setup()
    const openConcept = vi.fn()
    render(
      <DesktopServiceProvider service={createService()}>
        <div data-ai-explain-enabled="true">
          <span
            className="concept-mark"
            data-concept-explanation="通过尝试向他人讲清楚一个知识点来检验理解。"
            data-concept-id="concept-1"
            data-concept-name="费曼学习法"
            onClick={openConcept}
          >
            费曼学习法
          </span>
        </div>
        <TextContextMenu />
      </DesktopServiceProvider>,
    )
    window.getSelection()?.removeAllRanges()
    const mark = screen.getByText('费曼学习法')

    fireEvent.contextMenu(mark, { clientX: 40, clientY: 50 })

    const menu = await screen.findByRole('menu', { name: '文本操作' })
    const preview = screen.getByRole('complementary', { name: '概念解释：费曼学习法' })
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual([
      '复制',
      'AI 解释概念',
    ])
    expect(preview).toHaveTextContent('通过尝试向他人讲清楚一个知识点来检验理解。')
    expect(menu.nextElementSibling).toBe(preview)
    const previewHeader = preview.querySelector('.concept-context-preview-header')
    expect(previewHeader).toHaveTextContent('费曼学习法查看概念')
    expect(previewHeader?.querySelector('strong')?.nextElementSibling)
      .toBe(screen.getByRole('button', { name: '查看概念' }))

    await user.click(screen.getByRole('button', { name: '查看概念' }))
    expect(openConcept).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('disables selection actions when no text is selected and paste when clipboard is empty', async () => {
    renderMenu(createService({ readClipboardText: vi.fn().mockResolvedValue(null) }))
    const textarea = screen.getByLabelText('测试文本') as HTMLTextAreaElement
    textarea.setSelectionRange(0, 0)

    fireEvent.contextMenu(textarea, { clientX: 80, clientY: 90 })

    expect(await screen.findByRole('menuitem', { name: '剪切' })).toBeDisabled()
    expect(screen.getByRole('menuitem', { name: '复制' })).toBeDisabled()
    await waitFor(() => {
      expect(screen.getByRole('menuitem', { name: '粘贴' })).toBeDisabled()
    })
    expect(screen.getByRole('menuitem', { name: '全选' })).toBeEnabled()
  })

  it('stays open while scrolling or focus changes and closes on Escape or outside click', async () => {
    const user = userEvent.setup()
    renderMenu()
    const textarea = screen.getByLabelText('测试文本')

    fireEvent.contextMenu(textarea)
    expect(await screen.findByRole('menu')).toBeInTheDocument()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()

    fireEvent.contextMenu(textarea)
    fireEvent.mouseDown(document.querySelector('.text-context-dismiss-layer')!)
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()

    fireEvent.contextMenu(textarea)
    fireEvent.scroll(window)
    expect(screen.getByRole('menu')).toBeInTheDocument()

    fireEvent.scroll(screen.getByRole('menu').parentElement!)
    expect(screen.getByRole('menu')).toBeInTheDocument()

    fireEvent.mouseDown(screen.getByRole('menu'))
    expect(screen.getByRole('menu')).toBeInTheDocument()

    fireEvent.blur(window)
    expect(screen.getByRole('menu')).toBeInTheDocument()

    fireEvent.mouseDown(document.querySelector('.text-context-dismiss-layer')!)
    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
  })

  it('keeps the menu inside the viewport', async () => {
    renderMenu()
    const textarea = screen.getByLabelText('测试文本')
    fireEvent.contextMenu(textarea, {
      clientX: window.innerWidth - 1,
      clientY: window.innerHeight - 1,
    })

    const menu = await screen.findByRole('menu')
    const layer = menu.parentElement as HTMLElement
    expect(Number.parseFloat(layer.style.left)).toBeLessThan(window.innerWidth - 1)
    expect(Number.parseFloat(layer.style.top)).toBeLessThan(window.innerHeight - 1)
  })

  it('closes the menu and reports clipboard failures', async () => {
    const user = userEvent.setup()
    renderMenu(createService({
      writeClipboardText: vi.fn().mockRejectedValue({
        code: 'clipboard_write_failed',
        message: '无法写入剪贴板',
        recovery: '稍后重试',
      }),
    }))
    const textarea = screen.getByLabelText('测试文本') as HTMLTextAreaElement
    textarea.setSelectionRange(0, 2)
    fireEvent.contextMenu(textarea)

    await user.click(await screen.findByRole('menuitem', { name: '复制' }))

    expect(screen.queryByRole('menu')).not.toBeInTheDocument()
    expect(await screen.findByRole('alert')).toHaveTextContent('无法写入剪贴板')
    expect(screen.getByRole('alert')).toHaveTextContent('稍后重试')
  })
})
