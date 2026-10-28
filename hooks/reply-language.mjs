// Stop hook: when Claude's last reply to the user is English prose, block the stop once so Claude rewrites it in Vietnamese.
// Code blocks, inline code, blockquotes (text meant for Jira or a PR), links and paths are left out of the check; short replies pass.
const MIN_WORDS = 8;
const MIN_VIET = 0.15;
const VIET = /[àáạảãâầấậẩẫăằắặẳẵèéẹẻẽêềếệểễìíịỉĩòóọỏõôồốộổỗơờớợởỡùúụủũưừứựửữỳýỵỷỹđ]/i;

let input = '';
for await (const chunk of process.stdin) input += chunk;
let event = {};
try { event = JSON.parse(input); } catch (e) { process.exit(0); }
if (event.stop_hook_active || typeof event.last_assistant_message !== 'string') process.exit(0);

const prose = event.last_assistant_message
  .replace(/^(```|~~~)[\s\S]*?^\1/gm, ' ')
  .replace(/`[^`\n]*`/g, ' ')
  .replace(/^\s*>.*$/gm, ' ')
  .replace(/\]\([^)]*\)/g, ']')
  .replace(/\S*[/\\]\S*/g, ' ');
const words = prose.match(/\p{L}{2,}/gu) || [];
if (words.length < MIN_WORDS || words.filter((w) => VIET.test(w)).length / words.length >= MIN_VIET) process.exit(0);

console.log(JSON.stringify({
  decision: 'block',
  reason: 'Câu trả lời vừa rồi cho người dùng viết bằng tiếng Anh. Viết lại toàn bộ câu trả lời đó bằng tiếng Việt ngay bây giờ: '
    + 'giữ nguyên code, tên định danh, đường dẫn, lệnh; đoạn tiếng Anh viết cho Jira hay PR thì đặt trong code block hoặc blockquote. '
    + 'Không nhắc tới bước kiểm tra này.',
}));
