// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { EditorOptions } from "@tiptap/core";
import Editor from "./editor.svelte";
import { toast } from "svelte-sonner";

vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn() } }));
const commands = vi.hoisted(() => ({
  focus: vi.fn().mockReturnThis(),
  toggleBlockquote: vi.fn().mockReturnThis(),
  unsetAllMarks: vi.fn().mockReturnThis(),
  clearNodes: vi.fn().mockReturnThis(),
  insertContent: vi.fn().mockReturnThis(),
  undo: vi.fn().mockReturnThis(),
  redo: vi.fn().mockReturnThis(),
  run: vi.fn(),
}));

const probe = vi.hoisted(() => ({
  options: null as Partial<EditorOptions> | null,
  isActive: vi.fn((_name: string) => false),
  destroy: vi.fn(),
  getText: vi.fn(() => "Visible editor text"),
  setEditable: vi.fn(),
  getAttributes: vi.fn(() => ({ href: "https://example.com" })),
  canUndo: vi.fn(() => false),
  canRedo: vi.fn(() => false),
}));
vi.mock("@tiptap/core", async (original) => ({
  ...await original<typeof import("@tiptap/core")>(),
  Editor: class {
    constructor(options: Partial<EditorOptions>) { probe.options = options; }
    isActive = probe.isActive;
    setEditable = (_editable: boolean, emitUpdate = true) => {
      probe.setEditable(_editable, emitUpdate);
      if (emitUpdate) probe.options?.onUpdate?.({ editor: { getHTML: () => "<p></p>" } } as never);
    };
    isDestroyed = false;
    destroy = () => { this.isDestroyed = true; probe.destroy(); };
    chain = () => commands;
    can = () => ({ undo: probe.canUndo, redo: probe.canRedo });
    getText = probe.getText;
    getAttributes = probe.getAttributes;
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  probe.isActive.mockReturnValue(false);
  probe.canUndo.mockReturnValue(false);
  probe.canRedo.mockReturnValue(false);
});
afterEach(() => { vi.unstubAllGlobals(); });

test("updates toolbar state once per transaction and configures only StarterKit's link", async () => {
  const target = document.createElement("div");
  document.body.append(target);
  const onchange = vi.fn();
  const instance = mount(Editor, { target, props: { onchange } });
  flushSync();
  expect(instance.getText()).toBe("Visible editor text");
  expect(probe.getText).toHaveBeenCalledOnce();
  const frame = target.firstElementChild!;
  expect(frame.classList.contains("border")).toBe(false);
  expect(frame.classList.contains("rounded-md")).toBe(false);
  expect(frame.classList.contains("flex-col")).toBe(true);
  const writer = frame.lastElementChild!;
  expect(writer.classList.contains("overflow-y-auto")).toBe(true);
  expect(writer.classList.contains("flex-1")).toBe(true);
  const formatting = target.querySelector('[role="group"][aria-label="Text formatting"]')!;
  expect(formatting.querySelector('[aria-label="Insert link"]')).not.toBeNull();
  expect(target.querySelector('[aria-label="Attach files"]')).toBeNull();
  expect(target.querySelector('input[type="file"]')).toBeNull();
  const options = probe.options!;
  expect(options.extensions?.map((extension) => extension.name)).toEqual(["starterKit", "placeholder"]);
  expect(options.extensions?.[0].options.link.openOnClick).toBe(false);
  expect(onchange).not.toHaveBeenCalled();
  expect(probe.setEditable).toHaveBeenCalledWith(true, false);
  probe.isActive.mockClear();
  options.onTransaction?.({} as never);
  options.onUpdate?.({ editor: { getHTML: () => "<p>Updated</p>" } } as never);
  expect(onchange).toHaveBeenCalledOnce();
  expect(options.onSelectionUpdate).toBeUndefined();
  expect(probe.isActive.mock.calls.map(([name]) => name)).toEqual(["bold", "italic", "bulletList", "orderedList", "link", "blockquote"]);
  await unmount(instance);
  expect(probe.destroy).toHaveBeenCalledOnce();
  target.remove();
});

test.each(["", "https://example.com"])("the link popover opens on the first click, closes and reopens with URL '%s'", async (url) => {
  probe.getAttributes.mockReturnValue({ href: url });
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { onchange: vi.fn() } });
  try {
    flushSync();
    const trigger = target.querySelector<HTMLButtonElement>('[data-slot="popover-trigger"]')!;
    expect(trigger.querySelector("button")).toBeNull();
    flushSync(() => trigger.click());
    await vi.waitFor(() => expect(document.querySelector<HTMLInputElement>('input[type="url"]')?.value).toBe(url));
    flushSync(() => document.querySelector('input[type="url"]')!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })));
    await vi.waitFor(() => expect(document.querySelector('input[type="url"]')).toBeNull());
    flushSync(() => trigger.click());
    await vi.waitFor(() => expect(document.querySelector<HTMLInputElement>('input[type="url"]')?.value).toBe(url));
    flushSync(() => trigger.click());
    await vi.waitFor(() => expect(document.querySelector('input[type="url"]')).toBeNull());
  } finally { await unmount(instance); target.remove(); }
});

test("quote and clear-formatting commands stay on the left; undo and redo track availability on the right", async () => {
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { onchange: vi.fn() } });
  const button = (label: string) => target.querySelector<HTMLButtonElement>(`[aria-label="${label}"]`)!;
  try {
    flushSync();
    const history = target.querySelector('[aria-label="Edit history"]')!;
    const formatting = target.querySelector('[aria-label="Text formatting"]')!;
    expect(formatting.parentElement?.lastElementChild).toBe(history);
    expect(formatting.parentElement?.classList.contains("justify-between")).toBe(true);
    expect(history.contains(button("Undo"))).toBe(true);
    expect(history.contains(button("Redo"))).toBe(true);
    expect(button("Undo").disabled).toBe(true);
    expect(button("Redo").disabled).toBe(true);
    flushSync(() => { button("Quote").click(); button("Clear formatting").click(); });
    expect(commands.toggleBlockquote).toHaveBeenCalledOnce();
    expect(commands.unsetAllMarks).toHaveBeenCalledOnce();
    expect(commands.clearNodes).toHaveBeenCalledOnce();
    probe.isActive.mockImplementation((name) => name === "blockquote");
    probe.canUndo.mockReturnValue(true);
    flushSync(() => probe.options?.onTransaction?.({} as never));
    expect(button("Quote").getAttribute("aria-pressed")).toBe("true");
    expect(button("Undo").disabled).toBe(false);
    flushSync(() => button("Undo").click());
    expect(commands.undo).toHaveBeenCalledOnce();
    probe.canUndo.mockReturnValue(false);
    probe.canRedo.mockReturnValue(true);
    flushSync(() => probe.options?.onTransaction?.({} as never));
    expect(button("Undo").disabled).toBe(true);
    expect(button("Redo").disabled).toBe(false);
    flushSync(() => button("Redo").click());
    expect(commands.redo).toHaveBeenCalledOnce();
  } finally { await unmount(instance); target.remove(); }
});

test("plain-text paste preserves literal markup, Unicode and line breaks without inserting HTML", async () => {
  vi.stubGlobal("navigator", { clipboard: { readText: vi.fn().mockResolvedValue('<b>literal</b> & ñ\r\n\r\nnext line') } });
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { onchange: vi.fn() } });
  try {
    flushSync();
    target.querySelector<HTMLButtonElement>('[aria-label="Paste as plain text"]')!.click();
    await vi.waitFor(() => expect(commands.insertContent).toHaveBeenCalledWith([
      { type: "paragraph", content: [{ type: "text", text: "<b>literal</b> & ñ" }] },
      { type: "paragraph", content: [] },
      { type: "paragraph", content: [{ type: "text", text: "next line" }] },
    ]));
    expect(commands.focus).toHaveBeenCalledOnce();
    expect(commands.run).toHaveBeenCalledOnce();
  } finally { await unmount(instance); target.remove(); }
});

test("clipboard permission failures preserve content and offer the plain-text keyboard shortcut", async () => {
  vi.stubGlobal("navigator", { clipboard: { readText: vi.fn().mockRejectedValue(new Error("Permission denied")) } });
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { onchange: vi.fn() } });
  try {
    flushSync();
    const paste = target.querySelector<HTMLButtonElement>('[aria-label="Paste as plain text"]')!;
    flushSync(() => paste.click());
    await vi.waitFor(() => expect(toast.error).toHaveBeenCalledWith(expect.stringContaining("Ctrl+Shift+V")));
    expect(commands.insertContent).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(paste.disabled).toBe(false));
  } finally { await unmount(instance); target.remove(); }
});

test("a pending clipboard read cannot modify an editor after sending disables it", async () => {
  let finish!: (value: string) => void;
  vi.stubGlobal("navigator", { clipboard: { readText: () => new Promise<string>((resolve) => { finish = resolve; }) } });
  const controls = $state({ disabled: false });
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { get disabled() { return controls.disabled; }, onchange: vi.fn() } });
  try {
    flushSync();
    const paste = target.querySelector<HTMLButtonElement>('[aria-label="Paste as plain text"]')!;
    flushSync(() => paste.click());
    expect(paste.disabled).toBe(true);
    flushSync(() => { controls.disabled = true; });
    finish("Do not paste");
    await vi.waitFor(() => expect(paste.disabled).toBe(false));
    expect(commands.insertContent).not.toHaveBeenCalled();
  } finally { await unmount(instance); target.remove(); }
});

test("mounting and toggling editability never emit content changes, but typing does", async () => {
  const controls = $state({ disabled: false });
  const onchange = vi.fn();
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target, props: { get disabled() { return controls.disabled; }, onchange } });
  try {
    flushSync();
    expect(onchange).not.toHaveBeenCalled();
    flushSync(() => { controls.disabled = true; });
    expect(probe.setEditable).toHaveBeenLastCalledWith(false, false);
    flushSync(() => { controls.disabled = false; });
    expect(probe.setEditable).toHaveBeenLastCalledWith(true, false);
    expect(onchange).not.toHaveBeenCalled();
    probe.options?.onUpdate?.({ editor: { getHTML: () => "<p>User edit</p>" } } as never);
    expect(onchange).toHaveBeenCalledOnce();
  } finally { await unmount(instance); target.remove(); }
});
