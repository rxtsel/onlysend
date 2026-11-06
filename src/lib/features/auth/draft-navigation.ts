type DraftGuard = { confirm: () => boolean; approved: boolean; save?: () => Promise<void> };
const guards = new Map<string, DraftGuard>();

/** Register only the mounted composer; cleanup cannot remove a newer guard. */
export function registerDraftGuard(accountId: string, confirm: () => boolean, save?: () => Promise<void>): () => void {
  const guard = { confirm, approved: false, save };
  guards.set(accountId, guard);
  return () => { if (guards.get(accountId) === guard) guards.delete(accountId); };
}

/** Flush pending edits before navigation, account operations or app restart. */
export async function saveDrafts(accountId?: string): Promise<void> {
  const selected = accountId ? [guards.get(accountId)] : [...guards.values()];
  await Promise.all(selected.map((guard) => guard?.save?.()));
}

/** Updates can exit the whole app. Approval must not leak into later navigation. */
export function confirmAppDeparture(): boolean {
  for (const guard of guards.values()) {
    if (!guard.confirm()) return false;
  }
  return true;
}

/** Confirm before destructive account operations, while the draft is mounted. */
export function approveAccountDeparture(accountId: string): boolean {
  const guard = guards.get(accountId);
  if (!guard) return true;
  guard.approved = guard.confirm();
  return guard.approved;
}

/** The navigation hook consumes prior approval instead of asking twice. */
export function allowAccountNavigation(accountId: string): boolean {
  const guard = guards.get(accountId);
  if (!guard) return true;
  if (guard.approved) {
    guard.approved = false;
    return true;
  }
  return guard.confirm();
}

export function resetDepartureApproval(accountId: string): void {
  const guard = guards.get(accountId);
  if (guard) guard.approved = false;
}
