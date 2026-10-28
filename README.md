# OnlySend

Send email from your own domain with **Resend**, without leaving your desktop.
Connect multiple accounts, compose rich-text messages, and optionally receive mail.

[Download OnlySend](https://github.com/rxtsel/onlysend/releases)

![OnlySend demo](assets/preview.gif)

## Get started

1. Install OnlySend and connect your Resend account with OAuth or an API key.
2. Follow the setup wizard to choose your domains and sender addresses.
3. Compose and send. Enable receiving only if you want an inbox.

You need a Resend account and access to your domain's DNS settings. Sending
requires a verified domain. Enabling receiving can redirect existing mail through
new MX records; review the DNS instructions before changing them.

## What you can do

- Switch between accounts with separate mail and settings.
- Manage multiple domains, sender addresses, and DNS configuration.
- Compose formatted messages with attachments, CC and BCC.
- Reply to received messages using the same composer.
- Browse sent mail and an optional inbox with a dedicated reading pane.
- Search saved mail and filter by domain, including messages outside loaded pages.
- See previews of already downloaded messages without fetching each body again.
- Read downloaded message bodies offline, including after restarting the app.
  The footer shows local-copy progress and errors.

## Email content

Email images, stylesheets, and fonts load automatically, including remote resources.
Their servers may detect when you open a message. Sender scripts and forms remain
blocked, and links open in your system browser. Remote resources are not saved
for offline reading.

## Replies

Use **Reply** at the right of the **Message** toolbar to open the existing composer.
Recipients come from `Reply-To`, or the sender when it is absent. The matching
saved sender address is selected when available, otherwise your default is used.
The subject has a single **Re:** prefix and is read-only; Rust also enforces it
and adds `In-Reply-To` and `References` when sending. Manual Reply-To and Message-ID
controls have been removed from the composer. Original attachments are not copied
into replies automatically.

Older downloaded messages may need one online refresh to recover reply headers.
If those details cannot be recovered, the original offline message remains intact
and the reply cannot be sent without them. No Resend Inboxes subscription is needed.

## Attachments

Received and sent messages show attachment cards below the reading area, like
in the composer, rather than inside the subject/sender header. Click an image for a preview,
click a supported document to open it with the default application, or choose
**Save as…**. Downloads show progress and can be cancelled. PNG, JPEG, GIF and WebP
previews support files up to **5 MB**; downloads support **25 MB** per file.
Other file types, including executable files, HTML and SVG, must be saved rather
than opened automatically. Images referenced by `cid:` are loaded into the message
when available and are not repeated as attachment cards.

The composer uses the same cards, supports adding/removing files individually,
and accepts **25 MB total**. Image preview URLs are released when files are removed
or the draft closes. Attachment bytes are prepared only when sending.

An attachment is downloaded once per session and reused for previews, external
opening and **Save as…**, including offline. Missing or incomplete temporary files
are downloaded again. The external viewer gets a separate working copy, reused
within the session, so its edits cannot change the original attachment. **Save as…**
in OnlySend always copies the original; use the external viewer's own Save as to
keep edits before quitting OnlySend.
The index lives only in Rust memory, not SQLite. OnlySend attempts to remove its
private session folder and pending save temporaries when exiting; a crash or an
external application's file lock can leave temporary copies behind. A new session
needs to download again.

## Inbox notifications

OnlySend checks connected accounts from Rust every **5 minutes**, including when
the window is minimized or another screen is open. In **Settings → Notifications**,
choose 5, 10, 30 or 60 minutes, or turn automatic checks and notifications off.
The Refresh button still works when automatic checks are off.

The first successful check establishes a silent baseline. Subsequent checks
notify only new arrivals, with per-account checkpoints retained across restarts.
Notifications display a count, not email contents. Failed checks back off rather
than repeatedly hitting the API. System notification settings can suppress alerts.

OnlySend must remain running; quitting the app stops checks. Automatic checks
fetch new inbox metadata, not the full archive on every interval. An explicit
Refresh also retries the background archive download.

## Appearance

In **Settings → Appearance**, choose **Light**, **Dark**, or **System** (default).
System follows your device's appearance, including changes while OnlySend is open.

Email avatars default to **Blobavatars**; choose **Minimal** to use initials.
Avatars are generated locally from the displayed email address, so the same
address has the same avatar in Inbox and Sent even when its display name changes.
Appearance preferences are saved on this device and apply across accounts.

## Downloads and updates

Release builds target Linux (`.deb` and `.AppImage`), Windows (`.exe`), and macOS
(`.dmg`, Intel and Apple Silicon). Download the matching file from
[GitHub Releases](https://github.com/rxtsel/onlysend/releases).

Updater-enabled builds check once when opened. You can also use
**Settings → About → Check for updates**. Installation is your choice; the app
asks about unsaved drafts before closing. Debian installations use the package
manager rather than the in-app installer.

macOS builds are not notarized and may require approval in **Privacy & Security**.
Windows may show a SmartScreen warning. Older versions without the updater need
one manual installation to receive in-app updates.

## Your data

OnlySend uses Resend for sending and receiving; no separate OnlySend mail server
is required. Downloaded messages are kept locally even if they disappear from
Resend's lists. Disconnecting an account does not erase its local archive.

Local storage is **not encrypted**. The persistent mail archive includes message
bodies and attachment metadata, not attachment files or remote images. Downloaded
attachment bytes are kept separately in private session temporaries or saved by you.

## Contributing

For development, testing, and release instructions, see [CONTRIBUTIONS.md](CONTRIBUTIONS.md).

Licensed under [GPL-2.0-only](LICENSE).
