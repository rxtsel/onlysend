<script lang="ts">
  import DomainConfiguration from "./domain-configuration.svelte";
  import { toast } from "svelte-sonner";
</script>

<DomainConfiguration mode="settings" onSaved={() => toast.success("Domain selection saved")} />
