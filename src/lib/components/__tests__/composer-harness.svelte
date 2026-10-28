<script lang="ts">
  import { setContext } from "svelte";
  import { provideAccount } from "$lib/features/auth/account-context";
  import { COMPOSER_STATUS, type ComposerStatus } from "$lib/features/sending/composer-context";
  import Composer from "../../../routes/mail/[accountId]/composer/+page.svelte";
  const status = $state<ComposerStatus>({ ready: false, sending: false });
  provideAccount(() => "account-a");
  setContext(COMPOSER_STATUS, status);
</script>
<Composer />
<output data-composer-ready>{String(status.ready)}</output>
<output data-composer-sending>{String(status.sending)}</output>
