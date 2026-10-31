import { expect, test } from "vitest";
import { createMailSelection } from "./mail-selection.svelte";

const ids = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];

test("shift click selects an inclusive range in either direction and shrinks it", () => {
  const selection = createMailSelection();
  selection.click("b", ids, false, false);
  expect(selection.ids).toEqual([]);
  selection.click("i", ids, true, false);
  expect(selection.ids).toEqual(ids.slice(1));
  selection.click("d", ids, true, false);
  expect(selection.ids).toEqual(["b", "c", "d"]);
  selection.click("a", ids, true, false);
  expect(selection.ids).toEqual(["a", "b"]);
});

test("the first shift click selects one row without an active message", () => {
  const selection = createMailSelection();
  selection.click("b", ids, true, false);
  expect(selection.ids).toEqual(["b"]);
  selection.click("d", ids, true, false);
  expect(selection.ids).toEqual(["b", "c", "d"]);
});

test("shift can start from the previewed row", () => {
  const selection = createMailSelection();
  selection.click("d", ids, true, false, "b");
  expect(selection.ids).toEqual(["b", "c", "d"]);
});

test("ctrl/cmd toggles individual rows, not the rows between them", () => {
  const selection = createMailSelection();
  selection.click("b", ids, false, true);
  selection.click("h", ids, false, true);
  expect(selection.ids).toEqual(["b", "h"]);
  selection.click("b", ids, false, true);
  expect(selection.ids).toEqual(["h"]);
  selection.click("h", ids, false, true);
  expect(selection.ids).toEqual([]);
});

test("shift arrows extend, contract, reverse and stop at the loaded boundaries", () => {
  const selection = createMailSelection();
  selection.click("b", ids, false, false);
  expect(selection.move(ids, 1)).toBe("c");
  expect(selection.ids).toEqual(["b", "c"]);
  selection.move(ids, 1);
  expect(selection.ids).toEqual(["b", "c", "d"]);
  selection.move(ids, -1);
  selection.move(ids, -1);
  expect(selection.ids).toEqual(["b"]);
  selection.move(ids, -1);
  selection.move(ids, -1);
  expect(selection.ids).toEqual(["a", "b"]);
  for (let i = 0; i < 20; i++) selection.move(ids, 1);
  expect(selection.ids).toEqual(ids.slice(1));
});

test("empty lists, stale rows, reordered lists and appended pages keep ID-based selection", () => {
  const selection = createMailSelection();
  expect(selection.move([], 1)).toBeNull();
  selection.click("missing", ids, true, false);
  expect(selection.ids).toEqual([]);
  selection.click("b", ids, false, true);
  selection.click("h", ids, false, true);
  selection.reconcile(["h", "b", "new"]);
  expect(selection.ids).toEqual(["b", "h"]);
  selection.reconcile(["b", "new"]);
  expect(selection.ids).toEqual(["b"]);
  selection.move(["b", "new"], 1, "b");
  expect(selection.ids).toEqual(["b", "new"]);
  selection.clear();
  selection.click("new", ["b", "new"], true, false);
  expect(selection.ids).toEqual(["new"]);
});

test("ctrl shift can add a range to a disjoint selection without duplicates", () => {
  const selection = createMailSelection();
  selection.click("a", ids, false, true);
  selection.click("f", ids, false, true);
  selection.click("h", ids, true, true);
  expect(selection.ids).toEqual(["a", "f", "g", "h"]);
  selection.click("c", ids, false, false);
  expect(selection.ids).toEqual([]);
});
