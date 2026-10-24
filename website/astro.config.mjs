// @ts-check
import { defineConfig } from 'astro/config';

import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://rxtsel.github.io',
  base: '/onlysend',
  output: 'static',
  trailingSlash: 'always',
  i18n: {
    locales: ['en', 'es'],
    defaultLocale: 'es',
    routing: { prefixDefaultLocale: false },
  },
  vite: { plugins: [tailwindcss()] },
});