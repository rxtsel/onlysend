# OnlySend website

Static Astro + Tailwind landing page, created with the official minimal template.
Independent from the Svelte/Tauri desktop app.

## Develop

Use Node.js 24 and Bun (version pinned in package.json).

```sh
cd website
bun install --frozen-lockfile
bun run dev
```

Open http://localhost:4321/onlysend/.

- `bun run test`: build both languages and check generated links/assets.
- `bun run preview`: serve the production build locally.
- `bun run build`: generate static files in `dist/`.

## Content

Spanish (default): `/onlysend/`. English: `/onlysend/en/`.
Edit both dictionaries in `src/content.ts`. Both routes share
`src/components/Landing.astro`; Astro's i18n helper generates language links.

The full demo and its poster are hosted on Cloudflare at `https://static.xavel.app/`:
`onlysend-demo.mp4`, `onlysend-demo.webm`, and `onlysend-preview.webp`.
Only the poster loads initially. Video sources are activated on the first play
click, then native controls allow pausing and seeking (`preload="none"`).
Update the URLs in `src/components/Landing.astro` when replacing these files.
The README's GIF stays separate in the repository's `assets/preview.gif`.

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
