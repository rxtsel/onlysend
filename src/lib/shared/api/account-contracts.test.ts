import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import * as domains from "./domains";
import * as auth from "./auth";
import * as senders from "../from-emails";
import * as inbox from "../inbound";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset().mockResolvedValue(undefined));

const accountId = "connection-b";
const cases: Array<[string, () => Promise<unknown>]> = [
  ["get_domain_preferences", () => domains.getDomainPreferences(accountId)],
  ["save_domain_preferences", () => domains.saveDomainPreferences(accountId, { includedDomainIds: [] })],
  ["list_domains", () => domains.listDomains(accountId)],
  ["get_domain", () => domains.getDomain(accountId, "domain")],
  ["create_domain", () => domains.createDomain(accountId, { name: "example.com" })],
  ["delete_domain", () => domains.deleteDomain(accountId, "domain")],
  ["verify_domain", () => domains.verifyDomain(accountId, "domain")],
  ["set_domain_receiving", () => domains.setDomainReceiving(accountId, "domain", true)],
  ["save_selected_domain", () => domains.saveSelectedDomain(accountId, "example.com")],
  ["get_selected_domain", () => domains.getSelectedDomain(accountId)],
  ["get_active_domain", () => domains.getActiveDomain(accountId)],
  ["get_inbound_setup_cache", () => domains.getInboundSetupCache(accountId)],
  ["save_inbound_setup_cache", () => domains.saveInboundSetupCache(accountId, {
    id: "d", name: "example.com", status: "verified", records: [],
    capabilities: { sending: "enabled", receiving: "enabled" },
  })],
  ["get_connection_status", () => auth.getConnectionStatus(accountId)],
  ["get_onboarding_state", () => auth.getOnboardingState(accountId)],
  ["mark_setup_complete", () => auth.markSetupComplete(accountId)],
  ["set_inbox_enabled", () => auth.setInboxEnabled(accountId, true)],
  ["disconnect_resend", () => auth.disconnectResend(accountId)],
  ["list_from_emails", () => senders.listFromEmails(accountId)],
  ["create_from_email", () => senders.createFromEmail(accountId, { label: "Support", address: "support@example.com" })],
  ["update_from_email", () => senders.updateFromEmail(accountId, { id: "sender", label: "Updated" })],
  ["delete_from_email", () => senders.deleteFromEmail(accountId, "sender")],
  ["list_inbound_emails", () => inbox.listInboundEmails(accountId, 16, null)],
  ["get_inbound_email", () => inbox.getInboundEmail(accountId, "message")],
  ["get_read_inbound_ids", () => inbox.getReadInboundIds(accountId)],
  ["mark_inbound_read", () => inbox.markInboundRead(accountId, "message")],
];

test.each(cases)("%s includes explicit account ownership in IPC", async (command, call) => {
  await call();
  expect(invoke).toHaveBeenCalledExactlyOnceWith(command, expect.objectContaining({ accountId }));
});
