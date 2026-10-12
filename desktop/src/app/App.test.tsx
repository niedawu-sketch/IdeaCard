import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import type { DesktopService } from '../services/desktop/contracts'
import { tauriDesktopService } from '../services/desktop/tauriClient'
import { App } from './App'

describe('App', () => {
  it('renders the IdeaCard workspace', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: '收集箱' })).toBeInTheDocument()
  })

  it('collapses and expands the workspace rail, persisting the choice', async () => {
    window.localStorage.removeItem('ideacard.rail-collapsed')
    const user = userEvent.setup()
    const { container, unmount } = render(<App />)
    const shell = container.querySelector('.app-shell')

    expect(shell).not.toHaveClass('is-rail-collapsed')

    await user.click(screen.getByRole('button', { name: '收起侧栏' }))
    expect(shell).toHaveClass('is-rail-collapsed')
    expect(window.localStorage.getItem('ideacard.rail-collapsed')).toBe('1')

    await user.click(screen.getByRole('button', { name: '展开侧栏' }))
    expect(shell).not.toHaveClass('is-rail-collapsed')
    expect(window.localStorage.getItem('ideacard.rail-collapsed')).toBe('0')

    unmount()
    window.localStorage.setItem('ideacard.rail-collapsed', '1')
    const restored = render(<App />)
    expect(restored.container.querySelector('.app-shell')).toHaveClass('is-rail-collapsed')
  })

  it('offers model API and concept assistant settings from the footer', async () => {
    const user = userEvent.setup()
    const service = {
      ...tauriDesktopService,
      listCards: vi.fn().mockResolvedValue([]),
      listConcepts: vi.fn().mockResolvedValue([]),
      getConceptAssistantSettings: vi.fn().mockResolvedValue({
        systemPrompt: '自定义概念规则',
      }),
      saveConceptAssistantSettings: vi.fn(),
    generateCardTitle: vi.fn(),
    getTitleGenerationSettings: vi.fn(),
    saveTitleGenerationSettings: vi.fn(),
    } as DesktopService
    render(<App service={service} />)

    await user.click(screen.getByRole('button', { name: '设置' }))
    const menu = screen.getByRole('menu', { name: '设置' })
    expect(menu).toBeInTheDocument()
    expect(screen.getByRole('menuitem', { name: '模型 API 配置' })).toBeInTheDocument()

    await user.click(screen.getByRole('menuitem', { name: '提示词配置' }))
    await user.click(screen.getByRole('menuitem', { name: 'AI 概念助手' }))

    expect(service.getConceptAssistantSettings).toHaveBeenCalledTimes(1)
    expect(await screen.findByRole('dialog', { name: 'AI 概念助手' })).toBeInTheDocument()
    await waitFor(() => expect(screen.getByLabelText('System Prompt')).toHaveValue('自定义概念规则'))
  })

  it('deletes a concept explanation and selects the adjacent concept', async () => {
    const user = userEvent.setup()
    let concepts = [
      {
        id: 'concept-1',
        name: '第一个概念',
        explanation: '第一个解释',
        mentionCount: 2,
        updatedAt: '2026-07-29T08:00:00Z',
      },
      {
        id: 'concept-2',
        name: '第二个概念',
        explanation: '第二个解释',
        mentionCount: 1,
        updatedAt: '2026-07-29T07:00:00Z',
      },
    ]
    const service = {
      ...tauriDesktopService,
      listCards: vi.fn().mockResolvedValue([]),
      listConcepts: vi.fn().mockImplementation(async () => concepts),
      getConcept: vi.fn().mockImplementation(async (id: string) => ({
        ...concepts.find((concept) => concept.id === id),
        mentions: [],
      })),
      deleteConcept: vi.fn().mockImplementation(async (id: string) => {
        concepts = concepts.filter((concept) => concept.id !== id)
      }),
    } as DesktopService
    render(<App service={service} />)

    await user.click(screen.getByRole('button', { name: '概念集合' }))
    expect(await screen.findByRole('heading', { name: '第一个概念' })).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '删除概念' }))
    const dialog = screen.getByRole('dialog', { name: '删除概念' })
    await user.click(within(dialog).getByRole('button', { name: '删除概念' }))

    await waitFor(() => expect(service.deleteConcept).toHaveBeenCalledWith('concept-1'))
    expect(await screen.findByRole('heading', { name: '第二个概念' })).toBeInTheDocument()
    expect(screen.queryByText('第一个解释')).not.toBeInTheDocument()
  })
})
