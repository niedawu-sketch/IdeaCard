import { Mark, mergeAttributes } from '@tiptap/core'

export const ConceptMark = Mark.create({
  name: 'concept',

  addAttributes() {
    return {
      conceptId: { default: null },
      mentionId: { default: null },
    }
  },

  parseHTML() {
    return [{ tag: 'span[data-concept-id][data-mention-id]' }]
  },

  renderHTML({ HTMLAttributes }) {
    const { conceptId, mentionId } = HTMLAttributes
    return ['span', mergeAttributes(HTMLAttributes, {
      class: 'concept-mark',
      'data-concept-id': conceptId,
      'data-mention-id': mentionId,
    }), 0]
  },
})
