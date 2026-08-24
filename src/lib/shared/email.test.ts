import { describe, expect, test } from "vitest";
import { stripAt } from "./email.js";

describe("stripAt", () => {
  test("returns input untouched when there is no @", () => {
    expect(stripAt("hello")).toEqual({ local: "hello", typedDomain: "" });
  });

  test("splits at the first @", () => {
    expect(stripAt("hello@domain.com")).toEqual({
      local: "hello",
      typedDomain: "domain.com",
    });
  });

  test("uses only the first @ when multiple are present", () => {
    expect(stripAt("we@ird@@x.com")).toEqual({
      local: "we",
      typedDomain: "ird@@x.com",
    });
  });

  test("handles a trailing @ with no domain", () => {
    expect(stripAt("hello@")).toEqual({ local: "hello", typedDomain: "" });
  });
});
