<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { BookTemplate, Trash2 } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import Button from "$lib/components/ui/button/button.svelte";
  import Input from "$lib/components/ui/input/input.svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Field from "$lib/components/ui/field";
  import { errorMessage } from "$lib/shared/utils/errors";

  type Template = { id: string; name: string; subject: string; html: string };
  let { accountId, content, subject, disabled = false, onapply }: { accountId: string; content: string; subject: string; disabled?: boolean; onapply: (template: Template) => void } = $props();
  let open = $state(false);
  let saving = $state(false);
  let panel = $state<"library" | "save">("library");
  let templates = $state<Template[]>([]);
  let loading = $state(false);
  let error = $state("");
  let name = $state("");
  let disposed = false;
  import { onDestroy } from "svelte";
  onDestroy(() => { disposed = true; });

  async function load() {
    loading = true;
    error = "";
    try {
      const result = await invoke<Template[]>("list_email_templates", { accountId });
      if (!disposed) templates = result;
    } catch (cause) { if (!disposed) error = errorMessage(cause, "Could not load templates"); }
    finally { if (!disposed) loading = false; }
  }
  export function show(next: "library" | "save") {
    panel = next; name = ""; error = ""; open = true;
    if (next === "library") void load();
  }
  async function save(template?: Template) {
    if (saving || disabled) return;
    if (template && !window.confirm(`Replace “${template.name}” with the current subject and design?`)) return;
    saving = true;
    error = "";
    try {
      await invoke<Template>("save_email_template", { accountId, id: template?.id ?? null, name: template?.name ?? name.trim(), subject, html: content });
      if (disposed) return;
      toast.success(template ? "Template updated" : "Template saved");
      if (template) await load(); else open = false;
    } catch (cause) { if (!disposed) error = errorMessage(cause, "Could not save template"); }
    finally { if (!disposed) saving = false; }
  }
  async function remove(template: Template) {
    if (saving || disabled || !window.confirm(`Delete “${template.name}”? This cannot be undone.`)) return;
    saving = true;
    error = "";
    try {
      await invoke("delete_email_template", { accountId, id: template.id });
      if (!disposed) templates = templates.filter(({ id }) => id !== template.id);
    } catch (cause) { if (!disposed) error = errorMessage(cause, "Could not delete template"); }
    finally { if (!disposed) saving = false; }
  }
  function apply(template: Template) {
    if (disabled || saving) return;
    if ((content.trim() || subject.trim()) && !window.confirm("Replace the current subject and design with this template? Recipients and attachments will stay unchanged.")) return;
    onapply(template);
    open = false;
  }
</script>

<Dialog.Root bind:open onOpenChange={(next) => { if (!saving) open = next; }}>
  <Dialog.Content class="sm:max-w-xl">
    <Dialog.Header>
      <Dialog.Title>{panel === "save" ? "Save template" : "Your templates"}</Dialog.Title>
      <Dialog.Description>Stored on this device for this account. Templates contain a subject and design, never recipients or attachments.</Dialog.Description>
    </Dialog.Header>
    {#if panel === "save"}
      <Field.Group>
        <Field.Field>
          <Field.Label for="template-name">Template name</Field.Label>
          <Input id="template-name" bind:value={name} maxlength={120} placeholder="e.g. Monthly newsletter" disabled={saving} onkeydown={(event) => { if (event.key === "Enter" && name.trim()) { event.preventDefault(); void save(); } }} />
        </Field.Field>
      </Field.Group>
      <Dialog.Footer><Button type="button" variant="outline" disabled={saving} onclick={() => open = false}>Cancel</Button><Button type="button" disabled={saving || !name.trim() || !content.trim()} onclick={() => save()}>{saving ? "Saving…" : "Save template"}</Button></Dialog.Footer>
    {:else if loading}
      <p class="py-6 text-center text-sm text-muted-foreground" role="status">Loading templates…</p>
    {:else if templates.length}
      <ul class="flex max-h-80 flex-col gap-2 overflow-y-auto">
        {#each templates as template (template.id)}
          <li class="flex flex-wrap items-center gap-2 rounded-lg border p-3">
            <div class="min-w-0 flex-1"><p class="truncate text-sm font-medium">{template.name}</p><p class="truncate text-xs text-muted-foreground">{template.subject || "No subject"}</p></div>
            <Button type="button" variant="outline" size="sm" disabled={saving || disabled} onclick={() => apply(template)}>Use template</Button>
            <Button type="button" variant="ghost" size="sm" disabled={saving || disabled || !content.trim()} onclick={() => save(template)}>Update</Button>
            <Button type="button" variant="ghost" size="icon-sm" aria-label={`Delete ${template.name}`} disabled={saving || disabled} onclick={() => remove(template)}><Trash2 /></Button>
          </li>
        {/each}
      </ul>
    {:else if !error}
      <div class="flex flex-col items-center gap-2 py-8 text-center"><BookTemplate class="size-8 text-muted-foreground" /><p class="text-sm font-medium">Your next email starts here</p><p class="text-sm text-muted-foreground">Save a design from the composer to reuse it later.</p></div>
    {/if}
    {#if error}<p class="text-sm text-destructive" role="alert">{error}</p>{#if panel === "library"}<Button type="button" variant="outline" disabled={saving} onclick={load}>Try again</Button>{/if}{/if}
  </Dialog.Content>
</Dialog.Root>
