# Contributing to OnlySend

OnlySend uses Svelte 5/SvelteKit for the UI and Tauri 2/Rust for desktop services.

## Development

Install Bun, stable Rust, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
for your OS. On Linux with Nix, `nix develop` provides build dependencies and the
WebKit TLS/multimedia environment.

```sh
bun install --frozen-lockfile
bun run tauri dev
```

Do not run the app as root. Development uses the normal app-data directory, so
use test accounts: connecting, sending, and enabling receiving are real actions.

## Changes and pull requests

- Start from `develop` and open feature/fix PRs against `develop`.
- Keep commits focused, using lowercase Conventional Commits subjects, at most
  72 characters: `fix(mail): preserve cached bodies`.
- Describe what changed, how it was tested, and any user-visible limitations.
- Preserve account isolation, cached mail, and draft confirmation on destructive
  operations. See [docs/sqlite.md](docs/sqlite.md) for the archive design.
- Never commit credentials, signing keys, personal mail, database files, build
  artifacts, or local plans/benchmark experiments. Inspect the staged diff.
- For bugs, include OS, app version, reproduction steps, and redacted errors.
  Do not post API keys, OAuth tokens, private keys, or private email content.

## Validation

```sh
bun run check
bunx vitest run
bun run build
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
python3 -B -m unittest discover -s .github -p 'test_release.py'
git diff --check
```

When editing workflows, run `actionlint .github/workflows/release.yml`. Tests do
not replace desktop acceptance: check restart/offline reading, account switching,
HTML images, and update/draft behavior on the platforms affected.

Build a local desktop executable without installers:

```sh
bun run tauri build --no-bundle
```

For installers, use `bun run tauri build`. Local builds have no updater public
key by default and do not check/install updates; release CI embeds the public key.

## Manual releases (maintainers)

Pushes and merges **never release automatically**. Merge the desired changes from
`develop` into `main`, then select **Actions → Manual release → Run workflow**:

- Branch: `main`.
- Version: a stable SemVer greater than the current version and existing release
  tags, without a `v` prefix (for example `0.2.0`).

The workflow uses git-cliff to collect accumulated Conventional Commits and
aligns `package.json`, `src-tauri/tauri.conf.json`, Cargo.toml, and Cargo.lock.
It tests/builds Linux `.deb`/`.AppImage`, Windows NSIS `.exe`, and macOS `.dmg`
for Intel/Apple Silicon, plus the updater's signatures and macOS `.app.tar.gz`.
No RPM/MSI builds are produced.

Only after all builds succeed does it record the release commit/tag, upload to a
draft, and publish a complete `latest.json`. Signature verification must succeed
with the public key embedded in the apps. A newer tag blocks publishing an older
version as latest. A concurrent change to `main` rejects the normal atomic push;
no history is force-pushed.

The publishing job needs permission to update `main` and create tags. If branch
protection prohibits that, configure an allowed release identity before running
it. Retry failed jobs from the original run; do not replace published releases or
existing tags. Merge the resulting version/changelog commit back into `develop`
before starting the next development cycle.

### Signing setup (once)

Use Tauri's signer, **not OpenSSL**, outside the repository:

```sh
mkdir -p ~/.tauri
bunx tauri signer generate -w ~/.tauri/onlysend.key
```

Back up the private key and password securely. Under GitHub repository
**Settings → Secrets and variables → Actions**, configure repository-level values:

| Type | Name | Value |
| --- | --- | --- |
| Variable | `TAURI_UPDATER_PUBLIC_KEY` | Contents of `onlysend.key.pub` |
| Secret | `TAURI_SIGNING_PRIVATE_KEY` | Contents of `onlysend.key` |
| Secret | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password, if set |

These are file **contents**, not local paths. The workflow has no GitHub
Environment. Never print private keys or expose them to pull-request jobs.

Updater signatures are separate from OS signing. macOS uses ad-hoc signing for
now, without Apple notarization; Windows has no code-signing certificate configured.
Validate downloaded installers on clean systems before recommending a release.
In-app Linux installation is limited to AppImage; Debian uses the package manager.
The app checks once per startup, with additional explicit checks in Settings →
About. Download/signature verification and install/restart are separate actions.
Account identity and the SQLite data directory must remain unchanged across updates.
