// @vitest-environment jsdom
import { flushSync, mount, unmount } from "svelte";
import { expect, test, vi } from "vitest";
import EmailBody from "./email-body.svelte";

test("preserves authored widths, typography, spacing and responsive styles without injected overrides", async () => {
  const target = document.createElement("div");
  document.body.append(target);
  const html = `<html lang="en" style="font-size:18px"><head><style>
    .card { background:#fff; color:#000 }
    @media (prefers-color-scheme:dark) { .card { background:#171717; color:#f5f5f5 } }
    @media (max-width:480px) { .card { padding:12px !important } }
  </style></head><body style="margin:0;font-size:20px">
    <table width="100%"><tbody><tr><td>
      <table class="card" width="100%" style="max-width:465px;margin:40px auto;padding:20px"><tbody><tr><td>
        <img src="https://example.test/logo.png" width="42" height="42" style="max-width:120px;height:42px" />
        <h1 style="font-size:24px;margin:30px 0">Invoice</h1>
      </td></tr></tbody></table>
    </td></tr></tbody></table>
  </body></html>`;
  const component = mount(EmailBody, { target, props: { html, label: "Invoice", onopenlink: vi.fn() } });
  flushSync();
  const host = target.querySelector<HTMLElement>('[role="document"]')!;
  const root = host.shadowRoot!;
  expect(root.innerHTML).toBe(new DOMParser().parseFromString(html, "text/html").documentElement.outerHTML);
  expect(root.querySelector<HTMLElement>(".card")?.style.maxWidth).toBe("465px");
  expect(root.querySelector<HTMLImageElement>("img")?.style.height).toBe("42px");
  expect(root.querySelector<HTMLElement>("body")?.style.fontSize).toBe("20px");
  expect(root.querySelectorAll("style")).toHaveLength(1);
  expect(host.style.overflow).toBe("auto");
  expect(host.style.contain).toBe("layout paint");
  await unmount(component);
  target.remove();
});

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
  expect(host?.shadowRoot?.querySelector("body")?.hasAttribute("style")).toBe(false);
  expect(document.querySelector("p")).toBeNull();
  expect(host?.getAttribute("aria-label")).toBe("Message from sender@example.test");

  unmount(component);
  target.remove();
});
