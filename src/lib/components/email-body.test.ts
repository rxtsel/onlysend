// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { expect, test, vi } from "vitest";
import EmailBody from "./email-body.svelte";

test("renders the sanitized document in an isolated shadow root", () => {
  const target = document.createElement("div");
  document.body.append(target);
  const component = mount(EmailBody, {
    target,
    props: {
      html: "<html><head><style>p { color: red }</style></head><body><p>Email body</p></body></html>",
      label: "Message from sender@example.test",
      onopenlink: vi.fn(),
    },
  });
  flushSync();

  const host = target.querySelector('[role="document"]');
  expect(host?.shadowRoot?.querySelector("p")?.textContent).toBe("Email body");
  expect(host?.shadowRoot?.querySelector("style")?.textContent).toContain("color: red");
  expect(host?.shadowRoot?.querySelector("body")?.getAttribute("style")).toContain("font-size: 14px !important");
  expect(document.querySelector("p")).toBeNull();
  expect(host?.getAttribute("aria-label")).toBe("Message from sender@example.test");

  unmount(component);
  target.remove();
});
