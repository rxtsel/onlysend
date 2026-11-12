<script lang="ts">
  import { Calendar } from "bits-ui";
  import { parseDate, today, getLocalTimeZone } from "@internationalized/date";
  import { ChevronLeft, ChevronRight, ChevronDown } from "@lucide/svelte";
  import * as Popover from "$lib/components/ui/popover";
  import * as Dialog from "$lib/components/ui/dialog";
  import { Input } from "$lib/components/ui/input";
  import * as Field from "$lib/components/ui/field";
  import { Button, buttonVariants } from "$lib/components/ui/button";
  import { emailComposerSchema } from "$lib/schemas/email-composer.schema";

  let { enabled = $bindable(false), local = $bindable(""), error = "", onchange }: {
    enabled?: boolean; local?: string; error?: string; onchange: () => void;
  } = $props();
  let calendarOpen = $state(false);
  let dialogOpen = $state(false);
  let selectedDate = $state("");
  let time = $state("09:00:00");
  let trigger: HTMLElement | null = null;
  let validationError = $state("");
  const zone = getLocalTimeZone();
  const date = $derived(selectedDate ? parseDate(selectedDate) : undefined);
  export function open() {
    const [savedDate, savedTime] = local.split("T");
    selectedDate = savedDate || "";
    time = savedTime || "09:00:00";
    validationError = "";
    calendarOpen = false;
    trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogOpen = true;
  }
  function applySchedule() {
    const selection = `${selectedDate}T${time}`;
    const value = new Date(selection);
    if (!selectedDate || !Number.isFinite(value.getTime())) { validationError = "Choose a date and time"; return; }
    const parsed = emailComposerSchema.shape.scheduledAt.safeParse(value.toISOString());
    if (!parsed.success) { validationError = parsed.error.issues[0].message; return; }
    local = selection;
    enabled = true;
    onchange();
    dialogOpen = false;
  }
</script>

<Dialog.Root bind:open={dialogOpen}>
  {#if error}<Field.Error>{error}</Field.Error>{/if}
  <Dialog.Content class="sm:max-w-md" onCloseAutoFocus={(event) => { event.preventDefault(); trigger?.focus(); }}>
    <Dialog.Header>
      <Dialog.Title>Schedule send</Dialog.Title>
      <Dialog.Description>Choose when Resend should send this message, even if OnlySend is closed.</Dialog.Description>
    </Dialog.Header>
    <div class="flex flex-wrap items-end gap-3">
      <Field.Field class="w-fit">
        <Field.Label for="schedule-date">Date</Field.Label>
        <Popover.Root bind:open={calendarOpen}>
          <Popover.Trigger id="schedule-date">
            {#snippet child({ props })}
              <Button {...props} type="button" variant="outline" class="min-w-44 justify-between font-normal" aria-invalid={!!validationError} aria-describedby="schedule-help">
                {date ? date.toDate(zone).toLocaleDateString("en-US", { dateStyle: "medium" }) : "Select date"}<ChevronDown class="size-4" />
              </Button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content class="w-auto p-3" align="start">
            <Calendar.Root type="single" value={date} minValue={today(zone)} maxValue={today(zone).add({ days: 30 })} onValueChange={(value) => {
              if (value) { selectedDate = value.toString(); calendarOpen = false; validationError = ""; }
            }}>
              {#snippet children({ months, weekdays })}
                <Calendar.Header class="mb-3 flex items-center justify-between">
                  <Calendar.PrevButton class={buttonVariants({ variant: "outline", size: "icon" })}><ChevronLeft class="size-4" /></Calendar.PrevButton>
                  <Calendar.Heading class="text-sm font-medium" />
                  <Calendar.NextButton class={buttonVariants({ variant: "outline", size: "icon" })}><ChevronRight class="size-4" /></Calendar.NextButton>
                </Calendar.Header>
                {#each months as month}
                  <Calendar.Grid class="border-collapse">
                    <Calendar.GridHead><Calendar.GridRow>
                      {#each weekdays as day}<Calendar.HeadCell class="size-9 text-xs font-normal text-muted-foreground">{day}</Calendar.HeadCell>{/each}
                    </Calendar.GridRow></Calendar.GridHead>
                    <Calendar.GridBody>
                      {#each month.weeks as week}
                        <Calendar.GridRow>
                          {#each week as day}
                            <Calendar.Cell date={day} month={month.value} class="p-0">
                              <Calendar.Day class="flex size-9 cursor-default items-center justify-center rounded-md text-sm hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring data-selected:bg-primary data-selected:text-primary-foreground data-disabled:pointer-events-none data-disabled:opacity-30 data-outside-month:opacity-30" />
                            </Calendar.Cell>
                          {/each}
                        </Calendar.GridRow>
                      {/each}
                    </Calendar.GridBody>
                  </Calendar.Grid>
                {/each}
              {/snippet}
            </Calendar.Root>
          </Popover.Content>
        </Popover.Root>
      </Field.Field>
      <Field.Field class="w-32">
        <Field.Label for="schedule-time">Time</Field.Label>
        <Input
          type="time"
          id="schedule-time"
          step="1"
          bind:value={time}
          aria-invalid={!!validationError}
          aria-describedby="schedule-help"
          oninput={() => { validationError = ""; }}
          class="appearance-none bg-background [&::-webkit-calendar-picker-indicator]:hidden [&::-webkit-calendar-picker-indicator]:appearance-none"
        />
      </Field.Field>
    </div>
    <p id="schedule-help" class="text-xs text-muted-foreground">{zone} · 24-hour time · Up to 30 days ahead.</p>
    {#if validationError}<Field.Error>{validationError}</Field.Error>{/if}
    <Dialog.Footer>
      {#if enabled}
        <Button type="button" variant="ghost" onclick={() => { enabled = false; onchange(); dialogOpen = false; }}>Send now instead</Button>
      {/if}
      <Button type="button" variant="outline" onclick={() => { dialogOpen = false; }}>Cancel</Button>
      <Button type="button" onclick={applySchedule}>Save schedule</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
