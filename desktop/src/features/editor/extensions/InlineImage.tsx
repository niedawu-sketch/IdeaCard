import { mergeAttributes, Node } from '@tiptap/core'
import { ReactNodeViewRenderer } from '@tiptap/react'

import { InlineImageView } from './InlineImageView'

export interface LoadedInlineImage {
  mimeType: string
  dataUrl: string
}

export type InlineImageLoader = (attachmentId: string) => Promise<LoadedInlineImage>

export interface InlineImageOptions {
  loadImage?: InlineImageLoader
}

export function createInlineImageExtension(loadImage?: InlineImageLoader) {
  return Node.create<InlineImageOptions>({
    name: 'image',
    group: 'block',
    atom: true,
    draggable: false,

    addOptions() {
      return { loadImage }
    },

    addAttributes() {
      return {
        attachmentId: { default: null },
        tempId: { default: null },
        data: { default: null },
        alt: { default: '' },
        title: { default: '' },
      }
    },

    parseHTML() {
      return [{ tag: 'img[data-inline-image]' }]
    },

    renderHTML({ HTMLAttributes }) {
      return ['img', mergeAttributes(HTMLAttributes, { 'data-inline-image': '' })]
    },

    addNodeView() {
      return ReactNodeViewRenderer(InlineImageView)
    },
  })
}
