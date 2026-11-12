#!/usr/bin/env bash
# Install the items you pick from this repo into ~/.claude (menu, or --only/--all/--update).
# Existing files with the same name are moved to ~/.claude/backups/ first.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${CLAUDE_HOME:-$HOME/.claude}"
BACKUP="$DEST/backups/skills-install-$(date +%Y%m%d-%H%M%S)"
SELECTION="$DEST/.skills-repo-selection"

# Item table (parallel arrays, bash 3.2: no associative arrays). DEFAULTS: 1 = ticked in the menu.
ITEMS=(skills subagent vietnamese compact org-skills cmux hud widget)
DESCS=(
  "4 skills + 19 agents + web-pages rule + todo tools (add subagent when you use cook)"
  "subagent-models rule + context-guard hook (recommended with cook)"
  "language rule + reply-language hook + language=vietnamese"
  "main-context-compact mod"
  "skillOverrides that switch off org skills"
  "cmux settings (cmux/install.sh)"
  "OMC HUD (hud/install.sh)"
  "ClaudeSessions widget (installs the release pinned in claude-sessions/release.lock after verifying it; needs macOS)"
)
DEFAULTS=(1 0 0 0 0 0 0 0)
COUNT=${#ITEMS[@]}

# Whether these files existed before this run: only those are worth a backup.
HAD_SETTINGS=0
HAD_CLAUDE_MD=0
if [ -e "$DEST/settings.json" ]; then HAD_SETTINGS=1; fi
if [ -e "$DEST/CLAUDE.md" ]; then HAD_CLAUDE_MD=1; fi
SUMMARY_INSTALLED=""
SUMMARY_SKIPPED=""

usage() {
  cat <<EOT
Usage: bash install.sh [--list | --only a,b | --all | --update | -h]
  (no flag)   interactive menu (needs a terminal)
  --list      print the items and exit
  --only a,b  install just these items (added to the remembered selection)
  --all       install every item
  --update    reinstall the remembered selection
Items: ${ITEMS[*]}
EOT
}

die_usage() {
  echo "$1" >&2
  echo "Items: ${ITEMS[*]}" >&2
  exit 2
}

index_of() {  # $1 = item name; prints its index, returns 1 when unknown
  local i
  for ((i = 0; i < COUNT; i++)); do
    if [ "${ITEMS[$i]}" = "$1" ]; then echo "$i"; return 0; fi
  done
  return 1
}

install_entry() {  # $1 = source path, $2 = destination path
  if [ -e "$2" ]; then
    mkdir -p "$(dirname "$BACKUP/${2#$DEST/}")"
    mv "$2" "$BACKUP/${2#$DEST/}"
  fi
  mkdir -p "$(dirname "$2")"
  cp -R "$1" "$2"
}

backup_once() {  # $1 = 1 when the file existed before this run, $2 = the file; copies it once per run
  if [ "$1" = "1" ] && [ ! -e "$BACKUP/$(basename "$2")" ]; then
    mkdir -p "$BACKUP"
    cp "$2" "$BACKUP/$(basename "$2")"
  fi
}

# Settings: a fragment from settings/ is merged into $DEST/settings.json. Top-level values are set (env per variable);
# a hook group is added unless every command in it is already registered for that event.
merge_settings() {  # $1 = fragment name (settings/<name>.json)
  local fragment="$SRC/settings/$1.json"
  [ -f "$fragment" ] || return 0
  if command -v node >/dev/null 2>&1; then
    local target="$DEST/settings.json"
    backup_once "$HAD_SETTINGS" "$target"
    mkdir -p "$DEST"
    node - "$fragment" "$target" <<'EOF'
const fs = require('fs');
const [fragment, target] = process.argv.slice(2);
const add = JSON.parse(fs.readFileSync(fragment, 'utf8'));
const cur = fs.existsSync(target) ? JSON.parse(fs.readFileSync(target, 'utf8')) : {};
for (const [key, value] of Object.entries(add)) if (key !== 'hooks' && key !== 'env') cur[key] = value;
if (add.env) cur.env = { ...cur.env, ...add.env };
for (const [event, groups] of Object.entries(add.hooks || {})) {
  cur.hooks ??= {};
  cur.hooks[event] ??= [];
  const have = new Set(cur.hooks[event].flatMap((g) => (g.hooks || []).map((h) => h.command)));
  for (const g of groups) if (!g.hooks.every((h) => have.has(h.command))) cur.hooks[event].push(g);
}
fs.writeFileSync(target, JSON.stringify(cur, null, 2) + '\n');
EOF
    echo "  settings  settings/$1.json -> settings.json"
  else
    echo "node not found: settings/$1.json was not merged into $DEST/settings.json" >&2
  fi
}

# Always-on rules: claude-md/<name>.md is kept in $DEST/CLAUDE.md between its own markers.
# The rest of that file is left alone; the text between the markers is replaced on every install.
write_block() {  # $1 = block name (claude-md/<name>.md)
  local name="$1"
  local block="$SRC/claude-md/$name.md"
  [ -e "$block" ] || return 0
  local target="$DEST/CLAUDE.md"
  local begin="<!-- skills-repo: $name -->"
  local end="<!-- /skills-repo: $name -->"
  backup_once "$HAD_CLAUDE_MD" "$target"
  mkdir -p "$DEST"
  tmp="$(mktemp)"
  if [ -f "$target" ] && grep -qxF "$begin" "$target"; then
    if ! grep -qxF "$end" "$target"; then
      echo "$target has \"$begin\" without \"$end\"; fix it by hand and run again." >&2
      exit 1
    fi
    awk -v b="$begin" -v e="$end" -v f="$block" '
      $0 == b { print; while ((getline line < f) > 0) print line; skip = 1; next }
      $0 == e { skip = 0 }
      !skip { print }' "$target" > "$tmp"
  else
    if [ -s "$target" ]; then cat "$target"; printf '\n'; fi > "$tmp"
    { printf '%s\n' "$begin"; cat "$block"; printf '%s\n' "$end"; } >> "$tmp"
  fi
  cat "$tmp" > "$target"
  rm -f "$tmp"
  echo "  block  $name -> CLAUDE.md"
}

install_skills() {
  local skill agent name
  mkdir -p "$DEST/skills" "$DEST/agents"
  for skill in "$SRC"/skills/*/; do
    name="$(basename "$skill")"
    install_entry "${skill%/}" "$DEST/skills/$name"
    echo "  skill  $name"
  done
  for agent in "$SRC"/agents/*.md; do
    name="$(basename "$agent")"
    install_entry "$agent" "$DEST/agents/$name"
    echo "  agent  $name"
  done
  find "$DEST/skills" "$DEST/agents" -name '.DS_Store' -delete 2>/dev/null || true
  write_block web-pages
  merge_settings skills
}

install_hook() {  # $1 = hook file name
  install_entry "$SRC/hooks/$1" "$DEST/hooks/$1"
  echo "  hook   $1"
}

install_subagent() {
  install_hook subagent-context-guard.mjs
  write_block subagent-models
  merge_settings subagent
}

install_vietnamese() {
  install_hook reply-language.mjs
  write_block language
  merge_settings vietnamese
}

install_compact() {
  install_entry "$SRC/mods/main-context-compact" "$DEST/mods/main-context-compact"
  echo "  mod    main-context-compact"
  find "$DEST/mods" -name '.DS_Store' -delete 2>/dev/null || true
  merge_settings compact
}

install_org_skills() {
  merge_settings org-skills
}

run_external() {  # $1 = command to run (relative to the repo root)
  if [ "${SKILLS_INSTALL_DRY_EXTERNAL:-}" = "1" ]; then
    echo "  would run: $1"
  else
    (cd "$SRC" && bash "$1")
  fi
}

install_cmux() {
  run_external cmux/install.sh
}

install_hud() {
  if [ -n "${CLAUDE_HOME:-}" ] && [ "$CLAUDE_HOME" != "$HOME/.claude" ]; then
    echo "  warning: hud/install.sh writes to $HOME/.claude, not to CLAUDE_HOME ($CLAUDE_HOME)" >&2
  fi
  run_external hud/install.sh
}

# Pinned release path. Sets WIDGET_ERR and returns 1 on any failure (never relies on set -e).
WIDGET_TMP=""
WIDGET_DEST=""
WIDGET_OLD=""
WIDGET_MODE=""  # pinned | local | current | skipped | dry: which path ran, for the summary
WIDGET_REOPEN=0  # 1 while a widget we quit has not been reopened yet
widget_cleanup() {
  # An interrupt between the two mv calls leaves only the .old copy: put it back.
  if [ -n "$WIDGET_OLD" ] && [ -e "$WIDGET_OLD" ] && [ ! -e "$WIDGET_DEST" ]; then
    mv "$WIDGET_OLD" "$WIDGET_DEST" >/dev/null 2>&1 || true
  fi
  # An interrupt after we quit the widget must not leave it closed.
  if [ "$WIDGET_REOPEN" = 1 ] && [ -e "$WIDGET_DEST" ]; then
    WIDGET_REOPEN=0
    open "$WIDGET_DEST" >/dev/null 2>&1 || true
  fi
  if [ -n "$WIDGET_TMP" ]; then
    hdiutil detach -quiet -force "$WIDGET_TMP/mnt" >/dev/null 2>&1 || true
    rm -rf "$WIDGET_TMP"
    WIDGET_TMP=""
  fi
}

widget_fail() {
  WIDGET_ERR="$1"
  return 1
}

widget_version_of() {  # $1 = app bundle; prints the full version string or nothing
  plutil -extract CFBundleShortVersionString raw -o - "$1/Contents/Info.plist" 2>/dev/null || true
}

# Waits up to 5 s for the widget process to exit; returns 0 when it is gone.
widget_wait_exit() {
  local i
  for ((i = 0; i < 10; i++)); do
    pgrep -x ClaudeSessions >/dev/null 2>&1 || return 0
    sleep 0.5
  done
  ! pgrep -x ClaudeSessions >/dev/null 2>&1
}

widget_install_pinned() {  # $1 = lock file ("" when only SKILLS_WIDGET_DMG is set)
  local lock="$1" tag="" want_sha="" want="" dmg mnt app have dest new old
  dest="$HOME/Applications/ClaudeSessions.app"
  new="$HOME/Applications/.ClaudeSessions.app.new"
  old="$HOME/Applications/.ClaudeSessions.app.old"
  if [ -n "$lock" ]; then
    tag=$(sed -n 's/^tag=//p' "$lock" | head -n 1)
    want_sha=$(sed -n 's/^sha256=//p' "$lock" | head -n 1)
    case "$tag" in
      widget-v[0-9]*) ;;
      *) widget_fail "release.lock has no valid tag" || return 1 ;;
    esac
    case "$tag" in
      *[!0-9A-Za-z.+-]*) widget_fail "release.lock has no valid tag" || return 1 ;;
    esac
    case "$want_sha" in
      *[!0-9a-f]* | "") widget_fail "release.lock has no valid sha256" || return 1 ;;
    esac
    [ "${#want_sha}" = 64 ] || widget_fail "release.lock has no valid sha256" || return 1
    want="${tag#widget-v}"
    # The lock carries the version: nothing to download when it is already installed.
    if [ -z "${SKILLS_WIDGET_DMG:-}" ] && [ -d "$dest" ] && [ "$(widget_version_of "$dest")" = "$want" ]; then
      echo "  widget already $want"
      WIDGET_MODE=current
      return 0
    fi
  fi
  WIDGET_TMP=$(mktemp -d) || widget_fail "cannot create a temp dir" || return 1
  WIDGET_DEST="$dest"
  WIDGET_OLD="$old"
  trap widget_cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  dmg="$WIDGET_TMP/ClaudeSessions.dmg"
  mnt="$WIDGET_TMP/mnt"
  if [ -n "${SKILLS_WIDGET_DMG:-}" ]; then
    echo "  using local dmg $SKILLS_WIDGET_DMG: skipping download and checksum"
    cp "$SKILLS_WIDGET_DMG" "$dmg" || widget_fail "cannot read $SKILLS_WIDGET_DMG" || return 1
  else
    echo "  downloading $tag"
    curl --fail --silent --show-error --proto '=https' --proto-redir '=https' --tlsv1.2 -L -o "$dmg" \
      "https://github.com/dphong1995/skills/releases/download/$tag/ClaudeSessions.dmg" \
      || widget_fail "download of $tag failed" || return 1
    have=$(shasum -a 256 "$dmg" | awk '{print $1}')
    [ "$have" = "$want_sha" ] || widget_fail "checksum mismatch for $tag (expected $want_sha, got $have)" || return 1
  fi
  mkdir -p "$mnt"
  hdiutil attach -quiet -nobrowse -readonly -noautoopen -mountpoint "$mnt" "$dmg" >/dev/null \
    || widget_fail "cannot mount the dmg" || return 1
  app="$mnt/ClaudeSessions.app"
  [ -d "$app" ] || widget_fail "the dmg has no ClaudeSessions.app" || return 1
  codesign --verify --deep --strict "$app" >/dev/null 2>&1 || widget_fail "app signature check failed" || return 1
  [ "$(plutil -extract CFBundleIdentifier raw -o - "$app/Contents/Info.plist" 2>/dev/null)" = "local.claude-sessions" ] \
    || widget_fail "unexpected bundle id in the dmg" || return 1
  [ -n "$want" ] || want=$(widget_version_of "$app")
  have=""
  if [ -d "$dest" ]; then have=$(widget_version_of "$dest"); fi
  if [ -n "$want" ] && [ "$have" = "$want" ]; then
    echo "  widget already $want"
    WIDGET_MODE=current
    return 0
  fi
  rm -rf "$new"
  ditto "$app" "$new" || widget_fail "cannot copy the app into ~/Applications" || return 1
  codesign --verify --deep --strict "$new" >/dev/null 2>&1 \
    || { rm -rf "$new"; widget_fail "copied app signature check failed" || return 1; }
  if pgrep -x ClaudeSessions >/dev/null 2>&1; then
    WIDGET_REOPEN=1
    osascript -e 'quit app "ClaudeSessions"' >/dev/null 2>&1 || true
    if ! widget_wait_exit; then
      pkill -x ClaudeSessions >/dev/null 2>&1 || true
      if ! widget_wait_exit; then pkill -9 -x ClaudeSessions >/dev/null 2>&1 || true; widget_wait_exit || true; fi
    fi
  fi
  rm -rf "$old"
  if [ -e "$dest" ]; then
    if ! mv "$dest" "$old"; then
      rm -rf "$new"
      widget_cleanup
      widget_fail "cannot move the old app aside" || return 1
    fi
  fi
  if ! mv "$new" "$dest"; then
    rm -rf "$new"
    if [ -e "$old" ]; then mv "$old" "$dest" || true; fi
    widget_cleanup
    widget_fail "cannot move the new app into place" || return 1
  fi
  rm -rf "$old"
  WIDGET_REOPEN=0
  open "$dest" || true
  echo "  widget ${want:-installed}"
  if [ -n "${SKILLS_WIDGET_DMG:-}" ]; then WIDGET_MODE=local; else WIDGET_MODE=pinned; fi
  if [ -d /Applications/ClaudeSessions.app ]; then
    echo "  warning: /Applications/ClaudeSessions.app also exists; remove one of the two copies" >&2
  fi
  return 0
}

install_widget() {
  local lock="${SKILLS_WIDGET_LOCK:-$SRC/claude-sessions/release.lock}"
  if [ "$(uname -s)" != "Darwin" ]; then
    echo "  widget skipped: needs macOS"
    SUMMARY_SKIPPED="$SUMMARY_SKIPPED  widget: needs macOS"$'\n'
    WIDGET_MODE=skipped
    return 0
  fi
  if [ -f "$lock" ] || [ -n "${SKILLS_WIDGET_DMG:-}" ]; then
    [ -f "$lock" ] || lock=""
    if [ "${SKILLS_INSTALL_DRY_EXTERNAL:-}" = "1" ]; then
      if [ -n "$lock" ]; then
        echo "  would install pinned $(sed -n 's/^tag=//p' "$lock" | head -n 1)"
      else
        echo "  would install pinned local dmg"
      fi
      WIDGET_MODE=dry
      return 0
    fi
    WIDGET_ERR=""
    if ! widget_install_pinned "$lock"; then
      widget_cleanup
      echo "  widget skipped: $WIDGET_ERR"
      SUMMARY_SKIPPED="$SUMMARY_SKIPPED  widget: $WIDGET_ERR"$'\n'
      WIDGET_MODE=skipped
    fi
    widget_cleanup
    trap - EXIT INT TERM
    return 0
  fi
  # No release.lock and no local dmg: nothing to install.
  echo "  widget skipped: no release pinned"
  SUMMARY_SKIPPED="$SUMMARY_SKIPPED  widget: no release pinned"$'\n'
  WIDGET_MODE=skipped
  return 0
}

print_list() {
  local i mark
  for ((i = 0; i < COUNT; i++)); do
    mark=""
    if [ "${DEFAULTS[$i]}" = "1" ]; then mark="  [default]"; fi
    printf '%-11s %s%s\n' "${ITEMS[$i]}" "${DESCS[$i]}" "$mark"
  done
}

read_selection_file() {  # sets SEL[i]=1 for each known name in the file; warns on unknown ones
  local line idx
  [ -f "$SELECTION" ] || return 0
  while IFS= read -r line || [ -n "$line" ]; do
    [ -n "$line" ] || continue
    if idx="$(index_of "$line")"; then
      SEL[$idx]=1
    else
      echo "warning: unknown item \"$line\" in $SELECTION skipped" >&2
    fi
  done < "$SELECTION"
}

any_selected() {
  local i
  for ((i = 0; i < COUNT; i++)); do
    if [ "${SEL[$i]}" = "1" ]; then return 0; fi
  done
  return 1
}

run_menu() {
  local i line tok mark n
  SEL=("${DEFAULTS[@]}")
  while true; do
    echo "Pick the items to install (type numbers to toggle, a = all, Enter = install, q = quit):"
    for ((i = 0; i < COUNT; i++)); do
      mark=" "
      if [ "${SEL[$i]}" = "1" ]; then mark="x"; fi
      printf '  [%s] %d  %-11s %s\n' "$mark" "$((i + 1))" "${ITEMS[$i]}" "${DESCS[$i]}"
    done
    printf '> '
    if ! IFS= read -r line; then echo; exit 0; fi
    line="${line//,/ }"
    if [ -z "${line// /}" ]; then
      if any_selected; then return 0; fi
      echo "Nothing is ticked."
      continue
    fi
    set -f
    for tok in $line; do
      case "$tok" in
        q|Q) exit 0 ;;
        a|A) for ((i = 0; i < COUNT; i++)); do SEL[$i]=1; done ;;
        *[!0-9]*|"") echo "Ignored: $tok" ;;
        *)
          n=$((10#$tok))
          if [ "$n" -ge 1 ] && [ "$n" -le "$COUNT" ]; then
            if [ "${SEL[$((n - 1))]}" = "1" ]; then SEL[$((n - 1))]=0; else SEL[$((n - 1))]=1; fi
          else
            echo "Ignored: $tok"
          fi
          ;;
      esac
    done
    set +f
  done
}

# ---- flags ----
MODE=""
ONLY=""
set_mode() {
  if [ -n "$MODE" ]; then die_usage "Use only one of --list, --only, --all, --update."; fi
  MODE="$1"
}
while [ $# -gt 0 ]; do
  case "$1" in
    --list) set_mode list ;;
    --all) set_mode all ;;
    --update) set_mode update ;;
    -h|--help) usage; exit 0 ;;
    --only)
      set_mode only
      [ $# -ge 2 ] || die_usage "--only needs a comma-separated item list."
      ONLY="$2"
      shift
      ;;
    *) die_usage "Unknown option: $1" ;;
  esac
  shift
done

if [ "$MODE" = "list" ]; then print_list; exit 0; fi

SEL=()
for ((i = 0; i < COUNT; i++)); do SEL[$i]=0; done

case "$MODE" in
  only)
    ONLY="${ONLY//[[:space:]]/}"
    old_ifs="$IFS"; IFS=','
    set -f; names=($ONLY); set +f
    IFS="$old_ifs"
    bad=""
    for name in ${names[@]+"${names[@]}"}; do
      [ -n "$name" ] || continue
      if idx="$(index_of "$name")"; then SEL[$idx]=1; else bad="$bad \"$name\""; fi
    done
    if [ -n "$bad" ] || ! any_selected; then
      die_usage "Unknown item(s):${bad:- (none given)}"
    fi
    ;;
  all)
    for ((i = 0; i < COUNT; i++)); do SEL[$i]=1; done
    ;;
  update)
    read_selection_file
    if ! any_selected; then
      if [ -t 0 ]; then
        echo "No remembered selection; pick items now."
        run_menu
      else
        die_usage "--update needs a remembered selection in $SELECTION."
      fi
    fi
    ;;
  *)
    if [ -t 0 ]; then
      run_menu
    else
      echo "No terminal for the menu: pass --only, --all or --update." >&2
      usage >&2
      exit 2
    fi
    ;;
esac

# ---- run ----
mkdir -p "$DEST"
for ((i = 0; i < COUNT; i++)); do
  if [ "${SEL[$i]}" = "1" ]; then
    echo "== ${ITEMS[$i]}"
    "install_${ITEMS[$i]//-/_}"
    SUMMARY_INSTALLED="$SUMMARY_INSTALLED ${ITEMS[$i]}"
  fi
done

# ---- remember ----
# menu: overwrite; --only: union with the file; --all / --update: this run's set
if [ "$MODE" = "only" ]; then read_selection_file; fi
: > "$SELECTION"
for ((i = 0; i < COUNT; i++)); do
  if [ "${SEL[$i]}" = "1" ]; then echo "${ITEMS[$i]}" >> "$SELECTION"; fi
done

echo
echo "Installed into $DEST"
echo "Items:$SUMMARY_INSTALLED"
if [ -n "$SUMMARY_SKIPPED" ]; then
  echo "Skipped:"
  printf '%s' "$SUMMARY_SKIPPED"
fi
if [ -d "$BACKUP" ]; then echo "Replaced files backed up to $BACKUP"; fi
echo "Restart Claude Code (or run /doctor) to pick them up."
case "$SUMMARY_INSTALLED" in
  *" widget"*)
    case "$WIDGET_MODE" in
      pinned) echo "The installer verified the pinned widget release, then quit the running widget and reopened the new one." ;;
      local) echo "The installer installed the widget from the local dmg (no checksum), then quit the running widget and reopened the new one." ;;
      current)
        if [ -n "${SKILLS_WIDGET_DMG:-}" ]; then
          echo "The widget in the local dmg is already installed; the running widget was left alone."
        else
          echo "The pinned widget release is already installed; the running widget was left alone."
        fi
        ;;
      skipped) echo "The widget install was skipped; the installed widget (if any) is unchanged." ;;
      dry) echo "Dry run: nothing was downloaded, built, quit or reopened; the installed widget (if any) is unchanged." ;;
    esac
    ;;
esac
