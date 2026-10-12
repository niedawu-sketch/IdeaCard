import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { DesktopError } from '../../shared/errors/DesktopError'
import { QuickCapture } from './QuickCapture'

describe('QuickCapture', () => {
  it('keeps the draft and shows recovery guidance when saving fails', async () => {
    const user = userEvent.setup()
    const onSave = vi
      .fn()
      .mockRejectedValue(new DesktopError('database_operation_failed', '保存失败', '重新打开应用'))
    render(<QuickCapture onClose={vi.fn()} onSave={onSave} />)
    const dialog = screen.getByRole('dialog', { name: '快速记录' })

    fireEvent.input(within(dialog).getByLabelText('记录内容'), {
      target: { textContent: '不能丢失的草稿' },
    })
    await user.type(within(dialog).getByLabelText('我的想法'), '继续补充')
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('保存失败')
    expect(within(dialog).getByRole('alert')).toHaveTextContent('重新打开应用')
    expect(within(dialog).getByLabelText('记录内容')).toHaveTextContent('不能丢失的草稿')
    expect(within(dialog).getByLabelText('我的想法')).toHaveValue('继续补充')
  })

  it('allows an image-only record and uses the image fallback title', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(<QuickCapture onClose={vi.fn()} onSave={onSave} />)
    const dialog = screen.getByRole('dialog', { name: '快速记录' })
    const image = new File(['png'], '灵感截图.png', { type: 'image/png' })

    fireEvent.paste(within(dialog).getByLabelText('记录内容'), {
      clipboardData: { files: [image] },
    })
    await waitFor(() => {
      expect(within(dialog).getByRole('button', { name: '保存' })).toBeEnabled()
    })
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      title: '图片记录',
      originalContent: '',
      document: expect.objectContaining({
        content: expect.arrayContaining([expect.objectContaining({ type: 'image' })]),
      }),
    }))
  })
})
