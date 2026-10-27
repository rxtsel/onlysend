import { describe, expect, it } from "vitest";
import { sentFailureLabel } from "./sent-status";

describe("sent failure badges", () => {
  it.each([
    ["suppressed", "Suppressed"],
    ["bounced", "Bounced"],
    ["failed", "Failed"],
    ["canceled", "Canceled"],
  ])("labels %s", (event, label) => {
    expect(sentFailureLabel(event)).toBe(label);
  });

  it.each([
    "sent", "delivered", "opened", "clicked", "complained", "queued",
    "scheduled", "delivery_delayed", "future_event", "toString", "", null, undefined,
  ])("does not mark %s as unsent", (event) => {
    expect(sentFailureLabel(event)).toBeNull();
  });
});
