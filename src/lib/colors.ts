// Colour helpers: category palette, tints and stable per-domain colours.

export const CATEGORY_PALETTE = [
  "#2563EB",
  "#7C3AED",
  "#DB2777",
  "#DC2626",
  "#EA580C",
  "#D97706",
  "#16A34A",
  "#0D9488",
  "#475569",
  "#64748B",
] as const;

export const UNCATEGORIZED_COLOR = "#64748B";

function parse(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)) as [number, number, number];
}

function toHex(rgb: number[]): string {
  return "#" + rgb.map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, "0")).join("");
}

/** Mix `hex` with white. amount 0 = original, 1 = white. */
export function lighten(hex: string, amount: number): string {
  return toHex(parse(hex).map((c) => c + (255 - c) * amount));
}

/** Mix `hex` with black. */
export function darken(hex: string, amount: number): string {
  return toHex(parse(hex).map((c) => c * (1 - amount)));
}

/** Solid light tint for pills and chips (equivalent to colour at `alpha` over white). */
export function tint(hex: string, alpha = 0.1): string {
  return lighten(hex, 1 - alpha);
}

export function isHexColor(s: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(s);
}

const FAVICON_COLORS = ["#1E1E1E", "#2563EB", "#7C3AED", "#DB2777", "#EA580C", "#0D9488", "#16A34A", "#4F46E5", "#0EA5E9", "#B45309"];

/** Stable colour for a domain's letter tile. */
export function domainColor(domain: string): string {
  let h = 0;
  for (const ch of domain) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return FAVICON_COLORS[h % FAVICON_COLORS.length];
}

export function domainLetter(domain: string): string {
  const name = domain.replace(/^www\./, "");
  return (name.match(/[a-z0-9]/i)?.[0] ?? "?").toUpperCase();
}
