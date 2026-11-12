// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { beforeEach, expect, test, vi } from "vitest";
import type { EditorOptions } from "@tiptap/core";
import Editor from "./editor.svelte";

const probe = vi.hoisted(() => ({
  options: null as Partial<EditorOptions> | null,
  isActive: vi.fn((_name: string) => false),
  destroy: vi.fn(),
  getText: vi.fn(() => "Visible editor text"),
  setEditable: vi.fn(),
  getAttributes: vi.fn(() => ({ href: "https://example.com" })),
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
    destroy = probe.destroy;
    getText = probe.getText;
    getAttributes = probe.getAttributes;
  },
}));

beforeEach(() => { vi.clearAllMocks(); });

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
  options.onTransaction?.({} as never);
  options.onUpdate?.({ editor: { getHTML: () => "<p>Updated</p>" } } as never);
  expect(onchange).toHaveBeenCalledOnce();
  expect(options.onSelectionUpdate).toBeUndefined();
  expect(probe.isActive.mock.calls.map(([name]) => name)).toEqual(["bold", "italic", "bulletList", "orderedList", "link"]);
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
