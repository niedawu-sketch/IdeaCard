import type { CardType, SourceType } from "./model";

const sourceTypeLabels: Record<SourceType, string> = {
  personal: "个人记录",
  web: "网页文章",
  document: "文档资料",
  image: "图片材料",
  media: "音视频",
};

const sourceTypes = [
  "personal",
  "web",
  "document",
  "image",
  "media",
] as const satisfies ReadonlyArray<SourceType>;

export const sourceTypeOptions = sourceTypes.map((value) => ({
  value,
  label: sourceTypeLabels[value],
}));

export function getSourceTypeLabel(sourceType: SourceType): string {
  return sourceTypeLabels[sourceType];
}

export function isSourceType(value: unknown): value is SourceType {
  return (
    typeof value === "string" &&
    Object.prototype.hasOwnProperty.call(sourceTypeLabels, value)
  );
}

export function inferLegacySourceType(cardType: CardType): SourceType {
  const legacySourceTypes: Record<CardType, SourceType> = {
    thought: "personal",
    clipboard: "personal",
    article: "web",
    screenshot: "image",
  };

  return legacySourceTypes[cardType];
}
