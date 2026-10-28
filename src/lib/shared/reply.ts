import { invoke } from "@tauri-apps/api/core";

export interface ReplyContext {
  to: string[];
  subject: string;
  fromCandidates: string[];
}

export function prepareReply(accountId: string, emailId: string): Promise<ReplyContext> {
  return invoke("prepare_reply", { accountId, emailId });
}
