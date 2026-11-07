# Developing the brainstorm-pro pages

For Claude and developers; the user never sees this file. The interview and document pages are built by
`assets/render-interview.mjs` into one self-contained HTML file. Edit the sources, never a built page.
Edit in `~/.claude/skills-repo/skills/brainstorm-pro`, test there, then copy to `~/.claude/skills/brainstorm-pro`
and check with `diff -rq -x out <repo skill> <live skill>` (`tests/out/` is output, not source).

## File map

| File | Holds |
|---|---|
| `assets/render-interview.mjs` | CLI: `<round.json \| doc.md> <out.html> [--serve \| --open] [--lang vi\|en]` (any other `--lang` value is a usage error, exit 1); puts `lang` into the page data `D` |
| `assets/lib/lang.mjs` | `pageLang({ flag, env })`: `--lang` wins, else the `language` of `${CLAUDE_HOME:-~/.claude}/settings.json` (lowercased, diacritics stripped; `viet…` or `vi` is `vi`), else `en` |
| `assets/lib/round.mjs`, `doc.mjs` | Validate a round (`recommendation: { option, why }`, unknown fields refused with a hint); turn a Markdown file into page data and diff review rounds |
| `assets/lib/diff.mjs` | `wordDiff`: the word diff behind "Xem chỗ đổi" (lines through `alignRows`, words through `lcs`, both from `page/code.js`) |
| `assets/lib/embed.mjs` | Embed code files, `` `path` `` targets and images (data URIs); `InputError`, which the CLI prints as one stderr line (exit 1), and `readInput(path, src, enc, label?)`, a `readFileSync` whose failure is an `InputError` (`<src>: cannot read <label> (<code>)`) |
| `assets/lib/build.mjs` | Bundle `assets/page/` and fill `assets/page/shell.html` into the one HTML |
| `assets/lib/server.mjs` | `--serve`: token URL on 127.0.0.1, `ANSWERS` output, pasted images, 8 h timeout. The review POST body is `{ text, notes? }`; `notes` are saved into `.review-state.json` before `ANSWERS` prints (a failed save is one stderr line, the answers still print) |
| `assets/page/shell.html`, `theme.css`, `tailwind.css` | Page skeleton and styles |
| `assets/page/package.json` | `{"type":"module"}`, so Node reads the page modules `lib/` and the tests import (`strings.js`, `keys.js`, `code.js`) as ESM; `build.mjs` only follows `.js` imports and ignores it |
| `assets/page/main.js` | Picks the mode (interview, review, read-only doc) |
| `assets/page/interview.js`, `doc.js` | Interview cards and send/copy; document sections, notes and Duyệt/Gửi |
| `assets/page/chooser.js` | `sendChooser({ count, send, busy })`: the review page's send chooser (Duyệt / Cần sửa, `D` / `S`), returns `submit(decision)` (split out of `doc.js` for the line cap) |
| `assets/page/rounds.js` | Round 2: previous-notes boxes, "Xem chỗ đổi" toggle and diff box, `formatNote` (one note as a line: the answer text, the previous-notes boxes and the alert) and `noteData` (the `notes` payload) (split out of `doc.js`, which hit the build test's line cap) |
| `assets/page/fab.js` | Floating bar, `CARD`, `INTERVIEW_CARD`, the status line (`say`, `sayFor`) |
| `assets/page/leave.js` | `guardLeave()` (beforeunload guard until answered and after user activation; `#` links jump with `location.replace`), `markAnswered()`, `closeTab()` (`fab.js` `copyText` calls `markAnswered()` on a successful copy) |
| `assets/page/i18n.js` | `STRINGS` (`vi` and `en`, same keys), `setLang(l)`, `t(key, ...args)` (calls a function value, throws on an unknown key). Every fixed string of the chrome lives here; the answer protocol (text sent to Claude) does not. Until `main.js` calls `setLang(D.lang)`, `t()` reads `D.lang`, so modules that build elements at load (`viewer.js`, `fab.js`) get the right language. `shell.html` holds no text: `main.js` sets `<html lang>`, the title, `#topic` and the `#qnav` label |
| `assets/page/dom.js`, `strings.js`, `text.js`, `code.js` | Class constants, `el()`, `kbd()`, `modal()`, `writeClipboard()` (code block Copy; does not end the leave guard); escaping; inline Markdown; code view helpers, and `lcs(A, B)`, the pure common-subsequence walk under `alignRows` and `wordDiff` |
| `assets/page/blocks.js`, `viewer.js`, `images.js`, `passages.js` | Context blocks; file viewer (also the Tailwind class carrier); pasted images; selection comments |
| `assets/page/keys.js`, `shortcuts.js`, `interview-keys.js`, `doc-keys.js` | Key rules; ring, `?` dialog and the key listener; per-mode key tables |
| `tests/` | `unit/` (Node; the server test runs the script, no Chrome), `page/` (headless Chrome), `harness/`, `fixtures/`, `snapshots/`, `out/` |

## Build and module rules

- `build.mjs` joins `main.js` and what it imports into one IIFE, imports before importers, dropping imports
  and the `export ` prefix. All files share one scope: no two may declare the same top-level name
  (`D`, the injected page data, is taken), and an import cycle fails the build.
- Imports are one line only: `import { a, b } from './x.js';`. Exports are `export const|let|function|class`;
  no `export default`, `export { }`, `export *`.
- Pure modules (`i18n.js`, `strings.js`, `code.js`, `keys.js`) must not touch `document` or `window` at top level:
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
node --test --test-concurrency=1 'tests/**/*.test.mjs'    # all (277 tests), a few minutes; a bare tests/ argument fails on Node 24
node --test 'tests/unit/*.test.mjs'                       # unit only (126 tests), no Chrome, under 2 s
node --test tests/page/shortcuts.test.mjs                 # one file
UPDATE_SNAPSHOTS=1 node --test tests/page/snapshot.test.mjs
```

- After any edit run the tests of that part, then the whole suite. New behaviour gets a test.
- Snapshots (`tests/snapshots/`) change only on a deliberate DOM change; read the diff before keeping it.
- Test output goes to `tests/out/<name>/`. Delete with `fs.rmSync` and absolute paths.
- Shared helpers live in `tests/harness/`: `READY` (what `goto` waits for), `hint()`, `STATUS`, `CHOOSER` (the send chooser is open), `ADDER(kind)` (selector of a shown adder button), `GUARDED` (synthetic beforeunload, true when guarded) and `COPY_OK` (clipboard stub that records the text in `window.__copied`) in `page-helpers.mjs`; `T.adder()` in `PASSAGE_HELPERS` is `'hidden'`, `'passage'` or `'passage,edit'`;
  `b.handled(key, mods)` (press, then assert the page itself prevented the key) and `b.choose('d' | 's')` (wait for the chooser, press);
  `serve()` returns `answers()`, `serveRound2(dir, round1?)` serves a second review round (`round1(server)` may drive round 1 first; it is killed in a `finally`);
  `serve()`'s `done` (so `answers()`) rejects after `opts.timeout` ms (default 20 s), killing the child, with the last stderr lines; a test that races `s.done` against a short sleep is safe; `b.status()`, `b.twSettle()`, `b.leaveDialogs()` (count of accepted beforeunload dialogs; tests assert it).
  One Chrome per test file (a top-level `before`).
- The harness runs the CLI with `CLAUDE_HOME` = `tests/fixtures/claude-home-vi` (`language: vietnamese`) unless `opts.claudeHome` is given, so the page language never follows the developer's own settings; `tests/page/lang.test.mjs` serves with `args: ['--lang', 'en']` and scans for Vietnamese.
- After a handled key assert `__keys.at(-1).pagePrevented` (see macOS beeps): the harness would hide a missing `preventDefault`.

## Keys

Interview: `J K 1-9 N A-D R X E G Cmd+Enter ? Esc`. Document review: `J K 1-9 N C E G Cmd+Enter Cmd+Shift+Enter ? Esc`
(Cmd+Shift+Enter is Duyệt; with notes every send opens the chooser, whose `D` Duyệt / `S` Cần sửa are dialog-local; bare `d`/`s` outside it are a display-only row in the key table, `D / S` with no `k`, so it binds no key). Read-only document: `J K 1-9 ? Esc`.
`J`/`K` stop at the ends, `N` wraps. Handlers call `preventDefault`. `?` also opens from the header button; the page names its slot with `shortcuts({ helpSlot })` (it returns `{ helpBtn }`): the end of `#qnav` on the interview (it wraps with the chips, so the chips may wrap one row earlier), and after the nav in the document's own row (`doc.js` wraps `#qnav` in a `w-full` row, nav `flex-1`), so the doc header keeps its rows. A document with no `##` sections hides that row and `doc.js` moves the button to the title row. The engine sets `scroll-padding-top` on `<html>` to the header height + 12 px on every resize of the header (the static `scroll-pt-*` classes are only the first value), so links, `J` and digits never land a heading under the header. `tests/page/header.test.mjs` pins the relative rule: the header is as tall with the button as without it (not at the interview widths where the button makes a chip wrap). On phones (below `sm`) the review bar hides the key hints on Duyệt / Gửi góp ý. `X`/`E` check Khác /
Cần giải thích (never toggle off on a multi card) and focus the box.

## Traps

- Adder timing (`passages.js`, as in Hypothesis): Chrome fires `selectionchange` after `mouseup`, so the
  Góp ý button waits 10 ms after `mouseup` and 100 ms after other selection changes.
- `navigator.clipboard.writeText` can hang for a permission never asked, hence the 2 s race in `dom.js` `writeClipboard` (its textarea fallback restores the focus and the selection).
- Smooth scroll vs the current target: while the page scrolls itself, ring tracking is locked and unlocks
  150 ms after the last scroll event (1 s fallback). `scrollend` was flaky with repeated presses. A `wheel` or
  `touchmove`, or a scrolling key outside a text box (PageUp/PageDown/Space/arrows/Home/End), unlocks at once (the user's scroll; a sideways or ctrl+wheel does not), and `sc.hold()` locks for a scroll the page causes by focusing a box.
- IME: ignore keys while `isComposing`, or a shortcut fires mid-composition.
- Headless Chrome needs focus emulation (the harness sets it), or `:focus` and `document.activeElement` lie.
- macOS beeps: observed with headless Chrome on macOS, which plays the system alert for any key event the page does
  not consume. The harness (`browser.mjs`) silences it with a last-in-line window `keydown` listener that prevents
  unconsumed non-editable, non-modifier, non-default-action keys, and records `window.__keys` (`pagePrevented`) so
  tests still see what the page did. Never add key-press loops outside the harness.
- `window.close()` works only while the tab has one history entry, so guarded pages jump `#` links with
  `location.replace` (a plain `#` jump would add an entry and the tab would stay open after sending).
- The beforeunload dialog needs user activation, so the guard acts only when `navigator.userActivation.hasBeenActive`
  (else Chrome logs "Blocked attempt to show a 'beforeunload'…"). A test needs a real input (`b.click('#topic')`, not
  `eval(...click())` or `insertText`) after each `goto` before `GUARDED` is true; it dispatches a synthetic
  `new Event('beforeunload', { cancelable: true })` and reads `defaultPrevented`.
- The harness replaces `window.close` with the `window.__closed` counter, accepts beforeunload dialogs
  (`b.leaveDialogs()`; otherwise `Page.navigate` waits forever after a real click on a guarded page).
- The whole-stylesheet rebuild is counted by `b.twWrites()`; a test asserting 0 catches a missing carrier class.
- The send chooser (review page, with notes) puts autofocus on its box, so `Enter` does nothing (no default pick); the key
  engine ignores keys while any dialog but help is open, and `D`/`S` are handled by the chooser itself.
