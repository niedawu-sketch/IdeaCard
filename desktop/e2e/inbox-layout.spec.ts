import { expect, test, type Page } from '@playwright/test'

const pageErrors = new WeakMap<Page, Error[]>()

const cards = [
  {
    id: 'card-1',
    title: '从信息到判断的距离',
    originalContent: '产品的核心不是记录更多，而是缩短从信息到形成判断的距离。',
    note: '需要把记录后的加工动作放在主流程里。',
    sourceType: 'document',
    captureMethod: 'file',
    sourceName: '产品讨论纪要.pdf',
    sourceUri: '',
    status: 'annotated',
    attachments: [],
    createdAt: '2026-07-14T10:00:00Z',
    updatedAt: '2026-07-14T11:00:00Z',
    archivedAt: null,
  },
  {
    id: 'card-2',
    title: '待验证的问题',
    originalContent: '用户会不会在记录后的二十四小时内再次打开这张卡片？',
    note: '',
    sourceType: 'personal',
    captureMethod: 'quick',
    sourceName: '快速记录',
    sourceUri: '',
    status: 'unprocessed',
    attachments: [],
    createdAt: '2026-07-14T09:00:00Z',
    updatedAt: '2026-07-14T09:30:00Z',
    archivedAt: null,
  },
  {
    id: 'card-archived',
    title: '已归档的旧想法',
    originalContent: '已完成整理，暂时退出日常收集流程。',
    note: '保留以后复查',
    sourceType: 'document',
    captureMethod: 'file',
    sourceName: '旧版方案.pdf',
    sourceUri: '',
    status: 'archived',
    attachments: [],
    createdAt: '2026-07-10T09:00:00Z',
    updatedAt: '2026-07-10T10:00:00Z',
    archivedAt: '2026-07-10T10:00:00Z',
  },
]

test.beforeEach(async ({ page }) => {
  const errors: Error[] = []
  pageErrors.set(page, errors)
  page.on('pageerror', (error) => errors.push(error))

  await page.addInitScript((seedCards) => {
    let generatedTitleCount = 0
    let nextCallbackId = 1
    let nextListenerId = 1
    const callbacks = new Map<number, (event: unknown) => void>()
    let aiDeltaHandlerId: number | null = null
    let runtimeCards = [...seedCards]
    let runtimeConcepts = [{
      id: 'concept-1',
      name: '判断',
      explanation: '基于信息形成可行动的认识。',
      mentionCount: 1,
      updatedAt: '2026-07-19T10:00:00Z',
      mentions: [{
        mentionId: 'mention-1',
        cardId: 'card-1',
        cardTitle: '从信息到判断的距离',
        selectedText: '判断',
        cardStatus: 'annotated',
        createdAt: '2026-07-19T10:00:00Z',
      }],
    }]
    let titleGenerationSettings = { systemPrompt: '默认标题规则' }
    let conceptAssistantSettings = {
      systemPrompt: '默认概念助手规则',
    }
    let runtimeAiMessages = Array.from({ length: 16 }, (_, index) => ({
      id: `ai-message-${index + 1}`,
      role: index % 2 === 0 ? 'user' : 'assistant',
      content: index % 2 === 0
        ? `第 ${index / 2 + 1} 轮：接下来应该验证什么？`
        : index === 15
          ? '## 结论\n\n**先验证回访行为**，再观察后续动作。\n\n- 二十四小时内再次打开\n- 是否补充想法\n- 是否形成行动'
          : '先记录用户在捕获后二十四小时内是否再次打开卡片，再观察他们是否补充想法、形成判断或产生后续行动。',
      reasoningContent: index === 15 ? '先检索相关资料，再对照当前卡片的验证目标。' : '',
      webSearchEnabled: index === 14,
      citations: index === 15 ? [{
        url: 'https://example.com/research',
        title: '回访行为研究',
        position: 0,
      }] : [],
      createdAt: `2026-07-19T11:${String(index).padStart(2, '0')}:00Z`,
    }))
    const bridge = window as unknown as {
      __TAURI_INTERNALS__: {
        invoke(command: string, args?: Record<string, unknown>): Promise<unknown>
        transformCallback(callback?: unknown, once?: boolean): number
        unregisterCallback(id: number): void
      }
      __TAURI_EVENT_PLUGIN_INTERNALS__: {
        unregisterListener(event: string, eventId: number): void
      }
    }
    bridge.__TAURI_INTERNALS__ = {
      invoke: async (command, args) => {
        if (command === 'list_cards') {
          return args?.includeArchived
            ? runtimeCards
            : runtimeCards.filter((card) => card.status !== 'archived')
        }
        if (command === 'get_card_document') {
          const card = runtimeCards.find((item) => item.id === args?.cardId)
          if (!card) throw { code: 'card_not_found', message: '卡片不存在', recovery: '' }
          const document = card.id === 'card-1'
            ? {
                type: 'doc',
                content: [{
                  type: 'paragraph',
                  content: [
                    { type: 'text', text: '产品的核心不是记录更多，而是缩短从信息到形成' },
                    {
                      type: 'text',
                      text: '判断',
                      marks: [{
                        type: 'concept',
                        attrs: {
                          conceptId: 'concept-1',
                          mentionId: 'mention-1',
                        },
                      }],
                    },
                    { type: 'text', text: '的距离。' },
                  ],
                }],
              }
            : {
                type: 'doc',
                content: [{
                  type: 'paragraph',
                  content: [{ type: 'text', text: card.originalContent }],
                }],
              }
          return {
            cardId: card.id,
            document,
            schemaVersion: 1,
            updatedAt: card.updatedAt,
          }
        }
        if (command === 'get_ai_settings') {
          return {
            provider: 'openAi',
            baseUrl: 'https://api.openai.com/v1',
            model: 'gpt-5.6',
            webSearchEnabled: true,
            hasApiKey: new URLSearchParams(window.location.search).get('aiConfigured') === '1',
          }
        }
        if (command === 'update_card') {
          const patch = args?.patch as Partial<typeof runtimeCards[number]>
          const index = runtimeCards.findIndex((card) => card.id === args?.id)
          if (index < 0) throw new Error('卡片不存在')
          runtimeCards[index] = { ...runtimeCards[index], ...patch, updatedAt: new Date().toISOString() }
          return runtimeCards[index]
        }
        if (command === 'generate_card_title') return `AI 生成标题 ${++generatedTitleCount}`
        if (command === 'get_title_generation_settings') return titleGenerationSettings
        if (command === 'save_title_generation_settings') {
          titleGenerationSettings = args?.input as { systemPrompt: string }
          return titleGenerationSettings
        }
        if (command === 'get_concept_assistant_settings') {
          return conceptAssistantSettings
        }
        if (command === 'save_concept_assistant_settings') {
          const input = args?.input as { systemPrompt?: unknown } | undefined
          conceptAssistantSettings = {
            systemPrompt: String(input?.systemPrompt ?? ''),
          }
          return conceptAssistantSettings
        }
        if (command === 'list_card_ai_messages') {
          return runtimeAiMessages
        }
        if (command === 'send_card_ai_message') {
          const input = args?.input as {
            content: string
            requestId: string
          }
          const emitDelta = (delta: string) => {
            if (aiDeltaHandlerId === null) return
            callbacks.get(aiDeltaHandlerId)?.({
              event: 'ai://text-delta',
              id: 1,
              payload: { requestId: input.requestId, kind: 'answer', delta },
            })
          }
          const controlledStream = new URLSearchParams(window.location.search)
            .has('controlledAiStream')
          const waitForStreamStep = (step: 'first' | 'last') => new Promise<void>((resolve) => {
            window.addEventListener(`ideacard-test:stream-${step}`, () => resolve(), { once: true })
          })
          if (controlledStream) await waitForStreamStep('first')
          else await new Promise((resolve) => window.setTimeout(resolve, 120))
          emitDelta('先验证即时')
          if (controlledStream) await waitForStreamStep('last')
          else await new Promise((resolve) => window.setTimeout(resolve, 300))
          emitDelta('显示和连续追加。')
          runtimeAiMessages = [
            ...runtimeAiMessages,
            {
              id: 'ai-message-stream-user',
              role: 'user',
              content: input.content,
              reasoningContent: '',
              webSearchEnabled: false,
              citations: [],
              createdAt: '2026-07-31T01:00:00Z',
            },
            {
              id: 'ai-message-stream-assistant',
              role: 'assistant',
              content: '先验证即时显示和连续追加。',
              reasoningContent: '',
              webSearchEnabled: false,
              citations: [],
              createdAt: '2026-07-31T01:00:01Z',
            },
          ]
          return runtimeAiMessages
        }
        if (command === 'regenerate_card_ai_message') {
          const input = args?.input as { messageId: string; requestId: string }
          const emitDelta = (kind: 'reasoning' | 'answer', delta: string) => {
            if (aiDeltaHandlerId === null) return
            callbacks.get(aiDeltaHandlerId)?.({
              event: 'ai://text-delta',
              id: 1,
              payload: { requestId: input.requestId, kind, delta },
            })
          }
          emitDelta('reasoning', '重新核对证据')
          await new Promise((resolve) => window.setTimeout(resolve, 80))
          emitDelta('answer', '重新生成后的结论')
          runtimeAiMessages = runtimeAiMessages.map((message) => (
            message.id === input.messageId
              ? {
                  ...message,
                  content: '重新生成后的结论',
                  reasoningContent: '重新核对证据',
                  citations: [],
                }
              : message
          ))
          return runtimeAiMessages
        }
        if (command === 'explain_card_concept') {
          const input = args?.input as { selectedText: string }
          return {
            selectedText: input.selectedText,
            explanation: '## 一句话定义\n\n判断是把已有信息转化为可指导行动的认识。\n\n**核心作用**\n\n- 形成行动方向\n- 检验已有认识',
            citations: [],
          }
        }
        if (command === 'list_card_concepts') {
          return runtimeConcepts.flatMap((concept) => concept.mentions
            .filter((mention) => mention.cardId === args?.cardId)
            .map((mention) => ({
              id: concept.id,
              name: concept.name,
              explanation: concept.explanation,
              mentionId: mention.mentionId,
            })))
        }
        if (command === 'save_concept_annotation') {
          const input = args?.input as {
            cardId: string
            conceptId: string
            mentionId: string
            name: string
            explanation: string
            overwriteExisting: boolean
            document: unknown
          }
          if (!input.overwriteExisting) {
            throw { code: 'unexpected_annotation_mode', message: '应使用覆盖模式', recovery: '' }
          }
          runtimeConcepts = runtimeConcepts.map((concept) => concept.id === input.conceptId
            ? { ...concept, explanation: input.explanation }
            : concept)
          const concept = runtimeConcepts.find((item) => item.id === input.conceptId)
          return {
            concept,
            mention: {
              id: input.conceptId,
              name: input.name,
              explanation: input.explanation,
              mentionId: input.mentionId,
            },
            document: input.document,
            originalContent: runtimeCards.find((card) => card.id === input.cardId)?.originalContent ?? '',
            updatedAt: '2026-08-07T08:00:00Z',
          }
        }
        if (command === 'list_concepts') return runtimeConcepts
        if (command === 'get_concept') {
          const concept = runtimeConcepts.find((item) => item.id === args?.id)
          if (!concept) throw { code: 'concept_not_found', message: '概念不存在', recovery: '' }
          return concept
        }
        if (command === 'delete_concept') {
          runtimeConcepts = runtimeConcepts.filter((item) => item.id !== args?.id)
          return null
        }
        if (command === 'archive_card') {
          const card = runtimeCards.find((item) => item.id === args?.id)
          if (!card) throw { code: 'card_not_found', message: '卡片不存在', recovery: '' }
          const archivedCard = {
            ...card,
            status: 'archived',
            archivedAt: '2026-07-19T10:00:00Z',
            updatedAt: '2026-07-19T10:00:00Z',
          }
          runtimeCards = runtimeCards.map((item) => item.id === archivedCard.id ? archivedCard : item)
          return archivedCard
        }
        if (command === 'restore_card') {
          const card = runtimeCards.find((item) => item.id === args?.id)
          if (!card) throw { code: 'card_not_found', message: '卡片不存在', recovery: '' }
          const restoredCard = {
            ...card,
            status: card.note.trim() ? 'annotated' : 'unprocessed',
            archivedAt: null,
            updatedAt: '2026-07-19T10:01:00Z',
          }
          runtimeCards = runtimeCards.map((item) => item.id === restoredCard.id ? restoredCard : item)
          return restoredCard
        }
        if (command === 'delete_card') {
          runtimeCards = runtimeCards.filter((card) => card.id !== args?.id)
          return null
        }
        if (command === 'open_external_url') return null
        if (command === 'read_clipboard_text') return '剪贴板文字'
        if (command === 'write_clipboard_text') return null
        if (command === 'plugin:event|listen') {
          if (args?.event === 'ai://text-delta') {
            aiDeltaHandlerId = Number(args.handler)
          }
          return nextListenerId++
        }
        if (command === 'plugin:event|unlisten') {
          aiDeltaHandlerId = null
          return null
        }
        throw { code: 'unexpected_command', message: '测试命令未实现', recovery: '' }
      },
      transformCallback: (callback) => {
        const id = nextCallbackId++
        if (typeof callback === 'function') {
          callbacks.set(id, callback as (event: unknown) => void)
        }
        return id
      },
      unregisterCallback: (id) => {
        callbacks.delete(id)
      },
    }
    bridge.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => undefined,
    }
  }, cards)
})

test.afterEach(async ({ page }) => {
  expect(pageErrors.get(page) ?? []).toEqual([])
})

test('keeps the workspace within wide, standard, and minimum desktop widths', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  await expect(page.getByRole('heading', { name: '从信息到判断的距离' })).toBeVisible()
  const sources = page.getByRole('region', { name: '来源' })
  await expect(sources.getByRole('button', { name: '个人记录' })).toBeVisible()
  await expect(sources.getByRole('button', { name: '网页文章' })).toHaveCount(0)
  await expect(sources.getByRole('button', { name: '文档资料' })).toBeVisible()
  await expect(sources.getByRole('button', { name: '图片材料' })).toHaveCount(0)
  await expect(sources.getByRole('button', { name: '音视频' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: '保存修改' })).toHaveCount(0)
  await expect(page.getByText(/^已保存 \d{2}:\d{2}$/)).toBeVisible()

  const conceptMark = page.locator('.concept-mark')
  await expect(conceptMark).toBeVisible()
  const conceptDefaultStyle = await conceptMark.evaluate((element) => ({
    background: getComputedStyle(element).backgroundColor,
    boxShadow: getComputedStyle(element).boxShadow,
    outlineStyle: getComputedStyle(element).outlineStyle,
    weight: getComputedStyle(element).fontWeight,
  }))
  expect(conceptDefaultStyle.boxShadow).toBe('none')
  expect(conceptDefaultStyle.outlineStyle).toBe('none')
  expect(Number(conceptDefaultStyle.weight)).toBeGreaterThanOrEqual(600)
  await conceptMark.hover()
  await expect.poll(
    () => conceptMark.evaluate((element) => getComputedStyle(element).backgroundColor),
  ).not.toBe(conceptDefaultStyle.background)
  await page.mouse.move(900, 40)

  for (const scrollRegion of [
    page.locator('.inbox-detail-column'),
    page.locator('.card-detail .rich-editor'),
  ]) {
    const scrollbarStyle = await scrollRegion.evaluate((element) => ({
      width: getComputedStyle(element, '::-webkit-scrollbar').width,
      thumb: getComputedStyle(element, '::-webkit-scrollbar-thumb').backgroundColor,
      track: getComputedStyle(element, '::-webkit-scrollbar-track').backgroundColor,
      standardWidth: getComputedStyle(element).scrollbarWidth,
      standardColor: getComputedStyle(element).scrollbarColor,
    }))
    expect(scrollbarStyle.width).toBe('8px')
    expect(scrollbarStyle.thumb).not.toBe(scrollbarStyle.track)
    expect(scrollbarStyle.standardWidth).toBe('auto')
    expect(scrollbarStyle.standardColor).toBe('auto')
  }

  const nativeTextAreaHost = page.getByLabel('我的想法')
  await nativeTextAreaHost.evaluate((element) => element.classList.add('is-native-enabled'))
  await expect(nativeTextAreaHost).toHaveCSS('overflow', 'hidden')
  expect(await nativeTextAreaHost.evaluate(
    (element) => getComputedStyle(element, '::-webkit-scrollbar').display,
  )).toBe('none')

  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: '/tmp/ideacard-desktop-1440.png', fullPage: true })

  await page.setViewportSize({ width: 1280, height: 800 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: '/tmp/ideacard-desktop-1280.png', fullPage: true })

  await page.setViewportSize({ width: 1100, height: 700 })
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: '/tmp/ideacard-desktop-1100.png', fullPage: true })
})

test('keeps the original-content resize handle usable beside long scrolling content', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')

  const editor = page.locator('.card-detail .rich-editor')
  const editorContent = editor.locator('.rich-editor-content')
  await expect(editor).toHaveCSS('overflow-y', 'auto')
  await expect(editorContent).toHaveCSS('overflow-y', 'visible')

  const before = await editor.boundingBox()
  if (!before) throw new Error('missing original-content editor bounds')
  await page.mouse.move(before.x + before.width - 3, before.y + before.height - 3)
  await page.mouse.down()
  await page.mouse.move(before.x + before.width - 3, before.y + before.height + 70)
  await page.mouse.up()

  const after = await editor.boundingBox()
  if (!after) throw new Error('missing resized original-content editor bounds')
  expect(after.height).toBeGreaterThan(before.height + 40)
})

test('uses a full-height AI drawer with internal conversation scrolling', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/?aiConfigured=1')

  await page.getByRole('button', { name: 'AI 延伸', exact: true }).click()
  const drawer = page.getByRole('dialog', { name: 'AI 延伸' })
  await expect(drawer).toHaveCSS('transform', 'none')
  const conversation = page.locator('.ai-conversation')
  const messageList = page.getByLabel('AI 对话记录')
  const composer = page.locator('.ai-composer')
  const input = page.getByLabel('和 AI 讨论')
  const drawerBox = await drawer.boundingBox()
  const conversationBox = await conversation.boundingBox()
  const composerBox = await composer.boundingBox()
  const inputBox = await input.boundingBox()

  if (!drawerBox || !conversationBox || !composerBox || !inputBox) {
    throw new Error('missing AI conversation layout bounds')
  }
  expect(drawerBox.width).toBeLessThanOrEqual(400)
  expect(drawerBox.height).toBe(900)
  expect(drawerBox.x + drawerBox.width).toBe(1440)
  expect(conversationBox.width).toBeLessThan(drawerBox.width)
  expect(conversationBox.height).toBeGreaterThan(700)
  await expect(messageList).toHaveCSS('overflow-y', 'auto')
  expect(await messageList.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true)
  expect(await messageList.evaluate(
    (element) => Math.abs(element.scrollHeight - element.clientHeight - element.scrollTop),
  )).toBeLessThanOrEqual(1)
  expect(composerBox.width).toBeLessThan(conversationBox.width)
  expect(Math.abs(
    (composerBox.x - conversationBox.x)
    - (
      conversationBox.x + conversationBox.width
      - composerBox.x - composerBox.width
    ),
  )).toBeLessThanOrEqual(1)
  expect(composerBox.y + composerBox.height).toBeLessThanOrEqual(
    conversationBox.y + conversationBox.height + 1,
  )
  expect(inputBox.height).toBeLessThanOrEqual(46)
  const userMessage = page.getByRole('article', { name: '我的消息' }).first()
  const assistantMessage = page.getByRole('article', { name: 'AI 回复' }).first()
  const userBox = await userMessage.boundingBox()
  const assistantBox = await assistantMessage.boundingBox()
  if (!userBox || !assistantBox) throw new Error('missing multi-turn message bounds')
  expect(userBox.x).toBeGreaterThan(assistantBox.x)
  expect(userBox.width).toBeLessThan(assistantBox.width)
  await composer.scrollIntoViewIfNeeded()
  await page.screenshot({ path: '/tmp/ideacard-ai-drawer-1440.png', fullPage: true })
})

test('shows an AI turn immediately and replaces waiting with streamed text', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/?aiConfigured=1&controlledAiStream=1')
  await page.getByRole('button', { name: 'AI 延伸', exact: true }).click()

  const input = page.getByLabel('和 AI 讨论')
  await input.fill('这条消息需要立即出现')
  await page.getByRole('button', { name: '发送' }).click()

  await expect(page.getByRole('article', { name: '我的消息' }).last())
    .toContainText('这条消息需要立即出现')
  await expect(input).toHaveValue('')
  await expect(page.getByRole('status', { name: 'AI 正在思考' })).toBeVisible()

  await page.evaluate(() => window.dispatchEvent(new Event('ideacard-test:stream-first')))
  await expect(page.getByText('先验证即时')).toBeVisible()
  await expect(page.getByRole('status', { name: 'AI 正在思考' })).toHaveCount(0)
  await page.evaluate(() => window.dispatchEvent(new Event('ideacard-test:stream-last')))
  await expect(page.getByText('先验证即时显示和连续追加。')).toBeVisible()
})

test('renders structured AI answers with secondary reasoning and answer actions', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/?aiConfigured=1')
  await page.getByRole('button', { name: 'AI 延伸', exact: true }).click()

  const latestAnswer = page.getByRole('article', { name: 'AI 回复' }).last()
  await expect(latestAnswer.getByRole('heading', { name: '结论' })).toBeVisible()
  await expect(latestAnswer.getByText('先验证回访行为')).toBeVisible()
  await expect(latestAnswer.getByRole('button', { name: '回访行为研究' })).toBeVisible()
  const reasoning = latestAnswer.locator('.ai-reasoning')
  await expect(reasoning).not.toHaveAttribute('open', '')
  const [reasoningSize, answerSize, reasoningColor, answerColor] = await Promise.all([
    reasoning.evaluate((element) => Number.parseFloat(getComputedStyle(element).fontSize)),
    latestAnswer.locator('.ai-markdown').evaluate(
      (element) => Number.parseFloat(getComputedStyle(element).fontSize),
    ),
    reasoning.evaluate((element) => getComputedStyle(element).color),
    latestAnswer.locator('.ai-markdown').evaluate((element) => getComputedStyle(element).color),
  ])
  expect(reasoningSize).toBeLessThan(answerSize)
  expect(reasoningColor).not.toBe(answerColor)
  await page.screenshot({ path: '/tmp/ideacard-ai-response-markdown-1440.png', fullPage: true })

  await latestAnswer.getByRole('button', { name: '复制回答' }).click()
  await expect(latestAnswer.getByText('已复制')).toBeVisible()
  const userMessageCount = await page.getByRole('article', { name: '我的消息' }).count()
  await latestAnswer.getByRole('button', { name: '重新生成回答' }).click()
  await expect(latestAnswer.getByText('重新生成后的结论')).toBeVisible()
  await expect(page.getByRole('article', { name: '我的消息' })).toHaveCount(userMessageCount)
  await expect(latestAnswer.getByText('重新核对证据')).toBeAttached()
  await page.screenshot({ path: '/tmp/ideacard-ai-response-1440.png', fullPage: true })
})

test('opens quick capture without covering its actions', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 })
  await page.goto('/')
  await page.getByRole('button', { name: '快速记录', exact: true }).click()

  const dialog = page.getByRole('dialog', { name: '快速记录' })
  await expect(dialog.getByRole('button', { name: '保存' })).toBeVisible()
  await expect(dialog.getByRole('button', { name: '取消' })).toBeVisible()
  await page.screenshot({ path: '/tmp/ideacard-quick-capture-1280.png', fullPage: true })
})

test('keeps capture actions out of sources and opens the compact menu', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  const sourceFilter = page.getByRole('region', { name: '来源' })
  await expect(sourceFilter.getByRole('button', { name: '剪贴板' })).toHaveCount(0)
  await expect(sourceFilter.getByRole('button', { name: '导入文件' })).toHaveCount(0)

  await page.getByRole('button', { name: '更多捕获方式' }).click()
  const captureMenu = page.getByRole('menu')
  await expect(page.getByRole('menuitem', { name: '从剪贴板捕获' })).toBeVisible()
  await expect(page.getByRole('menuitem', { name: '导入本地文件' })).toBeVisible()
  await expectElementInside(captureMenu, page.locator('.inbox-list-column'))
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: '/tmp/ideacard-capture-menu-1100.png', fullPage: true })
})

test('opens both settings entries and persists the concept assistant prompt', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  await page.getByRole('button', { name: '设置' }).click()
  const menu = page.getByRole('menu', { name: '设置' })
  await expect(menu.getByRole('menuitem')).toHaveText(['模型 API 配置', '提示词配置'])
  await menu.getByRole('menuitem', { name: '提示词配置' }).hover()
  const submenu = page.getByRole('menu', { name: '提示词配置', exact: true })
  await expect(submenu).toBeVisible()
  await expectElementInsideViewport(submenu, page)
  const parentBounds = await menu.boundingBox()
  const childBounds = await submenu.boundingBox()
  expect(childBounds!.x).toBeGreaterThanOrEqual(parentBounds!.x + parentBounds!.width)
  await expectElementInsideViewport(menu, page)
  await page.screenshot({ path: '/tmp/ideacard-settings-menu-1100.png', fullPage: true })

  await menu.getByRole('menuitem', { name: 'AI 概念助手' }).click()
  const dialog = page.getByRole('dialog', { name: 'AI 概念助手' })
  const prompt = dialog.getByLabel('System Prompt')
  await expect(prompt).toHaveValue('默认概念助手规则')
  await prompt.fill('只解释概念，并指出它在原文中的具体含义。')
  await dialog.getByRole('button', { name: '保存' }).click()
  await expect(dialog.getByRole('status')).toContainText('已保存')
  await page.screenshot({ path: '/tmp/ideacard-concept-assistant-settings-1100.png', fullPage: true })
  await dialog.getByRole('button', { name: '关闭AI 概念助手' }).click()

  await page.getByRole('button', { name: '设置' }).click()
  await page.getByRole('menuitem', { name: '提示词配置' }).click()
  await page.getByRole('menuitem', { name: '标题生成' }).click()
  const titleDialog = page.getByRole('dialog', { name: '标题生成' })
  await expect(titleDialog.getByLabel('System Prompt')).toHaveValue('默认标题规则')
  await titleDialog.getByLabel('System Prompt').fill('提炼原文核心观点，输出一个简短标题。')
  await titleDialog.getByRole('button', { name: '保存', exact: true }).click()
  await expect(titleDialog.getByRole('status')).toContainText('已保存')
  await page.screenshot({ path: '/tmp/ideacard-title-settings-1100.png', fullPage: true })
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await page.getByRole('menuitem', { name: '提示词配置' }).click()
  await page.getByRole('menuitem', { name: '标题生成' }).click()
  await expect(page.getByLabel('System Prompt')).toHaveValue('提炼原文核心观点，输出一个简短标题。')
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: '收起侧栏' }).click()
  await page.getByRole('button', { name: '设置' }).click()
  await page.getByRole('menuitem', { name: '提示词配置' }).click()
  await expect(page.getByRole('menu', { name: '设置' })).toBeVisible()
  await expectElementInsideViewport(page.getByRole('menu', { name: '设置' }), page)
  await expectNoHorizontalOverflow(page)
})

test('shows only copy and AI explanation for a structured-text selection', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')
  const originalContent = page.getByLabel('原始内容')
  await originalContent.evaluate((element) => {
    const text = element.querySelector('p')?.firstChild
    if (!text) throw new Error('missing editor text')
    const range = document.createRange()
    range.setStart(text, 0)
    range.setEnd(text, 2)
    const selection = window.getSelection()
    selection?.removeAllRanges()
    selection?.addRange(range)
  })

  await originalContent.click({ button: 'right', position: { x: 40, y: 30 } })

  const menu = page.getByRole('menu', { name: '文本操作' })
  await expect(menu).toBeVisible()
  await expect(menu.getByRole('menuitem')).toHaveText(['复制', 'AI 解释概念'])
  await expectElementInsideViewport(menu, page)
  await page.screenshot({ path: '/tmp/ideacard-text-context-menu-1100.png', fullPage: true })
})

test('shows a saved concept explanation below its context menu', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  const conceptMark = page.locator('.concept-mark')
  await expect(conceptMark).toHaveAttribute('data-concept-name', '判断')
  await conceptMark.evaluate((element) => {
    element.setAttribute('data-concept-explanation', Array.from(
      { length: 12 },
      (_, index) => `第 ${index + 1} 点：基于信息形成可行动的认识，并说明它在当前卡片中的具体含义。`,
    ).join('\n\n'))
  })
  await conceptMark.click({ button: 'right' })

  const menu = page.getByRole('menu', { name: '文本操作' })
  const preview = page.getByRole('complementary', { name: '概念解释：判断' })
  await expect(menu).toBeVisible()
  await expect(preview).toContainText('第 1 点：基于信息形成可行动的认识')
  const [menuBox, previewBox] = await Promise.all([menu.boundingBox(), preview.boundingBox()])
  expect(menuBox).not.toBeNull()
  expect(previewBox).not.toBeNull()
  expect(previewBox!.y).toBeGreaterThanOrEqual(menuBox!.y + menuBox!.height)
  const previewTitle = preview.locator('.concept-context-preview-header strong')
  const openConcept = preview.getByRole('button', { name: '查看概念' })
  const [titleBox, actionBox, titleSize, actionSize] = await Promise.all([
    previewTitle.boundingBox(),
    openConcept.boundingBox(),
    previewTitle.evaluate((element) => Number.parseFloat(getComputedStyle(element).fontSize)),
    openConcept.evaluate((element) => Number.parseFloat(getComputedStyle(element).fontSize)),
  ])
  expect(titleBox).not.toBeNull()
  expect(actionBox).not.toBeNull()
  expect(actionBox!.x).toBeGreaterThan(titleBox!.x)
  expect(Math.abs(
    titleBox!.y + titleBox!.height / 2 - (actionBox!.y + actionBox!.height / 2),
  )).toBeLessThanOrEqual(4)
  expect(actionSize).toBeLessThan(titleSize)
  await expectElementInsideViewport(preview, page)
  await page.screenshot({ path: '/tmp/ideacard-concept-context-preview-header-1100.png', fullPage: true })
  expect(await preview.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true)
  await preview.hover()
  await page.mouse.wheel(0, 420)
  await expect(preview).toBeVisible()
  await expect.poll(() => preview.evaluate((element) => element.scrollTop)).toBeGreaterThan(0)
  await page.evaluate(() => window.dispatchEvent(new Event('blur')))
  await expect(preview).toBeVisible()
  const underlyingDetail = page.locator('.inbox-detail-column')
  const detailScrollBefore = await underlyingDetail.evaluate((element) => element.scrollTop)
  await page.mouse.wheel(0, 2_000)
  expect(await underlyingDetail.evaluate((element) => element.scrollTop)).toBe(detailScrollBefore)
  await page.screenshot({ path: '/tmp/ideacard-concept-context-preview-1100.png', fullPage: true })

  await page.locator('.text-context-dismiss-layer').click({ position: { x: 8, y: 8 } })
  await expect(preview).toHaveCount(0)
})

test('warns before replacing a saved concept explanation', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/?aiConfigured=1')

  const conceptMark = page.locator('.concept-mark')
  await conceptMark.click({ button: 'right' })
  await page.getByRole('menuitem', { name: 'AI 解释概念' }).click()

  const dialog = page.getByRole('dialog', { name: '解释概念' })
  await expect(dialog.getByRole('heading', { name: '一句话定义' })).toBeVisible()
  await expect(dialog.getByRole('listitem')).toHaveCount(2)
  await page.screenshot({ path: '/tmp/ideacard-concept-markdown.png', fullPage: true })
  await expect(dialog.getByRole('note')).toContainText('将覆盖原有解释')
  await expect(dialog.getByRole('note')).toContainText('所有引用「判断」的卡片')
  await expect(dialog.getByLabel('概念名称')).toHaveAttribute('readonly', '')
  await expect(dialog.getByRole('button', { name: '覆盖原解释' })).toBeVisible()
  await expectElementInsideViewport(dialog, page)
  await page.screenshot({ path: '/tmp/ideacard-concept-overwrite-warning-1100.png', fullPage: true })

  await dialog.getByRole('button', { name: '覆盖原解释' }).click()
  await expect(dialog).toHaveCount(0)
  await conceptMark.click({ button: 'right' })
  await expect(page.getByRole('complementary', { name: '概念解释：判断' }))
    .toContainText('判断是把已有信息转化为可指导行动的认识。')
})

test('opens the concept collection and deletes a concept after confirmation', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  await page.getByRole('button', { name: '概念集合' }).click()

  await expect(page.getByRole('heading', { name: '概念集合' })).toBeVisible()
  await expect(page.getByRole('heading', { name: '判断' })).toBeVisible()
  await expect(page.getByRole('button', { name: /从信息到判断的距离/ })).toBeVisible()
  await expectNoHorizontalOverflow(page)
  await page.screenshot({ path: '/tmp/ideacard-concepts-1100.png', fullPage: true })

  await page.getByRole('button', { name: '删除概念' }).click()
  const dialog = page.getByRole('dialog', { name: '删除概念' })
  await expect(dialog).toContainText('卡片文字不会被删除')
  await expectElementInsideViewport(dialog, page)
  await dialog.getByRole('button', { name: '删除概念' }).click()

  await expect(page.getByText('还没有保存的概念')).toBeVisible()
  await expect(page.getByRole('button', { name: '概念集合' })).toBeVisible()
  await expectNoHorizontalOverflow(page)
})

test('archives, restores, and permanently deletes cards across workspaces', async ({ page }) => {
  await page.setViewportSize({ width: 1100, height: 700 })
  await page.goto('/')

  await expect(page.getByRole('button', { name: '收集箱 2' })).toBeVisible()
  await expect(page.getByRole('button', { name: '已归档 1' })).toBeVisible()
  await page.getByRole('button', { name: '归档', exact: true }).click()
  await expect(page.getByRole('button', { name: '收集箱 1' })).toBeVisible()
  await expect(page.getByRole('button', { name: '已归档 2' })).toBeVisible()

  await page.getByRole('button', { name: '已归档 2' }).click()
  await expect(page.getByRole('button', { name: '恢复' })).toBeVisible()
  await expect(page.getByLabel('标题')).toHaveAttribute('readonly', '')
  await page.getByRole('button', { name: '恢复' }).click()
  await expect(page.getByRole('button', { name: '收集箱 2' })).toBeVisible()
  await expect(page.getByRole('button', { name: '已归档 1' })).toBeVisible()

  await page.getByRole('button', { name: '永久删除' }).click()
  const dialog = page.getByRole('dialog', { name: '永久删除卡片' })
  await expect(dialog).toContainText('删除后无法恢复，卡片内容和本地附件将一并删除。')
  await expectElementInsideViewport(dialog, page)
  await page.screenshot({ path: '/tmp/ideacard-delete-dialog-1100.png', fullPage: true })
  await dialog.getByRole('button', { name: '永久删除' }).click()

  await expect(page.getByRole('button', { name: '已归档 0' })).toBeVisible()
  await expect(page.getByText('还没有归档卡片。')).toBeVisible()
  await expectNoHorizontalOverflow(page)
})

async function expectElementInside(
  element: import('@playwright/test').Locator,
  container: import('@playwright/test').Locator,
) {
  const elementBox = await element.boundingBox()
  const containerBox = await container.boundingBox()

  expect(elementBox).not.toBeNull()
  expect(containerBox).not.toBeNull()
  expect(elementBox!.x).toBeGreaterThanOrEqual(containerBox!.x)
  expect(elementBox!.x + elementBox!.width).toBeLessThanOrEqual(containerBox!.x + containerBox!.width)
}

async function expectElementInsideViewport(
  element: import('@playwright/test').Locator,
  page: import('@playwright/test').Page,
) {
  const box = await element.boundingBox()
  const viewport = page.viewportSize()

  expect(box).not.toBeNull()
  expect(viewport).not.toBeNull()
  expect(box!.x).toBeGreaterThanOrEqual(0)
  expect(box!.y).toBeGreaterThanOrEqual(0)
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width)
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height)
}

async function expectNoHorizontalOverflow(page: import('@playwright/test').Page) {
  const dimensions = await page.evaluate(() => ({
    documentClientWidth: document.documentElement.clientWidth,
    documentScrollWidth: document.documentElement.scrollWidth,
    bodyClientWidth: document.body.clientWidth,
    bodyScrollWidth: document.body.scrollWidth,
  }))

  expect(dimensions.documentScrollWidth).toBe(dimensions.documentClientWidth)
  expect(dimensions.bodyScrollWidth).toBe(dimensions.bodyClientWidth)
}


test('regenerates a title repeatedly and autosaves the result', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.goto('/')
  const generate = page.getByRole('button', { name: 'AI 生成', exact: true })
  await generate.click()
  await expect(page.getByLabel('标题', { exact: true })).toHaveValue('AI 生成标题 1')
  await generate.click()
  await expect(page.getByLabel('标题', { exact: true })).toHaveValue('AI 生成标题 2')
  await expect(page.getByRole('heading', { name: 'AI 生成标题 2', exact: true })).toBeVisible()
  await page.screenshot({ path: '/tmp/ideacard-regenerate-title.png', fullPage: true })
})
