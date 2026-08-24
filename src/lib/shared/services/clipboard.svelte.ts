import { toast } from "svelte-sonner";

/**
 * Copies text to the clipboard with toast feedback.
 * Single implementation for every copy action in the app.
 *
 * @returns true when the copy succeeded, false otherwise — callers that
 * need branching (e.g. conditional UI) can check it; fire-and-forget
 * callers can ignore it.
 */
export async function copyToClipboard(
  value: string,
  label = "Copied to clipboard",
): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(label);
    return true;
  } catch (err) {
    console.error(err);
    toast.error("Could not copy");
    return false;
  }
}
