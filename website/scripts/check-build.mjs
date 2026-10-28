import assert from 'node:assert/strict';
import { readFile, access } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { join, resolve, extname } from 'node:path';

const dist = fileURLToPath(new URL('../dist/', import.meta.url));
for (const [locale, route] of [['es', 'index.html'], ['en', 'en/index.html']]) {
  const html = await readFile(join(dist, route), 'utf8');
  assert.ok(html.includes(`<html lang="${locale}" class="scroll-smooth motion-reduce:scroll-auto">`), `Language: ${locale}`);
  assert.equal((html.match(/<h1[\s>]/g) ?? []).length, 1, 'One main heading');
  assert.ok(html.includes('https://github.com/rxtsel/onlysend/releases'));
  assert.ok(html.includes('hreflang="en"') && html.includes('hreflang="es"'));
  assert.ok(html.includes('href="/onlysend/"') && html.includes('href="/onlysend/en/"'));
  const demo = html.match(/<video\b[^>]*id="demo-preview"[^>]*>[\s\S]*?<\/video>/)?.[0];
  assert.ok(demo, 'Native demo video exists');
  assert.ok(/\bplaysinline\b/.test(demo));
  assert.ok(html.includes('id="demo-play"') && html.includes('aria-controls="demo-preview"'));
  assert.ok(!/<(?:video|source)\b[^>]*\ssrc=/.test(demo), 'No video source is active before play');
  assert.ok(demo.includes('preload="none"') && !/\bautoplay\b/.test(demo), 'Video loads only on demand');
  assert.ok(demo.includes('poster="https://static.xavel.app/onlysend-preview.webp"'));
  assert.ok(demo.includes('data-src="https://static.xavel.app/onlysend-demo.mp4" type="video/mp4"'));
  assert.ok(demo.includes('data-src="https://static.xavel.app/onlysend-demo.webm" type="video/webm"'));
  assert.ok(html.includes('property="og:image" content="https://static.xavel.app/onlysend-preview.webp"'));
  assert.ok(html.includes('Made by ') && html.includes('From Colombia.'));
  const creditHref = html.match(/href="([^"]+)"[^>]*>rxtsel<\/a>/)?.[1];
  assert.ok(creditHref, 'Author link exists');
  const creditUrl = new URL(creditHref.replaceAll('&amp;', '&'));
  assert.equal(creditUrl.origin, 'https://rxtsel.dev');
  assert.equal(creditUrl.searchParams.get('utm_source'), 'onlysend');
  assert.equal(creditUrl.searchParams.get('utm_medium'), 'website');
  assert.equal(creditUrl.searchParams.get('utm_campaign'), 'project_credit');
  assert.ok(html.includes('id="comparison-heading"') && html.includes('https://resend.com/pricing'));
  assert.ok(html.includes('Rust') && html.includes('Tauri'));
  assert.ok(html.includes(locale === 'es' ? 'Resend procesa el envío y la recepción en sus servidores' : 'Resend processes sending and receiving on its servers'));
  assert.ok(html.includes(locale === 'es' ? 'El almacenamiento local no está cifrado' : 'Local storage is not encrypted'));
  assert.ok(html.includes('hreflang="x-default" href="https://rxtsel.github.io/onlysend/"'));
  assert.ok(!html.includes('.gif'), 'Website does not load or bundle the README GIF');
  const ids = new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
  for (const [, url] of html.matchAll(/\b(?:href|src|data-animation|data-poster)="([^"]+)"/g)) {
    if (url.startsWith('#')) {
      assert.ok(ids.has(url.slice(1)), `Anchor exists: ${url}`);
    } else if (url.startsWith('/')) {
      assert.ok(url.startsWith('/onlysend/'), `Repository base: ${url}`);
      const path = url.slice('/onlysend/'.length);
      const target = resolve(dist, path, ...(!extname(path) ? ['index.html'] : []));
      await access(target);
    }
  }
  console.log(`✓ ${locale}: metadata, language links, anchors, demo, and local assets`);
}
