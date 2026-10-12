const maxInlineImageBytes = 20 * 1024 * 1024
const supportedImageMimeTypes = new Set([
  'image/png',
  'image/jpeg',
  'image/heic',
  'image/webp',
])

export interface ConceptMark {
  type: 'concept'
  attrs: {
    conceptId: string
    mentionId: string
  }
}

export interface TextNode {
  type: 'text'
  text: string
  marks?: ConceptMark[]
}

export interface HardBreakNode {
  type: 'hardBreak'
}

export interface ParagraphNode {
  type: 'paragraph'
  content?: Array<TextNode | HardBreakNode>
}

export interface ImageNode {
  type: 'image'
  attrs: {
    attachmentId: string | null
    tempId?: string | null
    data?: string | null
    alt: string
    title: string
  }
}

export type RichNode = ParagraphNode | ImageNode

export interface RichDocument {
  type: 'doc'
  content: RichNode[]
}

export class DocumentContentError extends Error {
  readonly code: 'inline_image_invalid' | 'inline_image_too_large'

  constructor(
    code: 'inline_image_invalid' | 'inline_image_too_large',
    message: string,
  ) {
    super(message)
    this.code = code
    this.name = 'DocumentContentError'
  }
}

export function emptyDocument(): RichDocument {
  return {
    type: 'doc',
    content: [{ type: 'paragraph' }],
  }
}

export function documentFromPlainText(value: string): RichDocument {
  if (!value) return emptyDocument()

  return {
    type: 'doc',
    content: value.split(/\r?\n/).map((line) => ({
      type: 'paragraph',
      ...(line ? { content: [{ type: 'text' as const, text: line }] } : {}),
    })),
  }
}

export function documentPlainText(document: RichDocument): string {
  return document.content
    .flatMap((node) => {
      if (node.type !== 'paragraph') return []
      const text = (node.content ?? [])
        .map((child) => child.type === 'hardBreak' ? '\n' : child.text)
        .join('')
      return text ? [text] : []
    })
    .join('\n')
}

export function hasDocumentContent(document: RichDocument): boolean {
  return document.content.some((node) => (
    node.type === 'image'
    || (node.content ?? []).some((child) => child.type === 'text' && child.text.trim().length > 0)
  ))
}

export async function clipboardImageNode(
  file: File,
  createId: () => string = () => crypto.randomUUID(),
): Promise<ImageNode> {
  if (!supportedImageMimeTypes.has(file.type)) {
    throw new DocumentContentError(
      'inline_image_invalid',
      '请粘贴 PNG、JPEG、HEIC 或 WebP 图片',
    )
  }
  if (file.size > maxInlineImageBytes) {
    throw new DocumentContentError(
      'inline_image_too_large',
      '粘贴的图片不能超过 20 MB',
    )
  }

  const data = await readFileAsDataUrl(file)
  return {
    type: 'image',
    attrs: {
      attachmentId: null,
      tempId: createId(),
      data,
      alt: file.name,
      title: '',
    },
  }
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.addEventListener('load', () => {
      if (typeof reader.result === 'string') {
        resolve(reader.result)
      } else {
        reject(new DocumentContentError('inline_image_invalid', '无法读取粘贴的图片'))
      }
    })
    reader.addEventListener('error', () => {
      reject(new DocumentContentError('inline_image_invalid', '无法读取粘贴的图片'))
    })
    reader.readAsDataURL(file)
  })
}
