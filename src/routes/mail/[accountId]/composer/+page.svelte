<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { registerDraftGuard, allowAccountNavigation } from "$lib/features/auth/draft-navigation";
  const accountId = useAccountId();
  import * as Popover from "$lib/components/ui/popover";
  import { Input } from "@/lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import * as Field from "@/lib/components/ui/field";
  import { InputTag } from "@/lib/components/ui/input-tag";
  import type { Tag } from "@/lib/components/ui/input-tag/input-tag.svelte";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { Editor } from "@/lib/components/editor";
  import { ChevronsRight, X } from "@lucide/svelte";
  import { toggleVariants } from "@/lib/components/ui/toggle";
  import { buttonVariants } from "@/lib/components/ui/button";
  import Button from "@/lib/components/ui/button/button.svelte";
  import { emailComposerSchema } from "@/lib/schemas/email-composer.schema";
  import { toast } from "svelte-sonner";
  import { goto, beforeNavigate } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import type { FromEmail } from "@/lib/types";
  import { encodeAttachments } from "$lib/shared/attachments";
  import { errorMessage } from "$lib/shared/utils/errors";
  import ComposerAttachments from "$lib/components/composer-attachments.svelte";
  import { onMount, onDestroy } from "svelte";
  import { listFromEmails, formatFromEmail } from "@/lib/shared/from-emails";

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
  let isReplyToEnabled = $state<boolean>(false);
  let ccEmails = $state<Tag[]>([]);
  let bccEmails = $state<Tag[]>([]);

  let messageId = $state<string | null>(null);
  let morePopoverOpen = $state(false);
  let messageIdPopoverOpen = $state(false);

  let errors: Record<string, string> = $state({});
  let disposed = false;
  let sent = false;
  const unregisterGuard = registerDraftGuard(accountId, () =>
    sent || !(subject || content || toEmails.length || ccEmails.length ||
      bccEmails.length || files.length || isReplyToEnabled || messageId || from !== initialFrom) ||
    window.confirm("Leave this page and discard the current draft?"),
  );
  onDestroy(() => { disposed = true; unregisterGuard(); });
  beforeNavigate(({ cancel }) => {
    if (!allowAccountNavigation(accountId)) cancel();
  });

  async function handleSubmit(event: Event) {
    event.preventDefault();
    if (isSending) return;

    errors = {};
    const form = event.currentTarget as HTMLFormElement;
    const formData = new FormData(form);

    // Build payload from state, not direct FormData arrays
    const payload = {
      from, // already bound via Select
      to: toEmails.map((t) => t.text), // Tag[] -> string[]
      subject: messageId
        ? subject.startsWith("Re: ")
          ? subject
          : "Re: " + subject
        : subject,
      bcc: isBccEnabled ? bccEmails.map((t) => t.text) : [],
      cc: isCcEnabled ? ccEmails.map((t) => t.text) : [],
      replyTo: isReplyToEnabled
        ? (formData.get("replyTo") as string | null)
        : null,
      content,
      messageId,
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
      isReplyToEnabled = false;
      ccEmails = [];
      bccEmails = [];
      messageId = null;
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

  // Get From email options
  onMount(async () => {
    try {
      const list = await listFromEmails(accountId);
      if (disposed) return;
      fromEmails = list;

      const defaultFrom = list.find((f) => f.isDefault) ?? list[0];

      if (defaultFrom) {
        from = formatFromEmail(defaultFrom);
        initialFrom = from;
      }
    } catch (e) {
      console.error("[fromEmails] load error", e);
    }
  });
</script>

<form
  class="h-full w-full overflow-hidden px-1"
  onsubmit={handleSubmit}
  id="compose-email-form"
  aria-busy={isSending}
>
  <fieldset disabled={isSending} class="contents">
  <Field.Group class="h-full">
    <!-- From and Group buttons -->
    <div class="flex gap-1 items-end">
      <Field.Field class="flex-1">
        <Field.Label for="from">From</Field.Label>
        <Select.Root type="single" name="from" bind:value={from}>
          <Select.Trigger
            class="w-[180px]"
            id="from"
            aria-invalid={!!errors.from}
          >
            {#if from}
              {from}
            {:else}
              Select an email
            {/if}
          </Select.Trigger>
          <Select.Content>
            {#if fromEmails.length === 0}
              <span class="w-full text-center text-sm inline-block py-1">
                Create email address in: Settings > Email sender options
              </span>
            {:else}
              {#each fromEmails as option}
                <Select.Item value={formatFromEmail(option)}
                  >{formatFromEmail(option)}</Select.Item
                >
              {/each}
            {/if}
          </Select.Content>
        </Select.Root>
        {#if errors.from}
          <Field.Error>{errors.from}</Field.Error>
        {/if}
      </Field.Field>
      <Field.Field class="shrink-0 w-fit">
        <ToggleGroup.Root type="multiple" variant="outline">
          <ToggleGroup.Item
            value="cc"
            onclick={() => (isCcEnabled = !isCcEnabled)}
          >
            Cc
          </ToggleGroup.Item>
          <ToggleGroup.Item
            value="bcc"
            onclick={() => (isBccEnabled = !isBccEnabled)}
          >
            Bcc
          </ToggleGroup.Item>

          <Popover.Root bind:open={morePopoverOpen}>
            <Popover.Trigger
              class={toggleVariants({
                className: "border-l-0 rounded-l-none",
                variant: "outline",
                size: "default",
              })}><ChevronsRight class="size-4" /></Popover.Trigger
            >
            <Popover.Content class="w-fit space-y-1">
              <ToggleGroup.Item
                value="replyTo"
                class="px-5 rounded-md w-full"
                onclick={() => {
                  isReplyToEnabled = !isReplyToEnabled;
                  morePopoverOpen = false;
                  messageIdPopoverOpen = false;
                }}
              >
                Reply To
              </ToggleGroup.Item>
              {@render headers()}
            </Popover.Content>
          </Popover.Root>
        </ToggleGroup.Root>
      </Field.Field>
    </div>

    <!-- To and cc -->
    <div class="flex gap-1">
      <Field.Field class="flex-1">
        <Field.Label for="to">To</Field.Label>
        <InputTag
          id="to"
          type="email"
          bind:value={toEmails}
          name="to"
          placeholder="Add recipient email"
          aria-invalid={!!errors.to}
        />
        {#if errors.to}
          <Field.Error>{errors.to}</Field.Error>
        {/if}
      </Field.Field>
      {#if isCcEnabled}
        <Field.Field class="flex-1">
          <Field.Label for="cc">Cc</Field.Label>
          <InputTag
            id="cc"
            type="email"
            bind:value={ccEmails}
            name="cc"
            placeholder="Add Cc email"
          />
        </Field.Field>
      {/if}
    </div>

    <!-- Bcc and Reply-To -->
    {#if isBccEnabled || isReplyToEnabled}
      <div class="flex gap-1">
        {#if isBccEnabled}
          <Field.Field class="flex-1">
            <Field.Label for="bcc">Bcc</Field.Label>
            <InputTag
              id="bcc"
              type="email"
              bind:value={bccEmails}
              name="bcc"
              placeholder="Add Bcc email"
            />
          </Field.Field>
        {/if}
        {#if isReplyToEnabled}
          <Field.Field class="flex-1">
            <Field.Label for="replyTo">Reply To</Field.Label>
            <Input
              id="replyTo"
              type="email"
              name="replyTo"
              placeholder="Reply-To email"
            />
          </Field.Field>
        {/if}
      </div>
    {/if}

    <Field.Field>
      <Field.Label for="subject">Subject</Field.Label>
      <Input
        id="subject"
        type="text"
        name="subject"
        placeholder="Subject"
        bind:value={subject}
        aria-invalid={!!errors.subject}
        onkeydown={(event) => {
          if (event.key === "Tab" && !event.shiftKey && !event.ctrlKey && !event.metaKey && !event.altKey && composerEditor?.focus()) {
            event.preventDefault();
          }
        }}
      />
      {#if errors.subject}
        <Field.Error>{errors.subject}</Field.Error>
      {/if}
    </Field.Field>
    <Field.Field class="h-full">
      <Field.Label for="content">Content</Field.Label>
      <div class="h-full flex flex-col">
        <Editor
          bind:this={composerEditor}
          bind:content
          bind:files
          disabled={isSending}
          aria-invalid={!!errors.content}
        />
        <ComposerAttachments bind:files />
      </div>
    </Field.Field>
  </Field.Group>
  </fieldset>
</form>

{#snippet headers()}
  <Popover.Root bind:open={messageIdPopoverOpen}>
    <Popover.Trigger
      class={buttonVariants({
        className: "w-full block",
        variant: "ghost",
      })}
    >
      Add message ID to headers
    </Popover.Trigger>
    <Popover.Content>
      <div class="relative">
        <Input
          placeholder="Message-ID"
          class="w-full"
          name="messageId"
          bind:value={messageId}
          onkeydown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              morePopoverOpen = false;
              messageIdPopoverOpen = false;
              subject = subject.startsWith("Re: ") ? subject : "Re: " + subject;
            }
          }}
        />
        <Button
          class="absolute right-0 inset-y-0"
          variant="ghost"
          onclick={() => {
            messageId = null;
            morePopoverOpen = false;
            messageIdPopoverOpen = false;
            subject = subject.replace(/^Re:\s*/, "");
          }}
        >
          <X class="size-4" />
        </Button>
      </div>
    </Popover.Content>
  </Popover.Root>
{/snippet}
