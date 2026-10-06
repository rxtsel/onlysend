import { beforeEach, expect, test, vi } from "vitest";
import { finishAccountSetup } from "./finish-account-setup";
import { markSetupComplete } from "$lib/shared/api/auth";
import { saveSelectedDomain } from "$lib/shared/api/domains";
import { createFromEmail, listFromEmails, updateFromEmail } from "$lib/shared/from-emails";

vi.mock("$lib/shared/api/auth", () => ({ markSetupComplete: vi.fn() }));
vi.mock("$lib/shared/api/domains", () => ({ saveSelectedDomain: vi.fn() }));
vi.mock("$lib/shared/from-emails", () => ({ createFromEmail: vi.fn(), listFromEmails: vi.fn(), updateFromEmail: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(listFromEmails).mockResolvedValue([]);
  vi.mocked(createFromEmail).mockResolvedValue({ id: "sender", label: "Support", address: "support@b.example", isDefault: true });
});

test("zero domains or pending DNS can finish without a sender", async () => {
  await finishAccountSetup("b", [], "");
  expect(createFromEmail).not.toHaveBeenCalled();
  expect(saveSelectedDomain).not.toHaveBeenCalled();
  expect(markSetupComplete).toHaveBeenCalledWith("b");
});

test("partial writes do not mark setup complete; retries reuse saved sender identities", async () => {
  const sender = { label: "Support", address: "support@b.example" };
  vi.mocked(createFromEmail).mockRejectedValueOnce(new Error("disk full"));
  await expect(finishAccountSetup("b", [sender], "a.example")).rejects.toThrow("disk full");
  expect(markSetupComplete).not.toHaveBeenCalled();
  vi.mocked(listFromEmails).mockResolvedValue([{ ...sender, id: "existing", isDefault: false }]);
  await finishAccountSetup("b", [sender], "a.example");
  expect(updateFromEmail).toHaveBeenCalledWith("b", { ...sender, id: "existing", isDefault: true });
  expect(saveSelectedDomain).toHaveBeenCalledWith("b", "b.example");
  expect(markSetupComplete).toHaveBeenCalledWith("b");
  expect(createFromEmail).toHaveBeenCalledTimes(1);
});

test("identities from several domains use one account and exactly one chosen default", async () => {
  await finishAccountSetup("a", [
    { label: "Support", address: "support@b.example" },
    { label: "Billing", address: "billing@a.example" },
  ], "a.example");
  expect(createFromEmail).toHaveBeenNthCalledWith(1, "a", { label: "Support", address: "support@b.example", isDefault: true });
  expect(createFromEmail).toHaveBeenNthCalledWith(2, "a", { label: "Billing", address: "billing@a.example", isDefault: false });
  expect(saveSelectedDomain).toHaveBeenCalledWith("a", "b.example");
});
