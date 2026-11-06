import AppKit
import SwiftUI
import Testing
@testable import ClaudeSessionsKit

private let outDir: URL = {
    // Tests/ClaudeSessionsKitTests/X.swift -> package root is 3 levels up
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let dir = root.appendingPathComponent(".build/snapshots")
    try! FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir
}()

@MainActor
private func render(_ store: Store, _ panel: PanelState, _ name: String) {
    let view = ZStack {
        LinearGradient(colors: [Color(red: 0.25, green: 0.3, blue: 0.45), Color(red: 0.1, green: 0.12, blue: 0.2)],
                       startPoint: .top, endPoint: .bottom)
        WidgetView(store: store, panel: panel)
    }
    .frame(width: Layout.canvas.width, height: Layout.canvas.height)
    .environment(\.snapshotRendering, true)
    let r = ImageRenderer(content: view)
    r.scale = 2
    let cg = r.cgImage
    #expect(cg != nil, "render \(name)")
    guard let cg else { return }
    let data = NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:])!
    try! data.write(to: outDir.appendingPathComponent("\(name).png"))
}

@Suite(.serialized)
@MainActor
struct SnapshotTests {
    @Test func renderAllStates() {
        let dir = makeTempDir()
        let store = makeStore(dir: dir)
        let panel = PanelState()
        let t0 = Date()

        store.refresh(at: t0)
        render(store, panel, "01-empty-collapsed")

        writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
        store.refresh(at: t0)
        render(store, panel, "02-all-busy-collapsed")

        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(1))
        store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t0.addingTimeInterval(1))
        render(store, panel, "03-just-finished-peek")

        store.refresh(at: t0.addingTimeInterval(7))
        render(store, panel, "04-after-peek-collapsed")

        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: t0.addingTimeInterval(8))
        store.applyAlert(transitions: store.transitions, prChime: store.prChime, excluding: [], at: t0.addingTimeInterval(8))
        render(store, panel, "05-mixed-peek")

        store.refresh(at: t0.addingTimeInterval(14))
        render(store, panel, "06-mixed-collapsed")
        panel.hovering = true
        render(store, panel, "07-mixed-hover")
        panel.hovering = false

        store.refresh(at: t0.addingTimeInterval(30))
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(31))
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(32))

        writeSession(dir, "c", pid: me, status: "busy", ageSec: 5, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        store.refresh(at: t0.addingTimeInterval(40))
        panel.anchor = Anchor(h: .leading, up: true)
        panel.hovering = true
        render(store, panel, "08-bottom-left-hover-expands-up")
        panel.hovering = false
        render(store, panel, "09-bottom-left-collapsed")
    }

    @Test func renderTitles() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects, branch: { $0 == frontend ? "T6IR-14737" : nil })
        let panel = PanelState()
        _ = writeTranscript(projects, cwd: frontend, id: "b", lines: [
            commandLine("/clear", args: ""),
            userLine("sửa lỗi đăng nhập khi token hết hạn và thêm test cho trường hợp refresh thất bại"),
        ])
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "widget", cwd: backend, nameSource: "user")
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 90, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "c", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "15-titles-branch")
    }

    @Test func renderTodo() {
        let dir = makeTempDir(), projects = makeTempDir(), tasks = makeTempDir()
        let store = makeStore(dir: dir, projects: projects, tasks: tasks)
        let panel = PanelState()
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [assistantLine("Đã sửa xong, bạn xem lại diff giúp mình nhé.")])
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 90, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        writeTask(tasks, session: "a", id: 1, status: "completed", subject: "Phase 1")
        writeTask(tasks, session: "a", id: 2, status: "in_progress", subject: "Phase 2", activeForm: "Implementing phase 2")
        writeTask(tasks, session: "a", id: 3, status: "pending", subject: "Phase 3")
        writeTask(tasks, session: "a", id: 4, status: "pending", subject: "Phase 4")
        writeTask(tasks, session: "b", id: 1, status: "in_progress", subject: "Phase 1",
                  activeForm: "Implementing the deadlock retry in the transaction owner and adding a regression test for the lock timeout path")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "16-todo")
    }

    @Test func renderLastReply() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        func transcript(_ id: String, _ cwd: String, _ text: String) {
            _ = writeTranscript(projects, cwd: cwd, id: id, lines: [assistantLine(text)])
        }
        transcript("a", frontend, "Đã sửa xong lỗi đăng nhập và chạy lại toàn bộ test, tất cả đều qua. Bạn xem lại diff giúp mình rồi mình sẽ mở PR nhé, còn một chỗ nhỏ cần bạn quyết định.")
        transcript("c", "/Users/phong/ocr-service", "Mình cần bạn chọn giữa hai hướng: giữ API cũ hay đổi sang bản mới?")
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "10-last-reply-hover")
    }

    @Test func renderLongContent() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        let reply = "Đã xong. **bold** và `code iiWW` xem https://talosix.atlassian.net/browse/T6IR-14500 " + String(repeating: "nội dung dài ", count: 70)
            + " https://bitbucket.org/t/r/pull-requests/6867 https://github.com/x/y/pull/12"
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [userLine("/cook https://talosix.atlassian.net/browse/T6IR-14500 " + String(repeating: "Hãy sửa lỗi đăng nhập ", count: 6)), assistantLine(reply)])
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 90, name: "short-title", cwd: backend, nameSource: "user")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "17-long-content")
    }

    @Test func renderActivity() {
        let dir = makeTempDir(), projects = makeTempDir(), tasks = makeTempDir()
        let store = makeStore(dir: dir, projects: projects, tasks: tasks)
        let panel = PanelState()
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [toolUseLine(id: "t1", name: "Bash", input: ["command": "swift test --filter StoreTests"]), ""])
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [toolUseLine(id: "t2", name: "Bash", input: ["command": String(repeating: "echo long-command ", count: 12)]), ""])
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 90, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 40, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "idle", ageSec: 30, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeTask(tasks, session: "a", id: 1, status: "in_progress", subject: "Phase 4", activeForm: "Implementing phase 4")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "18-activity")
    }

    @Test func renderScroll() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        for i in 0..<12 {
            _ = writeTranscript(projects, cwd: frontend, id: "s\(i)", lines: [assistantLine(String(repeating: "câu trả lời dài \(i) ", count: 40))])
            writeSession(dir, "s\(i)", pid: me, status: "idle", ageSec: Double(30 + i), name: "session-\(i)", cwd: frontend)
        }
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "19-scroll")
    }

    @Test func renderPendingHover() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        let t0 = Date()
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("Đã sửa xong, bạn xem lại diff giúp mình nhé.")])
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"])])
        _ = writeTranscript(projects, cwd: "/Users/phong/ocr-service", id: "c", lines: [
            toolUseLine(id: "t2", name: "Bash", input: ["command": "git add -A\ngit commit -m \"fix\"\ngit push origin main"])])
        _ = writeTranscript(projects, cwd: "/Users/phong/talos-v2", id: "d", lines: [
            toolUseLine(id: "t3", name: "Bash", input: ["command": "ls"]), toolUseLine(id: "t4", name: "Edit", input: ["file_path": "/x"])])
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0)
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 5, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 9, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 12, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: t0.addingTimeInterval(1))
        panel.hovering = true
        render(store, panel, "11-pending-hover")
    }

    @Test func renderCollapsedOpacity() {
        let dir = makeTempDir()
        let store = makeStore(dir: dir)
        let panel = PanelState()
        writeSession(dir, "a", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 5, name: "a", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 5, name: "b", cwd: backend)
        writeSession(dir, "c", pid: me, status: "busy", ageSec: 5, name: "c", cwd: backend)
        store.refresh(at: Date())
        for (v, name) in [(0.0, "0"), (0.25, "25"), (0.92, "92")] {
            store.bgOpacity = v
            render(store, panel, "20-collapsed-opacity-\(name)")
        }
        store.bgOpacity = 0
        panel.hovering = true
        render(store, panel, "20-expanded-at-0")
    }

    @Test func renderCollapsedTwoDigits() {
        let dir = makeTempDir()
        let store = makeStore(dir: dir)
        let panel = PanelState()
        for i in 0..<10 { writeSession(dir, "w\(i)", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 5, name: "w\(i)", cwd: frontend) }
        for i in 0..<11 { writeSession(dir, "i\(i)", pid: me, status: "idle", ageSec: 5, name: "i\(i)", cwd: frontend) }
        for i in 0..<12 { writeSession(dir, "b\(i)", pid: me, status: "busy", ageSec: 5, name: "b\(i)", cwd: frontend) }
        store.refresh(at: Date())
        render(store, panel, "21-collapsed-two-digits")
    }

    private func newMarkStore(_ dir: URL, waiting: Int, idle: Int, busy: Int) -> Store {
        let store = makeStore(dir: dir)
        for i in 0..<waiting { writeSession(dir, "w\(i)", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 5, name: "w\(i)", cwd: frontend) }
        for i in 0..<idle { writeSession(dir, "i\(i)", pid: me, status: "idle", ageSec: 5, name: "i\(i)", cwd: backend) }
        for i in 0..<busy { writeSession(dir, "b\(i)", pid: me, status: "busy", ageSec: 5, name: "b\(i)", cwd: backend) }
        store.refresh(at: Date())
        return store
    }

    @Test func renderCollapsedNew() {
        let store = newMarkStore(makeTempDir(), waiting: 1, idle: 2, busy: 1)
        store.unseen = ["w0": NewMark(claude: true), "i1": NewMark(claude: true)]
        render(store, PanelState(), "23-collapsed-new")
    }

    @Test func renderCollapsedNewTwoDigits() {
        let store = newMarkStore(makeTempDir(), waiting: 3, idle: 12, busy: 1)
        store.unseen = Dictionary(uniqueKeysWithValues: (0..<12).map { ("i\($0)", NewMark(claude: true)) })
        render(store, PanelState(), "24-collapsed-new-two-digits")
    }

    @Test func renderCollapsedNoNew() {
        let store = newMarkStore(makeTempDir(), waiting: 1, idle: 2, busy: 1)
        store.unseen = [:]
        render(store, PanelState(), "25-collapsed-no-new")
    }

    @Test func renderExpandedNew() {
        let store = newMarkStore(makeTempDir(), waiting: 1, idle: 2, busy: 1)
        store.unseen = ["i1": NewMark(claude: true)]
        let panel = PanelState()
        panel.hovering = true
        render(store, panel, "26-expanded-new")
    }

    @Test func renderFinishedCollapsed() {
        let dir = makeTempDir()
        let store = makeStore(dir: dir)
        let panel = PanelState()
        let t0 = Date()
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "busy", ageSec: 30, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        store.refresh(at: t0)
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "talo6-dct-backend-3f", cwd: backend)
        store.refresh(at: t0.addingTimeInterval(1))
        store.refresh(at: t0.addingTimeInterval(10))
        #expect(!panel.expanded(store))
        render(store, panel, "14-finished-collapsed")
    }

    @Test func renderActions() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        let six = (1...6).map { "echo line \($0)" }.joined(separator: "\n")
        let twenty = (1...20).map { "echo line \($0)" }.joined(separator: "\n")
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [toolUseLine(id: "t1", name: "Bash", input: ["command": "npm test"])])
        _ = writeTranscript(projects, cwd: "/Users/phong/ocr-service", id: "c", lines: [toolUseLine(id: "t2", name: "Bash", input: ["command": six])])
        _ = writeTranscript(projects, cwd: "/Users/phong/talos-v2", id: "d", lines: [toolUseLine(id: "t3", name: "Bash", input: ["command": twenty])])
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [assistantLine("Đã sửa xong, bạn xem lại diff giúp mình nhé.")])
        _ = writeTranscript(projects, cwd: "/Users/phong/api", id: "e", lines: [assistantLine("Bạn muốn mình làm tiếp phần nào?")])
        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 5, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 9, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 12, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "e", pid: me, status: "idle", ageSec: 40, name: "api-5e", cwd: "/Users/phong/api")
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "12-actions-hover")

        panel.editing = Store.Editing(id: "b", kind: .deny, toolId: "t1")
        store.drafts["b"] = nil
        render(store, panel, "13-deny-reason")
    }

    @Test func renderPRLines() {
        let dir = makeTempDir(), projects = makeTempDir()
        let api = FakeBitbucket()
        let repo = "t6engineering/talo6-dct-backend"
        var open = prJSON(id: 7571, repo: repo, comments: 23, reviewers: 5)
        open["participants"] = (open["participants"] as! [[String: Any]]).map { $0["role"] as? String == "REVIEWER" && $0["approved"] as? Bool == true ? $0.merging(["approved": false]) { $1 } : $0 }
        api.set("/2.0/user", json: ["uuid": "{me}"])
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [open, prJSON(id: 7567, repo: repo)]])
        api.set("/statuses", json: ["values": []])
        let t0 = Date()
        let stamp = ISO8601DateFormatter().string(from: t0.addingTimeInterval(-180))
        api.set("/comments", json: ["values": [comment("{tin}", "Tin Tran", stamp)]])
        let links = [7571, 7567].map { "https://bitbucket.org/\(repo)/pull-requests/\($0)" }.joined(separator: " ")
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [assistantLine("Đã push, xem " + links), ""])
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        let store = makeStore(dir: dir, projects: projects, prs: PRMonitor(api: api, queue: nil, remote: { _ in nil }))
        let panel = PanelState()
        panel.hovering = true
        store.refresh(at: t0)
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [open]])
        api.set("/talo6-dct-backend/pullrequests/7567?", json: prJSON(id: 7567, repo: repo, state: "MERGED"))
        store.refresh(at: t0.addingTimeInterval(61))
        render(store, panel, "22-pr-lines")

        api.failAll = .transport
        store.refresh(at: t0.addingTimeInterval(122))
        store.refresh(at: t0.addingTimeInterval(420))
        render(store, panel, "22-pr-lines-stale")
    }

    @Test func ticketRowOverflow() {
        let dir = makeTempDir(), projects = makeTempDir()
        let api = FakeBitbucket()
        let repo = "t6engineering/talo6-dct-backend"
        api.set("/2.0/user", json: ["uuid": "{me}"])
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, repo: repo, comments: 1)]])
        api.set("/statuses", json: ["values": [rabbitStatus(description: "Review completed")]])
        api.set("/comments", json: ["values": pendingFinding(count: 2, at: "2026-11-03T03:40:00.000000+00:00")])
        let keys = (101...108).map { "T6IR-\($0)" }
        let urls = keys.map { "https://talo6.atlassian.net/browse/\($0)" }.joined(separator: " ")
        let pr = "https://bitbucket.org/\(repo)/pull-requests/7571"
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [userLine(urls), assistantLine("Đã push, xem " + pr), ""])
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        let store = makeStore(dir: dir, projects: projects, prs: PRMonitor(api: api, queue: nil, remote: { _ in nil }))
        let panel = PanelState()
        panel.hovering = true
        store.refresh(at: Date())
        render(store, panel, "23-ticket-row-overflow")
    }

    @Test func prAwaitingApproval() {
        let dir = makeTempDir(), projects = makeTempDir()
        let api = FakeBitbucket()
        let repo = "t6engineering/talo6-dct-backend"
        let c1 = "2026-11-03T03:40:00.000000+00:00", c2 = "2026-11-03T03:41:00.000000+00:00", c3 = "2026-11-03T03:42:00.000000+00:00"
        api.set("/2.0/user", json: ["uuid": "{me}"])
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7583, repo: repo, comments: 3, rabbitApproved: false)]])
        api.set("/statuses", json: ["values": [rabbitStatus(description: "Review completed", created: "2026-11-03T03:33:15.111541+00:00")]])
        api.set("/comments", json: ["values": [
            rabbitComment("**Actionable comments posted: 1**", at: c1), rabbitInline(id: 1, "nit", at: c1),
            humanReply(id: 2, parent: 1, "won't fix", at: c2), rabbitInline(id: 3, "withdrawn", at: c3, parent: 1)]])
        let pr = "https://bitbucket.org/\(repo)/pull-requests/7583"
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [userLine("Xem " + pr), assistantLine("Đã push, xem " + pr), ""])
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        let store = makeStore(dir: dir, projects: projects, prs: PRMonitor(api: api, queue: nil, remote: { _ in nil }))
        let panel = PanelState()
        panel.hovering = true
        store.refresh(at: Date())
        render(store, panel, "31-pr-awaiting-approval")
    }

    /// Two sessions: "b" with two open PRs (7571, 7567), "c" plain, so unmarked cards show next to marked ones.
    private func renderMarks(_ name: String, _ marks: [String: NewMark]) {
        let dir = makeTempDir(), projects = makeTempDir()
        let api = FakeBitbucket()
        let repo = "t6engineering/talo6-dct-backend"
        api.set("/2.0/user", json: ["uuid": "{me}"])
        api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": [prJSON(id: 7571, repo: repo, comments: 2), prJSON(id: 7567, repo: repo)]])
        api.set("/statuses", json: ["values": []])
        let links = [7571, 7567].map { "https://bitbucket.org/\(repo)/pull-requests/\($0)" }.joined(separator: " ")
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [assistantLine("Đã push, xem " + links), ""])
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "idle", ageSec: 40, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        let store = makeStore(dir: dir, projects: projects, prs: PRMonitor(api: api, queue: nil, remote: { _ in nil }))
        store.refresh(at: Date())
        store.unseen = marks
        let panel = PanelState()
        panel.hovering = true
        render(store, panel, name)
    }


    @Test func renderMarkClaude() { renderMarks("32-mark-claude", ["b": NewMark(claude: true)]) }
    @Test func renderMarkPR() { renderMarks("33-mark-pr", ["b": NewMark(prs: [prRef(7571)])]) }
    @Test func renderMarkBoth() { renderMarks("34-mark-both", ["b": NewMark(claude: true, prs: [prRef(7571)])]) }
    @Test func renderMarkPRHidden() {
        renderMarks("35-mark-pr-hidden", ["b": NewMark(prs: [prRef(9999)])])
    }

    @Test func renderFreshLongTitle() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        let panel = PanelState()
        _ = writeTranscript(projects, cwd: frontend, id: "a", lines: [
            userLine("Hãy sửa lỗi đăng nhập khi token hết hạn, thêm test cho luồng làm mới token và cập nhật tài liệu hướng dẫn cho đội"),
            assistantLine("Đã sửa xong, test đều qua."),
        ])
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.unseen = ["a": NewMark(claude: true)]
        store.refresh(at: Date())
        panel.hovering = true
        render(store, panel, "27-fresh-long-title")
    }

    private static func swatch(_ c: NSColor) -> NSImage {
        NSImage(size: NSSize(width: 16, height: 16), flipped: false) { r in
            c.setFill()
            NSBezierPath(roundedRect: r.insetBy(dx: 1, dy: 1), xRadius: 3, yRadius: 3).fill()
            return true
        }
    }

    private static let iconBug = URL(string: "https://talo6.atlassian.net/icon/bug")!
    private static let iconStory = URL(string: "https://talo6.atlassian.net/icon/story")!
    private static let iconTask = URL(string: "https://talo6.atlassian.net/icon/task")!

    /// One session `b` mentioning seven tickets (the last has no type or icon data) and PRs; icons are put straight into the store.
    private func linkStore(jira connected: Bool) -> (Store, PanelState) {
        let dir = makeTempDir(), projects = makeTempDir()
        let keys = (101...108).map { "T6IR-\($0)" }
        let urls = keys.map { "https://talo6.atlassian.net/browse/\($0)" }.joined(separator: " ")
        let prs = "https://github.com/a/b/pull/1 https://github.com/a/b/pull/2"
        _ = writeTranscript(projects, cwd: backend, id: "b", lines: [userLine(urls), assistantLine(prs), ""])
        writeSession(dir, "b", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        let api = FakeJira()
        let spec: [(String, String, String?, String?, (String, URL)?)] = [
            ("T6IR-101", "Add retry to the sync worker", "To Do", "new", ("Bug", Self.iconBug)),
            ("T6IR-102", "Fix site scope filter on the dashboard", "In Progress", "indeterminate", nil),
            ("T6IR-103", "Remove the legacy export endpoint", "Done", "done", ("Task", Self.iconTask)),
            ("T6IR-104", "Cache the permission lookups", "Ready for Grooming with Engineering", "new", ("Bug", Self.iconBug)),
            ("T6IR-105", "Odd workflow status", "Blocked", "weird", ("Story", Self.iconStory)),
            ("T6IR-106", "Ticket with a status and no category", "Review", nil, ("Task", Self.iconTask)),
            ("T6IR-107", "Ticket whose icon never arrived", "To Do", "new", nil),
            ("T6IR-108", String(repeating: "Very long ticket title that overflows the row ", count: 3), "To Do", "new", ("Task", Self.iconTask)),
        ]
        for (k, title, status, cat, type) in spec {
            api.issues[k] = (title, status)
            if let cat { api.categories[k] = cat }
            if let type { api.types[k] = (type.0, type.1.absoluteString) }
        }
        let store = makeStore(dir: dir, projects: projects, jira: connected ? JiraMonitor(api: api, queue: nil) : nil)
        store.jiraProjects = ["T6IR": "talo6.atlassian.net"]
        store.refresh(at: Date())
        if connected {
            store.issueIcons = [Self.iconBug: Self.swatch(.systemRed), Self.iconStory: Self.swatch(.systemGreen),
                                Self.iconTask: Self.swatch(.systemBlue)]
            // Bitbucket is not faked here: swap the untracked PR links for one PR in each state.
            let pr = { (id: Int, status: String, tone: ItemTone) in
                LinkItem(text: "PR #\(id)", url: URL(string: "https://github.com/a/b/pull/\(id)")!, kind: .pr, code: "#\(id)",
                         title: "Rework the \(status) pull request", status: status, tone: tone)
            }
            let i = store.sessions[0].menu.firstIndex { $0.title == "PR" }!
            store.sessions[0].menu[i] = LinkSection(title: "PR", items: [pr(41, "mở", .open), pr(42, "đã merge", .merged), pr(43, "đã đóng", .closed)])
        }
        let panel = PanelState()
        panel.hovering = true
        return (store, panel)
    }

    @Test func renderLinkMenuRow() {
        let (store, panel) = linkStore(jira: true)
        #expect(store.sessions.first?.menuCount == 11)
        render(store, panel, "28-link-menu-row")
    }

    @Test func renderLinkListOpen() {
        let (store, panel) = linkStore(jira: true)
        let s = store.sessions[0]
        panel.openMenus = [s.id]
        #expect(s.menuCount == 11)
        #expect(s.menu.map(\.title) == ["Bạn nhắc", "PR"])
        #expect(s.menu.reduce(0) { $0 + $1.items.count } == s.menuCount)
        render(store, panel, "29-link-list-open")
    }

    @Test func renderLinkListConnect() {
        let (store, panel) = linkStore(jira: false)
        let s = store.sessions[0]
        panel.openMenus = [s.id]
        #expect(s.menuNeedsJira)
        render(store, panel, "30-link-list-connect")
    }

    @Test func openMenusClearedOnCollapse() async throws {
        let (store, panel) = linkStore(jira: true)
        panel.openMenus = [store.sessions[0].id]
        let host = NSHostingView(rootView: WidgetView(store: store, panel: panel))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 640), styleMask: [.borderless], backing: .buffered, defer: false)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        panel.hovering = false
        host.layoutSubtreeIfNeeded()
        for _ in 0..<20 where !panel.openMenus.isEmpty { try await Task.sleep(for: .milliseconds(50)) }
        #expect(panel.openMenus.isEmpty)
    }
}
