import AppKit
import SwiftUI
import Testing
@testable import ClaudeSessionsKit

@MainActor
private func clickRow(dragTo dx: CGFloat?) -> [String] {
    let dir = makeTempDir()
    writeSession(dir, "s1", pid: getpid(), status: "idle", ageSec: 5, name: "x", cwd: "/tmp/x")
    let store = makeStore(dir: dir)
    store.refresh(at: Date())
    store.hovering = true
    var recorded: [String] = []
    _ = NSApplication.shared
    let host = DragHostingView(rootView: WidgetView(store: store, onAction: { if case .jump(let id) = $0 { recorded.append(id) } }))
    let panel = WidgetPanel(contentRect: NSRect(origin: .zero, size: Layout.canvas), styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.contentView = host
    host.frame = NSRect(origin: .zero, size: Layout.canvas)
    host.layoutSubtreeIfNeeded()
    panel.orderFrontRegardless()
    // Default anchor is trailing/down: canvas margin + header + list padding put row 0 at 68 pt from the top.
    let p = CGPoint(x: Layout.canvas.width - Layout.margin - 200, y: Layout.canvas.height - 68)
    func ev(_ t: NSEvent.EventType, _ pt: CGPoint) -> NSEvent {
        NSEvent.mouseEvent(with: t, location: pt, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: panel.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
    }
    let down = ev(.leftMouseDown, p)
    if let dx { NSApp.postEvent(ev(.leftMouseDragged, CGPoint(x: p.x + dx, y: p.y)), atStart: false) }
    NSApp.postEvent(ev(.leftMouseUp, CGPoint(x: p.x + (dx ?? 0), y: p.y)), atStart: false)
    host.mouseDown(with: down)
    RunLoop.current.run(until: Date().addingTimeInterval(0.2))
    panel.orderOut(nil)
    return recorded
}

@Suite(.serialized, .disabled("the body runs and records [\"s1\"], but the test process exits right after it, before Swift Testing reports (nested NSApp event loop under swift test); covered by the manual checkpoint")) @MainActor struct ClickTests {
    @Test func tapRecordsSessionId() {
        #expect(clickRow(dragTo: nil) == ["s1"])
    }

    @Test func smallMoveStillClicks() {
        #expect(clickRow(dragTo: 3) == ["s1"])
    }
}
