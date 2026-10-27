import { expect, test, vi } from "vitest";
import { APPEARANCE_KEY, createAppearance, isDarkTheme } from "./appearance.svelte";

function storage(value: string | null = null) {
  return { getItem: vi.fn(() => value), setItem: vi.fn((_key: string, next: string) => { value = next; }) };
}

test("appearance defaults to system theme and Blobavatars", () => {
  const appearance = createAppearance(storage());
  expect(appearance.theme).toBe("system");
  expect(appearance.emailAvatars).toBe("blobavatars");
});

test("both preferences persist together across reloads", () => {
  const saved = storage();
  const appearance = createAppearance(saved);
  appearance.setTheme("dark");
  appearance.setEmailAvatars("minimal");
  expect(saved.setItem).toHaveBeenLastCalledWith(APPEARANCE_KEY, JSON.stringify({ theme: "dark", emailAvatars: "minimal" }));
  const restored = createAppearance(saved);
  expect(restored.theme).toBe("dark");
  expect(restored.emailAvatars).toBe("minimal");
  restored.setTheme("system");
  expect(restored.emailAvatars).toBe("minimal");
});

test("invalid or inaccessible storage uses defaults; invalid selections do not erase preferences", () => {
  for (const value of ["bad json", "null", "42", '{"theme":"unknown","emailAvatars":"unknown"}']) {
    const appearance = createAppearance(storage(value));
    expect(appearance.theme).toBe("system");
    expect(appearance.emailAvatars).toBe("blobavatars");
  }
  const saved = storage();
  const appearance = createAppearance(saved);
  appearance.setTheme("");
  appearance.setEmailAvatars("random");
  expect(saved.setItem).not.toHaveBeenCalled();
  expect(createAppearance(null).theme).toBe("system");
  const denied = storage();
  denied.getItem.mockImplementation(() => { throw new Error("Storage denied"); });
  expect(createAppearance(denied).theme).toBe("system");
});

test("failed persistence does not pretend the preference was saved", () => {
  const saved = storage();
  saved.setItem.mockImplementation(() => { throw new Error("Storage denied"); });
  const appearance = createAppearance(saved);
  expect(() => appearance.setTheme("dark")).toThrow();
  expect(appearance.theme).toBe("system");
});

test("system theme tracks OS while explicit themes override it", () => {
  expect(isDarkTheme("system", true)).toBe(true);
  expect(isDarkTheme("system", false)).toBe(false);
  expect(isDarkTheme("light", true)).toBe(false);
  expect(isDarkTheme("dark", false)).toBe(true);
});
