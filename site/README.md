# BudMix website

A single static page (`index.html` plus `assets/`): no build step, no
tracking, no outside requests. A little inline script runs the theme
button, the menu on phones and the walkthrough player.

- Dark by default; the sun/moon button switches to light and the choice is
  remembered in the visitor's browser.
- The pictures and the walkthrough are inline SVG, so they follow the theme.

## Preview

Open `index.html` in a browser.

## Publish

The site is on Vercel (project `budmix`, root directory `site`). Every push
to `main` deploys it.

## Download button

The buttons point to
`https://github.com/Mrehankamboh/BudMix/releases/latest/download/BudMix-windows-x64-setup.exe`,
so every release must include the installer under that fixed name.
