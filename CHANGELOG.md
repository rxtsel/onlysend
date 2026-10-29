# Changelog

## v0.1.8 - 2026-10-29

### Features
- Persist account-scoped local email deletion (2c2adc8)
- Add delete controls with five-second undo (354cd8a)
- Confirm email deletion with a destructive alert dialog (85c7f44)
- Add dropdown and context actions to email rows (741088c)


### Fixes
- Use consistent sender avatars in list and preview (6a80ee2)
- Align subject line height with toolbar actions (20a400a)
- Match preview skeleton spacing to rendered messages (8bb51b2)
- Preserve authored email styles without overrides (bf7d333)
- Reuse sent cache without remote thread metadata (d22c693)
- Isolate refresh queues by account (439e122)


### Maintenance
- Use default toast colors (46f52e3)
- List local email deletion with undo (cb213f7)


### Performance
- Batch archive reads and index thread relationships (61bd942)
- Remove duplicate toolbar updates and link registration (66f5929)
- Lazy load attachment controls (a8dae04)
- Resize app logo for its rendered dimensions (bcc22b8)

## v0.1.7 - 2026-10-28

### Features
- Add native attachment actions and composer previews (6c65507)
- Reuse session downloads and isolate edits (1f703a6)
- Add threaded message replies (b40477d)
- Group messages into secure conversations (72ffec0)


### Fixes
- Launch external links through native browser service (4d70b09)
- Route sandboxed email links to the native browser (ef37b65)
- Preserve email styling and recover original HTML (71006e2)
- Strip document title text from rendered email bodies (1b8e15a)
- Preserve original reply recipients (454b8e8)


### Maintenance
- Cover preview reference cleanup (83ea366)
- Add jsdom for component tests (31100b5)
- Document session cache behavior (462d45a)
- Install node for release builds (c9960af)
- Cover reply composer and attachment layout (46467e9)
- Document reply workflow and checks (9664851)
- Add mailparse for reply headers (69fff94)
- Add horizontal footer margin (697d5f4)
- Group fields into bordered sections (d6d7428)
- Simplify content and add centered logo and badges (2318660)

## v0.1.6 - 2026-10-28

### Features
- Refresh app icons and creator attribution (8f98383)
- Add cloudflare web analytics (603d4ba)


### Fixes
- Allow the startup permission check (a549b07)


### Maintenance
- Align tauri rust and javascript versions (b13ccfc)
- Simplify the app description (168df4f)
- Remove the website section (8c81b03)
- Migrate from pnpm to bun (1350b33)
- Update the full preview and add a cover image (c72cb19)


### Performance
- Load the cdn demo only after play (68dc71e)

## v0.1.5 - 2026-10-28

### Features
- Show delivery failure badges in sent list (0760f63)
- Reveal failure badge label on hover (74ff798)
- Move settings entry into account menu (5bc3643)


### Fixes
- Tolerate suppressed and unknown delivery events (56ab3dc)
- Focus message body on tab from subject (9c207d0)


### Maintenance
- Expand bug report template (ed7f99e)
- Increase dialog height (317a5a4)

## v0.1.4 - 2026-10-27

### Features
- Add static website (9363378)
- Add in-app bug report and about shortcuts (f429e7a)
- Poll inbox in rust and send native notifications (658009f)
- Add native list query with previews and read state (c1858c0)
- Add native-backed list and reading pane (ec1dbeb)
- Collapse navigation to an icon rail (319a527)
- Show toggle shortcut in trigger tooltip (b100fcc)
- Open dialog from sidebar footer action (bd594d6)
- Add theme and avatar preferences (0b10138)


### Fixes
- Add explicit tsconfig path (9ade076)
- Handle resend postgres timestamps and calendar days (a48adb1)
- Ignore toggle shortcut handled by editors (fcbf49e)
- Keep primary button text on hover and active (902160c)
- Prevent horizontal overflow in navigation (7f6e9ce)
- Match toaster theme to appearance (504ca1a)


### Maintenance
- Remove assets (62b2cfc)
- Add issue report templates (8c9b5de)
- Compact account switcher and footer actions (070da53)
- Add tauri notification plugin (8628041)
- Document background inbox checks (26dc7ee)
- Document native search and reading pane (2873456)
- Use field and item primitives in panels (8b8f8d2)
- Document theme and avatar settings (18a9876)

## v0.1.3 - 2026-10-06

### Maintenance
- Update tauri cli for appimage mesa compatibility (4d182e7)

## v0.1.2 - 2026-10-06

### Features
- Sign in with resend via oauth and deep links (fa6c034)
- List, create and verify resend sending domains (597a213)
- Add shadcn card, empty and item components (a4412ce)
- Rebuild setup wizard with connect, domains and email steps (371a315)
- Add close button to toasts (a893e47)
- Add shadcn input-group and textarea components (38c9f12)
- Add live email preview and domain-locked address input (4c32f08)
- Persist setup completion and resume wizard state (532d089)
- Show blobatar identicon avatars keyed by domain (0dc4286)
- Rework sender options with domain-locked input and defaults (89514ab)
- Show blobatar avatars in saved sender list (3f534fc)
- Expose record groups, capabilities and domain deletion (624d67c)
- Fetch inbound emails and persist read state (d0e4355)
- Add dns records card and receiving setup flow (03b3290)
- Add inbox views with unified sidebar list and filters (fce02b4)
- Show sender avatars in wizard email list (0b2c328)
- Enforce single credential by replacing key or oauth grant (001d806)
- Reset setup state when disconnecting resend (1282b13)
- Restore session from surviving local data (64c4ebb)
- Ask for confirmation before disconnecting (ff942b9)
- Add actionable auth error toasts with real messages (16e82ec)
- Add multi-account credential storage (96d9c2c)
- Add account switcher to nav-user dropdown (07fade7)
- Scope mail routes and state to explicit accounts (4f8fb49)
- Add account-scoped multi-domain setup and settings (8da499c)
- Add sqlite infrastructure and atomic migrations (b0be46f)
- Migrate preferences and read markers to sqlite (84281f8)
- Archive mail locally with indexed queries and sync (e788920)
- Read local archive before remote refresh (a2994cc)
- Show local copy status in a global footer (a190020)
- Add signed updates with footer and about controls (c22af5e)


### Fixes
- Open external links via tauri opener (1ca4798)
- Remove duplicate domaincapabilities interface (dccbbdf)
- Serialize concurrent token refreshes with single-flight lock (5f56fbc)
- Save_oauth always creates new account, rotation uses update_active_oauth (48981f1)
- Reload email lists when active account changes (52201a4)
- Isolate account credentials and remove connections on logout (23d4722)
- Scope sender identities and read markers by account (919ba83)
- Reuse responses for safe schema fallback diagnostics (e35a7d8)
- Discard stale list and cache responses after account reset (50e655d)
- Reset mail views when switching accounts or logging out (752de03)
- Normalize receiving dns record groups (d90a68b)
- Preserve drafts until account departure is confirmed (14a21f8)
- Paginate inbox and sent history with resend cursors (5325938)
- Sandbox sent message html rendering (9b1e62a)
- Enable webkit tls for remote images (5193fa6)
- Make pagination fixture sockets blocking on macos (367ffb1)
- Upload validated release bundles with node24 actions (87bc8f1)
- Build macos app bundles for signed updater archives (295c6f6)
- Use ubuntu 24.04 for release signature verification (59a6101)


### Maintenance
- Add nix flake dev environment (d7917a8)
- Update js deps and add oauth/deep-link rust deps (d1f27f4)
- Reorder separator utility classes (484d48f)
- Update button and input to new shadcn variants (236ffd9)
- Extract shared store read/write primitives (5163ed2)
- Tighten email preview layout (f39e58f)
- Add blobatar avatar packages (0ac3c76)
- Extract stripAt helper to shared module (4c0e783)
- Remove profile model in favor of active domain (7253598)
- Place sender form fields side by side (a4c98a3)
- Add ammonia for html sanitization (f804a5c)
- Remove unused api key prompt components (1ead2d1)
- Add vertical padding to setup page (df0bbfa)
- Change unread dot color (91b1495)
- Rename commom folder, dedupe clipboard, unify DTO casing (1db5ae5)
- Move wizard components to features/setup, split shared/api (e48a160)
- Split dialog into feature section components (aff06dd)
- Extract EmailListItem component for twin lists (bf454a9)
- Extract create-domain-form component (8693070)
- Add vitest with node environment (89800aa)
- Extract domain setup store from wizard step (941dc3b)
- Add unit tests for domain setup, email and dates (8ffdfb5)
- Extract ConnectionStore shared by sidebar and settings (3d77009)
- Split store.rs into settings/credentials stores, add shared http client and test suite (890c1ac)
- Move email cache to sending feature, normalize copy toasts, invalidate sent list after send (16a5a96)
- Reformat toast helper and adjust message wording (b175423)
- Add gstreamer plugins for webkit multimedia (2765550)
- Restrict test discovery to application sources (02d229f)
- Build signed desktop releases on manual dispatch (516898d)
- Focus readme on users and add contribution guide (9320b1a)


### Performance
- Warm up oauth credentials at startup (3a693c8)

