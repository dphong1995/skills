# Claude Code skills — brainstorm-pro / ck-plan / cook / routing-bug-fixes-to-cook

Bộ skill thiết kế → lập kế hoạch → triển khai cho Claude Code, kèm các subagent mà chúng dispatch.

## Cài trên máy mới

```bash
git clone git@github.com:dphong1995/skills.git ~/.claude/skills-repo
bash ~/.claude/skills-repo/install.sh
```

`install.sh` copy nội dung vào `~/.claude/skills/` và `~/.claude/agents/`. Vị trí clone không quan
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
├── brainstorm-pro/   Thiết kế feature: deep scout → Q&A từng câu một → design theo section
│                     → viết spec → user duyệt → gọi ck-plan
├── ck-plan/          Lập kế hoạch: research → phân tích codebase → plan.md + phase-XX-*.md
│                     → tự động gọi cook
├── cook/             Triển khai: mỗi phase một subagent → test → review fan-out 3 agent → finalize
└── routing-bug-fixes-to-cook/
                      Cổng sau khi debug xong root cause: fix ≥ 3 file production, > 50 dòng
                      production (đếm gộp các repo) hoặc có nhiều cách sửa → gọi cook (Step 1 rút
                      gọn); nhỏ hơn → sửa thẳng

agents/               19 subagent definition mà cook/ck-plan gọi tới

cmux/                 Settings của cmux (không thuộc install.sh ở trên, xem mục cmux bên dưới)
```

Chuỗi thiết kế: `brainstorm-pro` → `ck-plan` → `cook` chạy liền mạch, không dừng hỏi ở giữa (trừ
các gate tương tác trong phase design của brainstorm-pro/cook).

## Ràng buộc cần biết

- **3 skill phải nằm cạnh nhau trong cùng một thư mục.** `cook/SKILL.md` đọc
  `../ck-plan/references/plan-organization.md` và `../ck-plan/references/output-standards.md` bằng
  đường dẫn tương đối để lấy format plan. Tách ra là hỏng.
- **`agents/` là dependency thật, không phải tuỳ chọn.** `cook` dispatch theo tên:
  `executor`, `code-reviewer`, `test-engineer`, `code-simplifier`, `writer`, `git-master`,
  `designer`, `debugger`, `explore`, `document-specialist`, `planner`. Thiếu thì cook vẫn chạy
  nhưng phải fallback sang agent mặc định và chất lượng giảm.
- **Không có CLI `ck`.** Tên `ck-plan` chỉ là di sản đặt tên; mọi thứ chạy trong skill.
- **`routing-bug-fixes-to-cook` cần 1 dòng trong `~/.claude/CLAUDE.md`** để được gọi đúng lúc (thêm
  thủ công):
  ```
  - Bug fix: once the root cause is confirmed, invoke the `routing-bug-fixes-to-cook` skill before writing the fix.
  ```
  Skill này dùng chế độ Step 1 rút gọn của `cook`, nên phải cài cùng bản `cook` trong repo.
- Repo này không đụng tới `settings.json`, `CLAUDE.md` hay plugin — những thứ đó cấu hình riêng.

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
