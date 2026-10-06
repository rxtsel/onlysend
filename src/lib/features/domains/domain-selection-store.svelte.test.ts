import { beforeEach, expect, test, vi } from "vitest";
import { DomainSelectionStore } from "./domain-selection-store.svelte";
import { listDomains, getDomainPreferences, saveDomainPreferences, type DomainSummary } from "$lib/shared/api/domains";

vi.mock("$lib/shared/api/domains", () => ({ listDomains: vi.fn(), getDomainPreferences: vi.fn(), saveDomainPreferences: vi.fn() }));
const domains: DomainSummary[] = [
  { id: "ready", name: "a.example", status: "verified", capabilities: { sending: "enabled", receiving: "disabled" } },
  { id: "pending", name: "b.example", status: "pending", capabilities: { sending: "disabled", receiving: "enabled" } },
];
beforeEach(() => {
  vi.mocked(listDomains).mockReset().mockResolvedValue(domains);
  vi.mocked(getDomainPreferences).mockReset().mockResolvedValue(null);
  vi.mocked(saveDomainPreferences).mockReset().mockResolvedValue(undefined);
});

test("new accounts include every domain, including pending ones", async () => {
  const store = new DomainSelectionStore("a");
  await store.load();
  expect(store.includedIds).toEqual(["ready", "pending"]);
  expect(listDomains).toHaveBeenCalledWith("a");
  expect(getDomainPreferences).toHaveBeenCalledWith("a");
  expect(saveDomainPreferences).not.toHaveBeenCalled();
});

test("saved empty selection survives reopening; new domains are not silently included", async () => {
  vi.mocked(getDomainPreferences).mockResolvedValue({ includedDomainIds: [] });
  const store = new DomainSelectionStore("a");
  await store.load();
  expect(store.included).toEqual([]);
  expect(await store.save()).toBe(true);
  expect(saveDomainPreferences).toHaveBeenCalledWith("a", { includedDomainIds: [] });
});

test("saved selection and inaccessible IDs are preserved for review", async () => {
  vi.mocked(getDomainPreferences).mockResolvedValue({ includedDomainIds: ["ready", "deleted"] });
  const store = new DomainSelectionStore("a");
  await store.load();
  expect(store.includedIds).toEqual(["ready", "deleted"]);
  expect(store.included).toEqual([domains[0]]);
  expect(store.missingIds).toEqual(["deleted"]);
});

test("select all and deselect all are local and pending DNS does not block save", async () => {
  const store = new DomainSelectionStore("b");
  await store.load();
  store.deselectAll();
  expect(store.included).toEqual([]);
  store.selectAll();
  expect(store.included).toEqual(domains);
  expect(saveDomainPreferences).not.toHaveBeenCalled();
  expect(await store.save()).toBe(true);
  expect(saveDomainPreferences).toHaveBeenCalledExactlyOnceWith("b", { includedDomainIds: ["ready", "pending"] });
});

test("a newly created domain is included without selecting existing excluded domains", async () => {
  vi.mocked(getDomainPreferences).mockResolvedValue({ includedDomainIds: [] });
  const store = new DomainSelectionStore("a");
  await store.load();
  store.updateDomain({ ...domains[0], id: "new" }, true);
  expect(store.includedIds).toEqual(["new"]);
});

test("permission failures are errors, not zero-domain accounts", async () => {
  vi.mocked(listDomains).mockRejectedValue(new Error("Insufficient permissions"));
  const store = new DomainSelectionStore("a");
  await store.load();
  expect(store.error).toContain("Insufficient permissions");
  expect(await store.save()).toBe(false);
  expect(saveDomainPreferences).not.toHaveBeenCalled();
});

test("preferences failing to load cannot overwrite a saved selection with defaults", async () => {
  vi.mocked(getDomainPreferences).mockRejectedValue(new Error("corrupt settings"));
  const store = new DomainSelectionStore("a");
  await store.load();
  expect(store.error).toContain("corrupt settings");
  expect(await store.save()).toBe(false);
});

test("late responses after closing account A cannot update A or account B", async () => {
  let resolveA!: (domains: DomainSummary[]) => void;
  vi.mocked(listDomains).mockImplementationOnce(() => new Promise((done) => { resolveA = done; }));
  const a = new DomainSelectionStore("a");
  const b = new DomainSelectionStore("b");
  const pending = a.load();
  a.dispose();
  await b.load();
  b.deselectAll();
  resolveA(domains);
  await pending;
  expect(a.domains).toEqual([]);
  expect(b.included).toEqual([]);
  expect(await a.save()).toBe(false);
});

test("save errors are retryable and keep the local selection", async () => {
  const store = new DomainSelectionStore("a");
  await store.load();
  store.toggle("pending", false);
  vi.mocked(saveDomainPreferences).mockRejectedValueOnce(new Error("disk full"));
  await expect(store.save()).rejects.toThrow("disk full");
  expect(store.isSaving).toBe(false);
  expect(store.includedIds).toEqual(["ready"]);
  expect(await store.save()).toBe(true);
});
