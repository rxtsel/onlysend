// @vitest-environment jsdom
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { flushSync, mount, unmount } from "svelte";
import { goto, beforeNavigate } from "$app/navigation";
import { invoke } from "@tauri-apps/api/core";
import Composer from "./__tests__/composer-harness.svelte";
import { prepareReply } from "$lib/shared/reply";
import { listFromEmails } from "$lib/shared/from-emails";
import { registerDraftGuard } from "$lib/features/auth/draft-navigation";

const { pageState } = vi.hoisted(() => ({ pageState: { url: new URL("https://app.test/mail/account-a/composer") } }));
vi.mock("$app/state", () => ({ page: pageState }));
vi.mock("$app/navigation", () => ({ goto: vi.fn(), beforeNavigate: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("$lib/shared/reply", () => ({ prepareReply: vi.fn() }));
vi.mock("$lib/shared/from-emails", async (original) => ({ ...await original<typeof import("$lib/shared/from-emails")>(), listFromEmails: vi.fn() }));
vi.mock("$lib/features/auth/draft-navigation", () => ({ registerDraftGuard: vi.fn(() => vi.fn()), allowAccountNavigation: vi.fn(() => true) }));
vi.mock("@/lib/components/editor", async () => ({ Editor: (await import("./__tests__/composer-editor-probe.svelte")).default }));
vi.mock("svelte-sonner", () => ({ toast: { loading: vi.fn(), dismiss: vi.fn(), success: vi.fn(), error: vi.fn() } }));

const emailId = "b3e2b2b6-3f0e-4c8e-9ad3-2f43a1e2c7f1";
const context = { to: ["reply@example.com"], subject: "Re: Invoice", fromCandidates: ["ME@example.com"] };
let target: HTMLElement;
let instance: ReturnType<typeof mount> | null;
beforeEach(() => {
  pageState.url = new URL(`https://app.test/mail/account-a/composer?reply=${emailId}`);
  vi.mocked(listFromEmails).mockReset().mockResolvedValue([
    { id: "default", label: "Default", address: "default@example.com", isDefault: true },
    { id: "me", label: "Me", address: "me@example.com", isDefault: false },
  ]);
  vi.mocked(prepareReply).mockReset().mockResolvedValue(context);
  vi.mocked(invoke).mockReset().mockResolvedValue(undefined);
  vi.mocked(registerDraftGuard).mockClear();
  vi.mocked(goto).mockClear();
  vi.mocked(beforeNavigate).mockClear();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(async () => {
  if (instance) await unmount(instance);
  instance = null;
  document.body.replaceChildren();
  vi.restoreAllMocks();
});
function open() { instance = mount(Composer, { target }); flushSync(); }
function input(selector: string) { return target.querySelector<HTMLInputElement>(selector)!; }
async function ready() {
  await vi.waitFor(() => expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("true"));
}
async function confirmSend() {
  await vi.waitFor(() => expect(document.querySelector('[data-slot="alert-dialog-action"]')).not.toBeNull());
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-action"]')!.click();
}
function submit() {
  flushSync(() => target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
}
function body(value: string) {
  const editor = target.querySelector<HTMLTextAreaElement>("textarea")!;
  flushSync(() => { editor.value = value; editor.dispatchEvent(new Event("input", { bubbles: true })); });
}

function clickButton(label: string) {
  flushSync(() => Array.from(document.querySelectorAll("button")).find((button) => button.textContent?.trim() === label)!.click());
}
function setScheduleTime(value: string) {
  flushSync(() => {
    const time = document.querySelector<HTMLInputElement>("#schedule-time")!;
    time.value = value;
    time.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

test("calendar and time picker schedule in local time and persist the selection", async () => {
  open();
  await ready();
  body("<p>Scheduled reply</p>");
  expect(document.querySelector("#schedule-date")).toBeNull();
  clickButton("Schedule send");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).not.toBeNull());
  clickButton("Save schedule");
  expect(document.querySelector('[role="dialog"]')?.textContent).toContain("Choose a date and time");
  expect(invoke).not.toHaveBeenCalledWith("send_email", expect.anything());
  flushSync(() => document.querySelector<HTMLButtonElement>("#schedule-date")!.click());
  const tomorrow = new Date();
  tomorrow.setDate(tomorrow.getDate() + 1);
  const date = `${tomorrow.getFullYear()}-${String(tomorrow.getMonth() + 1).padStart(2, "0")}-${String(tomorrow.getDate()).padStart(2, "0")}`;
  await vi.waitFor(() => expect(document.querySelector(`[data-calendar-day][data-value="${date}"]`)).not.toBeNull());
  flushSync(() => document.querySelector<HTMLElement>(`[data-calendar-day][data-value="${date}"]`)!.click());
  setScheduleTime("14:30:45");
  expect(document.querySelector<HTMLInputElement>("#schedule-time")?.value).toBe("14:30:45");
  clickButton("Save schedule");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  await vi.mocked(registerDraftGuard).mock.calls[0][2]!();
  expect(invoke).toHaveBeenCalledWith("save_draft", expect.objectContaining({ data: expect.objectContaining({ isScheduleEnabled: true, scheduledLocal: `${date}T14:30:45` }) }));
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Schedule this message?"));
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Resend dashboard");
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.objectContaining({ data: expect.objectContaining({ scheduledAt: new Date(`${date}T14:30:45`).toISOString() }) })));
});

test("canceling the schedule dialog leaves immediate sending unchanged", async () => {
  open();
  await ready();
  body("<p>Send now</p>");
  clickButton("Schedule send");
  await vi.waitFor(() => expect(document.querySelector("#schedule-time")).not.toBeNull());
  setScheduleTime("17:00:00");
  clickButton("Cancel");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  expect(target.querySelector("#schedule-date")).toBeNull();
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.objectContaining({ data: expect.not.objectContaining({ scheduledAt: expect.anything() }) })));
});

test("reply reuses the composer, locks its subject and chooses the matching sender", async () => {
  open();
  expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("false");
  await ready();
  expect(target.querySelector("form")?.textContent).not.toContain("Schedule send");
  expect(prepareReply).toHaveBeenCalledExactlyOnceWith("account-a", emailId);
  expect(input("#subject").value).toBe("Re: Invoice");
  expect(input("#subject").readOnly).toBe(true);
  expect(input('input[type="hidden"][name="to"]').value).toBe("reply@example.com");
  expect(target.querySelector("#from")?.textContent).toContain("Me <me@example.com>");
  expect(document.activeElement).toBe(target.querySelector("textarea"));
  expect(target.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe("");
  const guard = vi.mocked(registerDraftGuard).mock.calls[0][1];
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  expect(guard()).toBe(true); // Prefilled fields are not unsaved edits.
  expect(confirm).not.toHaveBeenCalled();
  body("<p>My reply</p>");
  expect(guard()).toBe(false);
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", { accountId: "account-a", data: {
    replyEmailId: emailId, subject: "Re: Invoice", to: ["reply@example.com"], from: "Me <me@example.com>", content: "<p>My reply</p>", attachments: [], cc: [], bcc: [],
  } }));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/mail/account-a/sent"));
});

test.each(["Please see the attached document.", "Te envío el documento adjunto."])("attachment reminder is English, preserves edits on cancel and allows sending: %s", async (text) => {
  vi.mocked(invoke).mockImplementation(async (command) => command === "mentions_attachment" ? true : undefined);
  open();
  await ready();
  body(`<p>${text}</p>`);
  expect(invoke).not.toHaveBeenCalledWith("mentions_attachment", expect.anything());
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Missing an attachment?"));
  expect(invoke).toHaveBeenCalledWith("mentions_attachment", { subject: "Re: Invoice", body: text });
  const title = document.querySelector('[data-slot="alert-dialog-title"]')!;
  expect(title.firstElementChild?.tagName.toLowerCase()).toBe("svg");
  expect(title.firstElementChild?.getAttribute("aria-hidden")).toBe("true");
  expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Your message appears to mention an attachment, but no files have been added. Send it without attachments?");
  expect(document.querySelector('[data-slot="alert-dialog-cancel"]')?.textContent).toBe("Keep editing");
  expect(document.querySelector('[data-slot="alert-dialog-action"]')?.textContent?.trim()).toBe("Send anyway");
  expect(invoke).not.toHaveBeenCalledWith("send_email", expect.anything());
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  expect(target.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe(`<p>${text}</p>`);
  expect(goto).not.toHaveBeenCalled();
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.objectContaining({ data: expect.objectContaining({ content: `<p>${text}</p>`, attachments: [] }) })));
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "send_email")).toHaveLength(1);
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
});

test("mentions in the subject also warn, but a selected attachment uses normal confirmation", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => command === "mentions_attachment" ? true : undefined);
  vi.mocked(prepareReply).mockResolvedValueOnce({ ...context, subject: "Re: Attached invoice" });
  open();
  await ready();
  body("<p>Here you go.</p>");
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Missing an attachment?"));
  document.querySelector<HTMLButtonElement>('[data-slot="alert-dialog-cancel"]')!.click();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')).toBeNull());
  const file = new File(["invoice"], "invoice.txt", { type: "text/plain" });
  Object.defineProperty(file, "arrayBuffer", { value: async () => new TextEncoder().encode("invoice").buffer });
  const picker = target.querySelector<HTMLInputElement>('[aria-label="Attach files"]')!;
  Object.defineProperty(picker, "files", { configurable: true, value: [file] });
  flushSync(() => picker.dispatchEvent(new Event("change", { bubbles: true })));
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Send this message?"));
  expect(document.querySelector('[role="alertdialog"]')?.textContent).not.toContain("Missing an attachment?");
  expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "mentions_attachment")).toHaveLength(1);
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.objectContaining({ data: expect.objectContaining({ attachments: [{ filename: "invoice.txt", content: "aW52b2ljZQ==", mimeType: "text/plain" }] }) })));
});

test("attachment keywords in a link URL do not warn about visible message text", async () => {
  open();
  await ready();
  body('<p><a href="https://example.com/attachments">View details</a></p>');
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Send this message?"));
  expect(document.querySelector('[role="alertdialog"]')?.textContent).not.toContain("Missing an attachment?");
  expect(invoke).toHaveBeenCalledWith("mentions_attachment", { subject: "Re: Invoice", body: "View details" });
  expect(document.querySelector('[data-slot="alert-dialog-title"] svg')).toBeNull();
});

test("attachment check failures keep the message editable and allow retry", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Check unavailable"));
  open();
  await ready();
  body("<p>Please see the attachment.</p>");
  submit();
  await vi.waitFor(() => expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("true"));
  expect(document.querySelector('[role="alertdialog"]')).toBeNull();
  expect(invoke).not.toHaveBeenCalledWith("send_email", expect.anything());
  expect(target.querySelector<HTMLTextAreaElement>("textarea")?.disabled).toBe(false);
  vi.mocked(invoke).mockResolvedValue(true);
  submit();
  await vi.waitFor(() => expect(document.querySelector('[role="alertdialog"]')?.textContent).toContain("Missing an attachment?"));
});

test("ordinary messages keep an editable subject and the default sender", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  expect(prepareReply).not.toHaveBeenCalled();
  expect(input("#subject").readOnly).toBe(false);
  expect(input("#subject").value).toBe("");
  expect(target.querySelector('[name="replyTo"]')).toBeNull();
  expect(target.querySelector('[name="messageId"]')).toBeNull();
  expect(target.textContent).not.toContain("Add message ID");
  expect(target.querySelector("#from")?.textContent).toContain("Default <default@example.com>");
});

test.each([false, true])("composer fields follow From, To, Subject without nested cards (reply: %s)", async (isReply) => {
  if (!isReply) pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  const labels = () => Array.from(target.querySelectorAll("form label")).map((label) => label.textContent?.trim());
  expect(labels()).toEqual(["From", "To", "Subject", "Content"]);
  expect(target.querySelector("form")?.classList.contains("overflow-hidden")).toBe(false);
  const group = target.querySelector('form [data-slot="field-group"]')!;
  expect(group.classList.contains("border")).toBe(false);
  expect(group.classList.contains("rounded-xl")).toBe(false);
  const from = target.querySelector("#from")!;
  expect(from.classList.contains("border")).toBe(true);
  expect(from.classList.contains("w-[180px]")).toBe(true);
  expect(from.closest('[data-slot="field"]')?.getAttribute("data-orientation")).toBe("vertical");
  const toggleGroup = target.querySelector('[data-slot="toggle-group"]')!;
  expect(toggleGroup.getAttribute("data-variant")).toBe("outline");
  expect(toggleGroup.classList.contains("*:flex-none")).toBe(true);
  expect(from.closest('[data-slot="field"]')?.parentElement?.contains(toggleGroup)).toBe(true);
  expect(input("#subject").classList.contains("border")).toBe(true);
  expect(input("#subject").classList.contains("h-9")).toBe(true);
  expect(input("#subject").classList.contains("h-8")).toBe(false);
  const to = input("#to");
  expect(to.parentElement?.classList.contains("border")).toBe(true);
  const toggles = Array.from(target.querySelectorAll<HTMLButtonElement>('[data-slot="toggle-group-item"]'));
  expect(toggles.map((button) => button.textContent)).toEqual(["Reply to", "Cc", "Bcc"]);
  flushSync(() => { toggles.forEach((button) => button.click()); });
  expect(labels()).toEqual(["From", "To", "Reply to", "Cc", "Bcc", "Subject", "Content"]);
  expect(input("#cc").parentElement?.classList.contains("border")).toBe(true);
  expect(input("#bcc").parentElement?.classList.contains("border")).toBe(true);
  expect(to.closest('[data-slot="field"]')?.parentElement).toBe(input("#replyTo").closest('[data-slot="field"]')?.parentElement);
  const copyRow = input("#cc").closest('[data-slot="field"]')?.parentElement;
  expect(copyRow).toBe(input("#bcc").closest('[data-slot="field"]')?.parentElement);
  expect(copyRow?.classList.contains("sm:grid-cols-2")).toBe(true);
  expect(copyRow).not.toBe(to.closest('[data-slot="field"]')?.parentElement);
});

test.each([
  [false, false, false], [true, false, false], [false, true, false], [false, false, true],
  [true, true, false], [true, false, true], [false, true, true], [true, true, true],
])("optional layout: Reply to=%s, Cc=%s, Bcc=%s", async (replyTo, cc, bcc) => {
  open();
  await ready();
  const buttons = Array.from(target.querySelectorAll<HTMLButtonElement>('[data-slot="toggle-group-item"]'));
  flushSync(() => { [replyTo, cc, bcc].forEach((enabled, index) => { if (enabled) buttons[index].click(); }); });
  expect(!!target.querySelector("#replyTo")).toBe(replyTo);
  expect(!!target.querySelector("#cc")).toBe(cc);
  expect(!!target.querySelector("#bcc")).toBe(bcc);
  const toRow = input("#to").closest('[data-slot="field"]')!.parentElement!;
  expect(toRow.querySelector("#replyTo") !== null).toBe(replyTo);
  for (const id of ["cc", "bcc"]) expect(toRow.querySelector(`#${id}`)).toBeNull();
  const copyInput = target.querySelector("#cc, #bcc");
  if (copyInput) {
    const copyRow = copyInput.closest('[data-slot="field"]')!.parentElement!;
    expect(copyRow.classList.contains("grid")).toBe(true);
    expect(copyRow.classList.contains("sm:grid-cols-2")).toBe(cc && bcc);
  }
});

test("Reply to is saved in the outgoing payload and autosaved before departure", async () => {
  open();
  await ready();
  const toggle = target.querySelector<HTMLButtonElement>('[data-slot="toggle-group-item"]')!;
  flushSync(() => toggle.click());
  const guard = vi.mocked(registerDraftGuard).mock.calls[0][1];
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  expect(guard()).toBe(true); // Visibility alone does not start a new draft.
  flushSync(() => {
    input("#replyTo").value = "  replies@example.com  ";
    input("#replyTo").dispatchEvent(new Event("input", { bubbles: true }));
  });
  expect(guard()).toBe(false);
  expect(confirm).not.toHaveBeenCalled();
  body("<p>Reply body</p>");
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", {
    accountId: "account-a", data: expect.objectContaining({ replyTo: "replies@example.com", replyEmailId: emailId }),
  }));
  await vi.waitFor(() => expect(goto).toHaveBeenCalled());
  expect(target.querySelector("#replyTo")).toBeNull();
  expect(guard()).toBe(true);
});

test("invalid Reply to blocks sending; hiding it excludes the preserved value", async () => {
  open();
  await ready();
  const toggle = target.querySelector<HTMLButtonElement>('[data-slot="toggle-group-item"]')!;
  flushSync(() => toggle.click());
  flushSync(() => {
    input("#replyTo").value = "not-an-email";
    input("#replyTo").dispatchEvent(new Event("input", { bubbles: true }));
  });
  body("<p>Reply body</p>");
  const submit = () => target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  flushSync(submit);
  expect(invoke).not.toHaveBeenCalled();
  expect(input("#replyTo").getAttribute("aria-invalid")).toBe("true");
  flushSync(() => { toggle.click(); });
  flushSync(() => { toggle.click(); });
  expect(input("#replyTo").value).toBe("not-an-email");
  flushSync(() => { toggle.click(); });
  flushSync(submit);
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.anything()));
  const outgoing = vi.mocked(invoke).mock.calls.find(([command]) => command === "send_email")![1] as { data: Record<string, unknown> };
  expect(outgoing.data).not.toHaveProperty("replyTo");
});

test("an enabled but empty Reply to remains optional", async () => {
  open();
  await ready();
  flushSync(() => target.querySelector<HTMLButtonElement>('[data-slot="toggle-group-item"]')!.click());
  body("<p>Reply body</p>");
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.anything()));
  const outgoing = vi.mocked(invoke).mock.calls.find(([command]) => command === "send_email")![1] as { data: Record<string, unknown> };
  expect(outgoing.data).not.toHaveProperty("replyTo");
});

test("Tab from Subject goes to the writer; Shift-Tab keeps native backwards navigation", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  const subject = input("#subject");
  subject.focus();
  const backwards = new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true, cancelable: true });
  subject.dispatchEvent(backwards);
  expect(backwards.defaultPrevented).toBe(false);
  const forwards = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
  subject.dispatchEvent(forwards);
  expect(forwards.defaultPrevented).toBe(true);
  expect(document.activeElement).toBe(target.querySelector("textarea"));
});

test("composer fields still show validation errors and do not submit invalid drafts", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  flushSync(() => target.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })));
  expect(input("#to").getAttribute("aria-invalid")).toBe("true");
  expect(input("#subject").getAttribute("aria-invalid")).toBe("true");
  expect(target.querySelectorAll('[data-slot="field-error"]').length).toBeGreaterThanOrEqual(2);
  expect(invoke).not.toHaveBeenCalled();
});

test("missing reply details block sending and remain retryable without losing the source", async () => {
  vi.mocked(prepareReply).mockRejectedValueOnce("Reply metadata is unavailable offline");
  open();
  await vi.waitFor(() => expect(target.querySelector('[role="alert"]')?.textContent).toContain("unavailable offline"));
  expect(target.querySelector("form")).toBeNull();
  expect(target.querySelector("[data-composer-ready]")?.textContent).toBe("false");
  expect(invoke).not.toHaveBeenCalled();
  flushSync(() => target.querySelector<HTMLButtonElement>("button")!.click());
  await ready();
  expect(input("#subject").readOnly).toBe(true);
});

test("a reply cannot initialize a composer after it has been closed", async () => {
  let resolve!: (value: typeof context) => void;
  vi.mocked(prepareReply).mockReturnValueOnce(new Promise((done) => { resolve = done; }));
  open();
  await unmount(instance!);
  instance = null;
  resolve(context);
  await Promise.resolve();
  flushSync();
  expect(target.querySelector("form")).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});

test("opening or internal editor initialization does not save until the user types", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  flushSync(() => {
    const editor = target.querySelector<HTMLTextAreaElement>("textarea")!;
    editor.value = "<p></p>";
    editor.dispatchEvent(new Event("input")); // Internal update, not a bubbled user edit.
    target.querySelector<HTMLButtonElement>('[data-slot="toggle-group-item"]')!.click();
  });
  await new Promise((resolve) => setTimeout(resolve, 600));
  expect(invoke).not.toHaveBeenCalled();
  expect(target.querySelector("[data-draft-status]")?.textContent).toBe("");
  body("<p>First typed text</p>");
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("save_draft", expect.anything()), { timeout: 1500 });
});

test("pasting recipients starts autosave without a native input event", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  const paste = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(paste, "clipboardData", { value: { getData: () => "pasted@example.com" } });
  flushSync(() => input("#to").dispatchEvent(paste));
  expect(paste.defaultPrevented).toBe(true);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("save_draft", expect.objectContaining({ data: expect.objectContaining({ to: ["pasted@example.com"] }) })), { timeout: 1500 });
});

test("autosaves the latest edit after debounce and keeps incomplete recipient input on blur", async () => {
  pageState.url = new URL("https://app.test/mail/account-a/composer");
  open();
  await ready();
  expect(invoke).not.toHaveBeenCalled(); // Opening a blank composer creates no draft.
  body("<p>First</p>");
  body("<p>Latest</p>");
  flushSync(() => {
    input("#to").value = "unfinished@";
    input("#to").dispatchEvent(new Event("input", { bubbles: true }));
    input("#to").dispatchEvent(new Event("blur"));
  });
  expect(input("#to").value).toBe("unfinished@");
  expect(invoke).not.toHaveBeenCalled();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1), { timeout: 1500 });
  expect(invoke).toHaveBeenCalledWith("save_draft", {
    accountId: "account-a", draftId: expect.any(String),
    data: expect.objectContaining({ content: "<p>Latest</p>", pendingTo: "unfinished@", attachments: [] }),
  });
  expect(vi.mocked(registerDraftGuard).mock.calls[0][1]()).toBe(true);
});

test("navigation flushes immediately and stays in the composer if saving fails", async () => {
  open();
  await ready();
  body("<p>Do not lose this</p>");
  const cancel = vi.fn();
  const navigate = vi.mocked(beforeNavigate).mock.calls[0][0];
  vi.mocked(invoke).mockRejectedValueOnce(new Error("Disk full"));
  navigate({ cancel, to: { url: new URL("https://app.test/mail/account-a/inbox") }, willUnload: false } as never);
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("save_draft", expect.anything()));
  await vi.waitFor(() => expect(target.querySelector("[data-draft-status]")?.textContent).toBe("Not saved"));
  expect(cancel).toHaveBeenCalledOnce();
  expect(goto).not.toHaveBeenCalled();
  navigate({ cancel, to: { url: new URL("https://app.test/mail/account-a/inbox") }, willUnload: false } as never);
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(new URL("https://app.test/mail/account-a/inbox")));
});

test.each([-1, 1])("Back/Forward resumes its original history delta (%s) after rollback and save", async (delta) => {
  open();
  await ready();
  body("<p>Save before history traversal</p>");
  let finish!: () => void;
  vi.mocked(invoke).mockReturnValueOnce(new Promise<void>((resolve) => { finish = resolve; }));
  const go = vi.spyOn(history, "go").mockImplementation(() => {});
  const cancel = vi.fn();
  const navigate = vi.mocked(beforeNavigate).mock.calls[0][0];
  navigate({ cancel, to: { url: new URL("https://app.test/mail/account-a/inbox") }, willUnload: false, type: "popstate", delta } as never);
  expect(cancel).toHaveBeenCalledOnce();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("save_draft", expect.anything()));
  finish();
  await Promise.resolve();
  expect(go).not.toHaveBeenCalled();
  window.dispatchEvent(new PopStateEvent("popstate")); // SvelteKit restored the old entry.
  await vi.waitFor(() => expect(go).toHaveBeenCalledExactlyOnceWith(delta));
  expect(goto).not.toHaveBeenCalled();
});

test("failed discard leaves a skipped queued edit dirty until it is actually saved", async () => {
  open();
  await ready();
  const guard = vi.mocked(registerDraftGuard).mock.calls[0][1];
  const flush = vi.mocked(registerDraftGuard).mock.calls[0][2]!;
  let finish!: () => void;
  const firstWrite = new Promise<void>((resolve) => { finish = resolve; });
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "save_draft") await firstWrite;
    if (command === "delete_draft") throw new Error("Delete failed");
  });
  body("<p>A</p>");
  const first = flush();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("save_draft", expect.anything()));
  body("<p>B</p>");
  const queued = flush();
  flushSync(() => target.querySelector<HTMLButtonElement>("[data-discard-draft]")!.click());
  finish();
  await Promise.all([first, queued]);
  await vi.waitFor(() => expect(target.querySelector("[data-composer-sending]")?.textContent).toBe("false"));
  expect(guard()).toBe(false);
  expect(goto).not.toHaveBeenCalled();
  await flush();
  expect(guard()).toBe(true);
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ data: expect.objectContaining({ content: "<p>B</p>" }) }));
});

test("reopens a persisted draft with all fields and attachments without creating a second draft", async () => {
  pageState.url = new URL(`https://app.test/mail/account-a/composer?draft=${emailId}`);
  vi.mocked(invoke).mockResolvedValueOnce({
    from: "Me <me@example.com>", to: ["to@example.com"], cc: ["cc@example.com"], bcc: ["bcc@example.com"],
    pendingTo: "unfinished@", pendingCc: "", pendingBcc: "", subject: "Saved subject", content: "<p>Saved body</p>",
    replyEmailId: null, replyTo: "reply@example.com", isReplyToEnabled: true, isCcEnabled: true, isBccEnabled: true,
    attachments: [{ filename: "note.txt", content: "aGVsbG8=", mimeType: "text/plain" }],
  });
  open();
  await ready();
  expect(invoke).toHaveBeenCalledExactlyOnceWith("get_draft", { accountId: "account-a", draftId: emailId });
  expect(input("#subject").value).toBe("Saved subject");
  expect(target.querySelector<HTMLTextAreaElement>("textarea")?.value).toBe("<p>Saved body</p>");
  expect(input("#to").value).toBe("unfinished@");
  expect(input("#replyTo").value).toBe("reply@example.com");
  expect(input('[name="cc"][type="hidden"]').value).toBe("cc@example.com");
  expect(input('[name="bcc"][type="hidden"]').value).toBe("bcc@example.com");
  expect(target.textContent).toContain("note.txt");
  expect(vi.mocked(registerDraftGuard).mock.calls[0][1]()).toBe(true);
  body("<p>Only text changed</p>");
  await vi.mocked(registerDraftGuard).mock.calls[0][2]!();
  expect(invoke).toHaveBeenLastCalledWith("save_draft", expect.objectContaining({ draftId: emailId, data: expect.not.objectContaining({ attachments: expect.anything() }) }));
});

test("restores a scheduled draft and rejects a date that expired while it was closed", async () => {
  pageState.url = new URL(`https://app.test/mail/account-a/composer?draft=${emailId}`);
  vi.mocked(invoke).mockResolvedValueOnce({
    from: "Me <me@example.com>", to: ["to@example.com"], cc: [], bcc: [],
    pendingTo: "", pendingCc: "", pendingBcc: "", subject: "Scheduled draft", content: "<p>Saved body</p>",
    replyEmailId: null, replyTo: "", isReplyToEnabled: false, isCcEnabled: false, isBccEnabled: false,
    isScheduleEnabled: true, scheduledLocal: "2020-01-01T14:30", attachments: [],
  });
  open();
  await ready();
  clickButton("Scheduled");
  await vi.waitFor(() => expect(document.querySelector("#schedule-date")?.textContent).toContain("Jan 1, 2020"));
  expect(document.querySelector<HTMLInputElement>("#schedule-time")?.value).toBe("14:30");
  setScheduleTime("18:00:00");
  clickButton("Cancel");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  clickButton("Scheduled");
  await vi.waitFor(() => expect(document.querySelector<HTMLInputElement>("#schedule-time")?.value).toBe("14:30"));
  clickButton("Cancel");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  submit();
  expect(target.textContent).toContain("Choose a future time within the next 30 days");
  expect(invoke).not.toHaveBeenCalledWith("send_email", expect.anything());
  clickButton("Scheduled");
  await vi.waitFor(() => expect(document.querySelector("#schedule-time")).not.toBeNull());
  clickButton("Send now instead");
  await vi.waitFor(() => expect(document.querySelector('[role="dialog"]')).toBeNull());
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.objectContaining({ data: expect.not.objectContaining({ scheduledAt: expect.anything() }) })));
});

test("discard removes the draft and never saves it again on unmount", async () => {
  open();
  await ready();
  body("<p>Discard me</p>");
  flushSync(() => target.querySelector<HTMLButtonElement>("[data-discard-draft]")!.click());
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("delete_draft", { accountId: "account-a", draftId: expect.any(String) }));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/mail/account-a/drafts"));
  await unmount(instance!);
  instance = null;
  expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual(["delete_draft"]);
});

test("failed sending preserves the saved draft and successful sending removes it", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "send_email") throw new Error("Offline");
  });
  open();
  await ready();
  body("<p>Keep me</p>");
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("send_email", expect.anything()));
  await vi.waitFor(() => expect(target.querySelector("[data-composer-sending]")?.textContent).toBe("false"));
  expect(invoke).not.toHaveBeenCalledWith("delete_draft", expect.anything());
  expect(goto).not.toHaveBeenCalled();
  vi.mocked(invoke).mockResolvedValue(undefined);
  submit();
  await confirmSend();
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("delete_draft", expect.anything()));
  await vi.waitFor(() => expect(goto).toHaveBeenCalledWith("/mail/account-a/sent"));
});
