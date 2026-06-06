# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Jekyll-based technical blog hosted on GitHub Pages at ddanilov.me. Uses the [Beautiful Jekyll](https://github.com/daattali/beautiful-jekyll) remote theme. Content focuses on C++, Rust, Linux, multithreading, and systems programming.

## Development Commands

All development runs inside Docker via Make:

```bash
make run        # Start Jekyll dev server at http://localhost:4000 (foreground)
make rund       # Start in background (detached)
make stop       # Stop running container
make login      # Open shell inside the Jekyll container
make bundle     # Install Ruby gem dependencies
make build-image # Build the Docker image
```

There is no separate lint or test command. Markdown linting rules are defined in `.markdownlint.yaml` (MD013 line-length disabled; inline `<div>` and `<span>` allowed).

## Content Structure

**New posts** go in `_posts/` as `YYYY-MM-DD-slug-title.md`. Front matter fields used across existing posts:

```yaml
---
layout: post
title: "Post Title"
tags: [c++, linux, docker]   # used for tag pages
comments: true               # Disqus; omit to disable
share-img: /img/some-image.png
---
```

**Static images** for posts live in `/img/`. Assets (CSS/JS) are in `/assets/`.

**Talks data** is in `_data/talks.yml` — each entry has `date`, `venue`, `title`, `youtube_id`, and `description`. The `talks.md` page renders from this file via the `_layouts/talks.html` layout.

## Architecture

### Layout Hierarchy

```
base.html          ← root: CSS/JS includes, dark-mode FOUC prevention, footer
  └── default.html
  └── home.html    ← paginated post list
  └── page.html    ← static pages (aboutme, posts, etc.)
  └── post.html    ← blog posts (adds share buttons, comments, tags)
  └── talks.html   ← iterates _data/talks.yml
  └── minimal.html
```

### Dark Mode

Implemented with CSS variables (`assets/css/dark-theme.css`) and `localStorage` persistence (`assets/js/dark-theme.js`). An inline `<script>` in `base.html` reads localStorage before page render to prevent flash of unstyled content.

### Theme Customization

The remote theme (`daattali/beautiful-jekyll`) is overridden by placing files at the same paths locally — anything in `_includes/`, `_layouts/`, or `assets/` shadows the theme's version. `_config.yml` exposes most visual options (navbar color, link color, footer color, etc.).

### Comments & Analytics

- **Disqus** is enabled site-wide (`disqus: ddanilov-me` in `_config.yml`); disable per-post with `comments: false`.
- **Google Analytics** (UA-155369174-1) and **Google Tag Manager** (GTM-T8QVS29) are active.
- **Staticman** is configured in `staticman.yml` but comments go to `_data/comments/{post-slug}/` and require a running Staticman instance.

## Commit Conventions

Observed commit style: `post:`, `fix()`, `refactor()`, `style()` prefixes — e.g. `post: add article on shared_ptr` or `fix(dark-mode): correct toggle state`.
