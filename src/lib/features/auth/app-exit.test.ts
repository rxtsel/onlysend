import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { protectAppExit } from "./app-exit";
import { saveDrafts, confirmAppDeparture } from "./draft-navigation";

const probe = vi.hoisted(() => ({
  close: vi.fn(), unlistenExit: vi.fn(), unlistenClose: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ onCloseRequested: probe.close }) }));
vi.mock("./draft-navigation", () => ({ saveDrafts: vi.fn(), confirmAppDeparture: vi.fn() }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
  vi.mocked(listen).mockResolvedValue(probe.unlistenExit);
  probe.close.mockResolvedValue(probe.unlistenClose);
  vi.mocked(saveDrafts).mockReset().mockResolvedValue(undefined);
  vi.mocked(confirmAppDeparture).mockReturnValue(true);
});
const quit = () => vi.mocked(listen).mock.calls[0][1]({} as never);

test("Quit locks editing, coalesces duplicate requests and awaits saving before exit approval", async () => {
  let finish!: () => void;
  vi.mocked(saveDrafts).mockReturnValueOnce(new Promise<void>((resolve) => { finish = resolve; }));
  const setBusy = vi.fn();
  const cleanup = await protectAppExit(setBusy);
  const closing = quit();
  await quit();
  expect(setBusy).toHaveBeenCalledExactlyOnceWith(true);
  expect(saveDrafts).toHaveBeenCalledOnce();
  expect(invoke).not.toHaveBeenCalled();
  finish();
  await closing;
  expect(invoke).toHaveBeenCalledExactlyOnceWith("complete_app_exit", { approved: true });
  expect(setBusy).not.toHaveBeenCalledWith(false);
  cleanup();
  expect(probe.unlistenExit).toHaveBeenCalledOnce();
  expect(probe.unlistenClose).toHaveBeenCalledOnce();
});

test("a failed save prevents exit, unlocks editing and permits retry", async () => {
  vi.mocked(saveDrafts).mockRejectedValueOnce(new Error("Disk full"));
  const setBusy = vi.fn();
  const cleanup = await protectAppExit(setBusy);
  await quit();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("complete_app_exit", { approved: false });
  expect(setBusy).toHaveBeenLastCalledWith(false);
  await quit();
  expect(invoke).toHaveBeenLastCalledWith("complete_app_exit", { approved: true });
  cleanup();
});

test("an in-flight send blocks Quit even when the draft has been flushed", async () => {
  vi.mocked(confirmAppDeparture).mockReturnValue(false);
  const setBusy = vi.fn();
  const cleanup = await protectAppExit(setBusy);
  await quit();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("complete_app_exit", { approved: false });
  expect(setBusy).toHaveBeenLastCalledWith(false);
  cleanup();
});

test("window close keeps the webview alive and follows the same Quit path", async () => {
  const cleanup = await protectAppExit(vi.fn());
  const preventDefault = vi.fn();
  probe.close.mock.calls[0][0]({ preventDefault });
  expect(preventDefault).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("request_app_exit");
  cleanup();
});

test("a failed close listener registration releases the exit listener", async () => {
  probe.close.mockRejectedValueOnce(new Error("Listener unavailable"));
  await expect(protectAppExit(vi.fn())).rejects.toThrow("Listener unavailable");
  expect(probe.unlistenExit).toHaveBeenCalledOnce();
});
