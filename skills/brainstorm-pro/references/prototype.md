# Prototype variants for an interview card

Build 2-4 full-size, flippable variants of one piece of UI, open them in a new tab from the question card, and let the user choose on the card.

## When to use

Use a `prototype` when the options differ in motion, interaction, personality or full-size layout. A static layout question keeps `mockup`.

## Steps

1. **Scope one piece.** One component, screen or flow per question. If the question covers more, split it.
2. **Recon the working repo's tokens.** Read colors, radii, spacing, fonts and easing from the repo and copy the values in. With no project, use the harness defaults: neutral grays, one accent, system font stack.
3. **Choose 2-4 directions, default 3.** Each has a name and an axis (what it varies). No two variants share a position on an axis. If two converge, merge them and say so on the card.
4. **Build from the harness.** Copy `~/.claude/skills/brainstorm-pro/assets/prototype/harness.html` to `<scratchpad>/interview-<slug>/proto-qNN.html`. Fill `variants` (an array of functions returning HTML strings) and `<style id="variant-css">`, and add the picker buttons, one per variant, in the same order. Build one variant at a time, full size, in realistic surrounding context with realistic content: no lorem ipsum, no dead buttons.

## Content rules

- Claude writes the content. Never paste external text (Jira, web pages, repo data) into variants as markup: paraphrase it or escape it.
- Inline every resource (CSS, fonts, images, data) where possible, and never point at a dev server's localhost.

## Craft bar

- Ease-out on entrances, never ease-in.
- UI motion under 300ms, except modals, drawers, sheets and toasts.
- Correct `transform-origin` (popovers grow from their trigger).
- Prefer `transform` and `opacity`.
- Handle `prefers-reduced-motion`.

## Picker is chrome

The `.proto-picker` markup, CSS and wiring are copied verbatim and stay identical across runs. Do not restyle them. Two allowed changes only: set `data-position="top"` when a variant occupies the bottom-center of the screen; drop the replay button and the divider when no variant animates.

## Variant JS

Never put a `<script>` inside variant HTML: `innerHTML` does not run it. Interaction goes in the harness's one shared `<script>`, in the marked region, as listeners delegated on `document` and keyed by `data-*` attributes. On-mount work hooks a delegated `animationstart`.

## Self-check

With the Chrome DevTools MCP: `new_page` with `background: true`, open each `?v=N`, check the console is clean, take a screenshot. Without the MCP, add a `note` block on the card saying the variants were not flipped through.

## Mapping to options

Option N is picker item N, in the same order. The option `label` is the direction name; `means` states the axis. Set the question's `prototype` field to the absolute (or `~/`) path of the `.html` file. It must be an `.html` file under 512 KB containing `class="proto-picker"`, `id="stage"`, and exactly `options.length` items with `class="proto-picker-item"`.

## After the pick

Copy the file to `<plan-dir>/prototypes/qNN-<kebab-name>.html`. Delete the losing entries from `variants` and their picker buttons. Cite the file from the spec's `### Components / Files` and in the `## Decisions` row.

## Credits

The picker and the build method are adapted from https://github.com/emilkowalski/skills/tree/64eb3deb9a5fbfa041faf783e0bc3fd635872e9c/skills/prototype (MIT, Copyright (c) 2026 Emil Kowalski).
