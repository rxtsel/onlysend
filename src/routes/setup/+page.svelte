<script lang="ts">
  import { provideAccount } from "$lib/features/auth/account-context";
  import { mailUrl } from "$lib/features/auth/mail-routes";
  import { listAccounts } from "$lib/shared/api/auth";
  let accountId = $state("");
  provideAccount(() => accountId);
  import { goto } from "$app/navigation";
  import { fly } from "svelte/transition";
  import { toast } from "svelte-sonner";
  import { Loader } from "@lucide/svelte";

  import { Button } from "@/lib/components/ui/button";
  import { createFromEmail, listFromEmails } from "@/lib/shared/from-emails";
  import {
    getOnboardingState,
    markSetupComplete,
  } from "@/lib/shared/api/auth";
  import {
    saveSelectedDomain,
    getSelectedDomain,
  } from "@/lib/shared/api/domains";
  import StepConnect from "@/lib/features/setup/components/step-connect.svelte";
  import StepDomains from "@/lib/features/setup/components/step-domains.svelte";
  import StepFromEmails from "@/lib/features/setup/components/step-from-emails.svelte";
  import Logo from "@/lib/components/logo.svelte";

  type Step = 1 | 2 | 3;

  let step = $state<Step>(1);
  let isSaving = $state(false);
  let selectedDomain = $state("");
  let emailOptions = $state<{ label: string; address: string }[]>([]);

  const HEADERS: Record<Step, { title: string; subtitle: string }> = {
    1: {
      title: "Add account",
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

  function nextStep() {
    step = (Math.min(step + 1, 3) as Step);
  }

  function prevStep() {
    step = (Math.max(step - 1, 1) as Step);
  }

  async function finish() {
    if (emailOptions.length === 0) return;

    try {
      isSaving = true;

      await Promise.all([
        saveSelectedDomain(accountId, selectedDomain),
        ...emailOptions.map((opt, i) =>
          createFromEmail(accountId, {
            label: opt.label,
            address: opt.address,
            isDefault: i === 0,
          }),
        ),
      ]);

      await markSetupComplete(accountId);
      toast.success("Account added!");
      goto(mailUrl(accountId, "sent"));
    } catch (err) {
      console.error(err);
      toast.error("Failed to save. Try again.");
    } finally {
      isSaving = false;
    }
  }
</script>

<svelte:head><title>Add Account — OnlySend</title></svelte:head>

<main
  class="container mx-auto min-h-svh flex justify-center items-center flex-col"
>
  <header class="text-center mb-8 w-full max-w-md">
    <Logo class="mx-auto mb-6" />
    <div class="grid items-start">
      {#key step}
        <div in:fly={{ y: 8, duration: 180 }}>
          <h1 class="text-2xl font-bold mb-3">{HEADERS[step].title}</h1>
          <p class="text-sm text-muted-foreground">{HEADERS[step].subtitle}</p>
        </div>
      {/key}
    </div>
  </header>

  {#if step === 1}
    <StepConnect onConnected={(id) => { accountId = id; nextStep(); }} />
  {:else if step === 2}
    <StepDomains bind:domain={selectedDomain} onContinue={nextStep} />
  {:else}
    <StepFromEmails
      bind:options={emailOptions}
      domain={selectedDomain}
      onBack={prevStep}
      onFinish={finish}
      {isSaving}
    />
  {/if}

  <p class="mt-6">
    <Button variant="link" size="sm" onclick={() => goto("/")}>
      Back to app
    </Button>
  </p>
</main>
