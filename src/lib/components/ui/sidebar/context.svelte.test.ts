import { expect, test, vi } from "vitest";
import { setSidebar } from "./context.svelte";

vi.mock("svelte", () => ({ getContext: vi.fn(), setContext: vi.fn((_key, value) => value) }));
vi.mock("@/lib/hooks/is-mobile.svelte.js", () => ({ IsMobile: class { current = false; } }));

function key(overrides: Partial<KeyboardEvent> = {}) {
  return { key: "b", ctrlKey: false, metaKey: false, defaultPrevented: false, preventDefault: vi.fn(), ...overrides } as unknown as KeyboardEvent;
}

test.each([{ ctrlKey: true }, { metaKey: true }])("Ctrl/Cmd+B toggles the desktop sidebar: %o", (modifier) => {
  const setOpen = vi.fn();
  const sidebar = setSidebar({ open: () => true, setOpen });
  const event = key(modifier);
  sidebar.handleShortcutKeydown(event);
  expect(setOpen).toHaveBeenCalledExactlyOnceWith(false);
  expect(event.preventDefault).toHaveBeenCalledOnce();
});

test("plain B and shortcuts already handled by an editor do not toggle", () => {
  const setOpen = vi.fn();
  const sidebar = setSidebar({ open: () => true, setOpen });
  sidebar.handleShortcutKeydown(key());
  const handled = key({ ctrlKey: true, defaultPrevented: true });
  sidebar.handleShortcutKeydown(handled);
  expect(setOpen).not.toHaveBeenCalled();
  expect(handled.preventDefault).not.toHaveBeenCalled();
});
