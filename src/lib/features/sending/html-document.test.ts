// @vitest-environment jsdom
import { expect, test } from "vitest";
import { HtmlDocument, isEmailLink, previewDocument } from "./html-document";

test("visual edits patch only source ranges and preserve Outlook markup, entities and styles", () => {
  const source = '<!DOCTYPE html>\n<html><head><style>p { color:red }</style></head><body><!--[if mso]><table><![endif]--><p style="margin:0">Hi &amp; María <b>Welcome</b></p><a HREF=\'https://example.com\'>Go</a></body></html>';
  const model = new HtmlDocument(source, "edit");
  expect(model.regions.filter((region) => region.kind === "text").map((region) => region.value)).toEqual(["Hi & María ", "Welcome", "Go"]);
  const first = model.regions.findIndex((region) => region.value === "Hi & María ");
  model.replace(first, 'Hello <Ana> & "team" ');
  const bold = model.regions.findIndex((region) => region.value === "Welcome");
  model.replace(bold, "Thanks");
  const link = model.regions.findIndex((region) => region.kind === "href");
  model.replace(link, "https://example.com/new?a=1&b=2");
  const go = model.regions.findIndex((region) => region.value === "Go");
  model.replace(go, "Visit");
  expect(model.html).toBe(source.replace("Hi &amp; María ", "Hello &lt;Ana&gt; &amp; &quot;team&quot; ").replace("Welcome", "Thanks").replace("HREF='https://example.com'", 'href="https://example.com/new?a=1&amp;b=2"').replace(">Go<", ">Visit<"));
  expect(model.html).not.toContain("<span id=");
});

test("annotations are separate from the original and skip active/raw content", () => {
  const source = '<script>bad()</script><style>div{color:red}</style><textarea>skip</textarea><svg><text>skip</text></svg><p>Hello <strong>friend</strong></p>';
  const model = new HtmlDocument(source, "safe");
  expect(model.html).toBe(source);
  expect(model.regions.map((region) => region.value)).toEqual(["Hello ", "friend"]);
  expect(model.annotated()).toContain('<span id="safe-0">Hello </span>');
  expect(model.annotated()).toContain('<strong><span id="safe-1">friend</span></strong>');
});

test("repeated edits and deletion keep following source offsets accurate", () => {
  const model = new HtmlDocument("<p>One</p><p>Two</p>", "edit");
  model.replace(0, "A longer name");
  model.replace(0, "");
  model.replace(1, "Three");
  model.replace(0, "Restored");
  expect(model.html).toBe("<p>Restored</p><p>Three</p>");
});

test("links reject active schemes and credentials", () => {
  for (const value of ["javascript:alert(1)", "data:text/html,bad", "file:///secret", "https://user:pass@example.com", "https://example.com\n"]) expect(isEmailLink(value)).toBe(false);
  for (const value of ["https://example.com", "mailto:hi@example.com", "tel:+123", "#footer"]) expect(isEmailLink(value)).toBe(true);
  const model = new HtmlDocument('<a href="https://example.com">Hi</a>', "edit");
  expect(() => model.replace(0, "javascript:alert(1)")).toThrow();
  expect(model.html).toBe('<a href="https://example.com">Hi</a>');
});

test("preview enforces its own restrictive policy before any template content", () => {
  const document = new DOMParser().parseFromString(previewDocument("<html><head><style>p{color:red}</style></head><body><p>Hi</p></body></html>"), "text/html");
  expect(document.head.firstElementChild?.getAttribute("content")).toContain("script-src 'none'");
  expect(document.head.firstElementChild?.getAttribute("content")).toContain("form-action 'none'");
  expect(document.body.textContent).toBe("Hi");
});
