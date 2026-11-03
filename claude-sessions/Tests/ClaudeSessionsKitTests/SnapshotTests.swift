import AppKit
import SwiftUI
import Testing
@testable import ClaudeSessionsKit

private let me = getpid()

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
        let dir = makeSessionsDir()
        let store = Store(dir: dir)
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
}
