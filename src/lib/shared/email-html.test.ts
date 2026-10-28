import { afterEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emailLinkFromClick, prepareEmailHtml } from "./email-html";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); });

function parsedEmail(source = "<html lang=\"en\"><body style=\"margin:0\"><p>mail</p></body></html>") {
  const image = { getAttribute: () => "cid:Logo%40example", setAttribute: vi.fn() };
  const parseFromString = vi.fn(() => ({
    documentElement: {
      attributes: [{ name: "lang", value: "en" }],
      get outerHTML() { return `<html><body><img src=\"${image.setAttribute.mock.calls.at(-1)?.[1] ?? "cid:Logo%40example"}\"></body></html>`; },
    },
    body: { attributes: [{ name: "style", value: "margin:0" }, { name: "onload", value: "bad()" }] },
    querySelectorAll: () => [image],
  }));
  vi.stubGlobal("DOMParser", class { parseFromString = parseFromString; });
  return { parseFromString, image, source };
}

test("passes source and presentation attributes through native sanitization", async () => {
  const parser = parsedEmail();
  vi.mocked(invoke).mockResolvedValue("<html><body><p>safe</p></body></html>");
  expect(await prepareEmailHtml("source")).toContain("safe");
  expect(parser.parseFromString).toHaveBeenCalledWith("source", "text/html");
  expect(invoke).toHaveBeenCalledWith("prepare_email_html", {
    html: "source", htmlAttributes: { lang: "en" },
    bodyAttributes: { style: "margin:0", onload: "bad()" },
  });
});

test("CID resources are replaced before native sanitization", async () => {
  parsedEmail();
  vi.mocked(invoke).mockResolvedValue("safe");
  await prepareEmailHtml("source", { "Logo@example": "data:image/png;base64,AA==" });
  expect(invoke).toHaveBeenCalledWith("prepare_email_html", expect.objectContaining({
    html: '<html><body><img src="data:image/png;base64,AA=="></body></html>',
  }));
});

function click(href: string, overrides: Partial<MouseEvent> = {}) {
  class Anchor {
    constructor(private value: string) {}
    getAttribute(name: string) { return name === "href" ? this.value : null; }
  }
  vi.stubGlobal("HTMLAnchorElement", Anchor);
  const preventDefault = vi.fn();
  const event = {
    isTrusted: true, defaultPrevented: false, button: 0, type: "click",
    composedPath: () => [new Anchor(href)], preventDefault, ...overrides,
  } as unknown as MouseEvent;
  const root = { getElementById: vi.fn(() => ({ scrollIntoView: vi.fn() })) } as unknown as ShadowRoot;
  const result = emailLinkFromClick(event, root);
  return { result, event, root };
}

test.each([
  ["https://example.test/a?b=1", "https://example.test/a?b=1"],
  ["//example.test/path", "https://example.test/path"],
  ["javascript:alert(1)", null],
  ["mailto:user@example.test", null],
  ["https://user:pass@example.test", null],
  ["/relative", null],
] as const)("only opens safe absolute web links: %s", (href, expected) => {
  const { result, event } = click(href);
  expect(result).toBe(expected);
  expect(event.preventDefault).toHaveBeenCalledOnce();
});

test("fragment links stay inside the rendered message", () => {
  const { result, root, event } = click("#payment%20details");
  expect(result).toBeNull();
  expect(root.getElementById).toHaveBeenCalledWith("payment details");
  expect(event.preventDefault).toHaveBeenCalledOnce();
});

test.each([{ isTrusted: false }, { defaultPrevented: true }, { button: 2 }, { type: "auxclick", button: 0 }])(
  "ignores synthetic, handled, and non-activation clicks: %j", (overrides) => {
    const { result, event } = click("https://example.test", overrides);
    expect(result).toBeNull();
    expect(event.preventDefault).not.toHaveBeenCalled();
  },
);
