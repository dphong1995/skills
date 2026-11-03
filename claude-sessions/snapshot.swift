import AppKit
import SwiftUI

private var failures = 0

private func check(_ ok: Bool, _ what: String) {
    print(ok ? "PASS" : "FAIL", what)
    if !ok { failures += 1 }
}

private func write(_ dir: URL, _ file: String, pid: Int32, status: String, waitingFor: String? = nil, ageSec: Double, name: String, cwd: String) {
    var o: [String: Any] = ["pid": pid, "sessionId": file, "name": name, "cwd": cwd, "status": status,
                            "statusUpdatedAt": (Date().timeIntervalSince1970 - ageSec) * 1000]
    if let waitingFor { o["waitingFor"] = waitingFor }
    try! JSONSerialization.data(withJSONObject: o).write(to: dir.appendingPathComponent("\(file).json"))
}

@MainActor
private func render(_ store: Store, _ out: URL, _ name: String) {
    let view = ZStack {
        LinearGradient(colors: [Color(red: 0.25, green: 0.3, blue: 0.45), Color(red: 0.1, green: 0.12, blue: 0.2)],
                       startPoint: .top, endPoint: .bottom)
        WidgetView(store: store)
    }
    .frame(width: Layout.canvas.width, height: Layout.canvas.height)
    let r = ImageRenderer(content: view)
    r.scale = 2
    guard let cg = r.cgImage else { print("FAIL render", name); failures += 1; return }
    let data = NSBitmapImageRep(cgImage: cg).representation(using: .png, properties: [:])!
    try! data.write(to: out.appendingPathComponent("\(name).png"))
}

@MainActor
func runSnapshots() {
    let out = URL(fileURLWithPath: CommandLine.arguments[1])
    let dir = out.appendingPathComponent("sessions")
    try? FileManager.default.removeItem(at: dir)
    try! FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let me = getpid()
    let store = Store(dir: dir)
    let t0 = Date()

    store.refresh(at: t0)
    check(store.sessions.isEmpty, "empty dir -> no sessions")
    render(store, out, "01-empty-collapsed")

    write(dir, "a", pid: me, status: "busy", ageSec: 42, name: "talo6-dct-frontend-d7", cwd: "/Users/phong/talo6-dct-frontend")
    write(dir, "b", pid: me, status: "busy", ageSec: 300, name: "talo6-dct-backend-3f", cwd: "/Users/phong/talo6-dct-backend")
    write(dir, "dead", pid: 999_999, status: "idle", ageSec: 10, name: "dead", cwd: "/tmp")
    store.refresh(at: t0)
    check(store.sessions.count == 2, "dead pid filtered (2 live sessions)")
    check(store.attention.isEmpty && !store.expanded, "first sight of busy sessions -> no attention, collapsed")
    render(store, out, "02-all-busy-collapsed")

    write(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: "/Users/phong/talo6-dct-frontend")
    store.refresh(at: t0.addingTimeInterval(1))
    check(store.attention == ["a"], "busy -> idle flags attention on that session")
    check(store.expanded, "busy -> idle peeks open")
    check(store.sessions.first?.id == "a", "idle sorts above busy")
    render(store, out, "03-just-finished-peek")

    store.refresh(at: t0.addingTimeInterval(7))
    check(!store.expanded, "peek closes after 5s")
    check(store.attention == ["a"], "attention kept after peek closes")
    render(store, out, "04-after-peek-collapsed")

    write(dir, "b", pid: me, status: "waiting", waitingFor: "permission prompt", ageSec: 0, name: "talo6-dct-backend-3f", cwd: "/Users/phong/talo6-dct-backend")
    write(dir, "c", pid: me, status: "waiting", waitingFor: "input needed", ageSec: 75, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
    write(dir, "d", pid: me, status: "busy", ageSec: 3700, name: "talo-v2-9c", cwd: "/Users/phong/talos-v2")
    store.refresh(at: t0.addingTimeInterval(8))
    check(store.attention == ["a", "b"], "busy -> waiting flags attention; new session c does not")
    check(store.sessions.map(\.id) == ["c", "b", "a", "d"], "order: waiting, idle, busy; by name within a status")
    check(store.sessions.first { $0.id == "b" }.map(label) == "chờ duyệt", "permission prompt -> chờ duyệt")
    check(store.sessions.first { $0.id == "c" }.map(label) == "chờ trả lời", "input needed -> chờ trả lời")
    render(store, out, "05-mixed-peek")

    store.refresh(at: t0.addingTimeInterval(14))
    render(store, out, "06-mixed-collapsed")
    store.hovering = true
    check(store.expanded, "hover expands")
    render(store, out, "07-mixed-hover")
    store.hovering = false

    store.refresh(at: t0.addingTimeInterval(30))
    check(store.attention.isEmpty, "attention expires after 20s")

    write(dir, "a", pid: me, status: "busy", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: "/Users/phong/talo6-dct-frontend")
    store.refresh(at: t0.addingTimeInterval(31))
    write(dir, "a", pid: me, status: "idle", ageSec: 0, name: "talo6-dct-frontend-d7", cwd: "/Users/phong/talo6-dct-frontend")
    store.refresh(at: t0.addingTimeInterval(32))
    check(store.attention == ["a"] && store.expanded, "second turn finishing peeks again")

    write(dir, "c", pid: me, status: "busy", ageSec: 5, name: "ocr-service-a1", cwd: "/Users/phong/ocr-service")
    store.refresh(at: t0.addingTimeInterval(40))
    store.anchor = Anchor(h: .leading, up: true)
    store.hovering = true
    render(store, out, "08-bottom-left-hover-expands-up")
    store.hovering = false
    render(store, out, "09-bottom-left-collapsed")

    let scr = NSRect(x: 0, y: 0, width: 1512, height: 982)
    check(Anchor.near(CGPoint(x: 1400, y: 950), in: scr) == Anchor(h: .trailing, up: false), "top-right -> grows down, right-aligned")
    check(Anchor.near(CGPoint(x: 100, y: 50), in: scr) == Anchor(h: .leading, up: true), "bottom-left -> grows up, left-aligned")
    check(Anchor.near(CGPoint(x: 756, y: 950), in: scr) == Anchor(h: .center, up: false), "top-center -> centered")
    for a in [Anchor(h: .leading, up: false), Anchor(h: .center, up: true), Anchor(h: .trailing, up: true)] {
        let o = CGPoint(x: 600, y: 300)
        let f = Layout.canvasFrame(pillOrigin: o, anchor: a)
        check(Layout.pill(in: f, size: Layout.collapsed, anchor: a).origin == o, "canvas/pill round trip \(a.h) up=\(a.up)")
    }
    let corner = Layout.canvasFrame(pillOrigin: Layout.clamp(pillOrigin: CGPoint(x: 1500, y: 975), to: scr), anchor: Anchor(h: .trailing, up: false))
    let open = Layout.pill(in: corner, size: Layout.expanded(rows: 6), anchor: Anchor(h: .trailing, up: false))
    check(scr.contains(open), "dragged past top-right corner -> clamped, expanded list stays on screen")

    print(failures == 0 ? "ALL PASS" : "\(failures) FAILED")
    exit(failures == 0 ? 0 : 1)
}
