export type AnalysisMode = "themes" | "conflicts" | "actions" | "custom";

export type AnalysisStatus =
  | "draft"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface CardReference {
  cardId: string;
  label: string;
}

export interface AnalysisOutput {
  summary: string;
  themes: Array<{
    title: string;
    detail: string;
    references: CardReference[];
  }>;
  conflicts: string[];
  actions: string[];
  openQuestions: string[];
}

export interface FollowUp {
  id: string;
  question: string;
  answer: string;
  cardId?: string;
  createdAt: string;
}

export interface AnalysisSet {
  id: string;
  title: string;
  goal: string;
  mode: AnalysisMode;
  cardIds: string[];
  status: AnalysisStatus;
  output?: AnalysisOutput;
  followUps: FollowUp[];
  createdAt: string;
  updatedAt: string;
}
