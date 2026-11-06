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
  `busy` mà lượt chính đã kết thúc còn agent nền đang chạy (dòng `turn_duration` cuối của transcript có `pendingBackgroundAgentCount`/`pendingWorkflowCount`) thì hiện `chờ N agent nền` với màu `xong`, tính vào nhóm `xong` và hiện câu trả lời cuối như dòng `xong`; có dòng user/assistant chính mới thì lại là `đang chạy`.
- Thu gọn: viên nhỏ rộng 126 pt, không có chữ "Claude", chỉ có chấm trạng thái + số session theo màu (đỏ chờ, cam xong, xanh đang chạy); khi có session có tin mới thì nhóm vàng `✦ N` hiện đầu tiên.
- Rê chuột vào thì mở danh sách rộng 760 pt (header chỉ có các nhóm `● n` bên phải; mỗi dòng có tiêu đề, `thư mục · nhánh`, chấm màu trạng thái, thời gian). Session vừa chuyển từ đang chạy
  sang xong/chờ thì widget tự mở 5 giây.
- Dấu mới bật khi session chuyển từ đang chạy sang xong/chờ, hoặc khi PR của session có sự kiện. Mỗi session là một thẻ nền sáng nhạt, các thẻ cách nhau 4 pt. Dấu ở phần Claude là vạch vàng + nền vàng nhạt chỉ trên đầu thẻ; sự kiện PR chỉ tô tím nhạt đúng dòng PR đó, kèm `✦`; PR có dấu nhưng nằm trong `☰ N` thì nút đó có chấm tím. Dấu vẫn xoá theo cả session. Dấu mất khi bấm dòng / link / dòng PR, khi chuột dừng 1 giây trên dòng trong khung mở rộng (vạch, nền và `✦` mờ đi; lướt qua không tính), khi `Nhờ Claude`/duyệt/từ chối gõ thành công, khi session chạy lại, hoặc khi pane của nó được focus trong cmux qua 2 lần kiểm tra liền nhau (mỗi 2 giây, chỉ khi cmux ở trước và còn dấu, bằng `cmux identify`). Dấu không còn sau khi mở lại app. Khi cmux đang ở trước và pane được focus của nó chính là pane của session vừa chuyển trạng thái, dấu vẫn bật (rồi xoá như trên) nhưng không kêu và widget không tự mở 5 giây; sự kiện PR giữ nguyên; cmux lỗi, quá thời gian hoặc session ngoài cmux thì xử lý như bình thường.
- Tên dòng lấy theo thứ tự: tên đặt bằng `/rename` → tiêu đề AI → prompt đầu (bỏ `/clear`, lệnh không đối số,
  thẻ hệ thống; cắt 200 ký tự, hiện tối đa 2 dòng) → tên mặc định. Mẹo: gõ `/rename` để Claude tự đặt tên.
- Session có task (TaskCreate/TaskUpdate, `~/.claude/tasks/<sessionId>/`) thì hiện thêm dòng `☑ đã xong/tổng · việc đang làm`;
  xong hết thì ẩn. Có task đang làm thì hiện chữ của nó (không có chữ thì chỉ hiện số), không thì hiện task chờ đầu tiên.
- Session đang chạy hiện thêm dòng `▸ Bash: …` là tool Claude vừa gọi gần nhất (chỉ đọc phần transcript mới được ghi từ giây trước, tối đa 256 KB, không đọc cả file, chỉ phân tích dòng có `"tool_use"`); hết chạy thì dòng này biến mất.
- Dòng `chờ duyệt` hiện lệnh hoặc file đang xin duyệt (đọc từ transcript); nhiều tool cùng chờ hoặc tool
  lạ thì hiện "Cần duyệt trong cmux".
- Mỗi màn hình đang cắm có một viên riêng với cùng danh sách. Vị trí lưu theo từng màn hình (UUID màn hình, toạ độ tính từ màn hình đó; rút rồi cắm lại thì viên về đúng chỗ cũ); vị trí cũ của viên duy nhất (`pillX`/`pillY`/`pillWidth`) được chuyển một lần sang màn hình đang chứa viên đó. Rê chuột và ô gõ lý do từ chối chỉ tác động lên viên của màn hình đó; kéo chỉ dừng ở mép màn hình của viên đó. Tự mở 5 giây, tự ẩn 30 giây (bên dưới), âm báo và menu chuột phải dùng chung cho mọi viên.
- Kéo để đổi vị trí (nhớ vị trí sau khi mở lại; vị trí lưu từ viên cũ rộng 232 pt được dịch một lần để viên neo bên phải vẫn sát mép phải). Kéo sát mép màn hình hay thanh menu thì macOS không bắt vào tính năng chia cửa sổ (tiling); viên nằm đúng chỗ buông (kéo quá mép màn hình thì viên nằm sát mép đó); nếu hướng mở đổi thì khung đang mở dời lại quanh viên. Hướng mở theo vị trí: nửa trên mở xuống, nửa dưới mở lên; phần ba
  trái/phải mở sang phải/trái; phần ba giữa mở đều hai bên. Chuột phải → `Độ đậm nền` (0/25/50/75/92%, mặc định 25%, nhớ sau khi mở lại; chỉ áp cho viên thu gọn, khung danh sách luôn nền đậm), `Tắt tiếng` / `Bật tiếng`, `Prompt Nhờ Claude…`, hai dòng kết nối Bitbucket/Jira (bên dưới), `Thoát`.
- Âm báo: session vừa xong (chạy → xong) kêu `Glass`, session vừa chuyển sang chờ bạn (chạy → chờ) kêu `Funk`; mỗi giây tối đa một tiếng, `Funk` ưu tiên, lần quét đầu không kêu; bỏ qua session có pane đang được focus trong cmux (cmux ở trước, kiểm tra bằng `cmux identify` chỉ khi có chuyển trạng thái). Chuột phải → `Tắt tiếng` / `Bật tiếng`, lựa chọn được nhớ sau khi mở lại app.
- Dòng PR (cần mục Keychain `bitbucket-api-token`, tạo bằng `security add-generic-password -s bitbucket-api-token -a <email> -w`; không có mục này thì không có tính năng PR và widget chạy như cũ). PR của session gồm: link `https://bitbucket.org/<workspace>/<repo>/pull-requests/<n>` xuất hiện trong prompt hoặc câu trả lời của Claude (không tính output của tool, không tính output của lệnh `!` bash-mode và lệnh local `<bash-stdout>`, `<bash-stderr>`, `<local-command-stdout>`, `<local-command-stderr>`, không tính subagent), cộng các PR đang mở của bạn trên nhánh của cwd (giữ lại cả khi đổi nhánh). Tối đa 3 dòng mỗi session: PR từ link theo thứ tự xuất hiện, rồi PR theo nhánh. Nút `↗ PR #N` chỉ bị bỏ khi dòng PR tương ứng đang hiện; link `/pull-requests/12` và `/pull-requests/12/diff` chỉ cho một PR. Link tới PR trong workspace bạn không đọc được (403/404, ví dụ link mẫu) chỉ được thử một lần rồi bỏ qua, không làm hỏng việc theo dõi các PR khác.
- Mỗi dòng PR hiện `PR #n · ✗ k yêu cầu sửa · ✓ đã duyệt/reviewer · 🐰 <CodeRabbit> · 💬 số bình luận · <tên> <thời gian bình luận cuối của người khác>`; phần nào không áp dụng thì ẩn. Phần `🐰` chỉ hiện trên PR đang mở, một trong `🐰 đang review`, `🐰 N góp ý`, `🐰 chờ duyệt`, `🐰 đang nhắc`, `🐰 xong`, `🐰 đã duyệt`, `🐰 bỏ qua`, `🐰 lỗi`. `🐰 đã duyệt` lấy từ việc tài khoản CodeRabbit (tài khoản đăng các bình luận có marker) đã duyệt PR, không lấy từ build status. Tài khoản CodeRabbit đã duyệt thì luôn hiện `🐰 đã duyệt`, kể cả khi build status của CodeRabbit vẫn đang chạy. `🐰 chờ duyệt` là khi mọi góp ý inline của CodeRabbit đã được trả lời và CodeRabbit đáp sau câu trả lời đó (PR không có góp ý nào cũng tính), mà CodeRabbit chưa duyệt; `🐰 đang nhắc` là khi có bình luận `@coderabbitai approve` (hoặc `@coderabbit approve`, của bất kỳ ai) mới hơn bình luận cấp cao nhất mới nhất của CodeRabbit; trạng thái này không phải trạng thái cuối, không kêu, và kết thúc khi CodeRabbit đăng bình luận cấp cao nhất kế tiếp (trả lời inline không làm nó kết thúc). `🐰 đang review` kết thúc ngay khi CodeRabbit đăng bình luận tóm tắt (summary) sau lúc bắt đầu review, không chờ build status của CodeRabbit (status này thường trễ vài phút). Nút `🐰 Nhắc duyệt` chỉ hiện ở `🐰 chờ duyệt` (trước nút `Nhờ Claude`): bấm là đăng bình luận `@coderabbitai approve` lên PR bằng tên bạn, không hỏi xác nhận, rồi dòng chuyển sang `🐰 đang nhắc` và lượt hỏi Bitbucket kế tiếp chạy ngay; lỗi (ví dụ 403 do token thiếu quyền ghi) thì chỉ kêu beep và dòng giữ nguyên, bấm lần hai khi lần đầu còn đang gửi thì bị bỏ qua. `🐰 chờ duyệt` kêu `Glass` một lần. `/comments` được đọc tối đa 20 trang × 100 bình luận. Bình luận của CodeRabbit (có marker `coderabbit.ai/#html-comment`) không tính vào phần bình luận cuối của người khác. Bấm dòng PR mở PR trên trình duyệt. PR đã đóng khi mới thấy thì ẩn hẳn; PR đóng khi đang theo dõi giữ dòng `đã merge` / `đã đóng` đến hết session và không hỏi lại.
- PR có sự kiện mới (bình luận mới, thêm duyệt, thêm yêu cầu sửa, merge, đóng) thì widget tự mở 5 giây và kêu: bình luận, yêu cầu sửa, đóng kêu `Funk`; duyệt, merge kêu `Glass` (`Funk` ưu tiên). CodeRabbit review xong cũng là một sự kiện, kêu một lần cho mỗi lượt review: có góp ý hoặc lỗi kêu `Funk`, sạch hoặc đã duyệt kêu `Glass`; bình luận của CodeRabbit không kêu; review đã xong sẵn lúc widget mới thấy PR thì không kêu. Hỏi Bitbucket mỗi 45 giây (và ngay khi session có PR mới hoặc đổi nhánh); trạng thái CodeRabbit còn thiếu của commit mới (10 phút đầu) được hỏi mỗi lượt, sau đó tối đa 5 phút một lần (`🐰 đang review` vẫn hỏi mỗi lượt). `🐰 đã duyệt` được thấy mỗi lượt qua tài khoản CodeRabbit trong `participants`; PR còn mở mà CodeRabbit đã review xong (`🐰 N góp ý` / `🐰 xong`) thì trạng thái vẫn được hỏi lại tối đa 5 phút một lần, để thấy lỗi về sau. Quá 300 giây không lấy được danh sách PR đang mở của workspace nào (ví dụ mất VPN) thì mỗi dòng thêm `cũ <thời gian>` tính từ lần thành công cuối.
- Nút `Nhờ Claude` ở cuối mỗi dòng PR đang mở gõ prompt vào pane cmux của session, khi session đang `xong` hoặc `chờ N agent nền` và chạy trong cmux; nếu không thì nút bị làm mờ (không bấm được) với gợi ý `Chỉ gửi được khi Claude đã xong lượt`, và beep chỉ còn khi trạng thái đổi giữa lúc vẽ và lúc bấm, không gõ gì. Prompt mặc định: `Đọc các bình luận chưa xử lý và kết quả pipeline của PR {url}, sửa code theo đó, push, rồi trả lời từng bình luận.` (`{url}` = link PR, `{id}` = số PR; xuống dòng đổi thành dấu cách). Ô nhập đang gõ dở được xoá trước như `Từ chối…` (Ctrl-Y trong Claude khôi phục). PR đã đóng không có nút.
- Chuột phải → `Prompt Nhờ Claude…` mở hộp sửa prompt với ba nút `Lưu` (để trống thì về mặc định) / `Huỷ` / `Mặc định`; prompt được nhớ sau khi mở lại app.
- Bấm một dòng → cmux lên trước, đúng pane của session đó. Session không chạy trong cmux thì chỉ kêu beep.
- Lần đầu macOS hỏi quyền "ClaudeSessions điều khiển cmux" (có thể hỏi lại sau mỗi lần build). Quyền Automation
  cho cmux này cần cho cả nhảy pane và duyệt. Lỡ từ chối thì bật lại ở System Settings → Privacy &
  Security → Automation.
- Kéo được từ mọi chỗ trên widget; dời từ 4 pt trở xuống vẫn tính là bấm.
- Session xong/chờ hiện thêm câu trả lời cuối của Claude (tối đa 4 dòng, cắt 600 ký tự, có chữ đậm và code, link hiện thành nhãn ngắn), đọc từ transcript
  `~/.claude/projects/…/<sessionId>.jsonl`.
- Ticket và link: nút `↗ nhãn` dưới dòng của session lấy từ cả session (prompt bạn gõ, và câu trả lời của Claude trong lượt bắt đầu bằng prompt bạn gõ, kể cả bản tóm tắt `/compact`; không tính output của tool, không tính báo cáo của agent chạy nền `<task-notification>` và câu trả lời của Claude trong lượt bắt đầu bằng báo cáo đó, trừ khi agent hoặc tác vụ nền đó được chạy trong lượt bạn bắt đầu (hoặc lượt kế thừa từ đó): khi ấy câu trả lời của Claude sau báo cáo vẫn được tính, còn nội dung báo cáo thì không): link Jira `/browse/KEY-N` hoặc `selectedIssue=KEY-N` (widget nhớ prefix và host vào `jiraProjects` trong `UserDefaults`), và mã ticket trần như `T6IR-123` khi prefix (2–10 ký tự) đã học được. Thứ tự: ticket bạn nhắc (lần nhắc đầu trước), ticket chỉ Claude nhắc (lần nhắc gần nhất trước), rồi link PR chưa có dòng PR (`↗ PR #N`; link PR không phụ thuộc lượt). Link khác không có nút. Các nút nằm trên một hàng vừa chiều rộng, sau đó là nút `☰ N` (thay cho `+N` cũ; không vừa nổi một nút thì chỉ hiện `☰ N`). `N` là số mục trong danh sách, không tính nút `Kết nối Jira…`. Bấm `☰ N` bung danh sách ngay trong widget (không còn menu), bấm lần nữa để gấp; danh sách cũng tự gấp khi widget thu lại. Danh sách chia nhóm `Bạn nhắc`, `Claude nhắc`, `PR` (nhóm rỗng thì ẩn). Mỗi dòng ticket là icon loại ticket từ Jira · mã · tiêu đề · tag trạng thái màu theo nhóm trạng thái của Jira (xám = chưa làm, xanh dương = đang làm, xanh lá = xong; tag cắt ở 130 pt); chưa có tiêu đề thì chỉ hiện mã. Mỗi dòng PR là icon PR · `#id` · tiêu đề · tag `mở` (xanh lá) / `đã merge` (tím) / `đã đóng` (xám). Bấm một dòng mở trình duyệt, không nhảy sang cmux. Khi Jira đã trả tiêu đề, nút ticket hiện icon + mã + chấm màu theo nhóm trạng thái; trước đó là `↗ mã`. Danh sách liệt kê đủ mọi PR (dòng PR dưới session chỉ hiện tối đa 3) và cả link PR không theo dõi được. Session chỉ có PR vẫn có hàng này. Rê chuột lên nút ticket hoặc dòng PR để xem tiêu đề đầy đủ. Nút của mỗi session hiện ngay khi transcript của session đó quét xong. Nút vẫn còn khi session đang chạy. Bấm nút mở trình duyệt mặc định, không nhảy sang cmux.
- Tiêu đề ticket Jira (cần mục Keychain `jira-api-token`, tài khoản = email Atlassian, tạo bằng `security add-generic-password -s jira-api-token -a <email> -w`, dùng token classic ở https://id.atlassian.com/manage-profile/security/api-tokens; không có mục này thì danh sách chỉ hiện mã ticket). Widget hỏi `POST https://<host>/rest/api/3/issue/bulkfetch` (lấy `summary`, `status` và `issuetype`; host học từ link Jira đã thấy; chỉ host `*.atlassian.net` được gửi token, site khác thì danh sách chỉ hiện mã ticket; icon loại ticket (URL `iconUrl` của `issuetype`) cũng chỉ tải qua https từ `*.atlassian.net`, mỗi icon một lần mỗi lần chạy, tải lỗi thì thử lại sau 60 giây, và bỏ qua khi token đang bị từ chối): ticket mới tối đa một lượt mỗi 5 giây, làm mới mọi ticket đang hiện mỗi 900 giây, ticket lỗi chờ 60 giây, ticket vắng mặt khỏi mọi session quá 30 giây thì bị bỏ. Khi danh sách có ticket mà Jira chưa kết nối được, cuối danh sách có nút `Kết nối Jira…`.
- Chuột phải có thêm hai dòng trạng thái kết nối: `Bitbucket: ✓ <email>` và `Jira: ✓ <email>` (hoặc `✗ chưa có token`, `<email> (đang kiểm tra)`, `✗ token bị từ chối (401)`, `✗ không kết nối được`; Jira chưa học được site nào thì `Jira: ✗ chưa biết site Jira` và bấm chỉ kêu beep). Bấm một dòng mở hộp `Kết nối <dịch vụ>`: ô email (điền sẵn từ Keychain) và ô token, ba nút `Lưu` / `Huỷ` / `Tạo token…` (mở trang tạo token rồi hiện lại hộp). `Lưu` kiểm token với dịch vụ trước (Bitbucket `/2.0/user`, Jira `/rest/api/3/myself`), chỉ ghi vào Keychain khi hợp lệ, rồi dùng ngay không cần mở lại app; sai thì hộp hiện lại với dòng lỗi (`401: email hoặc token sai`, `Không kết nối được`, `Lỗi HTTP <n>`). Token đi qua `security -i` bằng stdin, không nằm trong tham số dòng lệnh.
- Danh sách cuộn được khi cao hơn khung widget; danh sách ngắn giữ nguyên chiều cao.
- Dòng `chờ duyệt` có nút **Cho phép** và **Từ chối…** (bấm Từ chối… để gõ lý do, để trống = chỉ từ chối).
  Lệnh dài phải bấm **Xem đủ** mới hiện nút; lệnh quá dài, subagent, nhiều việc cùng chờ hoặc tool lạ
  (MCP…) thì chỉ nhảy tới cmux để duyệt.
- Cơ chế: widget gõ phím hộ vào terminal cmux qua AppleScript (`1` = Yes, Esc = từ chối, lý do từ chối = dán chữ sau Esc rồi Enter), chỉ cho session chạy trong cmux (session ngoài cmux: bấm Cho phép/Từ chối chỉ kêu beep), và kiểm lại trạng thái ngay trước khi gõ (đổi rồi thì
  chỉ kêu beep, không gõ gì).
- Quota: header khung mở rộng hiện `5h N% · HH:mm · tuần N%` (giờ là lúc cửa sổ 5 giờ reset, giờ địa phương) đọc từ `~/.claude/plugins/oh-my-claudecode/.usage-cache-anthropic.json` do OMC HUD ghi (chỉ đọc lại khi file đổi, tối đa 10 giây một lần; `error: true`, thiếu `data` hoặc không có file thì ẩn). Mỗi phần trăm vàng từ 80, đỏ từ 95. Có `→ HH:mm` sau phần 5h khi tốc độ tăng trong 30 phút gần nhất của cùng cửa sổ 5 giờ (cần ít nhất 2 mẫu cách nhau từ 5 phút) chạm 100% trước lúc reset. `lastSuccessAt` cũ hơn 15 phút thì cả cụm mờ đi và thêm `cũ`. Viên thu gọn chỉ hiện thêm `N%` (số lớn hơn của hai phần, màu theo ngưỡng) khi từ 80% trở lên, viên rộng thêm 44 pt. Không đọc khi đặt `CLAUDE_SESSIONS_ROOT`.
- `🤖×N`: thẻ `đang chạy` có `🤖×N` cạnh dòng `▸ …` khi N subagent chạy nổi (`Agent` / `Task`) chưa có kết quả trong phần đuôi transcript đã đọc (256 KB, cùng lần đọc với dòng `▸ …`, không đọc thêm file); rê chuột để xem `description` của từng agent, mỗi dòng một agent. Agent chạy nền (`run_in_background`) trả kết quả ngay nên không tính ở đây mà vẫn hiện qua `chờ N agent nền`; agent gọi từ trước 256 KB cuối thì không thấy.
- Phím tắt toàn hệ thống (Carbon `RegisterEventHotKey`, không cần quyền Accessibility): `⌃⌥Space` mở/đóng widget (khung mở nằm trên màn hình đang có chuột và giữ mở cho đến khi bấm lại hoặc chuột vào rồi rời khung; không làm gì khi widget đang ẩn vì không có session nào trong 30 giây), `⌃⌥→` nhảy tới pane cmux của session chờ lâu nhất (chờ duyệt/chờ trả lời trước, rồi session xong còn dấu mới; bấm lại trong 5 giây thì sang session kế tiếp, quá 5 giây thì quay về đầu; session ngoài cmux bị bỏ qua, không có thì beep). Chuột phải → `Phím tắt` → `Mở widget` / `Nhảy session chờ` để chọn tổ hợp khác hoặc `Tắt` (nhớ sau khi mở lại app); tổ hợp bị app khác chiếm thì dòng hiện `(đang bị app khác dùng)`. Không đăng ký phím khi chạy với `CLAUDE_SESSIONS_ROOT`.
- `Vừa đóng (N)`: dòng cuối của danh sách mở rộng (ẩn khi N = 0; bấm để mở/đóng, đóng lại khi widget thu gọn) liệt kê tối đa 5 session vừa kết thúc trong 24 giờ qua, mới nhất trước, mỗi dòng có tiêu đề, `thư mục · nhánh` và `đóng 12p`. Session được coi là đóng khi lần quét trước còn thấy mà lần này không còn và tiến trình của nó đã kết thúc (nếu tiến trình còn chạy lúc file session vừa biến mất thì widget kiểm tra lại trong khoảng 10 giây; `/clear` giữ nguyên pid nên không phải là đóng; lần quét đầu không đóng gì; session sống lại thì rời danh sách). Danh sách được nhớ qua `UserDefaults` (khoá `closedSessions`; không dùng khi chạy với `CLAUDE_SESSIONS_ROOT`). Bấm một dòng mở workspace cmux mới trong đúng thư mục cũ chạy `claude --resume <id>` (`cmux new-workspace --cwd … --command …`) rồi dòng biến mất; lỗi thì chép `cd '<thư mục>' && claude --resume <id>` vào clipboard và beep.
- `Trả lời…`: nút cạnh thời gian của thẻ `xong` (hoặc `chờ N agent nền`) mở ô nhập; Enter gõ nội dung vào pane cmux của session rồi gửi (để trống thì không gửi; Esc huỷ). Chỉ gửi khi session còn `xong` ngay trước lúc gõ, và nút bị làm mờ kèm chú thích `Chỉ dùng được trong cmux` khi session ngoài cmux.
- `Dừng`: nút cạnh thời gian của thẻ `đang chạy`; bấm lần đầu đổi thành `Bấm lần nữa để dừng`, bấm lần hai trong 3 giây thì gửi Esc vào pane cmux (Claude hiện "Interrupted"); quá 3 giây thì về lại `Dừng`. Chỉ gửi khi session còn `đang chạy` (không phải `chờ N agent nền`); ngoài cmux thì bị làm mờ như `Trả lời…`.
- Câu hỏi của Claude (AskUserQuestion): khi session `chờ trả lời` và cmux Feed có câu hỏi đó, thẻ hiện thẻ `header` + câu hỏi (thay phần câu trả lời cuối), mỗi lựa chọn là một nút (hỏi một lựa chọn) hoặc ô bật/tắt `☑`/`☐` (chọn nhiều; `description` hiện khi rê chuột), ô `Khác…` để gõ câu trả lời riêng cho từng câu hỏi, nút `Gửi` (chỉ bấm được khi mọi câu hỏi có lựa chọn hoặc chữ `Khác…`) và `còn m:ss` đếm tới hết cửa sổ 120 giây của Feed tính từ lúc câu hỏi được tạo. `Gửi` trả lời qua socket của cmux (`feed.list` rồi `feed.question.reply`, không cần mật khẩu từ ngoài cmux, không gõ phím vào terminal): sau khi gửi thẻ hiện `Đã gửi` đến khi session chạy lại. Hết 120 giây, câu hỏi biến khỏi Feed (đã trả lời ở terminal), gửi lỗi (kêu beep) hoặc hình dạng cmux không nhận (câu hỏi không có lựa chọn) thì chỉ còn nút `Trả lời trong cmux` (nhảy tới pane). Hỗ trợ (đã chạy thật với Claude, haiku): một câu hỏi chọn một, chọn nhiều (gửi các nhãn nối bằng `, `), nhiều câu hỏi, và `Khác…` gõ tự do. Widget chỉ hỏi Feed khi có session `đang chạy` hoặc `chờ trả lời` (không phải chờ duyệt), mỗi giây một lượt; lỗi socket thì tắt tính năng 15 giây rồi thử lại; chỉ trả lời requestId lấy từ `feed.list`. Câu hỏi mới xuất hiện khi session còn `đang chạy` được tính như session vừa chuyển sang chờ (dấu mới, kêu `Funk`, tự mở 5 giây) và không kêu lần nữa khi trạng thái đổi sang `chờ`. Không có file `~/.local/state/cmux/last-socket-path` thì tính năng tắt.
- Không còn session 30 giây thì widget ẩn, có session thì hiện lại; mở app lúc chưa có session nào thì widget ẩn ngay.

Cài (cần Command Line Tools: `xcode-select --install`), build ra `~/Applications/ClaudeSessions.app`:

```bash
bash ~/.claude/skills-repo/claude-sessions/build.sh && open ~/Applications/ClaudeSessions.app
```

App tự đăng ký chạy khi đăng nhập. Nếu macOS đòi duyệt (app ký ad-hoc, có thể hỏi lại sau mỗi lần build) thì bật ở System Settings → General → Login Items.

Test trạng thái (logic + ảnh PNG của từng trạng thái, không cần quyền Screen Recording):

```bash
cd ~/.claude/skills-repo/claude-sessions && swift test   # ảnh PNG nằm trong .build/snapshots/
```

Chạy `swift test` trong Bash sandbox của Claude Code có thể lỗi "plugin for module 'TestingMacros' not found"
(sandbox chặn plugin macro của Swift); chạy ở terminal thường thì qua.
Trong sandbox thì truyền đường dẫn plugin của Command Line Tools:

```bash
cd ~/.claude/skills-repo/claude-sessions && swift test -Xswiftc -plugin-path -Xswiftc /Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing
```

Đo hiệu năng của widget thật bằng `perf.sh` (cần tắt sandbox, mất khoảng 3 phút):

```bash
bash ~/.claude/skills-repo/claude-sessions/perf.sh   # in bảng PASS/FAIL, thoát 1 nếu có dòng FAIL
```

- `perf.sh` build bản release, tạo fixture 10 session (~680 MB transcript) trong `claude-sessions/.build/perf-fixture` (gitignored; xoá thư mục này để tạo lại), rồi chạy binary với `CLAUDE_SESSIONS_ROOT` trỏ vào fixture. Khi biến này được đặt, app không đọc Keychain và không gọi `SMAppService.mainApp.register()`; khi không đặt thì app chạy y như cũ. Trong lúc đo, một pill thứ hai hiện trên màn hình khoảng 2-3 phút. Script không đụng `~/.claude`, Keychain hay app đang cài.
- Hai pha, mỗi pha 60 giây (đổi bằng `PERF_PHASE_SECONDS`): pha idle (không ghi gì) và pha load (ghi thêm vào transcript liên tục). Mỗi dòng của bảng là một chỉ số trong một pha: footprint cuối pha, mức tăng footprint trong pha, CPU trung bình, số wakeup mỗi giây (chỉ pha idle). Ngưỡng là các biến ở đầu `perf.sh` (`FOOTPRINT_MAX_MB`, `GROWTH_MAX_MB`, `CPU_IDLE_MAX`, `CPU_LOAD_MAX`, `WAKEUPS_MAX`); không sửa ngưỡng để cho qua, chạy lại khi máy rảnh. Trước mỗi pha, script đợi footprint ổn định (hai lần đọc lệch dưới 1 MB, tối đa 30 giây) rồi mới đo; app thoát giữa chừng hoặc một chỉ số không đọc được đều tính là FAIL.
- Quy tắc: mọi thay đổi widget phải chạy `swift test` và `perf.sh` trước khi xong việc. Footprint còn ít dư địa (~96 MB so với ngưỡng 100 MB với fixture 10 session), nên thỉnh thoảng pha load có thể vọt lên và FAIL; chạy lại một lần khi máy rảnh trước khi kết luận.

Trước và sau khi tối ưu (cùng máy, `perf.sh` với pha 60 giây):

| chỉ số | trước | sau | ngưỡng |
|---|---|---|---|
| footprint cuối pha idle (MB) | 94.7 | 96.5 | < 100 |
| tăng footprint pha idle (MB) | 0.0 | 0.0 | < 5 |
| CPU pha idle (%) | 0.93 | 0.33 | < 1 |
| wakeup pha idle (/s) | 0.63 | 0.72 | < 5 |
| footprint cuối pha load (MB) | 94.8 | 97.3 | < 100 |
| tăng footprint pha load (MB) | 0.1 | 0.8 | < 5 |
| CPU pha load (%) | 0.98 | 0.42 | < 2 |
| `refresh()` trung vị trong `swift test` (debug, 10 session) | 1.84 ms | 0.45 ms | < 10 ms |
| byte transcript đọc mỗi refresh ổn định | 0 | 0 | ≤ phần mới ghi |
| mỗi lần refresh ổn định: parse `.json` / đọc `HEAD` / liệt kê thư mục task | 10 / 10 / 10 | 0 / 0 / 0 | không đọc lại gì |

Đẩy thay đổi ngược lên repo (bản đang dùng nằm ở `~/.claude/tools/claude-sessions/`):

```bash
cd ~/.claude/skills-repo
rsync -a --exclude .build ~/.claude/tools/claude-sessions/ claude-sessions/
git add claude-sessions && git commit -m "..." && git push
```
