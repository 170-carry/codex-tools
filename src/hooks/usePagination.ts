import { useState } from "react";
export function usePagination<T>(items: T[], size = 25) {
  const [selection, select] = useState({ source: items, page: 0 });
  const pages = Math.max(1, Math.ceil(items.length / size));
  const page = selection.source === items ? Math.min(selection.page, pages - 1) : 0;
  return { items: items.slice(page * size, (page + 1) * size), page, pages, size, total: items.length, onChange: (next: number) => select({ source: items, page: next }) };
}
