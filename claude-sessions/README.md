# Claude Sessions

Widget macOS nổi luôn nằm trên cùng, hiện các session Claude Code đang mở trên máy: trạng thái, việc đang làm, PR, ticket, và các nút trả lời nhanh.

![Widget Claude sessions](docs/images/07-mixed-hover.png)

## Tính năng

### Danh sách mở rộng
Rê chuột vào viên thì mở danh sách các session, mỗi thẻ có icon trạng thái, tiêu đề kèm tag trạng thái (`chờ duyệt`, `chờ trả lời`, `xong`, `đang chạy`, `chờ N agent nền`), thư mục, vạch màu ở mép trái và cột phải cố định gồm thời gian và nút icon nhanh. Header gom số session theo nhóm màu và hiện quota `5h` / `Tuần` dạng thanh ở bên phải, ngay trước số session (thanh `5h` kèm `reset HH:mm` hoặc dự báo `hết ~HH:mm` màu đỏ, rồi tới thanh `Tuần`); PR hiện thành các tag có icon. Câu hỏi của Claude hiện thành danh sách đánh số, mỗi đáp án có mô tả luôn hiện; `Cho phép` và `Gửi` màu xanh lá.

### Phím tắt bàn phím
Bấm phím tắt mở widget (`⌃⌥Space`) thì thẻ được chọn bằng vòng viền trắng và điều khiển bằng bàn phím:

| Phím | Việc làm |
|---|---|
| `↑` / `↓` | chọn thẻ trước / sau |
| `↩` | nhảy tới pane cmux của thẻ (widget vẫn mở) |
| `A` | cho phép (chỉ khi thẻ chờ duyệt và lệnh đã hiện đủ) |
| `D` | mở ô lý do từ chối (chỉ ở thẻ đang hiện nút `Từ chối…`) |
| `R` | mở ô trả lời (session xong, chạy trong cmux) |
| `1`-`9` | chọn đáp án thứ n của câu hỏi (đánh số liên tục qua các câu hỏi) |
| `⌘↩` | gửi câu trả lời khi mọi câu hỏi đã có lựa chọn |
| `Esc` | đang gõ: thôi gõ; không gõ: đóng widget và trả phím cho app trước |

Phím không áp dụng cho thẻ đang chọn thì kêu beep; phím không gán thì đi tiếp tới app đang dùng. Chế độ bàn phím chỉ bật khi bấm phím tắt mở widget (bấm khi widget đang mở do rê chuột thì chuyển sang chế độ bàn phím thay vì đóng; thẻ đóng khi bấm lại phím tắt, bấm `Esc`, hoặc chuột rời khung nếu chuột đã vào nó, còn thẻ mở bằng phím tắt mà chuột chưa vào thì giữ mở đến khi bấm phím tắt; bấm sang app khác chỉ kết thúc chế độ bàn phím, bấm phím tắt lần nữa (ví dụ sau khi `↩` nhảy sang cmux) thì bật lại); chỉ rê chuột thì không bao giờ lấy phím, và rê chuột vào widget khi đang gõ ở terminal không cướp phím.

### Viên thu gọn
Khi không rê chuột, widget chỉ là một viên nhỏ với số session theo nhóm: số session chờ nằm trong huy hiệu đỏ đặc, các nhóm khác (cam xong, xanh đang chạy) là chấm màu với số mờ hơn; nhóm vàng có icon sparkle là session có tin mới. Khi quota 5h hoặc quota tuần (lấy số lớn hơn) từ 80% trở lên, viên rộng thêm để hiện `N%`.

![Viên thu gọn](docs/images/06-mixed-collapsed.png)

### Câu trả lời cuối của Claude
Session đã xong hoặc đang chờ hiện thêm câu trả lời cuối của Claude, đọc từ transcript, để nắm kết quả mà không cần mở terminal.

![Câu trả lời cuối của Claude](docs/images/10-last-reply-hover.png)

### Chờ duyệt
Session xin duyệt tool hiện lệnh hoặc file đang chờ, kèm nút `Cho phép` / `Từ chối…` để duyệt ngay trên widget.

![Chờ duyệt](docs/images/11-pending-hover.png)

### Trả lời câu hỏi của Claude ngay trên widget
Khi Claude hỏi (AskUserQuestion), thẻ hiện câu hỏi và các lựa chọn; chọn xong bấm `Gửi` mà không cần quay lại terminal.

![Trả lời câu hỏi](docs/images/question-4-two-questions.png)

### Nút `Dừng`
Thẻ đang chạy có nút `Dừng`; bấm hai lần liên tiếp để gửi Esc vào pane của session.

![Nút Dừng](docs/images/stop-armed.png)

### Task đang làm
Session có task hiện dòng tiến độ `đã xong/tổng` cùng việc đang làm.

![Task đang làm](docs/images/16-todo.png)

### Tiêu đề session
Tên dòng lấy từ tên đặt bằng `/rename`, tiêu đề AI hoặc prompt đầu tiên của session.

![Tiêu đề session](docs/images/15-titles-branch.png)

### Agent nền
Khi lượt chính đã xong mà agent nền còn chạy, thẻ hiện `chờ N agent nền` và đếm số agent.

![Agent nền](docs/images/35-background-agents.png)

### Dòng PR Bitbucket
Mỗi PR của session có một dòng với số yêu cầu sửa, số người duyệt, bình luận, và nút `Nhờ Claude` để bảo Claude xử lý bình luận.

![Dòng PR](docs/images/22-pr-lines.png)

### CodeRabbit
PR hiện trạng thái review của CodeRabbit; khi đang chờ duyệt có nút `Nhắc duyệt` (icon con thỏ).

![CodeRabbit](docs/images/31-pr-awaiting-approval.png)

### Ticket Jira và link
Các ticket Jira và PR nhắc trong session thành chip bấm được (chip ticket tô màu theo trạng thái Jira); nút icon danh sách + `N` bung danh sách đầy đủ ngay trong widget.

![Ticket và link](docs/images/29-link-list-open.png)

### Dấu tin mới
Session vừa chuyển từ đang chạy sang xong/chờ, hoặc PR có sự kiện, được đánh dấu bằng icon sparkle và nền vàng nhạt trên thẻ (hoặc tô tím ở dòng PR) cho đến khi bạn xem.

![Dấu tin mới](docs/images/34-mark-both.png)

### Quota 5 giờ / tuần
Header hiện mức dùng quota của cửa sổ 5 giờ và của tuần, đổi màu khi gần hết, kèm dự báo giờ chạm 100% nếu tốc độ hiện tại giữ nguyên.

![Quota](docs/images/quota-3-high-forecast.png)

### Vừa đóng
Dòng `Vừa đóng` cuối danh sách liệt kê các session vừa kết thúc; bấm một dòng để mở lại bằng `claude --resume`.

![Vừa đóng](docs/images/closed-2-open.png)

## Cài và chạy

```bash
bash ~/.claude/skills-repo/install.sh --only widget
```

Cần Command Line Tools (`xcode-select --install`). Dòng PR cần mục Keychain `bitbucket-api-token` (tuỳ chọn; không có thì widget chạy như cũ, không có PR).

## Test

```bash
swift test       # trong claude-sessions/
bash perf.sh     # đo hiệu năng bản thật, cần tắt sandbox
```

Cờ `-plugin-path` khi chạy trong sandbox và test E2E: xem [README gốc](../README.md#widget-claude-sessions).

## Cập nhật ảnh

Ảnh minh hoạ sinh từ snapshot test; chạy lại khi UI đổi:

```bash
bash claude-sessions/screenshots.sh
# trong Bash sandbox của Claude Code:
bash claude-sessions/screenshots.sh -Xswiftc -plugin-path -Xswiftc /Library/Developer/CommandLineTools/usr/lib/swift/host/plugins/testing
```

`closed-2-open` và `quota-3-high-forecast` có giờ theo đồng hồ thật nên đổi mỗi lần chạy lại.

## Chi tiết

Toàn bộ hành vi của widget nằm ở [README gốc](../README.md#widget-claude-sessions).
