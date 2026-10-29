# Developing the brainstorm-pro pages

For Claude and developers; the user never sees this file. The interview and document pages are built by
`assets/render-interview.mjs` into one self-contained HTML file. Edit the sources, never a built page.
Edit in `~/.claude/skills-repo/skills/brainstorm-pro`, test there, then copy to `~/.claude/skills/brainstorm-pro`
and check with `diff -rq -x out <repo skill> <live skill>` (`tests/out/` is output, not source).

## File map

| File | Holds |
|---|---|
| `assets/render-interview.mjs` | CLI: `<round.json \| doc.md> <out.html> [--serve \| --open]` |
| `assets/lib/round.mjs`, `doc.mjs` | Validate a round; turn a Markdown file into page data and diff review rounds |
| `assets/lib/embed.mjs` | Embed code files, `` `path` `` targets and images (data URIs) |
| `assets/lib/build.mjs` | Bundle `page/` and fill `page/shell.html` into the one HTML |
| `assets/lib/server.mjs` | `--serve`: token URL on 127.0.0.1, `ANSWERS` output, pasted images, 8 h timeout |
| `page/shell.html`, `theme.css`, `tailwind.css` | Page skeleton and styles |
| `page/main.js` | Picks the mode (interview, review, read-only doc) |
| `page/interview.js`, `doc.js` | Interview cards and send/copy; document sections, notes, Duyệt/Gửi |
| `page/fab.js` | Floating bar, `CARD`, the status line (`say`) |
| `page/dom.js`, `strings.js`, `text.js`, `code.js` | Class constants and `el()`; escaping; inline Markdown; code view helpers |
| `page/blocks.js`, `viewer.js`, `images.js`, `passages.js` | Context blocks; file viewer (also the Tailwind class carrier); pasted images; selection comments |
| `page/keys.js`, `shortcuts.js`, `interview-keys.js`, `doc-keys.js` | Key rules; ring, `?` dialog and the key listener; per-mode key tables |
| `tests/` | `unit/` (Node), `page/` (headless Chrome), `harness/`, `fixtures/`, `snapshots/`, `out/` |

## Build and module rules

- `build.mjs` joins `main.js` and what it imports into one IIFE, imports before importers, dropping imports
  and the `export ` prefix. All files share one scope: no two may declare the same top-level name
  (`D`, the injected page data, is taken), and an import cycle fails the build.
- Imports are one line only: `import { a, b } from './x.js';`. Exports are `export const|let|function|class`;
  no `export default`, `export { }`, `export *`.
- Pure modules (`strings.js`, `code.js`, `keys.js`) must not touch `document` or `window` at top level:
  `tests/unit/` imports them in Node. Everything else is browser-only.
- Tailwind runs in the browser. A class used only by an element built late (dialog, popup, note box) must
  appear in the carrier in `viewer.js`, or its first use rebuilds the whole stylesheet (a visible hitch).

## Tests

Need Google Chrome (`CHROME=<path>` for another), network (CDNs: Tailwind, highlight.js, marked, Mermaid) and Node >= 22.12.
Run from the skill directory:

```
node --test --test-concurrency=1 'tests/**/*.test.mjs'    # all, ~100 s; a bare tests/ argument fails on Node 24
node --test 'tests/unit/*.test.mjs'                       # unit only, no Chrome
node --test tests/page/shortcuts.test.mjs                 # one file
UPDATE_SNAPSHOTS=1 node --test tests/page/snapshot.test.mjs
```

- After any edit run the tests of that part, then the whole suite. New behaviour gets a test.
- Snapshots (`tests/snapshots/`) change only on a deliberate DOM change; read the diff before keeping it.
- Test output goes to `tests/out/<name>/`. Delete with `fs.rmSync` and absolute paths.

## Keys

Interview: `J K 1-9 N A-D R X E G Cmd+Enter ? Esc`. Document review: `J K 1-9 N C G Cmd+Enter Cmd+Shift+Enter ? Esc`
(Cmd+Shift+Enter is Duyệt; with notes it needs a second press within 3 s). Read-only document: `J K 1-9 ? Esc`.
`J`/`K` stop at the ends, `N` wraps. Handlers call `preventDefault`. `?` also opens from the header button.

## Traps

- Adder timing (`passages.js`, as in Hypothesis): Chrome fires `selectionchange` after `mouseup`, so the
  Góp ý button waits 10 ms after `mouseup` and 100 ms after other selection changes.
- `navigator.clipboard.writeText` can hang for a permission never asked, hence the 2 s race in `fab.js`.
- Smooth scroll vs the current target: while the page scrolls itself, ring tracking is locked and unlocks
  150 ms after the last scroll event (1 s fallback). `scrollend` was flaky with repeated presses.
- IME: ignore keys while `isComposing`, or a shortcut fires mid-composition.
- Headless Chrome needs focus emulation (the harness sets it), or `:focus` and `document.activeElement` lie.
- macOS beeps: Chrome plays the system alert for any key event the page does not consume. The harness
  (`browser.mjs`) adds a last-in-line window `keydown` listener that prevents unconsumed non-editable,
  non-modifier, non-default-action keys, and records `window.__keys` (`pagePrevented`) so tests still see what
  the page did. Never add key-press loops outside the harness.
- Harness `press()` sends a separate `char` event. For a printable shortcut that moves focus into a box, use a
  keyDown-only press (see `tests/page/shortcuts.test.mjs`), or the character lands in the box.
- The whole-stylesheet rebuild is counted by `b.twWrites()`; a test asserting 0 catches a missing carrier class.
