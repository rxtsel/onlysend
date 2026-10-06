# OnlySend

Send email from your own domain with **Resend**, without leaving your desktop.
Connect multiple accounts, compose rich-text messages, and optionally receive mail.

[Download OnlySend](https://github.com/rxtsel/onlysend/releases)

<img width="1250" height="803" alt="Screenshot 2025-12-14 at 13 31 28" src="https://github.com/user-attachments/assets/cbc53014-88a8-4f2a-87aa-1387b348b9fe" />

<img width="1250" height="803" alt="Screenshot 2025-12-14 at 13 34 22" src="https://github.com/user-attachments/assets/027ead37-91b7-4c8e-9794-729c9cc39244" />

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
- Compose formatted messages with attachments, CC, BCC, and Reply-To.
- Browse sent mail and an optional inbox.
- Read downloaded message bodies offline, including after restarting the app.
  The footer shows local-copy progress and errors.

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

Local storage is **not encrypted**. Offline copies include message bodies and
attachment metadata, not attachment files or remote images.

## Contributing

For development, testing, and release instructions, see [CONTRIBUTIONS.md](CONTRIBUTIONS.md).

Licensed under [GPL-2.0-only](LICENSE).
