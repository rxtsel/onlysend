<script lang="ts">
  import { page } from "$app/state";
</script>

<main class="flex min-h-screen flex-col items-center justify-center gap-4 p-6">
  <h1 class="text-xl font-semibold">Account unavailable</h1>
  <p>{page.error?.message ?? "This account could not be opened."}</p>
  <a class="underline" href="/">Return to accounts</a>
</main>
