import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { documentFromPlainText, emptyDocument, type RichDocument } from '../../entities/document/model'
import type { CardConcept } from '../../entities/concept/model'
import { RichTextEditor } from './RichTextEditor'

describe('RichTextEditor', () => {
  it('renders a labelled plain-text document and reports text changes', async () => {
    const onChange = vi.fn()
    render(
      <RichTextEditor
        label="记录内容"
        value={documentFromPlainText('第一段')}
        onChange={onChange}
      />,
    )

    const editor = screen.getByRole('textbox', { name: '记录内容' })
    expect(editor).toHaveTextContent('第一段')

    fireEvent.input(editor, { target: { textContent: '修改后的内容' } })

    await waitFor(() => {
      expect(onChange).toHaveBeenCalled()
      expect(onChange.mock.lastCall?.[0].plainText).toContain('修改后的内容')
    })
  })

  it('inserts a clipboard image into the document at the current selection', async () => {
    const onChange = vi.fn()
    render(
      <RichTextEditor
        createId={() => 'temp-image-1'}
        label="记录内容"
        value={emptyDocument()}
        onChange={onChange}
      />,
    )

    const image = new File([new Uint8Array([137, 80, 78, 71])], 'capture.png', {
      type: 'image/png',
    })
    fireEvent.paste(screen.getByRole('textbox', { name: '记录内容' }), {
      clipboardData: { files: [image] },
    })

    await waitFor(() => {
      const value = onChange.mock.lastCall?.[0] as {
        document: RichDocument
        plainText: string
      }
      expect(value.document.content).toContainEqual({
        type: 'image',
        attrs: {
          attachmentId: null,
          tempId: 'temp-image-1',
          data: 'data:image/png;base64,iVBORw==',
          alt: 'capture.png',
          title: '',
        },
      })
      expect(value.plainText).toBe('')
    })
    expect(screen.getByRole('img', { name: 'capture.png' })).toHaveAttribute(
      'src',
      'data:image/png;base64,iVBORw==',
    )
  })

  it('rejects unsupported clipboard images without changing the document', async () => {
    const onChange = vi.fn()
    render(
      <RichTextEditor
        label="记录内容"
        value={emptyDocument()}
        onChange={onChange}
      />,
    )

    const image = new File(['gif'], 'moving.gif', { type: 'image/gif' })
    fireEvent.paste(screen.getByRole('textbox', { name: '记录内容' }), {
      clipboardData: { files: [image] },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent(
      '请粘贴 PNG、JPEG、HEIC 或 WebP 图片',
    )
    expect(onChange).not.toHaveBeenCalled()
  })

  it('does not accept image paste when the editor is read-only', () => {
    const onChange = vi.fn()
    render(
      <RichTextEditor
        label="记录内容"
        readOnly
        value={emptyDocument()}
        onChange={onChange}
      />,
    )

    const image = new File(['png'], 'capture.png', { type: 'image/png' })
    fireEvent.paste(screen.getByRole('textbox', { name: '记录内容' }), {
      clipboardData: { files: [image] },
    })

    expect(onChange).not.toHaveBeenCalled()
  })

  it('loads a persisted image without exposing its local path', async () => {
    const loadImage = vi.fn().mockResolvedValue({
      mimeType: 'image/png',
      dataUrl: 'data:image/png;base64,cGVyc2lzdGVk',
    })
    render(
      <RichTextEditor
        label="记录内容"
        loadImage={loadImage}
        value={{
          type: 'doc',
          content: [{
            type: 'image',
            attrs: {
              attachmentId: 'attachment-1',
              alt: '已保存图片',
              title: '',
            },
          }],
        }}
        onChange={vi.fn()}
      />,
    )

    expect(await screen.findByRole('img', { name: '已保存图片' })).toHaveAttribute(
      'src',
      'data:image/png;base64,cGVyc2lzdGVk',
    )
    expect(loadImage).toHaveBeenCalledWith('attachment-1')
  })

  it('preserves a rich-text selection for the AI explanation action', async () => {
    const onExplainSelection = vi.fn()
    render(
      <RichTextEditor
        label="原始内容"
        value={documentFromPlainText('需要解释的概念')}
        onChange={vi.fn()}
        onExplainSelection={onExplainSelection}
      />,
    )
    const editor = screen.getByRole('textbox', { name: '原始内容' })
    const text = editor.querySelector('p')?.firstChild
    const range = document.createRange()
    range.setStart(text!, 2)
    range.setEnd(text!, 4)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
    fireEvent.mouseUp(editor)

    fireEvent.contextMenu(editor)
    editor.parentElement?.parentElement?.dispatchEvent(new CustomEvent(
      'ideacard:explain-concept',
      { detail: { selectedText: '解释' } },
    ))

    await waitFor(() => {
      expect(onExplainSelection).toHaveBeenCalledWith(expect.objectContaining({
        text: '解释',
        revision: 0,
      }))
    })
  })

  it('can explain a saved concept directly from its right-click target', async () => {
    const onExplainSelection = vi.fn()
    render(
      <RichTextEditor
        concepts={[{
          id: 'concept-1',
          name: '费曼学习法',
          explanation: '通过讲解检验理解。',
          mentionId: 'mention-1',
        }]}
        label="原始内容"
        value={{
          type: 'doc',
          content: [{
            type: 'paragraph',
            content: [{
              type: 'text',
              text: '费曼学习法',
              marks: [{
                type: 'concept',
                attrs: { conceptId: 'concept-1', mentionId: 'mention-1' },
              }],
            }],
          }],
        }}
        onChange={vi.fn()}
        onExplainSelection={onExplainSelection}
      />,
    )
    const mark = screen.getByText('费曼学习法')
    window.getSelection()?.removeAllRanges()
    fireEvent.contextMenu(mark)
    mark.dispatchEvent(new CustomEvent('ideacard:explain-concept', {
      bubbles: true,
      detail: { selectedText: '费曼学习法' },
    }))

    await waitFor(() => {
      expect(onExplainSelection).toHaveBeenCalledWith(expect.objectContaining({
        text: '费曼学习法',
        revision: 0,
        conceptId: 'concept-1',
        mentionId: 'mention-1',
      }))
    })
  })

  it('recognizes a complete manual selection of a saved concept as an overwrite target', async () => {
    const onExplainSelection = vi.fn()
    render(
      <RichTextEditor
        concepts={[{
          id: 'concept-1',
          name: 'OPC',
          explanation: '原解释。',
          mentionId: 'mention-1',
        }]}
        label="原始内容"
        value={{
          type: 'doc',
          content: [{
            type: 'paragraph',
            content: [{
              type: 'text',
              text: 'OPC',
              marks: [{
                type: 'concept',
                attrs: { conceptId: 'concept-1', mentionId: 'mention-1' },
              }],
            }],
          }],
        }}
        onChange={vi.fn()}
        onExplainSelection={onExplainSelection}
      />,
    )
    const mark = screen.getByText('OPC')
    const range = document.createRange()
    range.selectNodeContents(mark)
    window.getSelection()?.removeAllRanges()
    window.getSelection()?.addRange(range)
    fireEvent.mouseUp(mark)
    mark.dispatchEvent(new CustomEvent('ideacard:explain-concept', { bubbles: true }))

    await waitFor(() => {
      expect(onExplainSelection).toHaveBeenCalledWith(expect.objectContaining({
        text: 'OPC',
        conceptId: 'concept-1',
        mentionId: 'mention-1',
      }))
    })
  })

  it('exposes a saved concept to the context menu and opens it on click', async () => {
    const user = userEvent.setup()
    const onOpenConcept = vi.fn()
    const concepts: CardConcept[] = [{
      id: 'concept-1',
      name: '费曼学习法',
      explanation: '通过尝试向他人讲清楚一个知识点来检验理解。',
      mentionId: 'mention-1',
    }]
    render(
      <RichTextEditor
        concepts={concepts}
        label="原始内容"
        readOnly
        value={{
          type: 'doc',
          content: [{
            type: 'paragraph',
            content: [{
              type: 'text',
              text: '费曼学习法',
              marks: [{
                type: 'concept',
                attrs: { conceptId: 'concept-1', mentionId: 'mention-1' },
              }],
            }],
          }],
        }}
        onChange={vi.fn()}
        onOpenConcept={onOpenConcept}
      />,
    )

    const mark = screen.getByText('费曼学习法')
    await waitFor(() => {
      expect(mark).toHaveAttribute('data-concept-name', '费曼学习法')
      expect(mark).toHaveAttribute(
        'data-concept-explanation',
        '通过尝试向他人讲清楚一个知识点来检验理解。',
      )
    })
    await user.hover(mark)
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument()
    await user.click(mark)
    expect(onOpenConcept).toHaveBeenCalledWith('concept-1')
  })

  it('applies a confirmed concept mark to the preserved selection', async () => {
    const onChange = vi.fn()
    const baseDocument = documentFromPlainText('费曼学习法')
    const { rerender } = render(
      <RichTextEditor
        label="原始内容"
        value={baseDocument}
        onChange={onChange}
      />,
    )

    rerender(
      <RichTextEditor
        annotationRequest={{
          conceptId: 'concept-1',
          mentionId: 'mention-1',
          from: 1,
          to: 6,
          revision: 0,
          nonce: 'annotation-1',
        }}
        label="原始内容"
        value={baseDocument}
        onChange={onChange}
      />,
    )

    await waitFor(() => {
      expect(onChange.mock.lastCall?.[0].document.content[0].content[0].marks).toEqual([{
        type: 'concept',
        attrs: {
          conceptId: 'concept-1',
          mentionId: 'mention-1',
        },
      }])
    })
  })
})
