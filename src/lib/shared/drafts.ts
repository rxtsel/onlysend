import { invoke } from "@tauri-apps/api/core";
import { encodeAttachments, validateAttachmentFiles } from "./attachments";
import type { AttachmentPayload } from "$lib/types";
import type { MailListItem } from "./local-mail";

export interface DraftFields {
  from: string;
  to: string[];
  cc: string[];
  bcc: string[];
  pendingTo: string;
  pendingCc: string;
  pendingBcc: string;
  subject: string;
  content: string;
  replyEmailId: string | null;
  replyTo: string;
  isReplyToEnabled: boolean;
  isCcEnabled: boolean;
  isBccEnabled: boolean;
}
export interface DraftData extends DraftFields { attachments: AttachmentPayload[] }
export interface DraftSummary { id: string; subject: string; from: string; to: string[]; preview: string; updatedAt: number }
export function draftListItem(draft: DraftSummary): MailListItem {
  return { id: draft.id, subject: draft.subject, from: draft.from, to: draft.to, preview: draft.preview, createdAt: new Date(draft.updatedAt).toISOString(), isRead: true };
}
export const listDrafts = (accountId: string): Promise<DraftSummary[]> => invoke("list_drafts", { accountId });
export const getDraft = (accountId: string, draftId: string): Promise<DraftData | null> => invoke("get_draft", { accountId, draftId });
export async function deleteDraft(accountId: string, draftId: string): Promise<void> {
  await invoke("delete_draft", { accountId, draftId });
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent("draft-deleted", { detail: { accountId, draftId } }));
}

export function restoreDraftFiles(attachments: AttachmentPayload[]): File[] {
  return attachments.map((attachment) => new File([
    Uint8Array.from(atob(attachment.content), (character) => character.charCodeAt(0)),
  ], attachment.filename, { type: attachment.mimeType }));
}

/** Serialize saves and deletion so an older save cannot resurrect a discarded draft. */
export function createDraftWriter(accountId: string, draftId: string, savedFiles: readonly File[] | null = null) {
  let pending: Promise<boolean> = Promise.resolve(true);
  let stopped = false;
  const encoded = new WeakMap<File, Promise<AttachmentPayload>>();
  return {
    save(fields: DraftFields, files: File[]): Promise<boolean> {
      if (stopped) return Promise.resolve(false);
      const operation = pending.catch(() => {}).then(async () => {
        if (stopped) return false;
        validateAttachmentFiles(files);
        const filesChanged = savedFiles === null || files.length !== savedFiles.length || files.some((file, index) => file !== savedFiles![index]);
        const attachments = filesChanged ? await Promise.all(files.map((file) => {
          let payload = encoded.get(file);
          if (!payload) {
            payload = encodeAttachments([file]).then(([attachment]) => attachment);
            encoded.set(file, payload);
          }
          return payload;
        })) : undefined;
        await invoke("save_draft", { accountId, draftId, data: { ...fields, ...(attachments ? { attachments } : {}) } });
        savedFiles = [...files];
        return true;
      });
      pending = operation;
      return operation;
    },
    async remove(): Promise<void> {
      stopped = true;
      await pending.catch(() => {});
      try { await deleteDraft(accountId, draftId); }
      catch (error) { stopped = false; throw error; }
    },
  };
}
