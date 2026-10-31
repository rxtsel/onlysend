/** Selection follows visible row IDs, never positions that can change on refresh. */
export function createMailSelection() {
  let selected = $state<string[]>([]);
  let anchor: string | null = null;
  let cursor: string | null = null;

  function clear() { selected = []; anchor = null; cursor = null; }
  function click(id: string, ids: string[], shift: boolean, toggle: boolean, fallback?: string) {
    if (!ids.includes(id)) return;
    if (shift) {
      const start = anchor && ids.includes(anchor) ? anchor : fallback && ids.includes(fallback) ? fallback : id;
      anchor = start;
      const [a, b] = [ids.indexOf(start), ids.indexOf(id)].sort((a, b) => a - b);
      const range = ids.slice(a, b + 1);
      selected = toggle ? [...new Set([...selected, ...range])] : range;
    } else {
      selected = toggle ? selected.includes(id) ? selected.filter((value) => value !== id) : [...selected, id] : [];
      anchor = id;
    }
    cursor = id;
  }
  return {
    get ids() { return selected; },
    clear,
    click,
    move(ids: string[], direction: -1 | 1, fallback?: string) {
      if (!ids.length) return null;
      const start = cursor && ids.includes(cursor) ? cursor : fallback && ids.includes(fallback) ? fallback : ids[0];
      const next = ids[Math.max(0, Math.min(ids.length - 1, ids.indexOf(start) + direction))];
      click(next, ids, true, false, start);
      return next;
    },
    reconcile(ids: string[]) {
      const remaining = selected.filter((id) => ids.includes(id));
      if (remaining.length !== selected.length) selected = remaining;
      if (anchor && !ids.includes(anchor)) anchor = null;
      if (cursor && !ids.includes(cursor)) cursor = null;
    },
  };
}
