import type { AnalysisOutput, FollowUp } from "../analysis/model";

export interface Insight {
  id: string;
  analysisId: string;
  title: string;
  goal: string;
  cardIds: string[];
  output: AnalysisOutput;
  followUps: FollowUp[];
  createdAt: string;
}
