import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { afterEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { emailLinkFromMessage, prepareEmailDocument } from "./email-document";
import { inlineImageIds } from "./attachments";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); });

const bridge = readFileSync(new URL("../../../src-tauri/src/email-links.js", import.meta.url), "utf8");
const token = "test-document";

function frameBridge() {
  const handlers = new Map<string, (event: object) => void>();
  const postMessage = vi.fn();
  const scrollIntoView = vi.fn();
  const getElementById = vi.fn(() => ({ scrollIntoView }));
  class Anchor {
    constructor(private href: string) {}
    getAttribute(name: string) { return name === "href" ? this.href : null; }
  }
  runInNewContext(bridge.replaceAll("__ONLYSEND_TOKEN__", token), {
    URL,
    HTMLAnchorElement: Anchor,
    window: { parent: { postMessage } },
    document: { addEventListener: (type: string, handler: (event: object) => void) => handlers.set(type, handler), getElementById },
  });
  function click(href: string, overrides: Record<string, unknown> = {}) {
    const preventDefault = vi.fn();
    const event = {
      type: "click", button: 0, isTrusted: true, defaultPrevented: false,
      composedPath: () => [{ tagName: "IMG" }, new Anchor(href)],
      preventDefault, ...overrides,
    };
    handlers.get(event.type)?.(event);
    return preventDefault;
  }
  return { click, postMessage, getElementById, scrollIntoView };
}

test("the frame is prepared natively, never by concatenating raw mail in Svelte", async () => {
  const document = { html: "safe document", token };
  const parseFromString = vi.fn(() => ({
    documentElement: { attributes: [{ name: "lang", value: "en" }] },
    body: { attributes: [{ name: "style", value: "margin:0;background:#eee" }, { name: "onload", value: "untrusted()" }] },
  }));
  vi.stubGlobal("DOMParser", class { parseFromString = parseFromString; });
  vi.mocked(invoke).mockResolvedValue(document);
  expect(await prepareEmailDocument("<p>mail</p>")).toEqual(document);
  expect(parseFromString).toHaveBeenCalledWith("<p>mail</p>", "text/html");
  expect(invoke).toHaveBeenCalledWith("prepare_email_document", {
    html: "<p>mail</p>", htmlAttributes: { lang: "en" },
    bodyAttributes: { style: "margin:0;background:#eee", onload: "untrusted()" },
  });
});

test("CID images are replaced only with resolved resources and still pass through native sanitization", async () => {
  let source = "cid:Logo%40example";
  const image = { getAttribute: () => source, setAttribute: (_name: string, value: string) => { source = value; } };
  vi.stubGlobal("DOMParser", class {
    parseFromString() {
      return {
        querySelectorAll: () => [image],
        documentElement: { attributes: [], get outerHTML() { return `<html><body><img src="${source}"></body></html>`; } },
        body: { attributes: [] },
      };
    }
  });
  expect([...inlineImageIds("source")]).toEqual(["Logo@example"]);
  const data = "data:image/png;base64,iVBORw0KGgo=";
  await prepareEmailDocument("source", { "Logo@example": data });
  expect(source).toBe(data);
  expect(invoke).toHaveBeenLastCalledWith("prepare_email_document", {
    html: `<html><body><img src="${data}"></body></html>`, htmlAttributes: {}, bodyAttributes: {},
  });
});

test.each([
  {}, { ctrlKey: true }, { metaKey: true }, { shiftKey: true },
  { type: "auxclick", button: 1 }, { detail: 0 },
])("buttons, nested logos, keyboard and modified clicks use the bridge: %j", (overrides) => {
  const { click, postMessage } = frameBridge();
  expect(click("https://example.com/pay?a=1&b=two#receipt", overrides)).toHaveBeenCalledOnce();
  expect(postMessage).toHaveBeenCalledExactlyOnceWith({
    type: "onlysend-email-link", token, url: "https://example.com/pay?a=1&b=two#receipt",
  }, "*");
});

test.each([
  "javascript:alert(1)", "file:///etc/passwd", "data:text/html,test",
  "mailto:user@example.com", "/relative", "https://user:pass@example.com",
])("unusable or dangerous URLs cannot navigate or invoke anything: %s", (href) => {
  const { click, postMessage } = frameBridge();
  expect(click(href)).toHaveBeenCalledOnce();
  expect(postMessage).not.toHaveBeenCalled();
});

test("protocol-relative web links become HTTPS", () => {
  const { click, postMessage } = frameBridge();
  click("//example.com/pay");
  expect(postMessage).toHaveBeenCalledWith(expect.objectContaining({ url: "https://example.com/pay" }), "*");
});

test("fragment links stay inside the message", () => {
  const { click, postMessage, getElementById, scrollIntoView } = frameBridge();
  click("#payment%20details");
  expect(getElementById).toHaveBeenCalledWith("payment details");
  expect(scrollIntoView).toHaveBeenCalledOnce();
  expect(postMessage).not.toHaveBeenCalled();
});

test.each([{ isTrusted: false }, { defaultPrevented: true }, { button: 2 }])("does not launch for synthetic, handled or right clicks: %j", (overrides) => {
  const { click, postMessage } = frameBridge();
  click("https://example.com", overrides);
  expect(postMessage).not.toHaveBeenCalled();
});

test("only the current opaque frame and its current document can request a link", () => {
  const frame = {} as Window;
  const event = { source: frame, origin: "null", data: { type: "onlysend-email-link", token, url: "https://example.com" } };
  expect(emailLinkFromMessage(event, frame, token)).toBe("https://example.com");
  expect(emailLinkFromMessage(event, null, token)).toBeNull();
  expect(emailLinkFromMessage(event, {} as Window, token)).toBeNull();
  expect(emailLinkFromMessage(event, frame, "previous-document")).toBeNull();
  expect(emailLinkFromMessage({ ...event, origin: "https://evil.test" }, frame, token)).toBeNull();
  for (const data of [null, "text", {}, { ...event.data, type: "other" }, { ...event.data, url: 123 }]) {
    expect(emailLinkFromMessage({ ...event, data }, frame, token)).toBeNull();
  }
});
