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
private func render(_ store: Store, _ name: String) {
    let view = ZStack {
        LinearGradient(colors: [Color(red: 0.25, green: 0.3, blue: 0.45), Color(red: 0.1, green: 0.12, blue: 0.2)],
                       startPoint: .top, endPoint: .bottom)
        WidgetView(store: store)
    }
    .frame(width: Layout.canvas.width, height: Layout.canvas.height)
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
        let t0 = Date()

        store.refresh(at: t0)
        render(store, "01-empty-collapsed")

        writeSession(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
        store.refresh(at: t0)
        render(store, "02-all-busy-collapsed")

        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(1))
        render(store, "03-just-finished-peek")

        store.refresh(at: t0.addingTimeInterval(7))
        render(store, "04-after-peek-collapsed")

        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: t0.addingTimeInterval(8))
        render(store, "05-mixed-peek")

        store.refresh(at: t0.addingTimeInterval(14))
        render(store, "06-mixed-collapsed")
        store.hovering = true
        render(store, "07-mixed-hover")
        store.hovering = false

        store.refresh(at: t0.addingTimeInterval(30))
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(31))
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        store.refresh(at: t0.addingTimeInterval(32))

        writeSession(dir, "c", pid: me, status: "busy", ageSec: 5, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        store.refresh(at: t0.addingTimeInterval(40))
        store.anchor = Anchor(h: .leading, up: true)
        store.hovering = true
        render(store, "08-bottom-left-hover-expands-up")
        store.hovering = false
        render(store, "09-bottom-left-collapsed")
    }

    @Test func renderLastReply() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
        func transcript(_ id: String, _ cwd: String, _ text: String) {
            _ = writeTranscript(projects, cwd: cwd, id: id, lines: [assistantLine(text)])
        }
        transcript("a", frontend, "Đã sửa xong lỗi đăng nhập và chạy lại toàn bộ test, tất cả đều qua. Bạn xem lại diff giúp mình rồi mình sẽ mở PR nhé, còn một chỗ nhỏ cần bạn quyết định.")
        transcript("c", "/Users/phong/ocr-service", "Mình cần bạn chọn giữa hai hướng: giữ API cũ hay đổi sang bản mới?")
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        writeSession(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
        store.refresh(at: Date())
        store.hovering = true
        render(store, "10-last-reply-hover")
    }

    @Test func renderUnseenPending() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
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
        store.hovering = true
        render(store, "11-unseen-pending-hover")
    }

    @Test func renderUnseenCollapsed() {
        let dir = makeTempDir()
        let store = makeStore(dir: dir)
        let t0 = Date()
        writeSession(dir, "a", pid: me, status: "busy", ageSec: 30, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "busy", ageSec: 30, name: "talo6-dct-backend-3f", cwd: backend)
        writeSession(dir, "c", pid: me, status: "busy", ageSec: 30, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
        store.refresh(at: t0)
        writeSession(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: frontend)
        writeSession(dir, "b", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 0, name: "talo6-dct-backend-3f", cwd: backend)
        store.refresh(at: t0.addingTimeInterval(1))
        store.refresh(at: t0.addingTimeInterval(10))
        #expect(store.unseen.count == 2 && !store.expanded)
        render(store, "14-unseen-collapsed")
    }

    @Test func renderActions() {
        let dir = makeTempDir(), projects = makeTempDir()
        let store = makeStore(dir: dir, projects: projects)
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
        store.hovering = true
        render(store, "12-actions-hover")

        store.editing = Store.Editing(id: "b", kind: .deny, toolId: "t1")
        store.drafts["b"] = nil
        render(store, "13-deny-reason")
    }
}
