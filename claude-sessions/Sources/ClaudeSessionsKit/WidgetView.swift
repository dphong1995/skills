import AppKit
import SwiftUI

func elapsed(_ since: Date?, _ now: Date) -> String {
    guard let since else { return "" }
    let s = max(0, Int(now.timeIntervalSince(since)))
    if s < 60 { return "\(s)s" }
    if s < 3600 { return "\(s / 60)m" }
    return String(format: "%dh%02d", s / 3600, (s % 3600) / 60)
}

/// `12p` / `3h` since a session closed.
func closedAgo(_ since: Date, _ now: Date) -> String {
    let s = max(0, Int(now.timeIntervalSince(since)))
    return s < 3600 ? "\(max(1, s / 60))p" : "\(s / 3600)h"
}

/// Inline markdown for a reply; links are dropped (phase buttons open them) and a parse failure shows the text as is.
func markdown(_ s: String) -> AttributedString {
    guard var a = try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) else {
        return AttributedString(s)
    }
    for run in a.runs {
        if run.link != nil { a[run.range].link = nil }
        if run.inlinePresentationIntent?.contains(.code) == true {
            let bold = run.inlinePresentationIntent?.contains(.stronglyEmphasized) == true
            a[run.range].font = Theme.Fonts.mono(bold: bold)
        }
    }
    return a
}

struct SnapshotRenderingKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    /// True while `ImageRenderer` draws a snapshot; AppKit-backed views render as a placeholder there, so views leave them out.
    var snapshotRendering: Bool {
        get { self[SnapshotRenderingKey.self] }
        set { self[SnapshotRenderingKey.self] = newValue }
    }
}

/// Modal editor for the Nhờ Claude prompt template.
func editPrompt(_ store: Store) {
    let alert = NSAlert()
    alert.messageText = "Prompt Nhờ Claude"
    alert.informativeText = "{url} = link PR, {id} = số PR"
    let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 380, height: 120))
    scroll.hasVerticalScroller = true
    scroll.borderType = .bezelBorder
    let text = NSTextView(frame: NSRect(origin: .zero, size: scroll.contentSize))
    text.isRichText = false
    text.minSize = scroll.contentSize
    text.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
    text.isVerticallyResizable = true
    text.isHorizontallyResizable = false
    text.autoresizingMask = [.width]
    text.textContainer?.containerSize = NSSize(width: scroll.contentSize.width, height: CGFloat.greatestFiniteMagnitude)
    text.textContainer?.widthTracksTextView = true
    text.string = store.prPrompt
    scroll.documentView = text
    alert.accessoryView = scroll
    alert.addButton(withTitle: "Lưu")
    alert.addButton(withTitle: "Huỷ")
    alert.addButton(withTitle: "Mặc định")
    alert.window.initialFirstResponder = text
    NSApp.activate(ignoringOtherApps: true)
    switch alert.runModal() {
    case .alertFirstButtonReturn:
        let t = text.string.trimmingCharacters(in: .whitespacesAndNewlines)
        store.prPrompt = t.isEmpty ? PullRequests.defaultPrompt : t
    case .alertThirdButtonReturn:
        store.prPrompt = PullRequests.defaultPrompt
    default: break
    }
}

/// Main menu with only a standard Edit menu. An accessory app shows no menu bar, but AppKit still
/// routes Cmd-X/C/V/A/Z in text views through the main menu's key equivalents; without it they do nothing.
public func editMainMenu() -> NSMenu {
    let edit = NSMenu(title: "Edit")
    edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
    edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
    edit.addItem(.separator())
    edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
    edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
    edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
    edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
    let item = NSMenuItem()
    item.submenu = edit
    let main = NSMenu()
    main.addItem(item)
    return main
}

public enum WidgetAction: Equatable {
    case jump(String)
    case approve(String, toolId: String)
    case deny(String, toolId: String, reason: String)
    case openLink(String, URL)
    case askClaude(String, prompt: String, pr: PRRef)
    /// Types a prompt of the user's own into a finished session.
    case reply(String, text: String)
    /// Sends Esc to a busy session.
    case stop(String)
    /// Answers the Feed question `requestId` through cmux's socket; picks per question and Other text by question index.
    case answer(String, requestId: String, answers: [[String]], other: [Int: String])
    case nudgeRabbit(String, PRRef)
    /// Opens a cmux workspace that resumes a closed session.
    case resume(ClosedSession)
    /// Opens the token dialog of a service; belongs to no session.
    case connect(Service)

    /// The send button this action belongs to; nil for actions that only open things.
    public var sendKey: SendKey? {
        switch self {
        case .askClaude(let id, _, let pr): SendKey(session: id, kind: .askClaude, pr: pr)
        case .nudgeRabbit(let id, let ref): SendKey(session: id, kind: .nudge, pr: ref)
        case .approve(let id, _): SendKey(session: id, kind: .approve)
        case .deny(let id, _, _): SendKey(session: id, kind: .deny)
        case .reply(let id, _): SendKey(session: id, kind: .reply)
        case .stop(let id): SendKey(session: id, kind: .stop)
        case .resume(let c): SendKey(session: c.id, kind: .resume)
        case .answer(let id, _, _, _): SendKey(session: id, kind: .answer)
        case .jump, .openLink, .connect: nil
        }
    }

    public var sessionId: String {
        switch self {
        case .jump(let id), .approve(let id, _), .deny(let id, _, _), .openLink(let id, _), .askClaude(let id, _, _), .reply(let id, _), .stop(let id), .answer(let id, _, _, _), .nudgeRabbit(let id, _): id
        case .resume(let c): c.id
        case .connect: ""
        }
    }
}

public struct WidgetView: View {
    @ObservedObject var store: Store
    @ObservedObject var panel: PanelState
    @FocusState var focused: Bool
    /// `"<session id>#<question index>"` of the `Khác…` field that has the keyboard, or is about to get it.
    @FocusState var otherFocus: String?
    @Environment(\.snapshotRendering) private var snapshotRendering

    let onAction: (WidgetAction) -> Void

    public init(store: Store, panel: PanelState, onAction: @escaping (WidgetAction) -> Void = { _ in }) {
        self.store = store
        self.panel = panel
        self.onAction = onAction
    }

    public var body: some View {
        let expanded = panel.expanded(store)
        let radius: CGFloat = expanded ? 20 : Layout.headerHeight / 2
        let o = expanded ? Layout.cardOpacity : store.bgOpacity

        VStack(spacing: 0) {
            if expanded && panel.anchor.up { list }
            header(expanded: expanded).frame(height: Layout.headerHeight)
            if expanded && !panel.anchor.up { list }
        }
        .frame(width: expanded ? Layout.expandedWidth : Layout.collapsedWidth(store), alignment: panel.anchor.up ? .bottom : .top)
        .frame(height: expanded ? nil : Layout.collapsed.height)
        .fixedSize(horizontal: false, vertical: expanded)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            guard panel.expanded(store) else { return }
            if abs(h - (panel.measuredHeight ?? 0)) > 0.5 { panel.measuredHeight = h }
        }
        .frame(maxHeight: expanded ? Layout.maxExpandedHeight : nil, alignment: panel.anchor.up ? .bottom : .top)
        .fixedSize(horizontal: false, vertical: expanded)
        .background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(expanded ? Theme.Colors.canvas.opacity(Theme.Colors.canvasOpacity) : Color.black.opacity(o)))
        .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous)
            .stroke(Theme.Colors.stroke.opacity(o / Layout.cardOpacity), lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        .shadow(color: .black.opacity(0.4 * o / Layout.cardOpacity), radius: expanded ? 12 : 6, y: 3)
        .contextMenu {
            Menu("Độ đậm nền") {
                ForEach(Layout.bgOpacities, id: \.self) { v in
                    Button("\(store.bgOpacity == v ? "✓ " : "")\(Int((v * 100).rounded()))%") { store.bgOpacity = v }
                }
            }
            Button(store.muted ? "Bật tiếng" : "Tắt tiếng") { store.muted.toggle() }
            Menu("Phím tắt") {
                Menu("Mở widget") { hotkeyItems(HotKeyCombo.openChoices, current: store.hotkeyOpen, failed: store.hotkeyFailed.contains("open")) { store.hotkeyOpen = $0 } }
                Menu("Nhảy session chờ") { hotkeyItems(HotKeyCombo.nextChoices, current: store.hotkeyNext, failed: store.hotkeyFailed.contains("next")) { store.hotkeyNext = $0 } }
            }
            Button("Prompt Nhờ Claude…") { editPrompt(store) }
            Divider()
            ForEach(Service.allCases, id: \.self) { svc in
                Button(store.connectionText(svc)) { onAction(.connect(svc)) }
            }
            Divider()
            Button("Thoát") { NSApp.terminate(nil) }
        }
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: expanded)
        .onChange(of: panel.keyboard) { _, on in
            // Keyboard mode starts on the first card and forgets the selection when it ends.
            panel.selected = on ? store.sessions.first?.id : nil
        }
        .onChange(of: expanded) { _, now in if !now { panel.openMenus = []; panel.closedOpen = false; panel.pruneQuestions(live: store.sessions.compactMap(\.question)) } }
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: store.sessions.count)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: panel.anchor.alignment)
        .padding(Layout.margin)
        .environment(\.colorScheme, .dark)
    }

    @ViewBuilder
    private func hotkeyItems(_ choices: [HotKeyCombo], current: HotKeyCombo?, failed: Bool, set: @escaping (HotKeyCombo?) -> Void) -> some View {
        ForEach(choices, id: \.label) { c in
            let on = c == current
            Button("\(on ? "✓ " : "")\(c.label)\(on && failed ? " (đang bị app khác dùng)" : "")") { set(c) }
        }
        Button("\(current == nil ? "✓ " : "")Tắt") { set(nil) }
    }

    private var list: some View {
        let rows = VStack(spacing: Layout.cardGap) {
            ForEach(store.sessions) { row($0) }
            if store.sessions.isEmpty {
                Text("Không có session nào").font(Theme.Fonts.label).foregroundStyle(Theme.Colors.tertiaryText)
                    .frame(height: Layout.rowHeight)
            }
            if !store.closed.items.isEmpty { closedGroup }
        }
        .padding(.horizontal, 10)
        .padding(panel.anchor.up ? .top : .bottom, Layout.listPaddingFar)
        .padding(panel.anchor.up ? .bottom : .top, Layout.listPaddingNear)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            if abs(h - (panel.listHeight ?? 0)) > 0.5 { panel.listHeight = h }
        }
        let height = min(panel.listHeight ?? (Layout.expanded(store.sessions, open: panel.openMenus, closed: store.closed.items.count, closedOpen: panel.closedOpen, blocks: store.blockStates).height - Layout.headerHeight), Layout.maxListHeight)
        return Group {
            if snapshotRendering {
                rows.fixedSize(horizontal: false, vertical: true)
                    .frame(maxHeight: Layout.maxListHeight, alignment: panel.anchor.up ? .bottom : .top).clipped()
            } else {
                ScrollViewReader { proxy in
                    ScrollView(.vertical) { rows }
                        .onChange(of: panel.selected) { _, id in
                            if let id { withAnimation(.easeOut(duration: 0.15)) { proxy.scrollTo(id) } }
                        }
                }
                .frame(height: height)
            }
        }
        .task(id: panel.dwellTarget(in: store)) {
            guard let id = panel.dwellTarget(in: store) else { return }
            try? await Task.sleep(for: Layout.seenDwell)
            if Task.isCancelled || panel.dwellTarget(in: store) != id { return }
            withAnimation(.easeOut(duration: 0.3)) { store.markSeen(id) }
        }
        .onChange(of: store.sessions.map(\.id)) { old, new in
            panel.noteOrderChange(from: old, to: new)
            panel.reconcileSelection(old: old, new: new)
        }
        .transition(.opacity)
    }

    private var closedGroup: some View {
        let items = store.closed.items
        return VStack(spacing: 0) {
            HStack(spacing: 6) {
                Image(systemName: panel.closedOpen ? "chevron.down" : "chevron.right").font(Theme.Fonts.micro)
                Text("Vừa đóng (\(items.count))").font(Theme.Fonts.smallStrong)
                Spacer(minLength: 0)
            }
            .foregroundStyle(Theme.Colors.secondaryText)
            .padding(.horizontal, 6)
            .frame(height: Layout.closedRowHeight)
            .contentShape(Rectangle())
            .onTapGesture { panel.closedOpen.toggle() }
            if panel.closedOpen {
                ForEach(items, id: \.id) { c in
                    let send = store.sendState(SendKey(session: c.id, kind: .resume))
                    HStack(spacing: 10) {
                        Text(c.title).font(Theme.Fonts.body.weight(.medium)).foregroundStyle(Theme.Colors.primaryText).lineLimit(1)
                        Text(c.subtitle).font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText).lineLimit(1)
                        Spacer(minLength: 12)
                        switch send {
                        case .sending?: Text("Đang mở…").font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText).fixedSize()
                        case .failed?: Text("Lỗi — đã copy lệnh").font(Theme.Fonts.small).foregroundStyle(.red).fixedSize()
                        case .sent?, nil:
                            Text("đóng \(closedAgo(c.closedAt, store.now))").font(Theme.Fonts.small.monospacedDigit())
                                .foregroundStyle(Theme.Colors.secondaryText).fixedSize()
                        }
                    }
                    .padding(.horizontal, 6)
                    .frame(height: Layout.closedRowHeight)
                    .background(Theme.Colors.card, in: RoundedRectangle(cornerRadius: 6))
                    .contentShape(Rectangle())
                    .onTapGesture { if send != .sending { onAction(.resume(c)) } }
                }
            }
        }
    }

}

public final class DragHostingView<V: View>: NSHostingView<V> {
    public var onDragEnd: ((_ start: CGPoint, _ raw: CGPoint, _ clamped: CGPoint) -> Void)?
    public var clampOrigin: ((CGPoint) -> CGPoint)?
    private var replaying = false
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    public override func mouseDown(with down: NSEvent) {
        guard !replaying, let window else { return super.mouseDown(with: down) }
        let mouse0 = window.convertPoint(toScreen: down.locationInWindow)
        let start = window.frame.origin
        var moving = false
        var lastRaw = start
        var lastClamped = start
        while let e = window.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
            if e.type == .leftMouseUp {
                if moving {
                    onDragEnd?(start, lastRaw, lastClamped)
                    return
                }
                // A click: replay the press and release so SwiftUI sees them through the normal event path.
                replaying = true
                window.sendEvent(down)
                replaying = false
                window.sendEvent(e)
                return
            }
            if !moving && Layout.isDrag(from: mouse0, to: NSEvent.mouseLocation) { moving = true }
            if moving {
                let raw = Layout.dragOrigin(start: start, mouseDown: mouse0, mouse: NSEvent.mouseLocation)
                let clamped = clampOrigin?(raw) ?? raw
                lastRaw = raw
                lastClamped = clamped
                window.setFrameOrigin(clamped)
            }
        }
    }
}

public final class WidgetPanel: NSPanel {
    public override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
    public weak var state: PanelState?
    public override var canBecomeKey: Bool { state?.wantsKey == true }

    private var keyMonitor: Any?
    /// Routes key presses to `handler` (nil result = consumed) until `stopKeyMonitor`.
    public func startKeyMonitor(_ handler: @escaping (NSEvent) -> NSEvent?) {
        stopKeyMonitor()
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown, handler: handler)
    }
    public func stopKeyMonitor() {
        if let m = keyMonitor { NSEvent.removeMonitor(m) }
        keyMonitor = nil
    }
    public override func close() {
        stopKeyMonitor()
        super.close()
    }
    deinit { stopKeyMonitor() }
}
