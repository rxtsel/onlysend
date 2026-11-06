import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "svelte-sonner";
import { saveDrafts, confirmAppDeparture } from "./draft-navigation";

/** Keep the webview alive and its controls locked until Quit has flushed drafts. */
export async function protectAppExit(setBusy: (busy: boolean) => void): Promise<() => void> {
  let busy = false;
  const unlistenExit = await listen("app-exit-requested", async () => {
    if (busy) return;
    busy = true;
    setBusy(true);
    let approved = false;
    try {
      await saveDrafts();
      approved = confirmAppDeparture();
      if (!approved) toast.error("Wait for the current operation to finish before closing.");
      await invoke("complete_app_exit", { approved });
    } catch {
      approved = false;
      toast.error("Could not save your draft. The app will stay open; try again.");
      await invoke("complete_app_exit", { approved: false }).catch(() => {});
    } finally {
      if (!approved) { busy = false; setBusy(false); }
    }
  });
  try {
    const unlistenClose = await getCurrentWindow().onCloseRequested((event) => {
      event.preventDefault();
      void invoke("request_app_exit").catch(() => toast.error("Could not close the app. Please try again."));
    });
    return () => { unlistenClose(); unlistenExit(); };
  } catch (error) { unlistenExit(); throw error; }
}
