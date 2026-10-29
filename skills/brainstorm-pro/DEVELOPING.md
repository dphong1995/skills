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
| `assets/lib/build.mjs` | Bundle `assets/page/` and fill `assets/page/shell.html` into the one HTML |
| `assets/lib/server.mjs` | `--serve`: token URL on 127.0.0.1, `ANSWERS` output, pasted images, 8 h timeout |
| `assets/page/shell.html`, `theme.css`, `tailwind.css` | Page skeleton and styles |
| `assets/page/package.json` | `{"type":"module"}`, so Node reads the page modules `lib/` and the tests import (`strings.js`, `keys.js`, `code.js`) as ESM; `build.mjs` only follows `.js` imports and ignores it |
| `assets/page/main.js` | Picks the mode (interview, review, read-only doc) |
| `assets/page/interview.js`, `doc.js` | Interview cards and send/copy; document sections, notes, Duyệt/Gửi |
| `assets/page/fab.js` | Floating bar, `CARD`, `INTERVIEW_CARD`, the status line (`say`, `sayFor`) |
| `assets/page/dom.js`, `strings.js`, `text.js`, `code.js` | Class constants, `el()`, `kbd()`, `modal()`; escaping; inline Markdown; code view helpers |
| `assets/page/blocks.js`, `viewer.js`, `images.js`, `passages.js` | Context blocks; file viewer (also the Tailwind class carrier); pasted images; selection comments |
| `assets/page/keys.js`, `shortcuts.js`, `interview-keys.js`, `doc-keys.js` | Key rules; ring, `?` dialog and the key listener; per-mode key tables |
| `tests/` | `unit/` (Node; the server test runs the script, no Chrome), `page/` (headless Chrome), `harness/`, `fixtures/`, `snapshots/`, `out/` |

## Build and module rules

- `build.mjs` joins `main.js` and what it imports into one IIFE, imports before importers, dropping imports
  and the `export ` prefix. All files share one scope: no two may declare the same top-level name
  (`D`, the injected page data, is taken), and an import cycle fails the build.
- Imports are one line only: `import { a, b } from './x.js';`. Exports are `export const|let|function|class`;
  no `export default`, `export { }`, `export *`.
- Pure modules (`strings.js`, `code.js`, `keys.js`) must not touch `document` or `window` at top level:
  `tests/unit/` imports them in Node, and so may `assets/lib/` (`doc.mjs` and `embed.mjs` take `plain` and `PATH` from
  `strings.js`) and the test harness (`keys.js`'s `isTyping`, injected into the page as source, so it stays
  self-contained). `dom.js` has no top-level DOM access either (only `navigator`, available in Node >= 21), so a pure
  module may import it. Everything else is browser-only.
- The build also compiles the joined script, so a redeclaration or syntax error fails with `bundle does not parse`;
  it rejects `</script` and `<!--` in the script, and an imported name the imported file does not declare.
- Tailwind runs in the browser. A class used only by an element built late (dialog, popup, note box) must
  appear in the carrier in `viewer.js`, or its first use rebuilds the whole stylesheet (a visible hitch).

## Tests

Need Google Chrome (`CHROME=<path>` for another), network (CDNs: Tailwind, highlight.js, marked, Mermaid) and Node >= 22.12.
Run from the skill directory:

```
node --test --test-concurrency=1 'tests/**/*.test.mjs'    # all (205 tests), ~100 s; a bare tests/ argument fails on Node 24
node --test 'tests/unit/*.test.mjs'                       # unit only (99 tests), no Chrome, under 2 s
node --test tests/page/shortcuts.test.mjs                 # one file
UPDATE_SNAPSHOTS=1 node --test tests/page/snapshot.test.mjs
```

- After any edit run the tests of that part, then the whole suite. New behaviour gets a test.
- Snapshots (`tests/snapshots/`) change only on a deliberate DOM change; read the diff before keeping it.
- Test output goes to `tests/out/<name>/`. Delete with `fs.rmSync` and absolute paths.
- Shared helpers live in `tests/harness/`: `READY` (what `goto` waits for), `hint()` and `STATUS` in `page-helpers.mjs`;
  `serve()` returns `answers()`, `serveRound2(dir)` serves a second review round; `b.status()`, `b.twSettle()`.
  One Chrome per test file (a top-level `before`).
- After a handled key assert `__keys.at(-1).pagePrevented` (see macOS beeps): the harness would hide a missing `preventDefault`.

## Keys

Interview: `J K 1-9 N A-D R X E G Cmd+Enter ? Esc`. Document review: `J K 1-9 N C G Cmd+Enter Cmd+Shift+Enter ? Esc`
(Cmd+Shift+Enter is Duyệt; with notes it needs a second press within 3 s). Read-only document: `J K 1-9 ? Esc`.
`J`/`K` stop at the ends, `N` wraps. Handlers call `preventDefault`. `?` also opens from the header button, the last
item of the header's nav: at the end of `#qnav` on the interview (it wraps with the chips), and after the nav in the document's own row (`doc.js` wraps `#qnav` in a `w-full` row that always exists, nav `flex-1`), so it never takes a header row of its own (`tests/page/header.test.mjs` pins the old heights). `X`/`E` check Khác /
Cần giải thích (never toggle off on a multi card) and focus the box.

## Traps

- Adder timing (`passages.js`, as in Hypothesis): Chrome fires `selectionchange` after `mouseup`, so the
  Góp ý button waits 10 ms after `mouseup` and 100 ms after other selection changes.
- `navigator.clipboard.writeText` can hang for a permission never asked, hence the 2 s race in `fab.js`.
- Smooth scroll vs the current target: while the page scrolls itself, ring tracking is locked and unlocks
  150 ms after the last scroll event (1 s fallback). `scrollend` was flaky with repeated presses. A `wheel` or
  `touchmove`, or a scrolling key outside a text box (PageUp/PageDown/Space/arrows/Home/End), unlocks at once (the user's scroll; a sideways or ctrl+wheel does not), and `sc.hold()` locks for a scroll the page causes by focusing a box.
- IME: ignore keys while `isComposing`, or a shortcut fires mid-composition.
- Headless Chrome needs focus emulation (the harness sets it), or `:focus` and `document.activeElement` lie.
- macOS beeps: observed with headless Chrome on macOS, which plays the system alert for any key event the page does
  not consume. The harness (`browser.mjs`) silences it with a last-in-line window `keydown` listener that prevents
  unconsumed non-editable, non-modifier, non-default-action keys, and records `window.__keys` (`pagePrevented`) so
  tests still see what the page did. Never add key-press loops outside the harness.
- The whole-stylesheet rebuild is counted by `b.twWrites()`; a test asserting 0 catches a missing carrier class.
