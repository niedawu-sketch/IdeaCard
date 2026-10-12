import { describe, expect, it } from 'vitest'

import {
  clipboardImageNode,
  documentFromPlainText,
  documentPlainText,
  emptyDocument,
  hasDocumentContent,
} from './model'

describe('rich document model', () => {
  it('converts legacy plain text into stable paragraphs', () => {
    expect(documentFromPlainText('第一段\r\n第二段')).toEqual({
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: '第一段' }] },
        { type: 'paragraph', content: [{ type: 'text', text: '第二段' }] },
      ],
    })
    expect(documentFromPlainText('')).toEqual(emptyDocument())
  })

  it('projects paragraphs, hard breaks, and concept text without image placeholders', () => {
    expect(documentPlainText({
      type: 'doc',
      content: [
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: '第一行' },
            { type: 'hardBreak' },
            {
              type: 'text',
              text: '概念',
              marks: [{
                type: 'concept',
                attrs: { conceptId: 'concept-1', mentionId: 'mention-1' },
              }],
            },
          ],
        },
        {
          type: 'image',
          attrs: { attachmentId: 'attachment-1', alt: '', title: '' },
        },
        { type: 'paragraph', content: [{ type: 'text', text: '第二段' }] },
      ],
    })).toBe('第一行\n概念\n第二段')
  })

  it('treats an image-only document as content but keeps its text projection empty', () => {
    const document = {
      type: 'doc' as const,
      content: [{
        type: 'image' as const,
        attrs: { attachmentId: 'attachment-1', alt: '', title: '' },
      }],
    }

    expect(hasDocumentContent(document)).toBe(true)
    expect(documentPlainText(document)).toBe('')
    expect(hasDocumentContent(emptyDocument())).toBe(false)
  })

  it('converts a supported clipboard image into a temporary inline node', async () => {
    const file = new File([new Uint8Array([137, 80, 78, 71])], 'capture.png', {
      type: 'image/png',
    })

    await expect(clipboardImageNode(file, () => 'temp-image-1')).resolves.toEqual({
      type: 'image',
      attrs: {
        attachmentId: null,
        tempId: 'temp-image-1',
        data: 'data:image/png;base64,iVBORw==',
        alt: 'capture.png',
        title: '',
      },
    })
  })

  it('rejects unsupported and oversized clipboard images', async () => {
    await expect(clipboardImageNode(
      new File(['gif'], 'moving.gif', { type: 'image/gif' }),
      () => 'temp-image-1',
    )).rejects.toMatchObject({ code: 'inline_image_invalid' })

    const oversized = new File(
      [new Uint8Array(20 * 1024 * 1024 + 1)],
      'large.png',
      { type: 'image/png' },
    )
    await expect(clipboardImageNode(oversized, () => 'temp-image-2'))
      .rejects.toMatchObject({ code: 'inline_image_too_large' })
  })
})
