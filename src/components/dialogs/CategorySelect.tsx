import type { Category } from "../../lib/types";

/** Ordered depth-first list with depth, for selects and menus. */
export function flattenCategories(categories: Category[], exclude?: Set<string>): { c: Category; depth: number }[] {
  const out: { c: Category; depth: number }[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const c of categories.filter((x) => (x.parentId ?? null) === parent)) {
      if (exclude?.has(c.id)) continue;
      out.push({ c, depth });
      walk(c.id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

export function descendantIds(categories: Category[], id: string): string[] {
  return categories.filter((c) => c.parentId === id).flatMap((c) => [c.id, ...descendantIds(categories, c.id)]);
}

export function CategorySelect({
  categories,
  value,
  onChange,
  noneLabel = "Uncategorized",
  exclude,
  id,
}: {
  categories: Category[];
  value: string | null;
  onChange: (v: string | null) => void;
  noneLabel?: string;
  exclude?: Set<string>;
  id?: string;
}) {
  return (
    <select
      id={id}
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value || null)}
      className="h-10 w-full rounded-[10px] border border-line-strong bg-canvas px-2.5 text-sm outline-none focus:border-brand-500 focus:ring-[3px] focus:ring-brand-500/15"
    >
      <option value="">{noneLabel}</option>
      {flattenCategories(categories, exclude).map(({ c, depth }) => (
        <option key={c.id} value={c.id}>
          {" ".repeat(depth)}
          {c.name}
        </option>
      ))}
    </select>
  );
}
