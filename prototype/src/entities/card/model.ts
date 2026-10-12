export type CardType = "thought" | "article" | "clipboard" | "screenshot";

export type SourceType = "personal" | "web" | "document" | "image" | "media";

export type CardStatus =
  | "unprocessed"
  | "annotated"
  | "pending-analysis"
  | "concluded"
  | "archived";

export interface Card {
  id: string;
  title: string;
  content: string;
  note: string;
  type: CardType;
  sourceType: SourceType;
  sourceName: string;
  sourceUrl: string;
  tags: string[];
  status: CardStatus;
  createdAt: string;
  updatedAt: string;
}

export type NewCardInput = Pick<
  Card,
  | "title"
  | "content"
  | "note"
  | "type"
  | "sourceType"
  | "sourceName"
  | "sourceUrl"
>;

export type CardUpdateInput = Partial<
  Pick<
    Card,
    | "title"
    | "content"
    | "note"
    | "type"
    | "sourceType"
    | "sourceName"
    | "sourceUrl"
    | "status"
  >
>;
