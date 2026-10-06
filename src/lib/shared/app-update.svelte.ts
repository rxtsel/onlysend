import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
const buildVersion = __APP_VERSION__;

export type UpdateInfo = {
  currentVersion: string;
  state: "disabled" | "current" | "available" | "error";
  version: string | null;
  notes: string | null;
  error: string | null;
  installSupported: boolean;
  downloaded: boolean;
  releaseUrl: string;
};
export type UpdateProgress = {
  phase: "downloading" | "verifying";
  downloaded: number;
  total: number | null;
};
type Dependencies = {
  enabled: () => boolean;
  check: (manual: boolean) => Promise<UpdateInfo>;
  download: (progress: (value: UpdateProgress) => void) => Promise<void>;
  install: () => Promise<void>;
};

export function createAppUpdateStore(deps: Dependencies) {
  const state = $state({
    currentVersion: buildVersion,
    phase: "disabled" as UpdateInfo["state"] | "checking" | UpdateProgress["phase"] | "downloaded" | "installing",
    info: null as UpdateInfo | null,
    error: null as string | null,
    downloaded: 0,
    total: null as number | null,
    verified: false,
  });
  let startup: Promise<void> | null = null;
  let operation: Promise<void> | null = null;
  let checking: Promise<void> | null = null;

  function check(manual: boolean): Promise<void> {
    if (checking) return checking;
    if (operation) return operation;
    checking = (async () => {
      if (!deps.enabled()) return;
      state.phase = "checking";
      try {
        const info = await deps.check(manual);
        state.info = info;
        state.currentVersion = info.currentVersion;
        state.phase = info.state;
        state.error = info.error;
        state.verified = info.downloaded;
      } catch (error) {
        state.phase = "error";
        state.error = error instanceof Error ? error.message : String(error);
      }
    })().finally(() => { checking = null; });
    return checking;
  }

  function checkOnce(): Promise<void> {
    startup ??= check(false);
    return startup;
  }

  function download(): Promise<void> {
    if (operation) return operation;
    if (checking) return checking;
    if (!state.info?.version || !state.info.installSupported || state.verified) return Promise.resolve();
    state.phase = "downloading";
    state.error = null;
    state.downloaded = 0;
    state.total = null;
    operation = (async () => {
      try {
        await deps.download((progress) => {
          state.phase = progress.phase;
          state.downloaded = progress.downloaded;
          state.total = progress.total;
        });
        state.verified = true;
        state.phase = "downloaded";
      } catch (error) {
        state.phase = "error";
        state.error = error instanceof Error ? error.message : String(error);
      }
    })().finally(() => { operation = null; });
    return operation;
  }

  function install(): Promise<void> {
    if (operation) return operation;
    if (checking) return checking;
    if (!state.verified || !state.info?.installSupported) return Promise.resolve();
    state.phase = "installing";
    state.error = null;
    operation = (async () => {
      try {
        await deps.install(); // Rust restarts only after successful installation.
      } catch (error) {
        state.phase = "error";
        state.error = error instanceof Error ? error.message : String(error);
      }
    })().finally(() => {
      if (state.phase === "error") operation = null;
    });
    return operation;
  }

  return { state, checkOnce, checkNow: () => check(true), canCheck: deps.enabled, download, install };
}

export const appUpdate = createAppUpdateStore({
  enabled: isTauri,
  check: (manual) => invoke<UpdateInfo>(manual ? "refresh_app_update" : "check_app_update"),
  download: (progress) => {
    const channel = new Channel<UpdateProgress>();
    channel.onmessage = progress;
    return invoke<void>("download_app_update", { onProgress: channel });
  },
  install: () => invoke<void>("install_app_update"),
});
