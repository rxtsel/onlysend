export type Theme = "system" | "light" | "dark";
export type EmailAvatarStyle = "blobavatars" | "minimal";
export const APPEARANCE_KEY = "onlysend.appearance";
type Preferences = { theme: Theme; emailAvatars: EmailAvatarStyle };
type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

function read(storage: PreferenceStorage | null): Preferences {
  let saved: Partial<Preferences> | null = null;
  try { saved = JSON.parse(storage?.getItem(APPEARANCE_KEY) ?? "null"); } catch { /* Use defaults if storage is unavailable or invalid. */ }
  return {
    theme: saved?.theme === "light" || saved?.theme === "dark" ? saved.theme : "system",
    emailAvatars: saved?.emailAvatars === "minimal" ? "minimal" : "blobavatars",
  };
}

/** Presentation-only preferences, shared across accounts in this webview. */
export function createAppearance(storage: PreferenceStorage | null) {
  let preferences = $state(read(storage));
  function save(next: Preferences) {
    if (!storage) throw new Error("Appearance storage is unavailable");
    storage.setItem(APPEARANCE_KEY, JSON.stringify(next));
    preferences = next;
  }
  return {
    get theme() { return preferences.theme; },
    get emailAvatars() { return preferences.emailAvatars; },
    setTheme(value: string) {
      if (value !== "system" && value !== "light" && value !== "dark") return;
      save({ ...preferences, theme: value });
    },
    setEmailAvatars(value: string) {
      if (value !== "blobavatars" && value !== "minimal") return;
      save({ ...preferences, emailAvatars: value });
    },
  };
}

function browserStorage(): PreferenceStorage | null {
  try { return typeof window === "undefined" ? null : window.localStorage; }
  catch { return null; }
}

export function isDarkTheme(theme: Theme, systemDark: boolean): boolean {
  return theme === "dark" || (theme === "system" && systemDark);
}

export const appearance = createAppearance(browserStorage());
