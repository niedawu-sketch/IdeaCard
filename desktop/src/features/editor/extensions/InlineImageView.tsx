import type { NodeViewProps } from '@tiptap/core'
import { NodeViewWrapper } from '@tiptap/react'
import { useEffect, useState } from 'react'

import type { InlineImageOptions } from './InlineImage'

interface InlineImageContentProps {
  alt: string
  attachmentId: string | null
  data: string | null
  loadImage: InlineImageOptions['loadImage']
  title: string
}

function InlineImageContent({
  alt,
  attachmentId,
  data,
  loadImage,
  title,
}: InlineImageContentProps) {
  const [source, setSource] = useState<string | null>(data)
  const [failed, setFailed] = useState(false)
  const cannotLoad = !data && (!attachmentId || !loadImage)

  useEffect(() => {
    if (data || !attachmentId || !loadImage) return
    let active = true
    void loadImage(attachmentId)
      .then((image) => {
        if (active) setSource(image.dataUrl)
      })
      .catch(() => {
        if (active) setFailed(true)
      })
    return () => {
      active = false
    }
  }, [attachmentId, data, loadImage])

  return (
    <NodeViewWrapper className="rich-editor-image" contentEditable={false}>
      {source ? <img alt={alt} src={source} title={title || undefined} /> : null}
      {!source && !failed && !cannotLoad
        ? <span className="rich-editor-image-placeholder">图片载入中...</span>
        : null}
      {failed || cannotLoad
        ? <span className="rich-editor-image-error">图片无法载入</span>
        : null}
    </NodeViewWrapper>
  )
}

export function InlineImageView({ node, extension }: NodeViewProps) {
  const { alt, attachmentId, data, tempId, title } = node.attrs as {
    alt: string
    attachmentId: string | null
    data: string | null
    tempId: string | null
    title: string
  }
  return (
    <InlineImageContent
      alt={alt}
      attachmentId={attachmentId}
      data={data}
      key={`${attachmentId ?? tempId ?? 'missing'}:${data ?? ''}`}
      loadImage={(extension.options as InlineImageOptions).loadImage}
      title={title}
    />
  )
}
