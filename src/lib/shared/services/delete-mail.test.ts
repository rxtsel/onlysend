// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { toast, type ExternalToast } from "svelte-sonner";
type ToastAction = Extract<NonNullable<ExternalToast["action"]>, { onClick: unknown }>;
import { setMailDeleted } from "../local-mail";
import { deleteLocalMail } from "./delete-mail";

vi.mock("../local-mail", () => ({ setMailDeleted: vi.fn() }));
vi.mock("svelte-sonner", () => ({ toast: { promise: vi.fn(), success: vi.fn(), info: vi.fn() } }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(setMailDeleted).mockResolvedValue(undefined);
});
afterEach(() => vi.useRealTimers());

function undo() {
  const options = vi.mocked(toast.success).mock.calls[0][1]!;
  expect(options.duration).toBe(5000);
  const action = options.action as ToastAction;
  const button = document.createElement("button");
  button.addEventListener("click", (event) => action.onClick(event as Parameters<ToastAction["onClick"]>[0]));
  button.click();
}

test("persists deletion and undo for the original account, mailbox and message", async () => {
  expect(await deleteLocalMail("account-a", "sent", "message")).toBe(true);
  expect(setMailDeleted).toHaveBeenNthCalledWith(1, "account-a", "sent", "message", true);
  undo();
  expect(setMailDeleted).toHaveBeenNthCalledWith(2, "account-a", "sent", "message", false);
  expect(toast.promise).toHaveBeenCalledTimes(2);
});

test("does not offer undo when persistence fails", async () => {
  vi.mocked(setMailDeleted).mockRejectedValueOnce(new Error("disk full"));
  expect(await deleteLocalMail("account-a", "inbox", "message")).toBe(false);
  expect(toast.promise).toHaveBeenCalled();
  expect(toast.success).not.toHaveBeenCalled();
});

test("starts the five-second undo window after persistence, not before", async () => {
  vi.useFakeTimers();
  let resolve!: () => void;
  vi.mocked(setMailDeleted).mockReturnValueOnce(new Promise<void>((done) => { resolve = done; }));
  const deletion = deleteLocalMail("account-a", "inbox", "message");
  vi.advanceTimersByTime(10000);
  expect(toast.success).not.toHaveBeenCalled();
  resolve();
  await deletion;
  vi.advanceTimersByTime(4999);
  undo();
  expect(setMailDeleted).toHaveBeenCalledTimes(2);
});

test("does not restore after five seconds even if the toast timer was paused", async () => {
  vi.useFakeTimers();
  await deleteLocalMail("account-a", "inbox", "message");
  vi.advanceTimersByTime(5000);
  undo();
  expect(setMailDeleted).toHaveBeenCalledTimes(1);
  expect(toast.info).toHaveBeenCalledWith("The undo window has expired");
});
