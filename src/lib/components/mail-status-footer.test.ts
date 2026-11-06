// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import Footer from "./mail-status-footer.svelte";
import { EMPTY_ARCHIVE_FOOTER } from "$lib/shared/archive-status";

vi.mock("$lib/shared/app-update.svelte", () => ({ appUpdate: { state: { currentVersion: "test-version" } } }));
vi.mock("./app-update-control.svelte", async () => ({ default: (await import("./__tests__/layout-frame-probe.svelte")).default }));
let instance: ReturnType<typeof mount>;
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });

test.each(["inbox", "sent", "drafts"] as const)("%s keeps the same fixed-height footer and About control", (mailbox) => {
  const target = document.createElement("div");
  document.body.append(target);
  instance = mount(Footer, { target, props: { mailbox, data: EMPTY_ARCHIVE_FOOTER } });
  flushSync();
  const footer = target.querySelector("footer")!;
  expect(footer.classList.contains("h-8")).toBe(true);
  expect(footer.classList.contains("fixed")).toBe(true);
  expect(footer.querySelector('[aria-label="About OnlySend, version test-version"]')).not.toBeNull();
  if (mailbox === "drafts") {
    expect(footer.textContent).toContain("Drafts stored locally");
    expect(footer.textContent).not.toContain("Checking local copy");
  }
});
