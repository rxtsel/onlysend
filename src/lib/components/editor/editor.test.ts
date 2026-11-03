// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { expect, test, vi } from "vitest";
import type { EditorOptions } from "@tiptap/core";
import Editor from "./editor.svelte";

const probe = vi.hoisted(() => ({
  options: null as Partial<EditorOptions> | null,
  isActive: vi.fn((_name: string) => false),
  destroy: vi.fn(),
  canUndo: vi.fn(() => false),
  canRedo: vi.fn(() => false),
  undo: vi.fn(),
  redo: vi.fn(),
}));
vi.mock("@tiptap/core", async (original) => ({
  ...await original<typeof import("@tiptap/core")>(),
  Editor: class {
    constructor(options: Partial<EditorOptions>) { probe.options = options; }
    isActive = probe.isActive;
    can = () => ({ undo: probe.canUndo, redo: probe.canRedo });
    chain = () => ({ focus: () => ({ undo: () => ({ run: probe.undo }), redo: () => ({ run: probe.redo }) }) });
    setEditable = vi.fn();
    destroy = probe.destroy;
  },
}));

test("updates toolbar state once per transaction and configures only StarterKit's link", async () => {
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target });
  flushSync();
  const frame = target.firstElementChild!;
  expect(frame.classList.contains("border")).toBe(false);
  expect(frame.classList.contains("rounded-md")).toBe(false);
  expect(frame.classList.contains("flex-col")).toBe(true);
  const writer = frame.lastElementChild!;
  expect(writer.classList.contains("overflow-y-auto")).toBe(true);
  expect(writer.classList.contains("flex-1")).toBe(true);
  const options = probe.options!;
  expect(options.extensions?.map((extension) => extension.name)).toEqual(["starterKit", "placeholder"]);
  expect(options.extensions?.[0].options.link.openOnClick).toBe(false);
  options.onTransaction?.({} as never);
  options.onUpdate?.({ editor: { getHTML: () => "<p>Updated</p>" } } as never);
  expect(options.onSelectionUpdate).toBeUndefined();
  expect(probe.isActive.mock.calls.map(([name]) => name)).toEqual(["bold", "italic", "bulletList", "orderedList", "link"]);
  await unmount(instance);
  expect(probe.destroy).toHaveBeenCalledOnce();
  target.remove();
});

test("rich-text toolbar uses the editor's native undo and redo history", async () => {
  vi.clearAllMocks();
  probe.canUndo.mockReturnValue(true);
  probe.canRedo.mockReturnValue(true);
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target });
  flushSync();
  flushSync(() => probe.options!.onTransaction?.({} as never));
  const undo = target.querySelector<HTMLButtonElement>('[aria-label="Undo"]')!;
  const redo = target.querySelector<HTMLButtonElement>('[aria-label="Redo"]')!;
  expect(undo.disabled).toBe(false);
  expect(redo.disabled).toBe(false);
  flushSync(() => { undo.click(); redo.click(); });
  expect(probe.undo).toHaveBeenCalledOnce();
  expect(probe.redo).toHaveBeenCalledOnce();
  probe.canUndo.mockReturnValue(false);
  probe.canRedo.mockReturnValue(false);
  flushSync(() => probe.options!.onTransaction?.({} as never));
  expect(undo.disabled).toBe(true);
  expect(redo.disabled).toBe(true);
  await unmount(instance);
  target.remove();
});
