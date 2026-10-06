type DraftGuard = { confirm: () => boolean; approved: boolean };
const guards = new Map<string, DraftGuard>();

/** Register only the mounted composer; cleanup cannot remove a newer guard. */
export function registerDraftGuard(accountId: string, confirm: () => boolean): () => void {
  const guard = { confirm, approved: false };
  guards.set(accountId, guard);
  return () => { if (guards.get(accountId) === guard) guards.delete(accountId); };
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
