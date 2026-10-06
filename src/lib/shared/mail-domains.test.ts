import { expect, test } from "vitest";
import { mailDomains } from "./mail-domains";
import type { InboundEmail } from "./inbound";
import type { SentEmail } from "$lib/types";

test("incoming mail belongs to every receiving domain, never its sender", () => {
  const mail = { from: "sender@outside.example", domains: ["a.example", "b.example"] } as InboundEmail;
  expect(mailDomains(mail)).toEqual(["a.example", "b.example"]);
  expect(mailDomains(mail)).not.toContain("outside.example");
});

test("unknown messages stay visible instead of being assigned to a known domain", () => {
  expect(mailDomains({ from: "sender@outside.example", domains: [] } as unknown as InboundEmail)).toEqual(["unknown"]);
});

test("sent messages use the sending address domain, including display names", () => {
  expect(mailDomains({ from: "Support <help@EXAMPLE.com>" } as SentEmail)).toEqual(["example.com"]);
});
