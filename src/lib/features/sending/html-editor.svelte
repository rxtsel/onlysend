<script lang="ts">
  import { onDestroy, type Snippet } from "svelte";
  import { Eye, Monitor, Smartphone, Undo2, Redo2 } from "@lucide/svelte";
  import Button from "$lib/components/ui/button/button.svelte";
  import Input from "$lib/components/ui/input/input.svelte";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { prepareEmailHtml } from "$lib/shared/email-html";
  import { errorMessage } from "$lib/shared/utils/errors";
  import { HtmlDocument, previewDocument } from "./html-document";
  import { cn } from "$lib/utils";

  let { content = $bindable(""), disabled = false, toolbarActions, "aria-invalid": ariaInvalid }: { content?: string; disabled?: boolean; toolbarActions?: Snippet; "aria-invalid"?: boolean } = $props();
  let view = $state("visual");
  let width = $state("desktop");
  let frame = $state<HTMLIFrameElement | null>(null);
  let srcdoc = $state("");
  let loading = $state(false);
  let error = $state("");
  let retry = $state(0);
  let model: HtmlDocument | null = null;
  let selectedLink = $state<number | null>(null);
  let linkUrl = $state("");
  let linkError = $state("");
  let undo = $state<string[]>([]);
  let redo = $state<string[]>([]);
  let cleanup: (() => void) | undefined;
  let restoreFocus: number | null = null;
  // Toolbar clicks blur the iframe before undo runs.
  let focusedRegion: number | null = null;

  export function replaceContent(html: string) {
    if (disabled) return;
    view = "visual";
    change(html);
  }

  export function focus(): boolean {
    const target = frame?.contentDocument?.querySelector<HTMLElement>('[contenteditable="plaintext-only"]');
    target?.focus();
    return !!target;
  }

  function change(next: string) {
    if (disabled || next === content) return;
    undo = [...undo.slice(-29), content];
    redo = [];
    content = next;
  }
  function history(direction: "undo" | "redo") {
    if (disabled) return;
    const stack = direction === "undo" ? undo : redo;
    if (!stack.length) return;
    restoreFocus = focusedRegion;
    const next = stack[stack.length - 1];
    if (direction === "undo") { undo = undo.slice(0, -1); redo = [...redo, content]; }
    else { redo = redo.slice(0, -1); undo = [...undo, content]; }
    content = next;
    selectedLink = null;
  }

  $effect(() => {
    const source = content;
    const currentView = view;
    void retry;
    if (currentView === "visual" && source === model?.html && srcdoc) return;
    model = null;
    let cancelled = false;
    loading = true;
    error = "";
    selectedLink = null;
    const timer = setTimeout(async () => {
      try {
        if (new TextEncoder().encode(source).length > 1024 * 1024) throw new Error("HTML must be no larger than 1 MB");
        const next = new HtmlDocument(source, `onlysend-${crypto.randomUUID()}`);
        const safe = await prepareEmailHtml(currentView === "visual" ? next.annotated() : source);
        if (cancelled) return;
        model = currentView === "visual" ? next : null;
        srcdoc = previewDocument(safe);
      } catch (cause) {
        if (!cancelled) { error = errorMessage(cause, "Could not prepare the email preview"); srcdoc = ""; model = null; }
      } finally { if (!cancelled) loading = false; }
    }, 100);
    return () => { cancelled = true; clearTimeout(timer); };
  });

  $effect(() => {
    const locked = disabled;
    frame?.contentDocument?.querySelectorAll<HTMLElement>("[contenteditable]").forEach((element) => {
      element.setAttribute("contenteditable", locked ? "false" : "plaintext-only");
    });
  });

  function connectFrame() {
    cleanup?.();
    const document = frame?.contentDocument;
    const current = model;
    if (!document) return;
    const remove: (() => void)[] = [];
    const listen = (type: string, handler: EventListener) => {
      document.addEventListener(type, handler);
      remove.push(() => document.removeEventListener(type, handler));
    };
    listen("click", (event) => event.preventDefault());
    listen("auxclick", (event) => event.preventDefault());
    listen("submit", (event) => event.preventDefault());
    if (view === "visual" && current) {
      const style = document.createElement("style");
      style.textContent = `[id^="${current.prefix}-"] { cursor:text; } [id^="${current.prefix}-"]:hover { outline:1px dashed #7c7c7c; outline-offset:3px; } [id^="${current.prefix}-"]:focus { outline:2px solid #6366f1; outline-offset:3px; }`;
      document.head.append(style);
      current.regions.forEach((region, index) => {
        if (region.kind !== "text") return;
        const element = document.getElementById(`${current.prefix}-${index}`);
        if (!element) return;
        element.setAttribute("contenteditable", disabled ? "false" : "plaintext-only");
        element.setAttribute("role", "textbox");
        element.setAttribute("aria-label", "Edit email text");
        element.setAttribute("spellcheck", "true");
        element.addEventListener("focus", () => {
          focusedRegion = index;
          selectedLink = region.link ?? null;
          linkUrl = region.link === undefined ? "" : current.regions[region.link].value;
          linkError = "";
        });
        element.addEventListener("input", () => {
          if (disabled || model !== current) return;
          change(current.replace(index, element.textContent ?? ""));
        });
        element.addEventListener("keydown", (event) => {
          if (event.key === "Enter") { event.preventDefault(); element.blur(); }
        });
        element.addEventListener("paste", (event) => {
          event.preventDefault();
          if (disabled || model !== current) return;
          const selection = document.getSelection();
          if (!selection?.rangeCount) return;
          const range = selection.getRangeAt(0);
          if (!element.contains(range.commonAncestorContainer)) return;
          range.deleteContents();
          const text = document.createTextNode(event.clipboardData?.getData("text/plain") ?? "");
          range.insertNode(text);
          range.setStartAfter(text);
          range.collapse(true);
          selection.removeAllRanges();
          selection.addRange(range);
          change(current.replace(index, element.textContent ?? ""));
        });
      });
      listen("keydown", (event) => {
        const key = event as KeyboardEvent;
        if ((key.ctrlKey || key.metaKey) && ["z", "y"].includes(key.key.toLowerCase())) {
          key.preventDefault();
          history(key.shiftKey || key.key.toLowerCase() === "y" ? "redo" : "undo");
        }
      });
      listen("beforeinput", (event) => {
        const input = event as InputEvent;
        if (input.inputType === "historyUndo" || input.inputType === "historyRedo") {
          event.preventDefault();
          history(input.inputType === "historyUndo" ? "undo" : "redo");
        }
      });
      if (restoreFocus !== null) {
        const target = document.getElementById(`${current.prefix}-${restoreFocus}`);
        target?.focus();
        if (target) {
          const selection = document.getSelection();
          selection?.selectAllChildren(target);
          selection?.collapseToEnd();
        }
        restoreFocus = null;
      }
    }
    cleanup = () => remove.forEach((remove) => remove());
  }
  onDestroy(() => cleanup?.());

  function applyLink() {
    if (selectedLink === null || !model || disabled) return;
    try { change(model.replace(selectedLink, linkUrl.trim())); linkError = ""; }
    catch (cause) { linkError = errorMessage(cause, "Invalid link"); }
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-2" aria-invalid={ariaInvalid || undefined}>
  <div class="flex shrink-0 flex-wrap items-center gap-1 py-2" role="toolbar" aria-label="Message editor">
    <Button type="button" variant="ghost" size="icon-sm" aria-label="Undo" disabled={disabled || !undo.length} onclick={() => history("undo")}><Undo2 /></Button>
    <Button type="button" variant="ghost" size="icon-sm" aria-label="Redo" disabled={disabled || !redo.length} onclick={() => history("redo")}><Redo2 /></Button>
    <Button type="button" variant={view === "preview" ? "secondary" : "ghost"} size="sm" aria-pressed={view === "preview"} {disabled} onclick={() => { view = view === "preview" ? "visual" : "preview"; }}><Eye />{view === "preview" ? "Edit" : "Preview"}</Button>
    <ToggleGroup.Root type="single" value={width} onValueChange={(next) => { if (next) width = next; }} variant="outline" size="sm" aria-label="Preview width">
      <ToggleGroup.Item value="desktop" aria-label="Desktop preview"><Monitor /></ToggleGroup.Item>
      <ToggleGroup.Item value="mobile" aria-label="Mobile preview"><Smartphone /></ToggleGroup.Item>
    </ToggleGroup.Root>
    {#if toolbarActions}<div class="ml-auto">{@render toolbarActions()}</div>{/if}
  </div>
  {#if selectedLink !== null && view === "visual"}
    <div class="flex shrink-0 items-center gap-2">
      <Input aria-label="Link destination" bind:value={linkUrl} aria-invalid={!!linkError} {disabled} onkeydown={(event) => { if (event.key === "Enter") { event.preventDefault(); applyLink(); } }} />
      <Button type="button" variant="outline" size="sm" {disabled} onclick={applyLink}>Update link</Button>
    </div>
    {#if linkError}<p class="text-sm text-destructive" role="alert">{linkError}</p>{/if}
  {/if}
  {#if error}
    <div class="flex flex-1 flex-col items-center justify-center gap-3 py-8"><p role="alert" class="text-sm text-destructive">{error}</p><Button type="button" variant="outline" size="sm" onclick={() => retry++}>Try again</Button></div>
  {:else}
    <div class="relative flex min-h-48 flex-1 justify-center overflow-auto rounded-lg border bg-muted/30 p-2" aria-busy={loading}>
      {#if loading}<span class="absolute top-4 rounded-md bg-background px-3 py-1 text-xs shadow-sm" role="status">Preparing preview…</span>{/if}
      {#if !content.trim() && !loading}<p class="absolute top-12 px-4 text-center text-sm text-muted-foreground">Use the HTML menu to insert a design or choose a saved template.</p>{/if}
      {#key srcdoc}
        <iframe bind:this={frame} title={view === "visual" ? "Editable email design" : "Email preview"} {srcdoc} sandbox="allow-same-origin" referrerpolicy="no-referrer" onload={connectFrame} inert={loading || disabled} class={cn("h-full min-h-48 border-0 bg-white", width === "mobile" ? "w-[375px] max-w-full" : "w-full")}></iframe>
      {/key}
    </div>
  {/if}
  <p class="shrink-0 text-xs text-muted-foreground">
    {view === "visual" ? "Click text to edit. Select linked text to update its destination. Layout stays intact." : "Preview only. Email clients may render your design differently."}
    Remote images and styles may load.
  </p>
</div>
