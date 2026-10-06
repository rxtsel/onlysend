import { describe, expect, it, vi } from "vitest";
import { createAppUpdateStore, type UpdateInfo, type UpdateProgress } from "./app-update.svelte";

const available: UpdateInfo = { currentVersion: "0.1.1", state: "available", version: "0.2.0", notes: "Changes", error: null, installSupported: true, downloaded: false, releaseUrl: "https://github.com/rxtsel/onlysend/releases/latest" };
function setup(info = available) {
  const deps = { enabled: () => true, check: vi.fn(async (_manual: boolean) => info), download: vi.fn(async (_progress: (value: UpdateProgress) => void) => {}), install: vi.fn(async () => {}) };
  return { deps, store: createAppUpdateStore(deps) };
}

describe("global application updater", () => {
  it("checks once across concurrent callers and subsequent account navigation", async () => {
    const { deps, store } = setup();
    await Promise.all([store.checkOnce(), store.checkOnce()]);
    await store.checkOnce();
    expect(deps.check).toHaveBeenCalledTimes(1);
    expect(store.state.info?.version).toBe("0.2.0");
    expect(store.state.currentVersion).toBe("0.1.1");
  });
  it("permits explicit manual checks after the automatic startup check", async () => {
    const { deps, store } = setup();
    await store.checkOnce(); await store.checkNow(); await store.checkNow(); await store.checkOnce();
    expect(deps.check.mock.calls.map((call) => call[0])).toEqual([false, true, true]);
  });
  it("shares concurrent manual checks with startup and does not invalidate verified downloads", async () => {
    const { deps, store } = setup();
    await Promise.all([store.checkOnce(), store.checkNow()]);
    expect(deps.check).toHaveBeenCalledTimes(1);
    await store.download();
    deps.check.mockResolvedValue({ ...available, downloaded: true });
    await store.checkNow(); expect(store.state.verified).toBe(true);
    deps.check.mockResolvedValue({ ...available, version: "0.3.0", downloaded: false });
    await store.checkNow(); expect(store.state.verified).toBe(false);
  });
  it("also caches check failures and never retries automatically", async () => {
    const { deps, store } = setup();
    deps.check.mockRejectedValue(new Error("offline"));
    await store.checkOnce(); await store.checkOnce();
    expect(deps.check).toHaveBeenCalledTimes(1);
    expect(store.state.phase).toBe("error");
    expect(store.state.error).toBe("offline");
  });
  it("does not invoke native commands in browser previews", async () => {
    const { deps } = setup();
    const store = createAppUpdateStore({ ...deps, enabled: () => false });
    await store.checkOnce();
    expect(deps.check).not.toHaveBeenCalled();
    expect(store.state.phase).toBe("disabled");
  });
  it("never downloads or installs automatically", async () => {
    const { deps, store } = setup(); await store.checkOnce();
    expect(deps.download).not.toHaveBeenCalled(); expect(deps.install).not.toHaveBeenCalled();
  });
  it("requires verified download before installation", async () => {
    const { deps, store } = setup(); await store.checkOnce(); await store.install();
    expect(deps.install).not.toHaveBeenCalled();
    await store.download();
    expect(store.state.verified).toBe(true);
    expect(store.state.phase).toBe("downloaded");
    expect(deps.install).not.toHaveBeenCalled();
    await store.install(); expect(deps.install).toHaveBeenCalledTimes(1);
  });
  it("shares downloads and installs without duplicate operations", async () => {
    const { deps, store } = setup(); await store.checkOnce();
    await Promise.all([store.download(), store.download()]);
    await Promise.all([store.install(), store.install()]);
    expect(deps.download).toHaveBeenCalledTimes(1); expect(deps.install).toHaveBeenCalledTimes(1);
  });
  it("allows an explicit download retry after signature/network failure", async () => {
    const { deps, store } = setup(); await store.checkOnce();
    deps.download.mockRejectedValueOnce(new Error("invalid signature"));
    await store.download(); expect(store.state.verified).toBe(false);
    await store.install(); expect(deps.install).not.toHaveBeenCalled();
    await store.download(); expect(store.state.verified).toBe(true);
  });
  it("retains verified bytes on install failure for an explicit retry", async () => {
    const { deps, store } = setup(); await store.checkOnce(); await store.download();
    deps.install.mockRejectedValueOnce(new Error("permission denied"));
    await store.install(); expect(store.state.error).toBe("permission denied");
    expect(store.state.verified).toBe(true);
    await store.install(); expect(deps.install).toHaveBeenCalledTimes(2);
  });
  it("does not replace system-managed installations in-app", async () => {
    const { deps, store } = setup({ ...available, installSupported: false });
    await store.checkOnce(); await store.download(); await store.install();
    expect(deps.download).not.toHaveBeenCalled(); expect(deps.install).not.toHaveBeenCalled();
  });
  it("reports progress with unknown sizes without inventing a total", async () => {
    const { deps, store } = setup(); await store.checkOnce();
    deps.download.mockImplementation(async (progress) => { progress({ phase: "downloading", downloaded: 1024, total: null }); });
    await store.download(); expect(store.state.downloaded).toBe(1024); expect(store.state.total).toBeNull();
  });
});
