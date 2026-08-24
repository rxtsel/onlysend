<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";

  import {
    getOnboardingState,
    getSelectedDomain,
    markSetupComplete,
    saveSelectedDomain,
  } from "@/lib/shared/store";
  import Logo from "@/lib/components/logo.svelte";

  import { toast } from "svelte-sonner";

  import { Skeleton } from "@/lib/components/ui/skeleton";
  import { createFromEmail, listFromEmails } from "@/lib/shared/from-emails";
  import StepConnect from "@/lib/components/setup/step-connect.svelte";
  import StepDomains from "@/lib/components/setup/step-domains.svelte";
  import StepFromEmails from "@/lib/components/setup/step-from-emails.svelte";
  import {
    SLIDE_IN,
    SLIDE_OUT,
    SLIDE_BACK_IN,
    SLIDE_BACK_OUT,
    createViewAnimator,
    VIEW_WRAP_CLASS,
  } from "@/lib/components/setup/view-transition.svelte";

  /* ---------------------------------------------------------
   * WIZARD STATE
   * --------------------------------------------------------- */
  let step = $state<1 | 2 | 3>(1);
  let direction = $state<"forward" | "back">("forward");

  let isChecking = $state(true);
  let isSaving = $state(false);

  let apiKeyValue = $state("");
  let selectedDomain = $state("");
  let emailOptions = $state<{ label: string; address: string }[]>([]);

  const animator = createViewAnimator();

  const HEADERS: Record<1 | 2 | 3, { title: string; subtitle: string }> = {
    1: {
      title: "Welcome to OnlySend",
      subtitle: "Connect your Resend account to get started.",
    },
    2: {
      title: "Choose a domain",
      subtitle: "Pick the domain you will send emails from.",
    },
    3: {
      title: "Email Options",
      subtitle: "Add the email identities you will use to send emails.",
    },
  };

  /* ---------------------------------------------------------
   * RESUME ONBOARDING ON LOAD
   * --------------------------------------------------------- */
  onMount(async () => {
    isChecking = true;
    try {
      const state = await getOnboardingState();

      if (state.complete) {
        goto("/mail/sent");
        return;
      }

      // Resume position is derived, not stored: with a credential the
      // user always re-confirms their domain at step 2.
      step = state.authenticated ? 2 : 1;
    } catch (err) {
      console.error(err);
    } finally {
      isChecking = false;
    }
  });

  /* ---------------------------------------------------------
   * STEP HANDLERS
   * --------------------------------------------------------- */
  async function switchStep(to: 1 | 2 | 3) {
    if (to === step) return;
    direction = to > step ? "forward" : "back";
    await animator.transition(`step-${to}`, () => {
      step = to;
    });
  }

  function nextStep() {
    switchStep(Math.min(step + 1, 3) as 1 | 2 | 3);
  }

  /**
   * Reconnect fast-path: when local data (identities + domain) survives a
   * disconnect, connecting restores the session without repeating setup.
   */
  async function handleConnected() {
    try {
      const [emails, savedDomain] = await Promise.all([
        listFromEmails(),
        getSelectedDomain(),
      ]);

      if (emails.length > 0 && savedDomain) {
        selectedDomain = savedDomain;
        await markSetupComplete();
        toast.success("Welcome back!");
        goto("/mail/sent");
        return;
      }
    } catch (err) {
      console.error(err);
    }
    nextStep();
  }

  function prevStep() {
    switchStep(Math.max(step - 1, 1) as 1 | 2 | 3);
  }

  async function finish() {
    if (emailOptions.length === 0) return;

    try {
      isSaving = true;

      await Promise.all([
        saveSelectedDomain(selectedDomain),
        markSetupComplete(),
        ...emailOptions.map((opt, i) =>
          createFromEmail({
            label: opt.label,
            address: opt.address,
            isDefault: i === 0,
          }),
        ),
      ]);

      toast.success("Setup complete! Welcome to OnlySend");
      goto("/mail/sent");
    } catch (err) {
      console.error(err);
      toast.error("Failed to save. Try again.");
    } finally {
      isSaving = false;
    }
  }
</script>

{#if isChecking}
  <main
    class="container mx-auto py-6 min-h-svh flex justify-center items-center flex-col"
  >
    <header class="text-center mb-8 w-full">
      <Skeleton class="size-16 rounded-xl mx-auto mb-6" />
      <Skeleton class="h-8 w-72 mx-auto mb-4" />
      <Skeleton class="h-4 w-80 mx-auto" />
    </header>
    <div class="w-full max-w-sm space-y-3">
      <Skeleton class="h-10 w-full rounded-md" />
      <Skeleton class="h-10 w-full rounded-md" />
      <Skeleton class="h-9 w-full rounded-md" />
    </div>
  </main>
{:else}
  <main
    class="container mx-auto py-6 min-h-svh flex justify-center items-center flex-col"
  >
    <header class="text-center mb-8 w-full max-w-md">
      <Logo class="mx-auto mb-6" />
      <div class="grid items-start">
        {#key step}
          <div
            class="[grid-area:1/1]"
            in:fade={{ duration: 180 }}
            out:fade={{ duration: 120 }}
          >
            <h1 class="text-3xl font-bold mb-4">{HEADERS[step].title}</h1>
            <p class="max-w-prose text-balance">{HEADERS[step].subtitle}</p>
          </div>
        {/key}
      </div>
    </header>

    <!-- -----------------------------------------------------
         ANIMATED STEP REGION
    ------------------------------------------------------ -->
    <div bind:this={animator.element} class={`w-full ${VIEW_WRAP_CLASS}`}>
      <!-- STEP 1: CONNECT RESEND (OAuth / API key) -->
      {#if step === 1}
        <div
          class="[grid-area:1/1] min-w-0 w-full flex flex-col items-center"
          data-view="step-1"
          in:fly={direction === "forward" ? SLIDE_IN : SLIDE_BACK_IN}
          out:fly={direction === "forward" ? SLIDE_OUT : SLIDE_BACK_OUT}
        >
          <StepConnect bind:apiKeyValue onConnected={handleConnected} />
        </div>

      <!-- STEP 2: DOMAINS -->
      {:else if step === 2}
        <div
          class="[grid-area:1/1] min-w-0 w-full flex flex-col items-center"
          data-view="step-2"
          in:fly={direction === "forward" ? SLIDE_IN : SLIDE_BACK_IN}
          out:fly={direction === "forward" ? SLIDE_OUT : SLIDE_BACK_OUT}
        >
          <StepDomains bind:domain={selectedDomain} onContinue={nextStep} />
        </div>

      <!-- STEP 3: FROM EMAIL OPTIONS -->
      {:else}
        <div
          class="[grid-area:1/1] min-w-0 w-full flex flex-col items-center"
          data-view="step-3"
          in:fly={direction === "forward" ? SLIDE_IN : SLIDE_BACK_IN}
          out:fly={direction === "forward" ? SLIDE_OUT : SLIDE_BACK_OUT}
        >
          <StepFromEmails
            bind:options={emailOptions}
            domain={selectedDomain}
            onBack={prevStep}
            onFinish={finish}
            {isSaving}
          />
        </div>
      {/if}
    </div>
  </main>
{/if}
