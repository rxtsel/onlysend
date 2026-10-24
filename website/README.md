# OnlySend website

Static Astro + Tailwind landing page, created with the official minimal template.
Independent from the Svelte/Tauri desktop app.

## Develop

Use Node.js 24 and pnpm (version pinned in package.json).

```sh
cd website
pnpm install
pnpm dev
```

Open http://localhost:4321/onlysend/.

- `pnpm test`: build both languages and check generated links/assets.
- `pnpm preview`: serve the production build locally.
- `pnpm build`: generate static files in `dist/`.

## Content

Spanish (default): `/onlysend/`. English: `/onlysend/en/`.
Edit both dictionaries in `src/content.ts`. Both routes share
`src/components/Landing.astro`; Astro's i18n helper generates language links.

The preview reuses the repository's `assets/preview.gif`. It only loads when
the visitor presses play, and stop restores the still image. To update the still:

```sh
magick '../assets/preview.gif[0]' public/preview.webp
```

Uses system fonts. No custom font files, external font requests, or backend.

`cookie` is explicitly listed because Astro's prerender bundle resolves it from
the project root; otherwise it picks up the desktop app's incompatible version.

## GitHub Pages

In the GitHub repository, set **Settings → Pages → Build and deployment →
Source → GitHub Actions**. Push to `main` or run the **Deploy website to
GitHub Pages** workflow manually.

The workflow builds and checks the website, then deploys `website/dist/`
to https://rxtsel.github.io/onlysend/. It does not build or release the desktop app.

For a different repository or custom domain, update `site` and `base` in
`astro.config.mjs` and the base-path checks in `scripts/check-build.mjs`.
