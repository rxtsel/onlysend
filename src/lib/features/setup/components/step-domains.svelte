<script lang="ts">
  import DomainConfiguration from "$lib/features/domains/components/domain-configuration.svelte";
  import type { DomainSummary } from "$lib/shared/api/domains";

  let {
    domain = $bindable(""),
    selectedDomains = $bindable([]),
    onContinue,
  }: {
    domain?: string;
    selectedDomains?: DomainSummary[];
    onContinue: (domains: DomainSummary[]) => void;
  } = $props();
</script>

<DomainConfiguration bind:domain bind:selectedDomains mode="setup" onSaved={onContinue} />
