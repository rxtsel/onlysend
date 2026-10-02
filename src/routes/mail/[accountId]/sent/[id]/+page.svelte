<script lang="ts">
  import { useAccountId } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  const accountId = useAccountId();
  import { page } from "$app/state";
  import { onMount } from "svelte";

  import { Button } from "@/lib/components/ui/button/index.js";
  import { goto } from "$app/navigation";
  import type { SentEmail } from "@/lib/types";
  import { getSentEmail, isTemplateHtml } from "@/lib/shared/sent";
  import { formatEmailDate } from "@/lib/shared/utils/dates";
  import Editor from "@/lib/components/editor/editor.svelte";
  import { ArrowLeft, Loader } from "@lucide/svelte";

  let email = $state<SentEmail | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  async function loadEmail() {
    const emailId = page.params.id;
    if (!emailId) {
      error = "No email ID provided";
      isLoading = false;
      return;
    }

    try {
      isLoading = true;
      error = null;
      email = await getSentEmail(accountId, emailId);
    } catch (err) {
      console.error("Error loading email:", err);
      error = "Failed to load email";
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadEmail();
  });

  // The layout keys detail pages by account and message ID.
</script>

<div class="flex h-full flex-col">
  {#if isLoading}
    <div class="min-h-full w-full flex items-center justify-center">
      <div class="text-muted-foreground">
        <Loader class="animate-spin" />
      </div>
    </div>
  {:else if error || !email}
    <div class="flex flex-col items-center justify-center h-full gap-4">
      <div class="text-muted-foreground">{error || "Email not found"}</div>
      <Button variant="outline" onclick={() => goto(mailUrl(accountId, "sent"))}>
        <ArrowLeft class="mr-2 h-4 w-4" />
        Back to Sent
      </Button>
    </div>
  {:else}
    <!-- Email Header -->
    <div class="border-b">
      <div class="pb-6">
        <div class="grid items-center justify-between mb-4 relative">
          <footer class="flex-1">
            <h1 class="text-2xl font-semibold mb-2">{email.subject}</h1>
            <div class="flex items-center gap-2 text-sm text-muted-foreground">
              <span class="font-medium min-w-10">From:</span>

              <span class="bg-muted px-2 py-0.5 text-black rounded"
                >{email.from}</span
              >
            </div>
          </footer>
        </div>

        <div class="space-y-2 text-sm">
          <div class="flex items-start gap-2">
            <span class="text-muted-foreground font-medium min-w-10">To:</span>
            <div class="flex flex-wrap gap-1">
              {#each email.to as recipient}
                <span class="bg-muted px-2 py-0.5 rounded">{recipient}</span>
              {/each}
            </div>
          </div>

          {#if email.cc && email.cc.length > 0}
            <div class="flex items-start gap-2">
              <span class="text-muted-foreground font-medium min-w-10">CC:</span
              >
              <div class="flex flex-wrap gap-1">
                {#each email.cc as recipient}
                  <span class="bg-muted px-2 py-0.5 rounded">{recipient}</span>
                {/each}
              </div>
            </div>
          {/if}

          {#if email.replyTo}
            <div class="flex items-start gap-2">
              <span class="text-muted-foreground font-medium min-w-12"
                >Reply-To:</span
              >
              <span>{email.replyTo}</span>
            </div>
          {/if}

          <div class="flex items-center gap-2">
            <span class="text-muted-foreground font-medium min-w-12">Date:</span
            >
            <span>{formatEmailDate(email.createdAt)}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Email Content -->
    <div class="flex-1 overflow-y-auto">
      {#if email.html}
        {#if isTemplateHtml(email.html)}
          {@html email.html}
        {:else}
          <Editor content={email.html} editable={false} />
        {/if}
      {:else}
        <div class="text-muted-foreground text-sm p-6">
          No content available
        </div>
      {/if}
    </div>
  {/if}
</div>
