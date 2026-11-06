<script lang="ts">
  import { getContext, onMount } from "svelte";
  import { LoaderCircle, Paperclip } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { useAccountId } from "$lib/features/auth/account-context";
  import { getDraft, listDrafts, draftListItem, type DraftData } from "$lib/shared/drafts";
  import type { MailListItem } from "$lib/shared/local-mail";
  import MailThreadMessage from "./mail-thread-message.svelte";
  import { errorMessage } from "$lib/shared/utils/errors";

  let { draftId }: { draftId: string } = $props();
  const accountId = useAccountId();
  const setSubject = getContext<((subject: string) => void) | undefined>("mail-subject");
  const setThreadStatus = getContext<((threaded: boolean) => void) | undefined>("mail-thread-status");
  let draft = $state<DraftData | null>(null);
  let mail = $state<MailListItem | null>(null);
  let loading = $state(true);
  let error = $state("");
  let disposed = false;
  async function load() {
    loading = true;
    error = "";
    try {
      const [data, summaries] = await Promise.all([getDraft(accountId, draftId), listDrafts(accountId)]);
      const summary = summaries.find((item) => item.id === draftId);
      if (!data || !summary) throw new Error("This draft no longer exists");
      if (disposed) return;
      draft = data;
      mail = draftListItem(summary);
      setSubject?.(data.subject || "(No subject)");
      setThreadStatus?.(false);
    } catch (cause) { if (!disposed) error = errorMessage(cause, "Could not open draft"); }
    finally { if (!disposed) loading = false; }
  }
  onMount(() => { void load(); return () => { disposed = true; }; });
</script>

{#if loading}
  <div class="flex h-full items-center justify-center" role="status" aria-label="Loading draft"><LoaderCircle class="size-5 animate-spin text-muted-foreground motion-reduce:animate-none" /></div>
{:else if error}
  <div class="flex h-full flex-col items-center justify-center gap-4 p-6 text-center"><p class="text-sm text-muted-foreground" role="alert">{error}</p><Button variant="outline" onclick={load}>Try again</Button></div>
{:else if draft && mail}
  <article class="h-full overflow-auto overscroll-contain" aria-label="Draft preview">
    <!-- Use the outbound message presentation with local details; no remote fetch or reply actions. -->
    <MailThreadMessage
      message={{ ...mail, mailbox: "sent" }} selectedId={mail.id} threaded={false} remoteAttachments={false} onDeleted={() => {}}
      initialDetail={{ from: draft.from, to: draft.to, subject: draft.subject, createdAt: mail.createdAt, html: draft.content,
        cc: draft.isCcEnabled ? draft.cc : [], bcc: draft.isBccEnabled ? draft.bcc : [], replyTo: draft.isReplyToEnabled ? draft.replyTo : null }}
    />
    {#if draft.attachments.length}
      <ul class="flex flex-wrap gap-2 px-5 pb-5" aria-label="Draft attachments">
        {#each draft.attachments as attachment}
          <li class="flex items-center gap-2 rounded-lg border px-3 py-2 text-xs"><Paperclip class="size-4 text-muted-foreground" aria-hidden="true" />{attachment.filename}</li>
        {/each}
      </ul>
    {/if}
  </article>
{/if}
