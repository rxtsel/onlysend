<p align="center">
  <img src="assets/app-icon.png" alt="OnlySend logo" width="128" />
</p>

<p align="center">
  <a href="https://github.com/rxtsel/onlysend/releases"><img src="https://img.shields.io/github/downloads/rxtsel/onlysend/total" alt="Total downloads" /></a>
  <a href="https://github.com/rxtsel/onlysend/releases/latest"><img src="https://img.shields.io/github/v/release/rxtsel/onlysend" alt="Latest release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--2.0--only-blue" alt="License: GPL-2.0-only" /></a>
</p>

# OnlySend

Send email from your own domain with **Resend**, without leaving your desktop.
Connect multiple accounts, compose rich-text messages, and optionally receive mail.

[Download OnlySend](https://github.com/rxtsel/onlysend/releases) · Linux, Windows, and macOS

![OnlySend demo](assets/preview.gif)

## Get started

1. Install OnlySend and connect your Resend account with OAuth or an API key.
2. Follow the setup wizard to choose your domains and sender addresses.
3. Compose and send. Enable receiving only if you want an inbox.

You need a Resend account and a verified domain. Enabling receiving requires DNS
changes and may redirect existing mail; review the instructions before changing MX records.

## Features

- [x] Multiple accounts, domains, and sender addresses.
- [x] Rich-text messages with attachments, CC, and BCC.
- [x] Sent mail, an optional inbox, and replies.
- [x] Search saved mail and filter by domain.
- [x] Read downloaded messages offline.
- [x] Preview images, open supported attachments, or save them.
- [x] Configurable inbox checks and notifications while the app is running.
- [x] Light, dark, and system themes, with customizable avatars.
- [x] Check for updates from Settings.

## Good to know

- Attachments: up to **25 MB total** when composing and **25 MB per file** when downloading.
- Remote email images and styles load automatically and may reveal when you open a message.
- Downloaded mail stays on this device, even after disconnecting an account.
  Local storage is **not encrypted**; attachments and remote images are not part of the offline archive.
- macOS builds are not notarized and may need approval in **Privacy & Security**.
  Windows may show a SmartScreen warning.

## Contributing

For development, testing, and release instructions, see [CONTRIBUTIONS.md](CONTRIBUTIONS.md).

Licensed under [GPL-2.0-only](LICENSE).
