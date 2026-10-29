// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { expect, test, vi } from "vitest";
import type { EditorOptions } from "@tiptap/core";
import Editor from "./editor.svelte";

const probe = vi.hoisted(() => ({
  options: null as Partial<EditorOptions> | null,
  isActive: vi.fn((_name: string) => false),
  destroy: vi.fn(),
}));
vi.mock("@tiptap/core", async (original) => ({
  ...await original<typeof import("@tiptap/core")>(),
  Editor: class {
    constructor(options: Partial<EditorOptions>) { probe.options = options; }
    isActive = probe.isActive;
    setEditable = vi.fn();
    destroy = probe.destroy;
  },
}));

test("updates toolbar state once per transaction and configures only StarterKit's link", async () => {
  const target = document.createElement("div");
  document.body.append(target);
  const instance = mount(Editor, { target });
  flushSync();
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
