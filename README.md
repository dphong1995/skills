# Claude Code skills — brainstorm-pro / ck-plan / cook / routing-bug-fixes-to-cook

Bộ skill thiết kế → lập kế hoạch → triển khai cho Claude Code, kèm các subagent mà chúng dispatch.

## Cài trên máy mới

```bash
git clone git@github.com:dphong1995/skills.git ~/.claude/skills-repo
bash ~/.claude/skills-repo/install.sh
```

`install.sh` copy nội dung vào `~/.claude/skills/`, `~/.claude/agents/`, `~/.claude/hooks/` và `~/.claude/mods/`, ghi các
khối quy tắc `claude-md/` vào `~/.claude/CLAUDE.md` (xem mục "Quy tắc luôn bật") và trộn
`settings/settings.json` vào `~/.claude/settings.json` (xem mục "Settings và hook"). Vị trí clone không quan
trọng (script tự dò đường dẫn của chính nó) — `~/.claude/skills-repo` chỉ là mặc định gợi ý.

File cùng tên đang có sẵn sẽ được **chuyển vào `~/.claude/backups/skills-install-<timestamp>/`**
trước khi ghi đè, không bị xoá. Skill/agent khác không nằm trong repo này thì không bị đụng tới.

Khởi động lại Claude Code sau khi cài.

## Cập nhật

```bash
cd ~/.claude/skills-repo && git pull && bash install.sh
```

Đây là chế độ **copy**, không phải symlink — `git pull` xong phải chạy lại `install.sh` thì thay đổi
mới có hiệu lực.

## Quy tắc luôn bật trong `~/.claude/CLAUDE.md`

Mỗi file `claude-md/<tên>.md` là một khối quy tắc mà `install.sh` giữ trong `~/.claude/CLAUDE.md`
của máy, giữa hai dòng `<!-- skills-repo: <tên> -->` và `<!-- /skills-repo: <tên> -->`: chưa có
thì thêm vào cuối file, có rồi thì thay đúng đoạn giữa hai dòng đó; phần còn lại của file giữ
nguyên, và file cũ được sao lưu vào `~/.claude/backups/skills-install-<timestamp>/` trước khi ghi.
Khối nằm trong CLAUDE.md riêng của máy, không nằm trong repo dự án nào, nên không ảnh hưởng ai pull
code về. Sửa quy tắc trong repo (`claude-md/`) chứ không sửa trong CLAUDE.md: lần cài sau sẽ ghi đè
đoạn giữa hai dòng đánh dấu.

- `web-pages.md`: câu hỏi cho người dùng đi trang phỏng vấn HTML; file Markdown đưa người dùng đọc
  hoặc duyệt (spec, thiết kế của cook, plan, draft ticket, báo cáo) mở bằng trang web dựng từ file
  md (`render-interview.mjs <file.md> … --serve | --open`).
- `language.md`: mọi tin nhắn cho người dùng viết bằng tiếng Việt (cả dòng báo tiến độ, tin nhắn
  sau compact, khi chuyển tiếp tài liệu tiếng Anh); tiếng Anh chỉ ở memory, commit, prompt cho
  subagent, nội dung cho Jira/PR.
- `subagent-models.md`: subagent thực thi (executor, test-engineer, debugger, code-simplifier, writer,
  git-master, general-purpose làm việc) luôn chạy `model: "sonnet"`; subagent chỉ review (red-team,
  code-reviewer của cook, critic, architect, verifier, security-reviewer) chạy `model: "opus"`, kể cả
  khi skill ghi model khác. Báo cáo bắt đầu bằng `PARTIAL` thì giao phần còn lại cho agent mới (không
  `SendMessage` agent cũ); `PARTIAL` của `fork` giao cho agent không phải fork, kèm brief viết sẵn.

## Settings và hook

`settings/settings.json` được trộn vào `~/.claude/settings.json` (cần `node`): khoá ở cấp cao nhất
được ghi đè (riêng `env` được trộn từng biến, biến khác của máy giữ nguyên), một nhóm hook được thêm nếu lệnh của nó chưa có cho event đó; phần còn lại của file giữ
nguyên, file cũ được sao lưu trước. Chạy lại `install.sh` không tạo hook trùng.

- `"language": "vietnamese"`: setting có sẵn của Claude Code, đưa ngôn ngữ trả lời vào system prompt.
- `env.CLAUDE_CODE_ENABLE_TODO_TOOLS: "1"`: bật các tool todo/task của Claude Code (`TaskCreate`, `TaskUpdate`,
  ...), mà các skill dùng để theo dõi từng mục checklist trong phiên.
- Hook `Stop` chạy `hooks/reply-language.mjs` (copy vào `~/.claude/hooks/`): khi câu trả lời cuối là
  văn xuôi tiếng Anh (bỏ qua code, blockquote, đường dẫn; câu dưới 8 chữ bỏ qua), nó chặn một lần
  để Claude viết lại bằng tiếng Việt. Tắt bằng `/hooks` hoặc xoá nhóm hook đó trong
  `~/.claude/settings.json`.
- Hook `PostToolUse` chạy `hooks/subagent-context-guard.mjs`: khi context của một subagent vượt ngưỡng
  (Sonnet 200k, Opus 300k) nó nhắc đúng một lần để subagent làm xong bước đang dở rồi trả báo cáo bắt
  đầu bằng `PARTIAL`; orchestrator giao phần còn lại cho agent mới. Main thread không bị đụng tới. Tắt
  bằng `/hooks` hoặc xoá nhóm hook đó trong `~/.claude/settings.json`, và phải xoá luôn khỏi
  `settings/settings.json` của repo, vì `install.sh` chỉ thêm nhóm hook chứ không bao giờ xoá, nên chạy
  lại sẽ thêm nó trở lại. Test: `node --test 'hooks/tests/*.test.mjs'` (dạng thư mục lỗi trên Node 24).
- `"skillOverrides"`: tắt 38 skill `anthropic-skills:*` của org không dùng cho code (marketing, sales,
  RFP, regulatory, ...), vì danh sách skill được nhét vào context của mọi agent; danh sách rút từ khoảng
  30k xuống khoảng 21k ký tự, tức khoảng 2,3k token mỗi lượt gọi.
  Giữ lại 10 skill: `built-in-browser`, `chrome-browser`, `deep-research`, `docs`, `docx`,
  `google-workspace`, `pdf`, `pptx`, `skill-creator`, `xlsx`. Bật lại một skill bằng cách xoá dòng của nó
  trong `settings/settings.json` của repo rồi chạy `install.sh` (không sửa ở `~/.claude/settings.json`
  vì `install.sh` ghi đè khoá `skillOverrides`).
- Mod `mods/main-context-compact` (nạp qua `env.CLAUDE_CODE_PLUGIN_DIRS`): tự compact context của main
  thread. Các ngưỡng so với con số HUD/status line (input + cache token của API), cao hơn `/context`
  khoảng 15–20k. Hết lượt mà context ≥ 300k thì compact (bỏ qua nếu bạn tự ngắt lượt); trước một request
  của main mà context ≥ 400k thì dừng lượt (kèm một dòng tiếng Việt), compact rồi gửi prompt CONTINUE để
  chạy tiếp (không gửi nếu giữa chừng bạn đã bắt đầu lượt mới); nếu ngay sau lần compact đó context vẫn
  ≥ 300k thì việc dừng lượt ở 400k bị tắt cho tới lần compact thành công kế tiếp của mod; hết lượt ở
  ≥ 200k mà bạn không gửi gì trong 55 phút thì compact trước khi cache 1 giờ hết hạn. Chỉ áp cho main
  thread, subagent vẫn dùng `subagent-context-guard`; phiên headless (`claude -p`) không bị đụng tới.
  Đổi các con số ở `/config` →
  main-context-compact (lưu trong `pluginConfigs` của `~/.claude/settings.json`). Tắt: xoá
  `CLAUDE_CODE_PLUGIN_DIRS` trong `~/.claude/settings.json` và trong `settings/settings.json` của repo,
  nếu không lần `install.sh` sau sẽ đặt lại. Muốn nạp thêm thư mục plugin khác thì thêm
  vào `env.CLAUDE_CODE_PLUGIN_DIRS` trong `settings/settings.json` của repo, ngăn cách bằng `:`, rồi chạy
  `install.sh` (giá trị chỉ sửa ở `~/.claude/settings.json` sẽ bị thay ở lần cài sau). Sau mỗi lần cập nhật
  Claude Code chạy `claude plugin validate ~/.claude/mods/main-context-compact` và
  `claude plugin test ~/.claude/skills-repo/mods/main-context-compact`.

## Đẩy thay đổi ngược lên repo

Nếu sửa skill trực tiếp ở `~/.claude/skills/`, copy ngược lại repo rồi commit:

```bash
cd ~/.claude/skills-repo
rsync -a --exclude '.DS_Store' ~/.claude/skills/{brainstorm-pro,ck-plan,cook,routing-bug-fixes-to-cook} skills/
rsync -a --exclude '.DS_Store' ~/.claude/agents/ agents/
git add -A && git commit -m "..." && git push
```

## Nội dung

```
skills/
├── brainstorm-pro/   Thiết kế feature: deep scout → Q&A từng câu một trên trang HTML cục bộ → viết spec theo
│                     section → user duyệt trên trang web (góp ý từng mục; phím tắt trên các trang, `?` liệt kê)
│                     → gọi ck-plan. Phase 1 lưu snapshot Jira (Story/Epic) và lập bảng truy vết AC; Phase 2 có thẻ phát lại, Phase 5 có builder check. Luồng thiết kế nằm ở `references/design-flow.md`, dùng chung với cook. Gồm `assets/render-interview.mjs` + `assets/lib/` + `assets/page/`,
│                     `tests/`, `DEVELOPING.md`
├── ck-plan/          Lập kế hoạch: research → phân tích codebase → plan.md + phase-XX-*.md (mẫu và luật viết trong `references/plan-writing.md`; Global Constraints, Review Focus, Interfaces; tự kiểm plan)
│                     → tự động gọi cook
├── cook/             Triển khai: mỗi phase một subagent, orchestrator commit sau mỗi phase → test → review theo kích thước diff (diff nhỏ 1 reviewer, diff lớn 2) → finalize, hỏi push
└── routing-bug-fixes-to-cook/
                      Cổng sau khi debug xong root cause: fix ≥ 3 file production, > 50 dòng
                      production (đếm gộp các repo) hoặc có nhiều cách sửa → gọi cook (Step 1 rút
                      gọn); nhỏ hơn → sửa thẳng

agents/               19 subagent definition mà cook/ck-plan gọi tới; tất cả đều chặn tool `Artifact` (`disallowedTools`)

hooks/                reply-language.mjs (Stop), subagent-context-guard.mjs (PostToolUse) và `tests/` của hook
                      (xem mục Settings và hook bên trên)

mods/                 main-context-compact: mod tự compact context của main thread (xem mục Settings và hook)

claude-md/            Khối quy tắc luôn bật, install.sh giữ trong ~/.claude/CLAUDE.md (xem mục bên trên)

cmux/                 Settings của cmux (không thuộc install.sh ở trên, xem mục cmux bên dưới)

hud/                  Statusline OMC HUD (không thuộc install.sh ở trên, xem mục OMC HUD bên dưới)

claude-sessions/      Widget macOS hiện các session Claude Code đang chạy (không thuộc install.sh ở trên, xem mục
                      Widget Claude sessions bên dưới)
```

Chuỗi thiết kế: `brainstorm-pro` → `ck-plan` → `cook` chạy liền mạch, không dừng hỏi ở giữa (trừ
các gate tương tác trong phase design của brainstorm-pro/cook; từ cook Step 2 trở đi chỉ dừng cho hai câu hỏi trên trang interview: câu NEEDS_CONTEXT khó đổi lại ở Step 3 và câu hỏi push ở Step 6).

## Ràng buộc cần biết

- **3 skill phải nằm cạnh nhau trong cùng một thư mục.** `cook/SKILL.md` đọc
  `../ck-plan/references/plan-writing.md`, `../ck-plan/references/plan-organization.md` và `../ck-plan/references/output-standards.md` bằng
  đường dẫn tương đối để lấy format plan. Tách ra là hỏng.
- **`agents/` là dependency thật, không phải tuỳ chọn.** `cook` dispatch theo tên:
  `executor`, `code-reviewer`, `test-engineer`, `code-simplifier`, `writer`,
  `designer`, `debugger`, `explore`, `document-specialist`. Thiếu thì cook vẫn chạy
  nhưng phải fallback sang agent mặc định và chất lượng giảm.
- **Không có CLI `ck`.** Tên `ck-plan` chỉ là di sản đặt tên; mọi thứ chạy trong skill.
- **`routing-bug-fixes-to-cook` cần 1 dòng trong `~/.claude/CLAUDE.md`** để được gọi đúng lúc (thêm
  thủ công):
  ```
  - Bug fix: once the root cause is confirmed, invoke the `routing-bug-fixes-to-cook` skill before writing the fix.
  ```
  Skill này dùng chế độ Step 1 rút gọn của `cook`, nên phải cài cùng bản `cook` trong repo.
- `install.sh` merge `settings/settings.json` vào `~/.claude/settings.json` (`env` theo từng biến, các
  key khác bị ghi đè, nhóm hook được thêm vào), ghi các khối trong `claude-md/`, cài `mods/` và đặt
  `CLAUDE_CODE_PLUGIN_DIRS`, nhưng không đụng tới plugin từ marketplace (`enabledPlugins`). Riêng
  `hud/install.sh` ghi 2 key `statusLine`, `omcHud`.

## cmux

cmux không tự sync settings giữa các máy, nên repo giữ bản copy của 2 file:

| File trong repo | Vị trí trên máy |
|---|---|
| `cmux/cmux.json` | `~/.config/cmux/cmux.json` — settings, shortcuts |
| `cmux/config.ghostty` | `~/Library/Application Support/com.cmuxterm.app/config.ghostty` — config Ghostty mà cửa sổ Settings của cmux ghi (font, cursor…) |

Cài trên máy mới (file đang có được giữ lại dạng `<file>.<timestamp>.bak`):

```bash
bash ~/.claude/skills-repo/cmux/install.sh
```

Đẩy thay đổi ngược lên repo:

```bash
cd ~/.claude/skills-repo
cp ~/.config/cmux/cmux.json "$HOME/Library/Application Support/com.cmuxterm.app/config.ghostty" cmux/
git add cmux && git commit -m "..." && git push
```

Setting chỉnh trong cửa sổ Settings mà **không có trong `cmux.json`** thì nằm trong plist của macOS
(`com.cmuxterm.app`), không đi theo repo. Muốn sync thì ghi nó vào `cmux.json`:
`cmux config set <setting.path> <value>` (`cmux config get <path>` cho biết giá trị đang lấy từ đâu).
Giá trị trong `cmux.json` đè lên giá trị trong cửa sổ Settings.

`cmux config doctor` báo lỗi ở các shortcut dạng object (`{"first": {...}}`). Dạng này do cửa sổ
Settings tự ghi và app vẫn đọc được, chỉ schema của doctor là không chấp nhận.

## OMC HUD

Statusline của plugin oh-my-claudecode (OMC). Repo mang theo runtime HUD đã tách khỏi plugin, nên máy mới
không cần cài plugin OMC (Claude Code phải đã đăng nhập để lấy rate limit). Repo giữ:

| File trong repo | Vị trí trên máy |
|---|---|
| `hud/omc-hud.mjs` | `~/.claude/hud/omc-hud.mjs` — wrapper gọi HUD (bản vendored trước, plugin cache sau), ẩn các bucket `sn:`/`fable:`/`extra:` |
| `hud/omc/` | `~/.claude/hud/omc/` — runtime HUD của OMC, phiên bản ghi trong `hud/omc/package.json`, tạo bằng `hud/vendor.sh`, không sửa tay |
| `hud/vendor.sh` | tạo lại `hud/omc/` từ plugin cache (bản mới nhất đã build, hoặc bản truyền vào) |
| `hud/PATCH-NOTES.md` | `~/.claude/hud/PATCH-NOTES.md` — các patch chỉ còn cần khi HUD rơi về plugin cache |
| `hud/settings.json` | key `statusLine` và `omcHud` của `~/.claude/settings.json` (layout, element bật/tắt) |

Cài trên máy mới (file đang có được giữ lại dạng `<file>.<timestamp>.bak`, thư mục `omc` cũ thành
`omc.<timestamp>.bak`; `settings.json` chỉ bị thay 2 key trên, các key khác giữ nguyên):

```bash
bash ~/.claude/skills-repo/hud/install.sh
```

Lên bản OMC mới: cập nhật plugin trên một máy có nó, rồi

```bash
cd ~/.claude/skills-repo
bash hud/vendor.sh        # tạo lại hud/omc từ bản plugin mới nhất
# kiểm tra output giống hệt plugin cache (diff phải rỗng):
IN='{"cwd":"'$PWD'","model":{"id":"claude-opus-5-5","display_name":"Opus 5.5"},"transcript_path":"/nonexistent"}'
V=$(node -p "require('./hud/omc/package.json').version")
diff <(echo "$IN" | node -e "import('$HOME/.claude/plugins/cache/omc/oh-my-claudecode/$V/dist/hud/index.js')" 2>&1) \
     <(echo "$IN" | node -e "import('$PWD/hud/omc/dist/hud/index.js')" 2>&1)
bash hud/install.sh
git add hud/omc && git commit -m "Vendor OMC HUD <version>"
```

Nếu hai output chỉ khác nhau ở số phút đếm ngược, chạy lại phần kiểm tra.

Đẩy thay đổi ngược lên repo:

```bash
cd ~/.claude/skills-repo
cp ~/.claude/hud/omc-hud.mjs ~/.claude/hud/PATCH-NOTES.md hud/
node -e 'const s=require(process.env.HOME+"/.claude/settings.json");console.log(JSON.stringify({statusLine:s.statusLine,omcHud:s.omcHud},null,2))' > hud/settings.json
git add hud && git commit -m "..." && git push
```

## Widget Claude sessions

App macOS nhỏ (SwiftUI, Swift package), hiện các session Claude Code đang mở trên máy trong một
widget nổi luôn nằm trên cùng, kể cả trên app toàn màn hình (YouTube fullscreen).

- Đọc `~/.claude/sessions/*.json` mỗi giây, file mà mỗi session Claude Code tự ghi: `status` là `busy`
  (đang chạy), `waiting` (`waitingFor`: `permission prompt` → chờ duyệt, còn lại → chờ trả lời) hoặc
  `idle` (xong, chờ bạn). Bỏ qua tiến trình đã chết. Không cần hook hay mod.
- Thu gọn: viên đen gồm chấm trạng thái + số session theo màu (đỏ chờ, cam xong, xanh đang chạy).
- Rê chuột vào thì mở danh sách (tên, thư mục, trạng thái, thời gian). Session vừa chuyển từ đang chạy
  sang xong/chờ thì widget tự mở 5 giây và có viền cam 20 giây.
- Kéo để đổi vị trí (nhớ vị trí). Hướng mở theo vị trí: nửa trên mở xuống, nửa dưới mở lên; phần ba
  trái/phải mở sang phải/trái. Chuột phải → Thoát.

Cài (cần Command Line Tools: `xcode-select --install`), build ra `~/Applications/ClaudeSessions.app`:

```bash
bash ~/.claude/skills-repo/claude-sessions/build.sh && open ~/Applications/ClaudeSessions.app
```

Muốn tự chạy khi đăng nhập: System Settings → General → Login Items → thêm `ClaudeSessions.app`.

Test trạng thái (logic + ảnh PNG của từng trạng thái, không cần quyền Screen Recording):

```bash
cd ~/.claude/skills-repo/claude-sessions && swift test   # ảnh PNG nằm trong .build/snapshots/
```

Đẩy thay đổi ngược lên repo (bản đang dùng nằm ở `~/.claude/tools/claude-sessions/`):

```bash
cd ~/.claude/skills-repo
rsync -a --exclude .build ~/.claude/tools/claude-sessions/ claude-sessions/
git add claude-sessions && git commit -m "..." && git push
```
