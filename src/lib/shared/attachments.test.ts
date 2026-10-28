import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  attachmentAction, cancelAttachmentDownload, listEmailAttachments, canPreviewImage,
  primaryAttachmentAction, formatAttachmentSize, contentId, encodeAttachments,
  validateAttachmentFiles, MAX_ATTACHMENT_BYTES,
} from "./attachments";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class { onmessage = (_event: unknown) => {}; },
}));
beforeEach(() => { vi.mocked(invoke).mockReset().mockResolvedValue(undefined); });

const context = { accountId: "account-b", mailbox: "inbox" as const, emailId: "email-id" };

test("attachment IPC keeps account, mailbox, email and attachment identity explicit", async () => {
  await listEmailAttachments(context);
  expect(invoke).toHaveBeenCalledWith("list_email_attachments", context);
  const onProgress = vi.fn();
  await attachmentAction({ ...context, attachmentId: "attachment-id", requestId: "request-id", action: "save" }, onProgress);
  const args = vi.mocked(invoke).mock.calls[1][1] as Record<string, unknown>;
  expect(args).toMatchObject({ ...context, attachmentId: "attachment-id", requestId: "request-id", action: "save" });
  expect(args).not.toHaveProperty("url");
  expect(args).not.toHaveProperty("path");
  (args.onProgress as { onmessage: (value: unknown) => void }).onmessage({ downloaded: 12, total: 30 });
  expect(onProgress).toHaveBeenCalledWith({ downloaded: 12, total: 30 });
  await cancelAttachmentDownload(context.accountId, "request-id");
  expect(invoke).toHaveBeenLastCalledWith("cancel_attachment_download", { accountId: "account-b", requestId: "request-id" });
});

test("only raster images are previewed; large images open externally and active files use Save as", () => {
  for (const type of ["image/png", "image/jpeg", "image/gif", "image/webp", "image/PNG; charset=binary"]) expect(canPreviewImage(type)).toBe(true);
  for (const type of ["image/svg+xml", "text/html", "application/pdf", ""]) expect(canPreviewImage(type)).toBe(false);
  const base = { id: "file", filename: "image.png", contentType: "image/png", size: 100 };
  expect(primaryAttachmentAction(base)).toBe("preview");
  expect(primaryAttachmentAction({ ...base, size: 6 * 1024 * 1024 })).toBe("open");
  expect(primaryAttachmentAction({ ...base, filename: "invoice.pdf", contentType: "application/pdf" })).toBe("open");
  for (const filename of ["script.exe", "script.desktop", "page.html", "image.svg", "document.txt"]) {
    expect(primaryAttachmentAction({ ...base, filename, contentType: "application/octet-stream" })).toBe("save");
  }
});

test("encoding the current file list cannot send removed attachments", async () => {
  const removed = new File(["old"], "old.txt", { type: "text/plain" });
  const kept = new File([new Uint8Array([0, 255, 128, 1])], "kept.bin");
  const added = new File(["new"], "new.txt", { type: "text/plain" });
  const selected = [removed, kept, added].filter((file) => file !== removed);
  const encoded = await encodeAttachments(selected);
  expect(encoded.map((item) => item.filename)).toEqual(["kept.bin", "new.txt"]);
  expect(encoded[0].content).toBe("AP+AAQ==");
  expect(encoded[0].mimeType).toBe("application/octet-stream");
  expect(encoded[1].content).toBe(btoa("new"));
});

test("large binary encoding crosses chunk boundaries without corrupting bytes", async () => {
  const bytes = new Uint8Array(65539).map((_, index) => index % 256);
  const [encoded] = await encodeAttachments([new File([bytes], "large.bin")]);
  const decoded = Uint8Array.from(atob(encoded.content), (character) => character.charCodeAt(0));
  expect(decoded).toEqual(bytes);
});

test("size validation happens before reading files and includes the whole selection", async () => {
  const arrayBuffer = vi.fn();
  const huge = { name: "huge.bin", size: MAX_ATTACHMENT_BYTES + 1, arrayBuffer } as unknown as File;
  await expect(encodeAttachments([huge])).rejects.toThrow("25 MB");
  expect(arrayBuffer).not.toHaveBeenCalled();
  expect(() => validateAttachmentFiles([{ size: MAX_ATTACHMENT_BYTES }, { size: 1 }] as File[])).toThrow("25 MB");
  expect(() => validateAttachmentFiles([])).not.toThrow();
});

test("CID normalization supports encoded and bracketed IDs without changing case", () => {
  expect(contentId("CID:%3CLogo%40example%3E")).toBe("Logo@example");
  expect(contentId("<Logo@example>")).toBe("Logo@example");
  expect(contentId("cid:bad%encoding")).toBe("bad%encoding");
});

test("file sizes handle empty files and unavailable metadata", () => {
  expect(formatAttachmentSize(null)).toBe("");
  expect(formatAttachmentSize(0)).toBe("0 B");
  expect(formatAttachmentSize(1024)).toBe("1.0 KB");
  expect(formatAttachmentSize(1024 * 1024)).toBe("1.0 MB");
});
