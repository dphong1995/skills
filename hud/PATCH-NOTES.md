# OMC HUD — Patch ghi chú (giữ qua các lần upgrade)

## HUD chạy bản vendored (từ OMC 5.7.31)
HUD giờ chạy bản sao trong `~/.claude/hud/omc/` (OMC 5.7.31, tạo bằng `hud/vendor.sh`, cài bằng `hud/install.sh`); wrapper `omc-hud.mjs` import nó trước plugin cache. Vì vậy các patch bên dưới chỉ còn liên quan khi HUD rơi về plugin cache:
- Patch 1: với 5.7.31 không còn cần. Upstream chỉ thay rate limit bằng enterprise cost khi `isEnterprise`; lưu ý `omcHud.elements.enterpriseMode: true` ép `isEnterprise`, nên nếu API trả `enterpriseSpentUsd` thì đoạn `5h:` bị thay. `hud/settings.json` của repo đặt `omcHud.elements.enterpriseMode: true`; nếu `5h:` bị thay bằng enterprise cost (`spent:`), đặt `enterpriseMode` thành `false` (hoặc xóa) trong `hud/settings.json` rồi chạy lại `hud/install.sh`, không patch `hud/omc`.
- Patch 2: nằm trong wrapper (`HIDE_SEGMENT`), áp cho cả bản vendored.
- Patch 3: không còn cần, 5.7.31 không import `zod`.
- Khi vendor bản OMC mới, cập nhật số phiên bản được nhắc trong file này.

## Mục đích
> Trạng thái: không còn cần với bản vendored 5.7.31, xem mục "HUD chạy bản vendored" ở đầu file.

Hiển thị `5h:X%(YhZm)` (rate limit window) thay vì `spent:$X/$Y` (enterprise cost) cho tài khoản enterprise.

## Vấn đề
HUD mặc định chặn render `rateLimits` khi API trả về `enterpriseSpentUsd`. Logic ở `render.js`:

```js
const hasEnterpriseData = context.rateLimitsResult?.rateLimits?.enterpriseSpentUsd !== undefined;
if (enabledElements.rateLimits && context.rateLimitsResult && !hasEnterpriseData) { ... }
```

Khiến enterprise user chỉ thấy `enterpriseCost`, không thấy `5h:` countdown — dù data thực tế có sẵn cả `fiveHourPercent` và `fiveHourResetsAt`.

## Patch
File: `~/.claude/plugins/cache/omc/oh-my-claudecode/<VERSION>/dist/hud/render.js`

Tìm dòng:
```js
const hasEnterpriseData = context.rateLimitsResult?.rateLimits?.enterpriseSpentUsd !== undefined;
if (enabledElements.rateLimits && context.rateLimitsResult && !hasEnterpriseData) {
```

Đổi thành:
```js
if (enabledElements.rateLimits && context.rateLimitsResult) {
```

(Xóa cả dòng `hasEnterpriseData` để tránh TS6133 warning.)

## Cấu hình settings.json đi kèm
Trong `~/.claude/settings.json` → `omcHud.layout.main`:
- Bỏ `enterpriseCost` (không dùng nữa).
- Giữ `rateLimits` (sẽ render `5h:` sau patch).

```json
"main": [
  "omcLabel",
  "rateLimits",
  "customBuckets",
  ...
]
```

## Áp dụng nhanh sau upgrade
```bash
VER=$(ls ~/.claude/plugins/cache/omc/oh-my-claudecode | grep -E '^[0-9]' | sort -V | tail -1)
F=~/.claude/plugins/cache/omc/oh-my-claudecode/$VER/dist/hud/render.js
# Patch (idempotent — sed bỏ dòng hasEnterpriseData và sửa điều kiện if)
sed -i.bak \
  -e '/const hasEnterpriseData = context\.rateLimitsResult/d' \
  -e 's/enabledElements\.rateLimits && context\.rateLimitsResult && !hasEnterpriseData/enabledElements.rateLimits \&\& context.rateLimitsResult/' \
  "$F"
echo "Patched: $F (backup: $F.bak)"
```

## Verify
```bash
grep -n "hasEnterpriseData\|rateLimits && context.rateLimitsResult" \
  ~/.claude/plugins/cache/omc/oh-my-claudecode/*/dist/hud/render.js
```
Nên thấy 1 dòng `if (enabledElements.rateLimits && context.rateLimitsResult) {` và KHÔNG còn dòng `const hasEnterpriseData`.

## Restart
Sau khi patch, restart Claude Code để statusline reload.

---

# Patch 2 — Ẩn bucket `fable:` (weekly scoped quota)

Từ OMC 5.5.0, `limits[]` (`kind: "weekly_scoped"`) có model lạ (vd "Fable") được render generic thành `fable:X%(…)`. Không có option config để tắt.

Từ 2026-10-05 ẩn ở wrapper `~/.claude/hud/omc-hud.mjs` (regex `HIDE_SEGMENT`, cùng chỗ ẩn `sn:`/`extra:`), nên KHÔNG cần patch `usage-api.js` của plugin nữa và không mất khi upgrade. Muốn ẩn thêm bucket nào thì thêm label vào cả `NEXT_TOKEN` lẫn nhóm `(?:sn|fable|extra)`.

Verify: `grep -n "fable" ~/.claude/hud/omc-hud.mjs`

---

# Patch 3 — Thiếu `zod` trong plugin cache (từ 5.7.5)

> Trạng thái: không còn cần với bản vendored 5.7.31, xem mục "HUD chạy bản vendored" ở đầu file.

5.7.5 thêm `import { z } from "zod"` vào `dist/lib/security-config.js` (#4388/#4389), HUD import file này, nhưng plugin cache không có `node_modules` (phần còn lại của OMC chạy bundle `bridge/*.cjs`). Import fail → wrapper nuốt lỗi và in message sai `Plugin dist/ exists but HUD not found. Run: ... npm run build` (build lại KHÔNG fix được).

Chỉ cần `zod` (không cần `npm install` cả plugin — kéo theo better-sqlite3, claude-agent-sdk...):
```bash
VER=$(ls ~/.claude/plugins/cache/omc/oh-my-claudecode | grep -E '^[0-9]' | sort -V | tail -1)
P=~/.claude/plugins/cache/omc/oh-my-claudecode/$VER
T=$(mktemp -d) && npm install --prefix "$T" --no-audit --no-fund --ignore-scripts 'zod@^3.23.8' \
  && mkdir -p "$P/node_modules" && cp -R "$T/node_modules/zod" "$P/node_modules/" && rm -rf "$T"
```

Verify: `echo '{}' | node ~/.claude/hud/omc-hud.mjs` phải in ra `5h:...`, không phải `[OMC HUD] ...`.
Xem lỗi thật: `echo '{}' | node -e "import('$P/dist/hud/index.js').catch(e=>console.error(e.message))"`
