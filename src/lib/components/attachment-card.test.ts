// @vitest-environment jsdom
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { afterEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { compile } from "tailwindcss";
import Card from "./attachment-card.svelte";
import ComposerAttachments from "./composer-attachments.svelte";

let instance: ReturnType<typeof mount>;
afterEach(async () => { await unmount(instance); document.body.replaceChildren(); });
const name = `${"very-long-filename-without-spaces-".repeat(15)}.pdf`;

test("long filenames and MIME types truncate while actions reserve one fixed-width row", async () => {
  const file = new File(["pdf"], name, { type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document" });
  instance = mount(ComposerAttachments, { target: document.body, props: { files: [file] } });
  flushSync();
  const card = document.querySelector('[data-slot="item"]')!;
  const title = card.querySelector('[data-slot="item-title"]')!;
  expect(title.classList.contains("w-full")).toBe(true);
  expect(title.classList.contains("w-fit")).toBe(false);
  expect(title.querySelector(".truncate")?.textContent).toBe(name);
  expect(title.querySelector(".truncate")?.getAttribute("title")).toBe(name);
  expect(card.querySelector('[data-slot="item-description"] > .truncate')).not.toBeNull();
  expect(card.querySelector('[data-slot="item-content"]')?.classList.contains("overflow-hidden")).toBe(true);
  const actions = card.querySelector('[data-slot="item-actions"]')!;
  expect(actions.classList.contains("flex-wrap")).toBe(false);
  expect(actions.classList.contains("flex-nowrap")).toBe(true);
  expect(actions.classList.contains("w-max")).toBe(true);
  expect(actions.classList.contains("col-span-2")).toBe(false);
  expect(actions.querySelector("button")?.getAttribute("aria-label")).toBe(`Remove ${name}`);

  // Compile the classes actually rendered, using the installed Tailwind theme.
  const theme = await readFile(createRequire(import.meta.url).resolve("tailwindcss/theme.css"), "utf8");
  const compiler = await compile(`${theme}\n@tailwind utilities;`);
  const classes = Array.from(document.querySelectorAll("[class]")).flatMap((element) => [...element.classList]);
  const css = compiler.build(classes);
  expect(css).toContain("@container attachments (width >= 36rem)");
  expect(css).toContain("text-overflow: ellipsis");
  expect(css).toContain("white-space: nowrap");
  expect(css).toContain("grid-template-columns: 2.5rem minmax(0,1fr) auto");
});

test.each([false, true])("cards without secondary actions retain their full accessible name (disabled=%s)", (disabled) => {
  const activate = vi.fn();
  instance = mount(Card, { target: document.body, props: {
    name, contentType: "application/pdf", size: 1024, status: "Loading a very long attachment preview…", onactivate: activate, disabled,
  } });
  flushSync();
  const button = document.querySelector<HTMLButtonElement>("button")!;
  expect(button.getAttribute("aria-label")).toBe(`Open ${name}`);
  expect(button.title).toBe(name);
  expect(document.querySelector('[role="status"]')?.textContent).toContain("Loading a very long attachment preview");
  expect(document.querySelector('[data-slot="item-actions"]')).toBeNull();
  button.click();
  expect(activate).toHaveBeenCalledTimes(disabled ? 0 : 1);
});
