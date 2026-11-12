<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  import { page } from "$app/state";
  import { COMPOSER_STATUS, type ComposerStatus } from "$lib/features/sending/composer-context";
  import { prepareReply, type ReplyContext } from "$lib/shared/reply";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { registerDraftGuard, allowAccountNavigation } from "$lib/features/auth/draft-navigation";
  const accountId = useAccountId();
  import { Input } from "@/lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Field from "@/lib/components/ui/field";
  import { InputTag } from "@/lib/components/ui/input-tag";
  import type { Tag } from "@/lib/components/ui/input-tag/input-tag.svelte";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { Editor } from "@/lib/components/editor";
  import { LoaderCircle, TriangleAlert } from "@lucide/svelte";
  import Button from "@/lib/components/ui/button/button.svelte";
  import { emailComposerSchema } from "@/lib/schemas/email-composer.schema";
  import { toast } from "svelte-sonner";
  import { goto, beforeNavigate } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import type { FromEmail } from "@/lib/types";
  import { encodeAttachments } from "$lib/shared/attachments";
  import { errorMessage } from "$lib/shared/utils/errors";
  import { cn } from "$lib/utils";
  import ComposerAttachments from "$lib/components/composer-attachments.svelte";
  import ComposerSchedule from "$lib/components/composer-schedule.svelte";
  import { getContext, onMount, onDestroy, tick, untrack } from "svelte";
  import { listFromEmails, formatFromEmail } from "@/lib/shared/from-emails";
  import { createDraftWriter, getDraft, restoreDraftFiles, type DraftFields } from "$lib/shared/drafts";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  // Each mounted composer owns one persisted draft, including replies.
  const existingDraftId = untrack(() => page.url.searchParams.get("draft"));
  const draftId = existingDraftId ?? crypto.randomUUID();
  let writer = createDraftWriter(accountId, draftId);
  let replyEmailId = $state<string | null>(untrack(() => page.url.searchParams.get("reply")));
  const isReply = $derived(replyEmailId !== null);
  const composerStatus = getContext<ComposerStatus>(COMPOSER_STATUS);
  let reply = $state<ReplyContext | null>(null);
  let initializing = $state(true);
  let initializationError = $state("");
  let toEmails = $state<Tag[]>([]);
  let fromEmails = $state<FromEmail[]>([]);
  let from = $state<string>("");
  let subject = $state<string>("");
  let composerEditor = $state<{ focus: () => boolean; getText: () => string } | null>(null);
  let content = $state<string>("");
  let files = $state<File[]>([]);
  let scheduleDialog = $state<{ open: () => void } | null>(null);
  let isScheduleEnabled = $state(false);
  let scheduledLocal = $state("");
  const scheduledLabel = $derived(scheduledLocal && Number.isFinite(new Date(scheduledLocal).getTime())
    ? new Date(scheduledLocal).toLocaleString("en-US", { dateStyle: "full", timeStyle: "short" }) : "");
  let isSending = $state(false);
  let confirmSend = $state(false);
  let attachmentWarning = $state(false);
  let checkingSend = $state(false);
  let sendTrigger = $state<HTMLElement | null>(null);

  let isReplyToEnabled = $state(false);
  let replyTo = $state("");
  let isCcEnabled = $state<boolean>(false);
  let isBccEnabled = $state<boolean>(false);
  let ccEmails = $state<Tag[]>([]);
  let bccEmails = $state<Tag[]>([]);
  let pendingTo = $state("");
  let pendingCc = $state("");
  let pendingBcc = $state("");

  let errors: Record<string, string> = $state({});
  let disposed = false;
  let sent = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let savedFields = "";
  let savedFiles: File[] = [];
  let saveState = $state("");
  let hasUserInput = $state(false);
  function snapshot(): DraftFields {
    return {
      from, to: toEmails.map((tag) => tag.text), cc: ccEmails.map((tag) => tag.text), bcc: bccEmails.map((tag) => tag.text),
      pendingTo, pendingCc, pendingBcc, subject, content, replyEmailId, replyTo,
      isReplyToEnabled, isCcEnabled, isBccEnabled, isScheduleEnabled, scheduledLocal,
    };
  }
  function hasChanges(data = snapshot(), selectedFiles = files): boolean {
    return !!(existingDraftId || hasUserInput || selectedFiles.length) && !initializing && !initializationError && !sent &&
      (JSON.stringify(data) !== savedFields || selectedFiles.length !== savedFiles.length || selectedFiles.some((file, index) => file !== savedFiles[index]));
  }
  async function saveNow(data = snapshot(), selectedFiles = [...files]): Promise<void> {
    clearTimeout(timer);
    if (!hasChanges(data, selectedFiles)) return;
    saveState = "Saving…";
    try {
      if (!await writer.save(data, selectedFiles)) { saveState = "Unsaved changes"; return; }
      savedFields = JSON.stringify(data);
      savedFiles = selectedFiles;
      saveState = hasChanges() ? "Unsaved changes" : "Saved";
    } catch (error) {
      saveState = "Not saved";
      toast.error(errorMessage(error, "Could not save draft. Do not close this message."));
      throw error;
    }
  }
  async function discard() {
    if (isSending || checkingSend || sent) return;
    clearTimeout(timer);
    isSending = true;
    try {
      await writer.remove();
      sent = true;
      await goto(mailUrl(accountId, "drafts"));
    } catch (error) { toast.error(errorMessage(error, "Could not discard draft")); }
    finally { isSending = false; }
  }
  const unregisterGuard = registerDraftGuard(accountId, () => (sent || (!isSending && !checkingSend)) && !hasChanges(), () => saveNow());
  $effect(() => { if (!initializing && files.length) hasUserInput = true; });
  $effect(() => {
    if (initializing || initializationError || sent || isSending) return;
    const data = snapshot();
    const selectedFiles = [...files];
    if (!untrack(() => hasChanges(data, selectedFiles))) return;
    saveState = "Unsaved changes";
    timer = setTimeout(() => { void saveNow(data, selectedFiles).catch(() => {}); }, 500);
    return () => clearTimeout(timer);
  });
  $effect(() => {
    composerStatus.ready = !initializing && !initializationError && !sent && !checkingSend;
    composerStatus.sending = isSending;
    composerStatus.scheduled = isScheduleEnabled;
    composerStatus.openSchedule = () => scheduleDialog?.open();
    composerStatus.saveState = saveState;
    composerStatus.discard = discard;
  });
  onDestroy(() => {
    clearTimeout(timer);
    void saveNow().catch(() => {});
    disposed = true;
    unregisterGuard();
    composerStatus.ready = false;
    composerStatus.sending = false;
    composerStatus.discard = undefined;
    composerStatus.openSchedule = undefined;
  });
  beforeNavigate(({ cancel, to, willUnload, type, delta }) => {
    if ((isSending || checkingSend) && !sent) { cancel(); toast.error("Wait for the current operation to finish."); return; }
    if (!hasChanges()) {
      if (!allowAccountNavigation(accountId)) cancel();
      return;
    }
    if (willUnload) return; // The native beforeunload prompt protects reloads.
    cancel();
    // SvelteKit first rolls back a cancelled Back/Forward traversal.
    const restored = type === "popstate"
      ? new Promise<void>((resolve) => window.addEventListener("popstate", () => resolve(), { once: true }))
      : Promise.resolve();
    if (to) void Promise.all([saveNow(), restored]).then(() => {
      if (disposed) return;
      if (type === "popstate") history.go(delta!);
      else return goto(to.url);
    }).catch(() => {});
  });

  async function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (isSending || checkingSend || initializing || initializationError || sent || (isReply && !reply)) return;

    errors = {};
    const form = event.currentTarget as HTMLFormElement;
    const scheduledDate = new Date(scheduledLocal);
    if (isScheduleEnabled && !Number.isFinite(scheduledDate.getTime())) {
      errors.scheduledAt = "Choose a date and time";
      return;
    }
    const payload = {
      from, // already bound via Select
      to: toEmails.map((t) => t.text), // Tag[] -> string[]
      subject: isReply ? reply!.subject : subject,
      bcc: isBccEnabled ? bccEmails.map((t) => t.text) : [],
      cc: isCcEnabled ? ccEmails.map((t) => t.text) : [],
      ...(isReplyToEnabled && replyTo.trim() ? { replyTo: replyTo.trim() } : {}),
      content,
      replyEmailId,
      ...(isScheduleEnabled ? { scheduledAt: scheduledDate.toISOString() } : {}),
    };

    const parsed = emailComposerSchema.safeParse(payload);

    if (!parsed.success) {
      parsed.error.issues.forEach((issue) => {
        const field = issue.path[0] as string;
        errors[field] = issue.message;
      });
      return;
    }

    if (event.submitter?.getAttribute("name") !== "confirm-send") {
      sendTrigger = event.submitter instanceof HTMLElement ? event.submitter
        : document.activeElement instanceof HTMLElement ? document.activeElement : null;
      checkingSend = true;
      try {
        attachmentWarning = files.length === 0 && await invoke<boolean>("mentions_attachment", {
          subject: parsed.data.subject, body: composerEditor?.getText() ?? "",
        });
        if (!disposed) confirmSend = true;
      } catch (error) {
        if (!disposed) toast.error(errorMessage(error, "Could not check for missing attachments. Please try again."));
      } finally { if (!disposed) checkingSend = false; }
      return;
    }

    confirmSend = false;
    isSending = true;
    const sendingToast = toast.loading(isScheduleEnabled ? "Scheduling message…" : "Sending message…");
    try {
      await saveNow();
      const attachments = await encodeAttachments(files);
      if (disposed) return;
      await invoke("send_email", { accountId, data: { ...parsed.data, attachments } });

      if (disposed) return;
      sent = true;
      clearTimeout(timer);
      try { await writer.remove(); }
      catch { toast.error("Message accepted by Resend, but its draft could not be removed. Delete it from Drafts; do not resend."); }
      // Reset form and states
      toEmails = [];
      from = "";
      subject = "";
      content = "";
      files = [];
      isReplyToEnabled = false;
      replyTo = "";
      isCcEnabled = false;
      isBccEnabled = false;
      ccEmails = [];
      bccEmails = [];
      form.reset();

      toast.success(isScheduleEnabled ? `Email scheduled for ${scheduledLabel}` : "Email sent successfully!");
      // Bust the list cache so the sidebar shows the new email immediately
      window.dispatchEvent(new CustomEvent("email-sent"));

      goto(mailUrl(accountId, "sent"));
    } catch (err) {
      if (disposed) return;
      console.error(err);
      toast.error(errorMessage(err, isScheduleEnabled ? "Failed to schedule email. Please try again." : "Failed to send email. Please try again."));
    } finally {
      toast.dismiss(sendingToast);
      if (!disposed) isSending = false;
    }
  }

  async function initialize() {
    initializing = true;
    initializationError = "";
    try {
      const draft = existingDraftId ? await getDraft(accountId, existingDraftId) : null;
      if (existingDraftId && !draft) throw new Error("This draft no longer exists");
      if (disposed) return;
      if (draft) replyEmailId = draft.replyEmailId;
      const [list, context] = await Promise.all([
        listFromEmails(accountId),
        isReply ? prepareReply(accountId, replyEmailId!) : Promise.resolve(null),
      ]);
      if (disposed) return;
      fromEmails = list;
      reply = context;
      if (context) {
        subject = context.subject;
        toEmails = context.to.map((text) => ({ id: crypto.randomUUID(), text }));
      }
      const defaultFrom = list.find((sender) => context?.fromCandidates.some((recipient) => recipient.toLowerCase() === sender.address.toLowerCase()))
        ?? list.find((sender) => sender.isDefault) ?? list[0];
      if (defaultFrom) from = formatFromEmail(defaultFrom);
      if (draft) {
        from = draft.from;
        subject = draft.subject;
        content = draft.content;
        toEmails = draft.to.map((text) => ({ id: crypto.randomUUID(), text }));
        ccEmails = draft.cc.map((text) => ({ id: crypto.randomUUID(), text }));
        bccEmails = draft.bcc.map((text) => ({ id: crypto.randomUUID(), text }));
        pendingTo = draft.pendingTo; pendingCc = draft.pendingCc; pendingBcc = draft.pendingBcc;
        replyTo = draft.replyTo;
        isReplyToEnabled = draft.isReplyToEnabled;
        isCcEnabled = draft.isCcEnabled;
        isBccEnabled = draft.isBccEnabled;
        isScheduleEnabled = draft.isScheduleEnabled ?? false;
        scheduledLocal = draft.scheduledLocal ?? "";
        files = restoreDraftFiles(draft.attachments);
        writer = createDraftWriter(accountId, draftId, [...files]);
      }
      savedFields = JSON.stringify(snapshot());
      savedFiles = [...files];
      saveState = draft ? "Saved" : "";
    } catch (cause) {
      if (!disposed) initializationError = errorMessage(cause, "Could not prepare this message");
    } finally {
      if (!disposed) initializing = false;
    }
    await tick();
    if (!disposed && isReply && !initializationError) composerEditor?.focus();
  }
  onMount(() => {
    void initialize();
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (hasChanges() || ((isSending || checkingSend) && !sent)) { event.preventDefault(); event.returnValue = ""; }
    };
    window.addEventListener("beforeunload", beforeUnload);
    return () => { window.removeEventListener("beforeunload", beforeUnload); };
  });
</script>

{#if initializing}
  <div class="flex h-full items-center justify-center gap-2 text-sm text-muted-foreground" role="status"><LoaderCircle class="size-4 animate-spin motion-reduce:animate-none" />{isReply ? "Preparing reply…" : "Loading composer…"}</div>
{:else if initializationError}
  <div class="flex h-full flex-col items-center justify-center gap-4 text-center">
    <p class="text-sm text-muted-foreground" role="alert">{initializationError}</p>
    <Button type="button" variant="outline" onclick={initialize}>Try again</Button>
  </div>
{:else}
<form
  class="h-full w-full"
  onsubmit={handleSubmit}
  oninput={() => { hasUserInput = true; }}
  id="compose-email-form"
  aria-busy={isSending || checkingSend}
>
  <fieldset disabled={isSending || checkingSend} class="contents">
  <Field.Group class="h-full min-h-0 gap-0">
    <div class="flex shrink-0 flex-col gap-3 pb-4">
      <div class="flex flex-wrap items-end gap-3">
        <Field.Field data-invalid={!!errors.from} class="min-w-[180px] flex-1">
          <Field.Label for="from">From</Field.Label>
          <Select.Root type="single" name="from" bind:value={from}>
            <Select.Trigger class="w-[180px]" id="from" aria-invalid={!!errors.from}>
              {from || "Select an email"}
            </Select.Trigger>
            <Select.Content>
              {#if fromEmails.length === 0}
                <span class="inline-block w-full py-1 text-center text-sm">
                  Create email address in: Settings > Email sender options
                </span>
              {:else}
                <Select.Group>
                  {#each fromEmails as option}
                    <Select.Item value={formatFromEmail(option)}>{formatFromEmail(option)}</Select.Item>
                  {/each}
                </Select.Group>
              {/if}
            </Select.Content>
          </Select.Root>
          {#if errors.from}<Field.Error>{errors.from}</Field.Error>{/if}
        </Field.Field>
        <Field.Field class="w-fit shrink-0">
          <ToggleGroup.Root type="multiple" value={[...(isReplyToEnabled ? ["replyTo"] : []), ...(isCcEnabled ? ["cc"] : []), ...(isBccEnabled ? ["bcc"] : [])]} variant="outline" class="*:flex-none">
            <ToggleGroup.Item value="replyTo" onclick={() => (isReplyToEnabled = !isReplyToEnabled)}>Reply to</ToggleGroup.Item>
            <ToggleGroup.Item value="cc" onclick={() => (isCcEnabled = !isCcEnabled)}>Cc</ToggleGroup.Item>
            <ToggleGroup.Item value="bcc" onclick={() => (isBccEnabled = !isBccEnabled)}>Bcc</ToggleGroup.Item>
          </ToggleGroup.Root>
        </Field.Field>
      </div>
      <div class="flex gap-3">
        <Field.Field data-invalid={!!errors.to} class="flex-1">
          <Field.Label for="to">To</Field.Label>
          <InputTag id="to" type="email" bind:value={toEmails} bind:inputValue={pendingTo} onchange={() => { hasUserInput = true; }} name="to" placeholder="Add recipient email" aria-invalid={!!errors.to} />
          {#if errors.to}<Field.Error>{errors.to}</Field.Error>{/if}
        </Field.Field>
        {#if isReplyToEnabled}
          <Field.Field data-invalid={!!errors.replyTo} class="min-w-0 flex-1">
            <Field.Label for="replyTo">Reply to</Field.Label>
            <Input id="replyTo" type="email" name="replyTo" placeholder="Add reply-to email" class="h-9" bind:value={replyTo} aria-invalid={!!errors.replyTo} />
            {#if errors.replyTo}<Field.Error>{errors.replyTo}</Field.Error>{/if}
          </Field.Field>
        {/if}
      </div>
      {#if isCcEnabled || isBccEnabled}
        <div class={cn("grid grid-cols-1 gap-3", isCcEnabled && isBccEnabled && "sm:grid-cols-2")}>
          {#if isCcEnabled}
            <Field.Field data-invalid={!!errors.cc} class="min-w-0">
              <Field.Label for="cc">Cc</Field.Label>
              <InputTag id="cc" type="email" bind:value={ccEmails} bind:inputValue={pendingCc} onchange={() => { hasUserInput = true; }} name="cc" placeholder="Add Cc email" aria-invalid={!!errors.cc} />
              {#if errors.cc}<Field.Error>{errors.cc}</Field.Error>{/if}
            </Field.Field>
          {/if}
          {#if isBccEnabled}
            <Field.Field data-invalid={!!errors.bcc} class="min-w-0">
              <Field.Label for="bcc">Bcc</Field.Label>
              <InputTag id="bcc" type="email" bind:value={bccEmails} bind:inputValue={pendingBcc} onchange={() => { hasUserInput = true; }} name="bcc" placeholder="Add Bcc email" aria-invalid={!!errors.bcc} />
              {#if errors.bcc}<Field.Error>{errors.bcc}</Field.Error>{/if}
            </Field.Field>
          {/if}
        </div>
      {/if}
      <Field.Field data-invalid={!!errors.subject}>
        <Field.Label for="subject">Subject</Field.Label>
          <Input
            id="subject"
            type="text"
            name="subject"
            placeholder="Subject"
            readonly={isReply}
            class={cn("h-9", isReply && "bg-muted/40")}
            bind:value={subject}
            aria-invalid={!!errors.subject}
            onkeydown={(event) => {
              if (event.key === "Tab" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && composerEditor?.focus()) {
                event.preventDefault();
              }
            }}
          />
          {#if errors.subject}<Field.Error>{errors.subject}</Field.Error>{/if}
      </Field.Field>
    </div>
    <ComposerSchedule bind:this={scheduleDialog} bind:enabled={isScheduleEnabled} bind:local={scheduledLocal} error={errors.scheduledAt} onchange={() => { hasUserInput = true; errors.scheduledAt = ""; }} />
    <Field.Field data-invalid={!!errors.content} class="min-h-0 flex-1 gap-0 border-t pt-2">
      <Field.Label for="content" class="sr-only">Content</Field.Label>
      <div class="flex h-full min-h-0 flex-col gap-3">
        <Editor
          bind:this={composerEditor}
          bind:content
          bind:files
          onchange={() => { hasUserInput = true; }}
          disabled={isSending || checkingSend}
          aria-invalid={!!errors.content}
        />
        {#if errors.content}<Field.Error>{errors.content}</Field.Error>{/if}
        <ComposerAttachments bind:files />
      </div>
    </Field.Field>
  </Field.Group>
  </fieldset>
</form>
{/if}

<AlertDialog.Root bind:open={confirmSend}>
  <AlertDialog.Content onCloseAutoFocus={(event) => {
    event.preventDefault();
    if (!isSending && !disposed) sendTrigger?.focus();
  }}>
    <AlertDialog.Header>
      <AlertDialog.Title class="flex items-center gap-2">
        {#if attachmentWarning}<TriangleAlert class="size-5 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />{/if}
        {attachmentWarning ? "Missing an attachment?" : isScheduleEnabled ? "Schedule this message?" : "Send this message?"}
      </AlertDialog.Title>
      <AlertDialog.Description>
        {#if isScheduleEnabled}
          Resend will send this message on {scheduledLabel} ({Intl.DateTimeFormat().resolvedOptions().timeZone}), even if OnlySend is closed. You can cancel or reschedule it in the Resend dashboard.
          {#if attachmentWarning}Your message appears to mention an attachment, but no files have been added.{/if}
        {:else}
          {attachmentWarning ? "Your message appears to mention an attachment, but no files have been added. Send it without attachments?" : "Once sent, this message cannot be recalled."}
        {/if}
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
      <AlertDialog.Action form="compose-email-form" type="submit" name="confirm-send" disabled={isSending}>
        {isScheduleEnabled ? (attachmentWarning ? "Schedule anyway" : "Schedule message") : attachmentWarning ? "Send anyway" : "Send message"}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
