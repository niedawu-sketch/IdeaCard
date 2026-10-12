import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AnalysisSet } from "../entities/analysis/model";
import { useIdeaCardStore } from "./useIdeaCardStore";

const storageKey = "ideacard-prototype-v1";

describe("useIdeaCardStore", () => {
  beforeEach(() => {
    localStorage.clear();
    useIdeaCardStore.getState().reset();
  });

  afterEach(() => {
    localStorage.clear();
  });

  it("creates a card at the top of the inbox with content and note kept separate", () => {
    const id = useIdeaCardStore.getState().createCard({
      title: "临时问题",
      content: "如何缩短记录动作？",
      note: "先减少保存前的必填项。",
      type: "thought",
      sourceType: "personal",
      sourceName: "快速记录",
      sourceUrl: "",
    });

    expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
      id,
      content: "如何缩短记录动作？",
      note: "先减少保存前的必填项。",
      sourceType: "personal",
      status: "unprocessed",
      tags: [],
    });
  });

  it("rehydrates empty storage without losing seed data or store methods", async () => {
    localStorage.clear();

    await expect(useIdeaCardStore.persist.rehydrate()).resolves.toBeUndefined();

    expect(useIdeaCardStore.getState().cards).toHaveLength(6);
    expect(useIdeaCardStore.getState().cards[0].id).toBe(
      "card-reading-distance",
    );
    expect(useIdeaCardStore.getState().createCard).toBeTypeOf("function");
    expect(useIdeaCardStore.getState().updateCard).toBeTypeOf("function");
    expect(useIdeaCardStore.getState().reset).toBeTypeOf("function");
    expect(useIdeaCardStore.persist.hasHydrated()).toBe(true);
  });

  it("ignores damaged persisted cards without rewriting storage", async () => {
    const persistedState = {
      state: {
        cards: [
          null,
          "damaged card",
          {
            id: "card-unsupported",
            title: "不支持的类型",
            content: "应被忽略",
            note: "",
            type: "unsupported",
            sourceName: "来源",
            sourceUrl: "",
            status: "unprocessed",
            createdAt: "2026-01-01T00:00:00.000Z",
            updatedAt: "2026-01-01T00:00:00.000Z",
          },
          {
            id: "card-legacy-article",
            title: "旧网页卡片",
            content: "合法旧卡片内容",
            note: "旧笔记",
            type: "article",
            sourceName: "旧来源",
            sourceUrl: "https://example.com/legacy",
            status: "unprocessed",
            tags: ["旧标签", 42, null],
            createdAt: "2026-01-01T00:00:00.000Z",
            updatedAt: "2026-01-01T00:00:00.000Z",
          },
        ],
        analyses: [],
        insights: [],
        selectedCardIds: [],
      },
      version: 0,
    };
    const rawValue = JSON.stringify(persistedState);
    useIdeaCardStore.setState({
      cards: [],
      analyses: [],
      insights: [],
      selectedCardIds: [],
    });
    localStorage.setItem(storageKey, rawValue);

    await expect(useIdeaCardStore.persist.rehydrate()).resolves.toBeUndefined();

    expect(useIdeaCardStore.getState().cards).toEqual([
      expect.objectContaining({
        id: "card-legacy-article",
        sourceType: "web",
        tags: ["旧标签"],
      }),
    ]);
    expect(localStorage.getItem(storageKey)).toBe(rawValue);
  });

  it("does not let persisted data overwrite store methods", async () => {
    const originalMethods = {
      createCard: useIdeaCardStore.getState().createCard,
      updateCard: useIdeaCardStore.getState().updateCard,
      reset: useIdeaCardStore.getState().reset,
    };
    const persistedCards = useIdeaCardStore.getState().cards;
    localStorage.setItem(
      storageKey,
      JSON.stringify({
        state: {
          cards: persistedCards,
          analyses: [],
          insights: [],
          selectedCardIds: [],
          createCard: "corrupted",
          updateCard: null,
          reset: false,
        },
        version: 0,
      }),
    );

    try {
      await useIdeaCardStore.persist.rehydrate();

      const state = useIdeaCardStore.getState();
      expect(state.createCard).toBeTypeOf("function");
      expect(state.updateCard).toBeTypeOf("function");
      expect(state.reset).toBeTypeOf("function");
      expect(state.createCard).toBe(originalMethods.createCard);
      expect(state.updateCard).toBe(originalMethods.updateCard);
      expect(state.reset).toBe(originalMethods.reset);
    } finally {
      useIdeaCardStore.setState(originalMethods);
    }
  });

  it.each([
    ["string", JSON.stringify("invalid")],
    ["array", JSON.stringify([])],
    ["null", JSON.stringify(null)],
  ])("ignores a persisted %s state without losing seed data", async (_, value) => {
    localStorage.setItem(storageKey, value);

    await expect(useIdeaCardStore.persist.rehydrate()).resolves.toBeUndefined();

    expect(useIdeaCardStore.getState().cards).toHaveLength(6);
    expect(useIdeaCardStore.getState().createCard).toBeTypeOf("function");
    expect(useIdeaCardStore.getState().updateCard).toBeTypeOf("function");
    expect(useIdeaCardStore.getState().reset).toBeTypeOf("function");
  });

  it("marks an unprocessed card as annotated when a non-empty note is first added", () => {
    const card = useIdeaCardStore
      .getState()
      .cards.find((item) => item.status === "unprocessed");

    expect(card).toBeDefined();
    useIdeaCardStore
      .getState()
      .updateCard(card!.id, { note: "需要让入口随时可达" });

    const updatedCard = useIdeaCardStore
      .getState()
      .cards.find((item) => item.id === card!.id);
    expect(updatedCard?.status).toBe("annotated");
    expect(updatedCard?.updatedAt).not.toBe(card!.updatedAt);
  });

  it("marks an unprocessed card as annotated when replacing an existing note", () => {
    const id = useIdeaCardStore.getState().createCard({
      title: "已有笔记的未处理卡片",
      content: "原始内容",
      note: "原有笔记",
      type: "thought",
      sourceType: "personal",
      sourceName: "快速记录",
      sourceUrl: "",
    });

    useIdeaCardStore.getState().updateCard(id, { note: "更新后的笔记" });

    const updatedCard = useIdeaCardStore
      .getState()
      .cards.find((item) => item.id === id);
    expect(updatedCard?.status).toBe("annotated");
  });

  it("keeps an unprocessed card unchanged when updating only its title", () => {
    const id = useIdeaCardStore.getState().createCard({
      title: "已有笔记的未处理卡片",
      content: "原始内容",
      note: "原有笔记",
      type: "thought",
      sourceType: "personal",
      sourceName: "快速记录",
      sourceUrl: "",
    });

    useIdeaCardStore.getState().updateCard(id, { title: "更新后的标题" });

    const updatedCard = useIdeaCardStore
      .getState()
      .cards.find((item) => item.id === id);
    expect(updatedCard?.status).toBe("unprocessed");
  });

  it("respects an explicit status when updating a note", () => {
    const card = useIdeaCardStore
      .getState()
      .cards.find((item) => item.status === "unprocessed");

    expect(card).toBeDefined();
    useIdeaCardStore.getState().updateCard(card!.id, {
      note: "这张卡片应直接进入待分析。",
      status: "pending-analysis",
    });

    const updatedCard = useIdeaCardStore
      .getState()
      .cards.find((item) => item.id === card!.id);
    expect(updatedCard?.status).toBe("pending-analysis");
  });

  it("toggles card selection and clears all selected cards", () => {
    const [firstId, secondId] = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);

    useIdeaCardStore.getState().toggleCardSelection(firstId);
    useIdeaCardStore.getState().toggleCardSelection(secondId);
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([
      firstId,
      secondId,
    ]);

    useIdeaCardStore.getState().toggleCardSelection(firstId);
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([secondId]);

    useIdeaCardStore.getState().clearSelection();
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([]);
  });

  it("creates a draft analysis set from selected cards and clears selection", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);

    cardIds.forEach((id) =>
      useIdeaCardStore.getState().toggleCardSelection(id),
    );
    const analysisId = useIdeaCardStore
      .getState()
      .createAnalysisSet(useIdeaCardStore.getState().selectedCardIds);

    expect(useIdeaCardStore.getState().analyses[0]).toMatchObject({
      id: analysisId,
      title: "未命名分析",
      goal: "",
      mode: "themes",
      cardIds,
      status: "draft",
      followUps: [],
    });
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([]);
  });

  it("copies card ids when creating an analysis set and clears selection", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const expectedCardIds = [...cardIds];
    cardIds.forEach((id) =>
      useIdeaCardStore.getState().toggleCardSelection(id),
    );

    const analysisId = useIdeaCardStore
      .getState()
      .createAnalysisSet(useIdeaCardStore.getState().selectedCardIds);

    cardIds.push(useIdeaCardStore.getState().cards[2].id);

    const analysis = useIdeaCardStore
      .getState()
      .analyses.find((item) => item.id === analysisId);
    expect(analysis?.cardIds).toEqual(expectedCardIds);
    expect(analysis?.cardIds).not.toBe(cardIds);
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual([]);
  });

  it("rejects analysis sets with more than 20 cards", () => {
    const cardIds = Array.from({ length: 21 }, (_, index) => `card-${index}`);

    useIdeaCardStore.setState({ selectedCardIds: cardIds });

    expect(() =>
      useIdeaCardStore.getState().createAnalysisSet(cardIds),
    ).toThrow("单次分析最多选择 20 张卡片");
    expect(useIdeaCardStore.getState().analyses).toEqual([]);
    expect(useIdeaCardStore.getState().selectedCardIds).toEqual(cardIds);
  });

  it("updates an analysis while preserving fields outside the patch", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const analysisId = useIdeaCardStore
      .getState()
      .createAnalysisSet(cardIds);

    useIdeaCardStore.getState().updateAnalysis(analysisId, {
      goal: "归纳材料里的共同主题",
      mode: "actions",
      status: "running",
    });

    expect(
      useIdeaCardStore
        .getState()
        .analyses.find((analysis) => analysis.id === analysisId),
    ).toMatchObject({
      id: analysisId,
      goal: "归纳材料里的共同主题",
      mode: "actions",
      status: "running",
      cardIds,
    });
  });

  it("sets analysis output and marks the analysis completed", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const analysisId = useIdeaCardStore
      .getState()
      .createAnalysisSet(cardIds);

    const output = {
      summary: "核心结论",
      themes: [
        {
          title: "主题",
          detail: "说明",
          references: [{ cardId: cardIds[0], label: "来源" }],
        },
      ],
      conflicts: ["分歧"],
      actions: ["行动"],
      openQuestions: ["问题"],
    };

    useIdeaCardStore.getState().updateAnalysis(analysisId, {
      goal: "保留目标",
      mode: "conflicts",
      status: "running",
    });
    useIdeaCardStore.getState().setAnalysisOutput(analysisId, output);

    expect(
      useIdeaCardStore
        .getState()
        .analyses.find((analysis) => analysis.id === analysisId),
    ).toMatchObject({
      goal: "保留目标",
      mode: "conflicts",
      cardIds,
      status: "completed",
      output,
    });
  });

  it("adds a follow-up and saves a completed analysis as an insight", () => {
    const allCards = useIdeaCardStore.getState().cards;
    const cards = allCards.slice(0, 2);
    const untouchedCard = allCards[2];
    const analysis: AnalysisSet = {
      id: "analysis-completed",
      title: "阅读后的思考沉淀",
      goal: "归纳最值得验证的产品机会",
      mode: "actions",
      cardIds: cards.map((card) => card.id),
      status: "completed",
      output: {
        summary: "低摩擦捕获和后续回访缺一不可。",
        themes: [
          {
            title: "捕获与加工",
            detail: "捕获应快，判断应可追溯。",
            references: [{ cardId: cards[0].id, label: cards[0].title }],
          },
        ],
        conflicts: ["即时整理与稍后整理存在操作成本冲突。"],
        actions: ["验证用户在记录后 24 小时内的回访率。"],
        openQuestions: ["什么提醒方式不会造成打扰？"],
      },
      followUps: [],
      createdAt: "2026-06-10T08:00:00.000Z",
      updatedAt: "2026-06-10T08:00:00.000Z",
    };

    useIdeaCardStore.setState({ cards: allCards, analyses: [analysis], insights: [] });
    useIdeaCardStore.getState().addFollowUp(analysis.id, {
      question: "最值得优先验证的建议是什么？",
      answer: "优先验证快速记录后的回访处理率。",
    });

    const insightId = useIdeaCardStore.getState().saveInsight(analysis.id);

    expect(useIdeaCardStore.getState().insights[0]).toMatchObject({
      id: insightId,
      analysisId: analysis.id,
      title: analysis.title,
      goal: analysis.goal,
      cardIds: analysis.cardIds,
      followUps: [
        {
          question: "最值得优先验证的建议是什么？",
          answer: "优先验证快速记录后的回访处理率。",
        },
      ],
    });
    expect(
      useIdeaCardStore
        .getState()
        .cards.filter((card) => analysis.cardIds.includes(card.id))
        .every((card) => card.status === "concluded"),
    ).toBe(true);
    expect(
      useIdeaCardStore.getState().cards.find((card) => card.id === untouchedCard.id)
        ?.status,
    ).toBe(untouchedCard.status);
  });

  it("rejects invalid follow-up writes", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const draftAnalysisId = useIdeaCardStore.getState().createAnalysisSet(cardIds);

    expect(() =>
      useIdeaCardStore.getState().addFollowUp(draftAnalysisId, {
        question: "",
        answer: "回答",
      }),
    ).toThrow("追问内容和回答不能为空");

    expect(() =>
      useIdeaCardStore.getState().addFollowUp(draftAnalysisId, {
        question: "问题",
        answer: " ",
      }),
    ).toThrow("追问内容和回答不能为空");

    expect(() =>
      useIdeaCardStore.getState().addFollowUp(draftAnalysisId, {
        question: "问题",
        answer: "回答",
      }),
    ).toThrow("只能向已完成的分析追加追问");
  });

  it("returns the existing insight when saving the same analysis twice", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const analysisId = useIdeaCardStore.getState().createAnalysisSet(cardIds);
    const output = {
      summary: "核心结论",
      themes: [
        {
          title: "主题",
          detail: "说明",
          references: [{ cardId: cardIds[0], label: "来源" }],
        },
      ],
      conflicts: ["分歧"],
      actions: ["行动"],
      openQuestions: ["问题"],
    };

    useIdeaCardStore.getState().updateAnalysis(analysisId, {
      title: "可复用结论",
      goal: "形成稳定输出",
    });
    useIdeaCardStore.getState().setAnalysisOutput(analysisId, output);

    const firstInsightId = useIdeaCardStore.getState().saveInsight(analysisId);
    const secondInsightId = useIdeaCardStore.getState().saveInsight(analysisId);

    expect(secondInsightId).toBe(firstInsightId);
    expect(useIdeaCardStore.getState().insights).toHaveLength(1);
  });

  it("saves insight snapshots that do not change when the analysis is edited later", () => {
    const cardIds = useIdeaCardStore
      .getState()
      .cards.slice(0, 2)
      .map((card) => card.id);
    const analysisId = useIdeaCardStore.getState().createAnalysisSet(cardIds);
    const output = {
      summary: "核心结论",
      themes: [
        {
          title: "主题",
          detail: "说明",
          references: [{ cardId: cardIds[0], label: "来源" }],
        },
      ],
      conflicts: ["分歧"],
      actions: ["行动"],
      openQuestions: ["问题"],
    };

    useIdeaCardStore.getState().updateAnalysis(analysisId, {
      title: "快照结论",
      goal: "形成稳定输出",
    });
    useIdeaCardStore.getState().setAnalysisOutput(analysisId, output);
    useIdeaCardStore.getState().addFollowUp(analysisId, {
      question: "问题一",
      answer: "回答一",
    });

    const insightId = useIdeaCardStore.getState().saveInsight(analysisId);
    const analysisAfterSave = useIdeaCardStore
      .getState()
      .analyses.find((item) => item.id === analysisId);

    analysisAfterSave!.cardIds.push(useIdeaCardStore.getState().cards[2].id);
    analysisAfterSave!.output!.summary = "被直接污染的结论";
    analysisAfterSave!.output!.themes[0].references[0].label = "被直接污染的来源";
    analysisAfterSave!.followUps[0].question = "被直接污染的问题";
    useIdeaCardStore.getState().updateAnalysis(analysisId, {
      goal: "被修改的目标",
      cardIds: [cardIds[0]],
      output: {
        ...output,
        summary: "被修改的结论",
      },
      followUps: [],
    });

    const insight = useIdeaCardStore
      .getState()
      .insights.find((item) => item.id === insightId);
    expect(insight).toMatchObject({
      goal: "形成稳定输出",
      cardIds,
      output: {
        summary: "核心结论",
        themes: [
          expect.objectContaining({
            references: [expect.objectContaining({ label: "来源" })],
          }),
        ],
      },
      followUps: [expect.objectContaining({ question: "问题一" })],
    });
  });

  it("infers source type and preserves tags when rehydrating a legacy card", async () => {
    const legacyCard = {
      id: "legacy-web-card",
      title: "旧网页卡片",
      content: "刷新后仍然存在",
      note: "",
      type: "article",
      sourceName: "旧网页",
      sourceUrl: "https://example.com/legacy",
      tags: ["旧标签"],
      status: "unprocessed",
      createdAt: "2025-01-01T00:00:00.000Z",
      updatedAt: "2025-01-01T00:00:00.000Z",
    };

    useIdeaCardStore.setState({
      cards: [],
      analyses: [],
      insights: [],
      selectedCardIds: [],
    });
    const persistedState = JSON.stringify({
      state: {
        cards: [legacyCard],
        analyses: [],
        insights: [],
        selectedCardIds: [],
      },
      version: 0,
    });
    localStorage.setItem(storageKey, persistedState);

    await useIdeaCardStore.persist.rehydrate();

    expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
      id: "legacy-web-card",
      sourceType: "web",
      tags: ["旧标签"],
    });
    expect(localStorage.getItem(storageKey)).toBe(persistedState);
  });

  it("infers source type when a persisted card contains an unknown value", async () => {
    const persistedState = JSON.stringify({
      state: {
        cards: [
          {
            id: "legacy-invalid-source",
            title: "非法来源值的旧网页卡片",
            content: "旧数据可能包含未支持的来源值",
            note: "",
            type: "article",
            sourceType: "unknown-source",
            sourceName: "旧网页",
            sourceUrl: "https://example.com/legacy-invalid-source",
            tags: ["旧标签"],
            status: "unprocessed",
            createdAt: "2025-01-01T00:00:00.000Z",
            updatedAt: "2025-01-01T00:00:00.000Z",
          },
        ],
        analyses: [],
        insights: [],
        selectedCardIds: [],
      },
      version: 0,
    });
    localStorage.setItem(storageKey, persistedState);

    await useIdeaCardStore.persist.rehydrate();

    expect(useIdeaCardStore.getState().cards[0]).toMatchObject({
      id: "legacy-invalid-source",
      sourceType: "web",
      tags: ["旧标签"],
    });
    expect(localStorage.getItem(storageKey)).toBe(persistedState);
  });

  it("restores fresh seed data on every reset", () => {
    const firstSeedCards = useIdeaCardStore.getState().cards;
    firstSeedCards[0].tags.push("测试污染");

    useIdeaCardStore.getState().reset();

    const restoredCards = useIdeaCardStore.getState().cards;
    expect(restoredCards).not.toBe(firstSeedCards);
    expect(restoredCards[0]).not.toBe(firstSeedCards[0]);
    expect(restoredCards[0].tags).not.toContain("测试污染");
    expect(useIdeaCardStore.getState()).toMatchObject({
      analyses: [],
      insights: [],
      selectedCardIds: [],
    });
  });
});
