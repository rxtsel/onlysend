import { toast } from "svelte-sonner";

/** Opens the settings dialog on a given section (shell listens globally). */
export function openSettings(section: string): void {
  window.dispatchEvent(new CustomEvent("open-settings", { detail: { section } }));
}

/** True when an error looks like a missing/expired credential. */
export function isAuthError(err: unknown): boolean {
  const message = String(err ?? "");
  return (
    message.includes("Not authenticated") ||
    message.includes("authorization expired") ||
    message.includes("Not connected")
  );
}

export interface ToastOptions {
  duration?: number;
}

/**
 * Auth failures get a sticky toast with an action that opens
 * Settings > Connection, so recovery is one click away.
 */
export function authErrorToast(err: unknown): void {
  const message = isAuthError(err)
    ? "Not authenticated. Reconnect Resend to continue."
    : String(err ?? "")
        .replace(/^\[ERROR\]\s*/, "")
        .trim() || "Something went wrong";

  if (!isAuthError(err)) {
    toast.error(message);
    return;
  }

  toast.error(message, {
    action: {
      label: "Open settings",
      onClick: () => openSettings("Connection"),
    },
    duration: Infinity,
  });
}
