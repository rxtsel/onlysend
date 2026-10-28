import { Channel, invoke } from "@tauri-apps/api/core";
import type { AttachmentPayload } from "$lib/types";
import type { Mailbox } from "./local-mail";

export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;
export interface EmailAttachment {
  id: string;
  filename: string | null;
  contentType: string;
  size: number | null;
  contentId?: string | null;
  contentDisposition?: string | null;
}
export interface AttachmentContext { accountId: string; mailbox: Mailbox; emailId: string }
export interface DownloadProgress { downloaded: number; total: number | null }
export type AttachmentAction = "preview" | "save" | "open";
export type AttachmentResult = { kind: "preview"; data: string } | { kind: "saved" | "opened" | "cancelled" };

export function listEmailAttachments(context: AttachmentContext): Promise<EmailAttachment[]> {
  return invoke("list_email_attachments", { ...context });
}
export function attachmentAction(
  request: AttachmentContext & { attachmentId: string; requestId: string; action: AttachmentAction },
  onProgress: (progress: DownloadProgress) => void,
): Promise<AttachmentResult> {
  const channel = new Channel<DownloadProgress>();
  channel.onmessage = onProgress;
  return invoke("attachment_action", { ...request, onProgress: channel });
}
export function cancelAttachmentDownload(accountId: string, requestId: string): Promise<void> {
  return invoke("cancel_attachment_download", { accountId, requestId });
}
export function canPreviewImage(type: string): boolean {
  return ["image/png", "image/jpeg", "image/gif", "image/webp"].includes(type.split(";")[0].trim().toLowerCase());
}
export function primaryAttachmentAction(item: EmailAttachment): AttachmentAction {
  if (canPreviewImage(item.contentType) && (item.size == null || item.size <= 5 * 1024 * 1024)) return "preview";
  const extension = (item.filename || "").split(".").at(-1)?.toLowerCase();
  return extension && ["png", "jpg", "jpeg", "gif", "webp", "pdf", "zip", "docx", "xlsx", "pptx", "odt", "ods", "odp"].includes(extension) ? "open" : "save";
}

export function attachmentStatus(action: AttachmentAction, progress: DownloadProgress): string {
  const label = action === "open" ? "Opening" : action === "preview" ? "Loading preview" : "Saving";
  return progress.total && progress.downloaded > 0 && progress.downloaded < progress.total
    ? `${label} · ${Math.floor(progress.downloaded / progress.total * 100)}%`
    : `${label}…`;
}

export function formatAttachmentSize(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
export function validateAttachmentFiles(files: readonly File[]): void {
  if (files.reduce((total, file) => total + file.size, 0) > MAX_ATTACHMENT_BYTES) {
    throw new Error("Attachments must total no more than 25 MB");
  }
}
export async function encodeAttachments(files: readonly File[]): Promise<AttachmentPayload[]> {
  validateAttachmentFiles(files);
  const result: AttachmentPayload[] = [];
  for (const file of files) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    let binary = "";
    for (let offset = 0; offset < bytes.length; offset += 32768) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + 32768));
    }
    result.push({ filename: file.name, content: btoa(binary), mimeType: file.type || "application/octet-stream" });
  }
  return result;
}

export function contentId(value: string): string {
  const id = value.replace(/^cid:/i, "");
  try { return decodeURIComponent(id).trim().replace(/^<|>$/g, ""); }
  catch { return id.trim().replace(/^<|>$/g, ""); }
}
export function inlineImageIds(html: string): Set<string> {
  const parsed = new DOMParser().parseFromString(html, "text/html");
  return new Set(Array.from(parsed.querySelectorAll("img[src]"), (image) => image.getAttribute("src") || "")
    .filter((src) => /^cid:/i.test(src)).map(contentId));
}
