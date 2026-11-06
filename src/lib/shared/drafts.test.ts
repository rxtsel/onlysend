// @vitest-environment jsdom
import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { createDraftWriter, restoreDraftFiles, type DraftFields } from "./drafts";
import { encodeAttachments } from "./attachments";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: vi.fn() }));
const fields: DraftFields = {
  from: "me@example.com", to: [], cc: [], bcc: [], pendingTo: "unfinished@", pendingCc: "", pendingBcc: "",
  subject: "Subject", content: "<p>Body</p>", replyEmailId: null, replyTo: "", isReplyToEnabled: false, isCcEnabled: false, isBccEnabled: false,
};
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(undefined); });

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => { resolve = done; });
  return { promise, resolve };
}

test("saves run in order, with explicit account and draft ownership", async () => {
  const first = deferred();
  vi.mocked(invoke).mockReturnValueOnce(first.promise);
  const writer = createDraftWriter("a", "draft-a");
  const saving = writer.save(fields, []);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  const latest = writer.save({ ...fields, subject: "Latest" }, []);
  await Promise.resolve();
  expect(invoke).toHaveBeenCalledTimes(1);
  first.resolve();
  await Promise.all([saving, latest]);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", {
    accountId: "a", draftId: "draft-a", data: { ...fields, subject: "Latest" },
  });
});

test("discard waits for in-flight saves, skips queued saves and cannot resurrect a draft", async () => {
  const first = deferred();
  vi.mocked(invoke).mockReturnValueOnce(first.promise);
  const writer = createDraftWriter("a", "draft-a");
  const saving = writer.save(fields, []);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  const queued = writer.save({ ...fields, subject: "Queued" }, []);
  const removing = writer.remove();
  first.resolve();
  await Promise.all([saving, removing]);
  expect(await queued).toBe(false);
  expect(await writer.save(fields, [])).toBe(false);
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["save_draft", "delete_draft"]);
  expect(invoke).toHaveBeenLastCalledWith("delete_draft", { accountId: "a", draftId: "draft-a" });
});

test("a failed save can be retried without poisoning the queue", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Disk full"));
  const writer = createDraftWriter("a", "draft-a");
  await expect(writer.save(fields, [])).rejects.toThrow("Disk full");
  await expect(writer.save(fields, [])).resolves.toBe(true);
  expect(invoke).toHaveBeenCalledTimes(2);
});

test("failed discard does not acknowledge a skipped save, and the latest edit can be retried", async () => {
  const first = deferred();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "save_draft") await first.promise;
    if (command === "delete_draft") throw new Error("Delete failed");
  });
  const writer = createDraftWriter("a", "draft-a");
  const saving = writer.save(fields, []);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  const latest = { ...fields, subject: "Latest" };
  const queued = writer.save(latest, []);
  const removing = writer.remove();
  const failedRemoval = expect(removing).rejects.toThrow("Delete failed");
  first.resolve();
  expect(await saving).toBe(true);
  expect(await queued).toBe(false);
  await failedRemoval;
  expect(await writer.save(latest, [])).toBe(true);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", { accountId: "a", draftId: "draft-a", data: latest });
});

test("text edits do not retransmit attachments, and clearing the selection does", async () => {
  const file = new File(["hello"], "note.txt", { type: "text/plain" });
  Object.defineProperty(file, "arrayBuffer", { value: async () => new TextEncoder().encode("hello").buffer });
  const writer = createDraftWriter("a", "draft-a");
  await writer.save(fields, [file]);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ data: { ...fields, attachments: [{ filename: "note.txt", content: "aGVsbG8=", mimeType: "text/plain" }] } }));
  await writer.save({ ...fields, subject: "Updated" }, [file]);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ data: { ...fields, subject: "Updated" } }));
  await writer.save(fields, []);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ data: { ...fields, attachments: [] } }));
  const reopened = createDraftWriter("a", "draft-a", [file]);
  await reopened.save(fields, [file]);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ data: fields }));
});

test("restores attachment bytes, filename and MIME type", async () => {
  const file = restoreDraftFiles([{ filename: "note.txt", content: "aGVsbG8=", mimeType: "text/plain" }])[0];
  expect(file.name).toBe("note.txt");
  expect(file.type).toBe("text/plain");
  // jsdom's File lacks arrayBuffer; read it using its supported FileReader.
  const bytes = await new Promise<ArrayBuffer>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.readAsArrayBuffer(file);
  });
  Object.defineProperty(file, "arrayBuffer", { value: async () => bytes });
  expect(await encodeAttachments([file])).toEqual([{ filename: "note.txt", content: "aGVsbG8=", mimeType: "text/plain" }]);
});
