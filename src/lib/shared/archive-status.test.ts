import { describe, expect, test } from "vitest";
import { archiveStatusPresentation, EMPTY_ARCHIVE_FOOTER, selectArchiveFooter, type ArchiveFooterData, type ArchiveFooterSnapshot } from "./archive-status";

function healthy(): ArchiveFooterData {
  return {
    status: { downloadedMessages: 17, downloadedBodies: 17, running: false, sync: { startedAt: 100, metadataCompletedAt: 110, completedAt: 120, lastError: null } },
    statusError: null, refreshError: null, downloadedAt: null,
  };
}

describe("mail status footer", () => {
  test("only a completed healthy copy can collapse to an icon", () => {
    expect(archiveStatusPresentation(healthy()).kind).toBe("ready");
    expect(archiveStatusPresentation(EMPTY_ARCHIVE_FOOTER).kind).toBe("checking");
  });
  test("running downloads stay visible even with an older completion", () => {
    const data = healthy(); data.status!.running = true;
    expect(archiveStatusPresentation(data).kind).toBe("syncing");
  });
  test("metadata-only completion and missing bodies cannot appear healthy", () => {
    const data = healthy(); data.status!.sync!.completedAt = null;
    expect(archiveStatusPresentation(data).kind).toBe("partial");
    data.status!.sync!.completedAt = 120; data.status!.downloadedBodies = 16;
    expect(archiveStatusPresentation(data).kind).toBe("partial");
  });
  test.each(["statusError", "refreshError"] as const)("%s overrides a completed copy", (key) => {
    const data = healthy(); data[key] = "Could not refresh";
    expect(archiveStatusPresentation(data).kind).toBe("error");
  });
  test("a stopped failed download stays visible when counts happen to match", () => {
    const data = healthy(); data.status!.sync!.lastError = "Permission denied";
    expect(archiveStatusPresentation(data).kind).toBe("error");
  });
  test("a known empty mailbox is healthy after completed download", () => {
    const data = healthy(); data.status!.downloadedBodies = 0; data.status!.downloadedMessages = 0;
    expect(archiveStatusPresentation(data).kind).toBe("ready");
  });
  test("account/mailbox navigation never displays another scope's last status", () => {
    const snapshot: ArchiveFooterSnapshot = { accountId: "a", mailbox: "inbox", data: healthy() };
    expect(selectArchiveFooter(snapshot, "a", "inbox")).toBe(snapshot.data);
    expect(selectArchiveFooter(snapshot, "b", "inbox")).toBe(EMPTY_ARCHIVE_FOOTER);
    expect(selectArchiveFooter(snapshot, "a", "sent")).toBe(EMPTY_ARCHIVE_FOOTER);
    expect(selectArchiveFooter(null, "a", "inbox")).toBe(EMPTY_ARCHIVE_FOOTER);
  });
});
