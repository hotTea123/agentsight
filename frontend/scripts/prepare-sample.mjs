// Copies the pre-built sample snapshot from docs/ into the frontend public
// directory so the demo landing page can load it.  Also copies screenshot
// assets for the demo landing banner.

import { copyFileSync, mkdirSync, existsSync } from 'fs';
import { dirname, resolve } from 'path';
import { fileURLToPath } from 'url';

const here = dirname(fileURLToPath(import.meta.url));
const SRC = resolve(here, '../../docs/sample-snapshot.json');
const OUT = resolve(here, '../public/sample-snapshot.json');
const IMG_DIR = resolve(here, '../public/images');

mkdirSync(dirname(OUT), { recursive: true });
copyFileSync(SRC, OUT);
console.log(`Copied snapshot -> ${OUT}`);

// Copy screenshots for the demo banner
mkdirSync(IMG_DIR, { recursive: true });
const images = ['demo-timeline.png', 'demo-tree.png', 'demo-metrics.png', 'top-mode-demo.png'];
for (const img of images) {
  const src = resolve(here, `../../docs/${img}`);
  if (existsSync(src)) {
    copyFileSync(src, resolve(IMG_DIR, img));
    console.log(`Copied ${img}`);
  }
}
