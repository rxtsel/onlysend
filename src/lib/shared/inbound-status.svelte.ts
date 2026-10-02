// Readiness belongs to a connection, never to the application as a whole.
const accounts = new Map<string, { ready: boolean }>();

export function getInboundStatus(accountId: string): { ready: boolean } {
  if (!accountId) throw new Error("Account ID is required");
  let status = accounts.get(accountId);
  if (!status) {
    const created = $state({ ready: false });
    status = created;
    accounts.set(accountId, status);
  }
  return status;
}

export function clearInboundStatus(accountId: string): void {
  const status = accounts.get(accountId);
  if (status) status.ready = false;
  accounts.delete(accountId);
}
