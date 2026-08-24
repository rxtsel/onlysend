<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";

  import {
    SLIDE_IN,
    SLIDE_OUT,
    SLIDE_BACK_IN,
    SLIDE_BACK_OUT,
    createViewAnimator,
    VIEW_WRAP_CLASS,
  } from "../view-transition.svelte";

  type View = "buttons" | "manual" | "connecting";

  /** Alerts stay on screen until the user manually dismisses them. */
  const ALERT = { duration: Infinity } as const;

  import {
    connectResend,
    probeFullAccess,
    saveApiKey,
  } from "@/lib/shared/api/auth";
  import { apiKeySchema } from "@/lib/schemas/api-key.schema";
  import { toast } from "svelte-sonner";
  import { copyToClipboard } from "@/lib/shared/services/clipboard.svelte";

  import { Button } from "@/lib/components/ui/button";
  import * as Field from "@/lib/components/ui/field";
  import { Input } from "@/lib/components/ui/input";
  import { ArrowRight, Loader } from "@lucide/svelte";

  let {
    apiKeyValue = $bindable(""),
    onConnected,
  }: {
    apiKeyValue?: string;
    onConnected: () => void;
  } = $props();

  let view = $state<View>("buttons");
  let errors = $state<Record<string, string>>({});
  let authorizeUrl = $state("");

  /* ---------------------------------------------------------
   * ANIMATED VIEW SWITCHING (slide + height)
   * --------------------------------------------------------- */
  const animator = createViewAnimator();

  async function switchView(next: View) {
    await animator.transition(next, () => {
      view = next;
    });
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;

    listen<{ success: boolean; warning?: string; error?: string }>(
      "oauth://done",
      (event) => {
        if (event.payload.success) {
          if (event.payload.warning === "send_only") {
            switchView("buttons");
            toast.warning(
              "Connected, but the grant only allows sending. OnlySend needs Full access. Reconnect and approve the requested permissions.",
              ALERT,
            );
            return;
          }

          onConnected();
          return;
        }

        switchView("buttons");
        toast.error(event.payload.error ?? "Failed to connect with Resend");
      },
    ).then((fn) => (unlisten = fn));

    return () => unlisten?.();
  });

  async function handleConnect(e: MouseEvent) {
    e.preventDefault();
    errors = {};

    try {
      await switchView("connecting");
      authorizeUrl = await connectResend();
    } catch (err) {
      await switchView("buttons");
      console.error(err);
      toast.error("Failed to start the connection. Try again.");
    }
  }

  async function handleCancelConnect() {
    authorizeUrl = "";
    await switchView("buttons");
  }

  async function handleCopyLink() {
    await copyToClipboard(authorizeUrl);
  }

  function submitManual(e: SubmitEvent) {
    e.preventDefault();
    errors = {};

    const result = apiKeySchema.safeParse(apiKeyValue);
    if (!result.success) {
      errors.apiKey = result.error.issues[0].message;
      return;
    }

    verifyAndContinue();
  }

  /* ---------------------------------------------------------
   * PERMISSION CHECK: reject send-only API keys
   * --------------------------------------------------------- */
  let isVerifying = $state(false);

  async function verifyAndContinue() {
    isVerifying = true;

    try {
      const probe = await probeFullAccess(apiKeyValue);

      if (!probe.fullAccess) {
        const msg =
          "This API key only has sending access. OnlySend needs Full access to manage your emails and domains.";
        errors.apiKey = msg;
        toast.error(msg, ALERT);
        return;
      }

      // Persist immediately so later wizard steps (domains) can use it.
      await saveApiKey(apiKeyValue);
      onConnected();
    } catch (err) {
      // Network/API failure: warn but don't block the user.
      console.error(err);
      toast.warning(
        "Could not verify the API key permissions right now, continuing anyway.",
        ALERT,
      );
      await saveApiKey(apiKeyValue);
      onConnected();
    } finally {
      isVerifying = false;
    }
  }
</script>

<div class="w-full max-w-sm">
  <div bind:this={animator.element} class={VIEW_WRAP_CLASS}>
    <!-- -----------------------------------------------------
         WAITING VIEW (slides in from the right)
    ------------------------------------------------------ -->
    {#if view === "connecting"}
      <div
        class="[grid-area:1/1] min-w-0 flex flex-col items-center text-center gap-3 py-6"
        data-view="connecting"
        in:fly={SLIDE_BACK_IN}
        out:fly={SLIDE_BACK_OUT}
      >
        <Loader class="animate-spin" />
        <p class="font-medium">Waiting for authorization...</p>
        <p class="text-sm text-muted-foreground">
          Complete the sign in from your browser.
        </p>

        <p class="text-xs text-muted-foreground mt-4">
          Browser didn't open? Click to copy the link:
        </p>
        <button
          type="button"
          class="max-w-full font-mono text-xs text-muted-foreground truncate cursor-pointer hover:bg-muted/60 hover:text-foreground rounded px-2 py-1 border border-dashed"
          title={`Copy ${authorizeUrl}`}
          onclick={handleCopyLink}
        >
          {authorizeUrl}
        </button>

        <Button
          type="button"
          variant="outline"
          size="sm"
          class="mt-4"
          onclick={handleCancelConnect}
        >
          Cancel
        </Button>
      </div>
    <!-- -----------------------------------------------------
         MANUAL FORM VIEW (slides in from the left)
    ------------------------------------------------------ -->
    {:else if view === "manual"}
      <div
        class="[grid-area:1/1] min-w-0 w-full"
        data-view="manual"
        in:fly={SLIDE_IN}
        out:fly={SLIDE_OUT}
      >
        <form onsubmit={submitManual}>
          <Field.Group>
            <Field.Field>
              <Field.Label for="apiKey">Resend API Key</Field.Label>
              <Input
                id="apiKey"
                bind:value={apiKeyValue}
                placeholder="re_123abc..."
                aria-invalid={!!errors.apiKey}
                autofocus
              />
              {#if errors.apiKey}
                <Field.Error>{errors.apiKey}</Field.Error>
              {/if}
            </Field.Field>
            <Field.Field>
              <Field.Description>
                You can find your API key in your
                <button
                  type="button"
                  class="underline underline-offset-4 font-medium"
                  onclick={() => openUrl("https://resend.com/api-keys")}
                >
                  Resend dashboard
                </button>.
              </Field.Description>
            </Field.Field>

            <Button type="submit" class="w-full" disabled={isVerifying}>
              {isVerifying ? "Verifying..." : "Continue"}
              {#if !isVerifying}<ArrowRight />{/if}
            </Button>

            <p class="text-center text-sm text-muted-foreground mt-4">
              You prefer automatically?
              <button
                type="button"
                class="underline underline-offset-4 font-medium hover:text-foreground"
                onclick={() => {
                  errors = {};
                  switchView("buttons");
                }}
              >
                Connect with Resend
              </button>
            </p>
          </Field.Group>
        </form>
      </div>
    <!-- -----------------------------------------------------
         BUTTONS VIEW (default)
    ------------------------------------------------------ -->
    {:else}
      <div
        class="[grid-area:1/1] min-w-0 w-full"
        data-view="buttons"
        in:fly={SLIDE_BACK_IN}
        out:fly={SLIDE_BACK_OUT}
      >
        <Button type="button" class="w-full" onclick={handleConnect}>
          Connect with Resend
        </Button>

        <p class="text-center text-sm text-muted-foreground mt-4">
          You prefer manually?
          <button
            type="button"
            class="underline underline-offset-4 font-medium hover:text-foreground"
            onclick={() => switchView("manual")}
          >
            Enter your API key
          </button>
        </p>
      </div>
    {/if}
  </div>
</div>
