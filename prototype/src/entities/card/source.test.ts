import { describe, expect, it } from "vitest";
import {
  getSourceTypeLabel,
  inferLegacySourceType,
  isSourceType,
  sourceTypeOptions,
} from "./source";

describe("card source types", () => {
  it("exposes the fixed source type options", () => {
    expect(sourceTypeOptions).toEqual([
      { value: "personal", label: "个人记录" },
      { value: "web", label: "网页文章" },
      { value: "document", label: "文档资料" },
      { value: "image", label: "图片材料" },
      { value: "media", label: "音视频" },
    ]);
  });

  it("returns the user-facing source type label", () => {
    expect(getSourceTypeLabel("document")).toBe("文档资料");
  });

  it.each([
    ["personal", true],
    ["media", true],
    ["unknown", false],
    ["toString", false],
    [null, false],
  ])("validates source type value %s", (value, expected) => {
    expect(isSourceType(value)).toBe(expected);
  });

  it.each([
    ["thought", "personal"],
    ["clipboard", "personal"],
    ["article", "web"],
    ["screenshot", "image"],
  ] as const)("maps legacy card type %s to %s", (cardType, sourceType) => {
    expect(inferLegacySourceType(cardType)).toBe(sourceType);
  });
});
