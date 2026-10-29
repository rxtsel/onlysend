// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { mount, unmount } from "svelte";
import EmailListItem from "./email-list-item.svelte";
import MailAvatar from "$lib/components/mail-avatar.svelte";

const instances: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  await Promise.all(instances.splice(0).map((instance) => unmount(instance)));
  document.body.replaceChildren();
});

test.each(["inbox", "sent"] as const)("%s list uses the same sender avatar as the reader", async (mode) => {
  const list = document.createElement("div");
  const readerAvatar = document.createElement("div");
  document.body.append(list, readerAvatar);
  instances.push(mount(EmailListItem, { target: list, props: {
    mode, onclick: () => {},
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
