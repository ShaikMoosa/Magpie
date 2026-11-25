/** Accent colours for Settings › General › Appearance (Figma 12). */
export type Accent = "violet" | "blue" | "teal" | "pink" | "orange" | "slate";

const SHADES = ["50", "100", "200", "300", "400", "500", "600", "700"] as const;

export const ACCENTS: Record<Accent, { label: string; swatch: string; shades: string[] }> = {
  violet: { label: "Violet", swatch: "#7a12ec", shades: ["#f6f0ff", "#ebddfe", "#dcc4fd", "#c79cfc", "#ad53fb", "#8917f9", "#7a12ec", "#4b00c1"] },
  blue: { label: "Blue", swatch: "#2563eb", shades: ["#eff6ff", "#dbeafe", "#bfdbfe", "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8"] },
  teal: { label: "Teal", swatch: "#0d9488", shades: ["#f0fdfa", "#ccfbf1", "#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e"] },
  pink: { label: "Pink", swatch: "#db2777", shades: ["#fdf2f8", "#fce7f3", "#fbcfe8", "#f9a8d4", "#f472b6", "#ec4899", "#db2777", "#be185d"] },
  orange: { label: "Orange", swatch: "#ea580c", shades: ["#fff7ed", "#ffedd5", "#fed7aa", "#fdba74", "#fb923c", "#f97316", "#ea580c", "#c2410c"] },
  slate: { label: "Slate", swatch: "#475569", shades: ["#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155"] },
};

/** Point the brand colour tokens at the chosen accent. */
export function applyAccent(accent: Accent) {
  const palette = ACCENTS[accent] ?? ACCENTS.violet;
  const root = document.documentElement.style;
  SHADES.forEach((s, i) => root.setProperty(`--color-brand-${s}`, palette.shades[i]));
}
