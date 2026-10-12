import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
  AnalysisOutput,
  AnalysisSet,
  FollowUp,
} from "../entities/analysis/model";
import type {
  Card,
  CardStatus,
  CardType,
  CardUpdateInput,
  NewCardInput,
} from "../entities/card/model";
import {
  inferLegacySourceType,
  isSourceType,
} from "../entities/card/source";
import type { Insight } from "../entities/insight/model";
import { createId } from "../shared/lib/id";
import { seedCards } from "./seed";

interface IdeaCardState {
  cards: Card[];
  analyses: AnalysisSet[];
  insights: Insight[];
  selectedCardIds: string[];
  createCard: (input: NewCardInput) => string;
  updateCard: (id: string, patch: CardUpdateInput) => void;
  toggleCardSelection: (id: string) => void;
  clearSelection: () => void;
  createAnalysisSet: (cardIds: string[]) => string;
  updateAnalysis: (id: string, patch: Partial<AnalysisSet>) => void;
  setAnalysisOutput: (id: string, output: AnalysisOutput) => void;
  addFollowUp: (
    analysisId: string,
    input: { question: string; answer: string; cardId?: string },
  ) => void;
  saveInsight: (analysisId: string) => string;
  reset: () => void;
}

type StoredCard = Omit<Card, "sourceType" | "tags"> & {
  sourceType?: unknown;
  tags?: unknown;
};

function normalizeStoredCard(card: StoredCard): Card {
  return {
    ...card,
    sourceType: isSourceType(card.sourceType)
      ? card.sourceType
      : inferLegacySourceType(card.type),
    tags: Array.isArray(card.tags)
      ? card.tags.filter((tag): tag is string => typeof tag === "string")
      : [],
  };
}

function isCardType(value: unknown): value is CardType {
  return (
    value === "thought" ||
    value === "article" ||
    value === "clipboard" ||
    value === "screenshot"
  );
}

function isCardStatus(value: unknown): value is CardStatus {
  return (
    value === "unprocessed" ||
    value === "annotated" ||
    value === "pending-analysis" ||
    value === "concluded" ||
    value === "archived"
  );
}

function isStoredCard(value: unknown): value is StoredCard {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value.id === "string" &&
    typeof value.title === "string" &&
    typeof value.content === "string" &&
    typeof value.note === "string" &&
    typeof value.sourceName === "string" &&
    typeof value.sourceUrl === "string" &&
    typeof value.createdAt === "string" &&
    typeof value.updatedAt === "string" &&
    isCardType(value.type) &&
    isCardStatus(value.status)
  );
}

const createInitialState = () => ({
  cards: seedCards.map((card) => ({ ...card, tags: [...card.tags] })),
  analyses: [] as AnalysisSet[],
  insights: [] as Insight[],
  selectedCardIds: [] as string[],
});

export const useIdeaCardStore = create<IdeaCardState>()(
  persist(
    (set, get) => ({
      ...createInitialState(),
      createCard: (input) => {
        const now = new Date().toISOString();
        const id = createId("card");
        const card: Card = {
          ...input,
          tags: [],
          id,
          status: "unprocessed",
          createdAt: now,
          updatedAt: now,
        };

        set({ cards: [card, ...get().cards] });
        return id;
      },
      updateCard: (id, patch) =>
        set({
          cards: get().cards.map((card) => {
            if (card.id !== id) {
              return card;
            }

            const shouldAnnotate =
              patch.status === undefined &&
              card.status === "unprocessed" &&
              patch.note !== undefined &&
              Boolean(patch.note.trim());
            const status =
              patch.status ??
              (shouldAnnotate ? "annotated" : card.status);

            return {
              ...card,
              ...patch,
              status,
              updatedAt: new Date().toISOString(),
            };
          }),
        }),
      toggleCardSelection: (id) =>
        set({
          selectedCardIds: get().selectedCardIds.includes(id)
            ? get().selectedCardIds.filter((cardId) => cardId !== id)
            : [...get().selectedCardIds, id],
        }),
      clearSelection: () => set({ selectedCardIds: [] }),
      createAnalysisSet: (cardIds) => {
        if (cardIds.length > 20) {
          throw new Error("单次分析最多选择 20 张卡片");
        }

        const now = new Date().toISOString();
        const id = createId("analysis");
        const analysis: AnalysisSet = {
          id,
          title: "未命名分析",
          goal: "",
          mode: "themes",
          cardIds: [...cardIds],
          status: "draft",
          followUps: [],
          createdAt: now,
          updatedAt: now,
        };

        set({
          analyses: [analysis, ...get().analyses],
          selectedCardIds: [],
        });
        return id;
      },
      updateAnalysis: (id, patch) =>
        set({
          analyses: get().analyses.map((analysis) => {
            if (analysis.id !== id) {
              return analysis;
            }

            return {
              ...analysis,
              ...patch,
              id: analysis.id,
              cardIds:
                patch.cardIds === undefined
                  ? analysis.cardIds
                  : [...patch.cardIds],
              followUps:
                patch.followUps === undefined
                  ? analysis.followUps
                  : [...patch.followUps],
              createdAt: analysis.createdAt,
              updatedAt: new Date().toISOString(),
            };
          }),
        }),
      setAnalysisOutput: (id, output) =>
        set({
          analyses: get().analyses.map((analysis) =>
            analysis.id === id
              ? {
                  ...analysis,
                  output: cloneAnalysisOutput(output),
                  status: "completed",
                  updatedAt: new Date().toISOString(),
                }
              : analysis,
          ),
        }),
      addFollowUp: (analysisId, input) => {
        const question = input.question.trim();
        const answer = input.answer.trim();

        if (!question || !answer) {
          throw new Error("追问内容和回答不能为空");
        }

        const analysis = get().analyses.find((item) => item.id === analysisId);

        if (!analysis || analysis.status !== "completed" || !analysis.output) {
          throw new Error("只能向已完成的分析追加追问");
        }

        const followUp: FollowUp = {
          id: createId("follow-up"),
          question,
          answer,
          cardId: input.cardId,
          createdAt: new Date().toISOString(),
        };
        const now = new Date().toISOString();

        set({
          analyses: get().analyses.map((item) =>
            item.id === analysisId
              ? {
                  ...item,
                  followUps: [...item.followUps, followUp],
                  updatedAt: now,
                }
              : item,
          ),
        });
      },
      saveInsight: (analysisId) => {
        const existingInsight = get().insights.find(
          (insight) => insight.analysisId === analysisId,
        );

        if (existingInsight) {
          return existingInsight.id;
        }

        const analysis = get().analyses.find((item) => item.id === analysisId);

        if (!analysis || analysis.status !== "completed" || !analysis.output) {
          throw new Error("只有已完成的分析可以保存为结论");
        }

        const now = new Date().toISOString();
        const insight: Insight = {
          id: createId("insight"),
          analysisId: analysis.id,
          title: analysis.title,
          goal: analysis.goal,
          cardIds: [...analysis.cardIds],
          output: cloneAnalysisOutput(analysis.output),
          followUps: analysis.followUps.map(cloneFollowUp),
          createdAt: now,
        };

        set({
          insights: [insight, ...get().insights],
          cards: get().cards.map((card) =>
            analysis.cardIds.includes(card.id)
              ? {
                  ...card,
                  status: "concluded",
                  updatedAt: now,
                }
              : card,
          ),
        });

        return insight.id;
      },
      reset: () => set(createInitialState()),
    }),
    {
      name: "ideacard-prototype-v1",
      merge: (persistedState, currentState) => {
        const persisted = isRecord(persistedState) ? persistedState : {};

        return {
          ...currentState,
          cards: Array.isArray(persisted.cards)
            ? persisted.cards.filter(isStoredCard).map(normalizeStoredCard)
            : currentState.cards,
          analyses: Array.isArray(persisted.analyses)
            ? [...persisted.analyses]
            : currentState.analyses,
          insights: Array.isArray(persisted.insights)
            ? [...persisted.insights]
            : currentState.insights,
          selectedCardIds: Array.isArray(persisted.selectedCardIds)
            ? [...persisted.selectedCardIds]
            : currentState.selectedCardIds,
        };
      },
    },
  ),
);

function cloneAnalysisOutput(output: AnalysisOutput): AnalysisOutput {
  return {
    summary: output.summary,
    themes: output.themes.map((theme) => ({
      title: theme.title,
      detail: theme.detail,
      references: theme.references.map((reference) => ({ ...reference })),
    })),
    conflicts: [...output.conflicts],
    actions: [...output.actions],
    openQuestions: [...output.openQuestions],
  };
}

function cloneFollowUp(followUp: FollowUp): FollowUp {
  return { ...followUp };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
