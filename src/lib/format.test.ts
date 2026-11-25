import { describe, expect, it } from "vitest";
import { displayUrl, formatRelative, plural } from "./format";
import { domainLetter, isHexColor, lighten, tint } from "./colors";

describe("format", () => {
  it("shows relative days", () => {
    const now = new Date(2026, 10, 25, 15, 0);
    expect(formatRelative(new Date(2026, 10, 24, 9, 0).toISOString(), now)).toBe("Yesterday");
    expect(formatRelative(new Date(2026, 10, 25, 9, 5).toISOString(), now)).toMatch(/^Today, /);
    expect(formatRelative(null)).toBe("—");
  });

  it("pluralises", () => {
    expect(plural(1, "bookmark")).toBe("1 bookmark");
    expect(plural(1248, "bookmark")).toBe("1,248 bookmarks");
  });

  it("trims URLs for display", () => {
    expect(displayUrl("https://www.figma.com/community/")).toBe("figma.com/community");
  });
});

describe("colors", () => {
  it("validates hex", () => {
    expect(isHexColor("#0D9488")).toBe(true);
    expect(isHexColor("0D9488")).toBe(false);
    expect(isHexColor("#0D948")).toBe(false);
  });

  it("tints toward white", () => {
    expect(lighten("#000000", 1)).toBe("#ffffff");
    expect(tint("#7C3AED", 0.1)).toBe("#f2ebfd");
  });

  it("picks a letter for domains", () => {
    expect(domainLetter("www.figma.com")).toBe("F");
    expect(domainLetter("123.io")).toBe("1");
  });
});
