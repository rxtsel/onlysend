// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import EmailListItem from "./email-list-item.svelte";
import MailAvatar from "$lib/components/mail-avatar.svelte";

const { avatarPreferences } = vi.hoisted(() => ({ avatarPreferences: { emailAvatars: "blobavatars" } }));
vi.mock("$lib/shared/appearance.svelte", () => ({ appearance: avatarPreferences }));

const instances: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  await Promise.all(instances.splice(0).map((instance) => unmount(instance)));
  document.body.replaceChildren();
  avatarPreferences.emailAvatars = "blobavatars";
});

test.each([
  ["blobavatars", "inbox", true], ["minimal", "inbox", true],
  ["blobavatars", "inbox", false], ["minimal", "inbox", false],
  ["blobavatars", "sent", true], ["minimal", "sent", true],
] as const)("%s %s unread=%s shows the badge only on unread inbox avatars", async (style, mode, unread) => {
  avatarPreferences.emailAvatars = style;
  const target = document.createElement("div");
  document.body.append(target);
  instances.push(mount(EmailListItem, { target, props: {
    mode, isUnread: unread, onclick: () => {}, onreply: () => {}, ondelete: async () => {},
    mail: {
      id: "message", from: "sender@example.com", to: ["recipient@example.com"],
      subject: "Subject", createdAt: "2026-01-01T00:00:00Z", preview: "Body", isRead: !unread,
    },
  } }));
  flushSync();
  const row = target.querySelector<HTMLButtonElement>("[data-mail-id]")!;
  const wrapper = row.firstElementChild!;
  const avatar = wrapper.firstElementChild!;
  expect(avatar.classList.contains("rounded-lg")).toBe(true);
  expect(avatar.classList.contains("rounded-full")).toBe(false);
  expect(avatar.classList.contains("overflow-hidden")).toBe(true);
  expect(avatar.getAttribute("aria-hidden")).toBe("true");
  expect((avatar as HTMLElement).style.width).toBe("32px");
  if (style === "blobavatars") await vi.waitFor(() => expect(avatar.querySelector("img")).not.toBeNull());
  else expect(avatar.textContent?.trim()).toBe("S");
  const badges = row.querySelectorAll('[title="Unread"]');
  expect(badges).toHaveLength(mode === "inbox" && unread ? 1 : 0);
  if (badges.length) {
    const badge = badges[0];
    expect(badge.parentElement).toBe(wrapper);
    expect(avatar.contains(badge)).toBe(false);
    expect(badge.classList.contains("absolute")).toBe(true);
    expect(badge.classList.contains("-left-0.5")).toBe(true);
    expect(badge.classList.contains("-top-0.5")).toBe(true);
    expect(badge.classList.contains("bg-mail-unread")).toBe(true);
    expect(badge.classList.contains("ring-[var(--mail-row-bg)]")).toBe(true);
    expect(badge.querySelector(".sr-only")?.textContent).toBe("Unread");
    expect(wrapper.classList.contains("overflow-hidden")).toBe(false);
  }
});

test.each([
  ["inbox", "dropdown", 3], ["sent", "dropdown", 3],
  ["inbox", "context", 3], ["sent", "context", 3],
  ["inbox", "dropdown", 1], ["sent", "context", 1],
] as const)("%s %s menu (%i messages) exposes only available actions without opening the row", async (mode, menu, threadCount) => {
  const target = document.createElement("div");
  document.body.append(target);
  const onclick = vi.fn();
  const onreply = vi.fn();
  const ondelete = vi.fn().mockResolvedValue(undefined);
  instances.push(mount(EmailListItem, { target, props: {
    mode, onclick, onreply, ondelete, threadCount,
    mail: {
      id: "message", from: "sender@example.com", to: ["recipient@example.com"],
      subject: "Thread subject", createdAt: "2026-01-01T00:00:00Z", preview: "Body", isRead: true,
    },
  } }));
  flushSync();
  const row = target.querySelector<HTMLButtonElement>("button")!;
  const openMenu = async () => {
    if (menu === "dropdown") {
      const trigger = target.querySelector<HTMLButtonElement>('[aria-label="Actions for Thread subject"]')!;
      if (threadCount === 1) trigger.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true }));
      else trigger.click();
    } else {
      row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2 }));
    }
    await vi.waitFor(() => expect(document.querySelector('[role="menuitem"]')).not.toBeNull());
  };
  await openMenu();
  expect(onclick).not.toHaveBeenCalled();
  expect(target.querySelector("button button")).toBeNull();
  const reply = Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).find((item) => item.textContent === "Reply");
  if (mode === "inbox") {
    reply!.click();
    expect(onreply).toHaveBeenCalledExactlyOnceWith("message");
    await vi.waitFor(() => expect(document.querySelector('[role="menuitem"]')).toBeNull());
    await openMenu();
  } else {
    expect(reply).toBeUndefined();
  }
  const label = threadCount > 1 ? "Delete this message from this device" : "Delete from this device";
  const remove = Array.from(document.querySelectorAll<HTMLElement>('[role="menuitem"]')).find((item) => item.textContent === label)!;
  expect(remove.getAttribute("data-variant")).toBe("destructive");
  remove.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).not.toBeNull());
  expect(ondelete).not.toHaveBeenCalled();
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("not deleted from Resend");
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  expect(ondelete).not.toHaveBeenCalled();
  await openMenu();
  document.querySelector<HTMLElement>('[role="menuitem"][data-variant="destructive"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[data-slot="alert-dialog-action"]')).not.toBeNull());
  const confirm = document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!;
  expect(confirm.classList.contains("bg-destructive/10")).toBe(true);
  confirm.click();
  await vi.waitFor(() => expect(ondelete).toHaveBeenCalledExactlyOnceWith("message"));
  expect(onclick).not.toHaveBeenCalled();
});

test.each(["inbox", "sent"] as const)("%s list uses the same sender avatar as the reader", async (mode) => {
  const list = document.createElement("div");
  const readerAvatar = document.createElement("div");
  document.body.append(list, readerAvatar);
  instances.push(mount(EmailListItem, { target: list, props: {
    mode, onclick: () => {}, onreply: () => {}, ondelete: async () => {},
    mail: {
      id: "message", from: 'Cristhian <ME@example.com>', to: ["support@example.com"],
      subject: "Subject", createdAt: "2026-01-01T00:00:00Z", preview: "Body", isRead: true,
    },
  } }));
  instances.push(mount(MailAvatar, { target: readerAvatar, props: {
    address: "me@example.com", name: "Cristhian",
  } }));
  await vi.waitFor(() => expect(list.querySelector("img")).not.toBeNull());
  await vi.waitFor(() => expect(readerAvatar.querySelector("img")).not.toBeNull());
  expect(list.querySelector("img")?.src).toBe(readerAvatar.querySelector("img")?.src);
  expect(list.textContent).toContain(mode === "sent" ? "support@example.com" : "Cristhian");
});
