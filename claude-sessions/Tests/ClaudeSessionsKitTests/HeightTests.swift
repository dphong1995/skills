import AppKit
import SwiftUI
import Testing
@testable import ClaudeSessionsKit

/// The height of one rendered card at the real list width.
@MainActor
private func measuredHeight(_ s: Session) -> CGFloat {
    let store = makeStore(dir: makeTempDir())
    store.sessions = [s]
    let view = WidgetView(store: store, panel: PanelState()).row(s)
        .frame(width: Layout.expandedWidth - 20)
        .fixedSize(horizontal: false, vertical: true)
        .environment(\.snapshotRendering, true)
    return NSHostingView(rootView: view).fittingSize.height
}

@MainActor @Test func predictedHeightMatchesRenderedRow() {
    let plain = Session(id: "a", name: "a", cwd: "/x", status: "busy", waitingFor: nil, since: nil)
    var todo = plain
    todo.todo = Todo(done: 1, total: 4, current: "x")
    var activity = plain
    activity.activity = "Bash: ls"
    var both = todo
    both.activity = "Bash: ls"
    var title = plain
    title.firstPrompt = String(repeating: "word ", count: 11)
    // The 13 pt title wraps just past `Layout.titleWrapCount` characters of ordinary text.
    var fits = plain
    fits.firstPrompt = String(String(repeating: "word ", count: 16).prefix(Layout.titleWrapCount))
    var wraps = plain
    wraps.firstPrompt = String(String(repeating: "word ", count: 17).prefix(Layout.titleWrapCount + 4))
    for (name, s) in [("plain", plain), ("todo", todo), ("activity", activity), ("todo+activity", both), ("title 55", title), ("title at wrap count", fits), ("title past wrap count", wraps)] {
        let measured = measuredHeight(s)
        #expect(abs(measured - Layout.height(of: s)) <= 2, "\(name): measured \(measured), predicted \(Layout.height(of: s))")
    }
}
