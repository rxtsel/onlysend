import { expect, test } from "vitest";
import { emailAvatarSeed } from "./email-avatar";

test("avatar identity is the email, never the display name or mailbox", () => {
  expect(emailAvatarSeed('Alice <HELLO@Example.com>')).toBe("hello@example.com");
  expect(emailAvatarSeed('"Different Name" <hello@example.com>')).toBe(emailAvatarSeed(" hello@example.com "));
  expect(emailAvatarSeed('"Name <not-an-email>" <hello@example.com>')).toBe("hello@example.com");
});

test("separate addresses and plus aliases keep separate deterministic identities", () => {
  expect(emailAvatarSeed("a+one@example.com")).not.toBe(emailAvatarSeed("a+two@example.com"));
  expect(emailAvatarSeed("a.b@example.com")).not.toBe(emailAvatarSeed("ab@example.com"));
  expect(emailAvatarSeed("")).toBe("unknown");
});
