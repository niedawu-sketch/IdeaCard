import { describe, expect, it } from 'vitest'

import { canSaveCaptureDraft, captureSourceLabel } from './model'

describe('capture model', () => {
  it('requires content unless an attachment exists', () => {
    expect(canSaveCaptureDraft({ content: '   ', attachmentCount: 0 })).toBe(false)
    expect(canSaveCaptureDraft({ content: '   ', attachmentCount: 1 })).toBe(true)
  })

  it('labels automatic source classification', () => {
    expect(captureSourceLabel('web')).toBe('文档资料')
    expect(captureSourceLabel('image')).toBe('个人记录')
    expect(captureSourceLabel('media')).toBe('文档资料')
  })
})
