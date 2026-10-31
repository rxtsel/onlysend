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
  import { LoaderCircle } from "@lucide/svelte";
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
  import { getContext, onMount, onDestroy, tick, untrack } from "svelte";
  import { listFromEmails, formatFromEmail } from "@/lib/shared/from-emails";

  // The layout keys this page by reply ID, so each message owns a separate draft.
  const replyEmailId = untrack(() => page.url.searchParams.get("reply"));
  const isReply = replyEmailId !== null;
  const composerStatus = getContext<ComposerStatus>(COMPOSER_STATUS);
  let reply = $state<ReplyContext | null>(null);
  let initializing = $state(true);
  let initializationError = $state("");
  let initialTo = "";
  let toEmails = $state<Tag[]>([]);
  let fromEmails = $state<FromEmail[]>([]);
  let from = $state<string>("");
  let initialFrom = "";
  let subject = $state<string>("");
  let composerEditor = $state<{ focus: () => boolean } | null>(null);
  let content = $state<string>("");
  let files = $state<File[]>([]);
  let isSending = $state(false);

  let isCcEnabled = $state<boolean>(false);
  let isBccEnabled = $state<boolean>(false);
  let ccEmails = $state<Tag[]>([]);
  let bccEmails = $state<Tag[]>([]);

  let errors: Record<string, string> = $state({});
  let disposed = false;
  let sent = false;
  const unregisterGuard = registerDraftGuard(accountId, () =>
    sent || !((!isReply && subject) || content || toEmails.map((tag) => tag.text).join("\n") !== initialTo || ccEmails.length ||
      bccEmails.length || files.length || from !== initialFrom) ||
    window.confirm("Leave this page and discard the current draft?"),
  );
  $effect(() => {
    composerStatus.ready = !initializing && !initializationError && !sent;
    composerStatus.sending = isSending;
  });
  onDestroy(() => {
    disposed = true;
    unregisterGuard();
    composerStatus.ready = false;
    composerStatus.sending = false;
  });
  beforeNavigate(({ cancel }) => {
    if (!allowAccountNavigation(accountId)) cancel();
  });

  async function handleSubmit(event: Event) {
    event.preventDefault();
    if (isSending || initializing || initializationError || sent || (isReply && !reply)) return;

    errors = {};
    const form = event.currentTarget as HTMLFormElement;
    const payload = {
      from, // already bound via Select
      to: toEmails.map((t) => t.text), // Tag[] -> string[]
      subject: isReply ? reply!.subject : subject,
      bcc: isBccEnabled ? bccEmails.map((t) => t.text) : [],
      cc: isCcEnabled ? ccEmails.map((t) => t.text) : [],
      content,
      replyEmailId,
    };

    const parsed = emailComposerSchema.safeParse(payload);

    if (!parsed.success) {
      parsed.error.issues.forEach((issue) => {
        const field = issue.path[0] as string;
        errors[field] = issue.message;
      });
      return;
    }

    isSending = true;
    const sendingToast = toast.loading("Sending message…");
    try {
      const attachments = await encodeAttachments(files);
      if (disposed) return;
      await invoke("send_email", { accountId, data: { ...parsed.data, attachments } });

      if (disposed) return;
      sent = true;
      // Reset form and states
      toEmails = [];
      from = "";
      subject = "";
      content = "";
      files = [];
      isCcEnabled = false;
      isBccEnabled = false;
      ccEmails = [];
      bccEmails = [];
      form.reset();

      toast.success("Email send successfully!");
      // Bust the list cache so the sidebar shows the new email immediately
      window.dispatchEvent(new CustomEvent("email-sent"));

      goto(mailUrl(accountId, "sent"));
    } catch (err) {
      if (disposed) return;
      console.error(err);
      toast.error(errorMessage(err, "Failed to send email. Please try again."));
    } finally {
      toast.dismiss(sendingToast);
      if (!disposed) isSending = false;
    }
  }

  async function initialize() {
    initializing = true;
    initializationError = "";
    try {
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
        initialTo = context.to.join("\n");
      }
      const defaultFrom = list.find((sender) => context?.fromCandidates.some((recipient) => recipient.toLowerCase() === sender.address.toLowerCase()))
        ?? list.find((sender) => sender.isDefault) ?? list[0];
      if (defaultFrom) { from = formatFromEmail(defaultFrom); initialFrom = from; }
    } catch (cause) {
      if (!disposed) initializationError = errorMessage(cause, "Could not prepare this message");
    } finally {
      if (!disposed) initializing = false;
    }
    await tick();
    if (!disposed && isReply && !initializationError) composerEditor?.focus();
  }
  onMount(() => { void initialize(); });
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
  id="compose-email-form"
  aria-busy={isSending}
>
  <fieldset disabled={isSending} class="contents">
  <Field.Group class="h-full min-h-0 gap-0">
    <div class="flex shrink-0 flex-col gap-3 pb-4">
      <div class="flex items-end gap-3">
        <Field.Field data-invalid={!!errors.from} class="flex-1">
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
          <ToggleGroup.Root type="multiple" variant="outline">
            <ToggleGroup.Item value="cc" onclick={() => (isCcEnabled = !isCcEnabled)}>Cc</ToggleGroup.Item>
            <ToggleGroup.Item value="bcc" onclick={() => (isBccEnabled = !isBccEnabled)}>Bcc</ToggleGroup.Item>
          </ToggleGroup.Root>
        </Field.Field>
      </div>
      <div class="flex gap-3">
        <Field.Field data-invalid={!!errors.to} class="flex-1">
          <Field.Label for="to">To</Field.Label>
          <InputTag id="to" type="email" bind:value={toEmails} name="to" placeholder="Add recipient email" aria-invalid={!!errors.to} />
          {#if errors.to}<Field.Error>{errors.to}</Field.Error>{/if}
        </Field.Field>
        {#if isCcEnabled}
          <Field.Field data-invalid={!!errors.cc} class="flex-1">
            <Field.Label for="cc">Cc</Field.Label>
            <InputTag id="cc" type="email" bind:value={ccEmails} name="cc" placeholder="Add Cc email" aria-invalid={!!errors.cc} />
            {#if errors.cc}<Field.Error>{errors.cc}</Field.Error>{/if}
          </Field.Field>
        {/if}
      </div>
      {#if isBccEnabled}
        <Field.Field data-invalid={!!errors.bcc}>
          <Field.Label for="bcc">Bcc</Field.Label>
          <InputTag id="bcc" type="email" bind:value={bccEmails} name="bcc" placeholder="Add Bcc email" aria-invalid={!!errors.bcc} />
          {#if errors.bcc}<Field.Error>{errors.bcc}</Field.Error>{/if}
        </Field.Field>
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
    <Field.Field data-invalid={!!errors.content} class="min-h-0 flex-1 gap-0 border-t pt-2">
      <Field.Label for="content" class="sr-only">Content</Field.Label>
      <div class="flex h-full min-h-0 flex-col gap-3">
        <Editor
          bind:this={composerEditor}
          bind:content
          bind:files
          disabled={isSending}
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
