# BudMix website

A single static page (`index.html` plus `assets/`): no build step, no
scripts, no tracking. Light and dark follow the visitor's system theme.

## Preview

Open `index.html` in a browser.

## Publish for free

- **GitHub Pages** (free for public repositories): "Deploy from a branch"
  only serves `/` or `/docs`, so publish `site/` with a Pages workflow
  (*Settings → Pages → Source: GitHub Actions*, then a workflow that
  uploads `site/` with `actions/upload-pages-artifact`).
- **Netlify / Cloudflare Pages** (free plans): drag the `site` folder into
  the dashboard, or connect the repository with `site` as the publish folder.

## Download button

The buttons point to
`https://github.com/Mrehankamboh/BudMix/releases/latest`, which works once
a release is published and the repository is public.
