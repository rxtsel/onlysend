// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import ComposerAttachments from "./composer-attachments.svelte";
import MessageAttachments from "./message-attachments.svelte";
import { attachmentAction, listEmailAttachments, cancelAttachmentDownload } from "$lib/shared/attachments";

// Observe caller-owned references even when the real dialog would have unmounted its DOM.
vi.mock("./attachment-preview.svelte", async () => ({
  default: (await import("./__tests__/attachment-preview-probe.svelte")).default,
}));
vi.mock("$lib/shared/attachments", async (original) => ({
  ...await original<typeof import("$lib/shared/attachments")>(),
  attachmentAction: vi.fn(), listEmailAttachments: vi.fn(), cancelAttachmentDownload: vi.fn(),
}));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

const instances: Parameters<typeof unmount>[0][] = [];
let target: HTMLElement;
const createUrl = vi.fn((file: Blob | MediaSource) => `blob:${file instanceof File ? file.name : "image"}`);
const revokeUrl = vi.fn<(url: string) => void>();
beforeEach(() => {
  target = document.createElement("div");
  document.body.append(target);
  createUrl.mockClear();
  revokeUrl.mockClear();
  const NativeURL = globalThis.URL;
  vi.stubGlobal("URL", class extends NativeURL {
    static createObjectURL = createUrl;
    static revokeObjectURL = revokeUrl;
  });
  vi.mocked(listEmailAttachments).mockReset().mockResolvedValue([]);
  vi.mocked(attachmentAction).mockReset();
  vi.mocked(cancelAttachmentDownload).mockReset().mockResolvedValue(undefined);
});
afterEach(async () => {
  for (const instance of instances.splice(0)) await unmount(instance);
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});
function button(label: string): HTMLButtonElement {
  const result = Array.from(document.querySelectorAll<HTMLButtonElement>("button"))
    .find((element) => element.getAttribute("aria-label") === label);
  if (!result) throw new Error(`Missing button: ${label}`);
  return result;
}
function value(attribute: string): string | null {
  return target.querySelector(`[${attribute}]`)?.textContent ?? null;
}

test("closing a message preview releases its base64 and focus-element references", async () => {
  const item = { id: "image", filename: "photo.png", contentType: "image/png", size: 100 };
  const data = "data:image/png;base64,large-preview-payload";
  vi.mocked(listEmailAttachments).mockResolvedValue([item]);
  vi.mocked(attachmentAction).mockResolvedValue({ kind: "preview", data });
  instances.push(mount(MessageAttachments, { target, props: {
    context: { accountId: "account", mailbox: "inbox", emailId: "message" },
    initialItems: [item], oninlineimages: async () => {},
  } }));
  flushSync();
  flushSync(() => button("Preview photo.png").click());
  await vi.waitFor(() => expect(value("data-preview-src")).toBe(data));
  expect(value("data-preview-focus")).toBe("true");
  flushSync(() => button("Close preview probe").click());
  expect(value("data-preview-src")).toBe("");
  expect(value("data-preview-focus")).toBe("false");
  expect(value("data-preview-name")).toBe("Image");
});

test("composer releases selected files on close/removal and revokes thumbnail URLs", async () => {
  const first = new File(["one"], "first.png", { type: "image/png" });
  const second = new File(["two"], "second.png", { type: "image/png" });
  const instance = mount(ComposerAttachments, { target, props: { files: [first, second] } });
  instances.push(instance);
  flushSync();
  flushSync(() => button("Preview first.png").click());
  expect(value("data-preview-name")).toBe("first.png");
  flushSync(() => button("Close preview probe").click());
  expect(value("data-preview-name")).toBe("Image");
  expect(value("data-preview-src")).toBe("");
  expect(revokeUrl).not.toHaveBeenCalled(); // The attached thumbnail is still needed.
  flushSync(() => button("Preview first.png").click());
  flushSync(() => button("Remove first.png").click());
  expect(value("data-preview-name")).toBe("Image");
  expect(value("data-preview-src")).toBe("");
  expect(value("data-preview-open")).toBe("false");
  expect(revokeUrl).toHaveBeenCalledWith("blob:first.png");
  expect(createUrl).toHaveBeenCalledTimes(2);
  instances.splice(instances.indexOf(instance), 1);
  await unmount(instance);
  expect(revokeUrl).toHaveBeenCalledWith("blob:second.png");
  expect(revokeUrl).toHaveBeenCalledTimes(2);
});

test("the real dialog releases caller data after restoring keyboard focus", async () => {
  const { default: Preview } = await vi.importActual<typeof import("./attachment-preview.svelte")>("./attachment-preview.svelte");
  const trigger = document.createElement("button");
  document.body.append(trigger);
  let focusRestored = false;
  const onclose = vi.fn(() => { focusRestored = document.activeElement === trigger; });
  instances.push(mount(Preview, { target, props: {
    open: true, name: "photo.png", src: "data:image/png;base64,preview", restoreFocus: trigger, onclose,
  } }));
  flushSync();
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).not.toBeNull());
  const close = Array.from(document.querySelectorAll<HTMLButtonElement>('[role="dialog"] button'))
    .find((element) => element.textContent?.trim() === "Close");
  expect(close).toBeDefined();
  flushSync(() => close!.click());
  await vi.waitFor(() => expect(onclose).toHaveBeenCalledOnce());
  expect(focusRestored).toBe(true);
});
