import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ConceptExplainDialog } from './ConceptExplainDialog'

describe('ConceptExplainDialog', () => {
  it('renders Markdown while saving the original text and opening safe links', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)
    const onOpenLink = vi.fn()
    const explanation = '## 定义\n\n**核心指标**\n\n- 衡量价值\n\n---\n\n| 指标 | 含义 |\n| --- | --- |\n| 留存 | 持续价值 |\n\n[参考](https://example.com)\n\n<script>alert(1)</script>'
    const { container } = render(<ConceptExplainDialog selectedText="北极星指标"
      explanation={explanation} generationStatus="complete" onSave={onSave}
      onOpenLink={onOpenLink} onCancel={vi.fn()} onRegenerate={vi.fn()} />)
    expect(screen.getByRole('heading', { name: '定义' })).toBeInTheDocument()
    expect(screen.getByText('核心指标').tagName).toBe('STRONG')
    expect(screen.getByRole('listitem')).toHaveTextContent('衡量价值')
    expect(screen.getByRole('table')).toBeInTheDocument()
    expect(container.querySelector('script')).toBeNull()
    await user.click(screen.getByRole('link', { name: '参考' }))
    expect(onOpenLink).toHaveBeenCalledWith('https://example.com/')
    await user.click(screen.getByRole('button', { name: '保存概念' }))
    expect(onSave).toHaveBeenCalledWith({ name: '北极星指标', explanation })
  })

  it('previews an explanation and saves the editable concept name', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <ConceptExplainDialog
        onOpenLink={vi.fn()}
        explanation="一句定义。当前语境中用于检验自己是否真正理解。"
        generationStatus="complete"
        selectedText="费曼学习法"
        onCancel={vi.fn()}
        onRegenerate={vi.fn()}
        onSave={onSave}
      />,
    )

    const name = screen.getByLabelText('概念名称')
    await user.clear(name)
    await user.type(name, '费曼方法')
    await user.click(screen.getByRole('button', { name: '保存概念' }))

    expect(onSave).toHaveBeenCalledWith({
      name: '费曼方法',
      explanation: '一句定义。当前语境中用于检验自己是否真正理解。',
    })
  })

  it('keeps the preview open and shows a recoverable save error', async () => {
    const user = userEvent.setup()
    render(
      <ConceptExplainDialog
        onOpenLink={vi.fn()}
        explanation="解释内容"
        generationStatus="complete"
        selectedText="概念"
        onCancel={vi.fn()}
        onRegenerate={vi.fn()}
        onSave={vi.fn().mockRejectedValue({
          code: 'concept_selection_stale',
          message: '原文选区已变化',
          recovery: '请重新选择文字后再试',
        })}
      />,
    )

    await user.click(screen.getByRole('button', { name: '保存概念' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('原文选区已变化')
    expect(screen.getByRole('dialog', { name: '解释概念' })).toBeInTheDocument()
  })

  it('shows a waiting state and prevents saving before generation completes', () => {
    const onSave = vi.fn()
    render(
      <ConceptExplainDialog
        onOpenLink={vi.fn()}
        explanation=""
        generationStatus="waiting"
        selectedText="涌现"
        onCancel={vi.fn()}
        onRegenerate={vi.fn()}
        onSave={onSave}
      />,
    )

    expect(screen.getByRole('status', { name: 'AI 正在思考' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '生成中...' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '重新生成' })).toBeDisabled()
  })

  it('warns before overwriting an existing explanation and locks its name', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn().mockResolvedValue(undefined)
    render(
      <ConceptExplainDialog
        onOpenLink={vi.fn()}
        explanation="新的 OPC 解释。"
        generationStatus="complete"
        overwriteConceptName="OPC"
        selectedText="OPC"
        onCancel={vi.fn()}
        onRegenerate={vi.fn()}
        onSave={onSave}
      />,
    )

    expect(screen.getByRole('note')).toHaveTextContent('将覆盖原有解释')
    expect(screen.getByRole('note')).toHaveTextContent('所有引用「OPC」的卡片')
    expect(screen.getByLabelText('概念名称')).toHaveAttribute('readonly')

    await user.click(screen.getByRole('button', { name: '覆盖原解释' }))
    expect(onSave).toHaveBeenCalledWith({
      name: 'OPC',
      explanation: '新的 OPC 解释。',
    })
  })
})
