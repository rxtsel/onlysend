import { getContext, setContext } from "svelte";

const ACCOUNT_CONTEXT = Symbol("mail-account");

/** Getter supports the wizard; mail children capture it inside an account key. */
export function provideAccount(getId: () => string): void {
  setContext(ACCOUNT_CONTEXT, getId);
}

export function useAccountId(): string {
  const getId = getContext<(() => string) | undefined>(ACCOUNT_CONTEXT);
  const id = getId?.();
  if (!id) throw new Error("An explicit account context is required");
  return id;
}
