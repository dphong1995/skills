import AppKit
import Combine
import SwiftUI

struct Session: Identifiable, Equatable {
    let id: String
    let name: String
    let cwd: String
    let status: String
    let waitingFor: String?
    let since: Date?
}

enum HAnchor { case leading, center, trailing }

struct Anchor: Equatable {
    var h: HAnchor = .trailing
    var up = false

    var alignment: Alignment {
        switch (h, up) {
        case (.leading, false): .topLeading
        case (.center, false): .top
        case (.trailing, false): .topTrailing
        case (.leading, true): .bottomLeading
        case (.center, true): .bottom
        case (.trailing, true): .bottomTrailing
        }
    }

    static func near(_ p: CGPoint, in s: NSRect) -> Anchor {
        let third = s.width / 3
        let h: HAnchor = p.x < s.minX + third ? .leading : p.x > s.maxX - third ? .trailing : .center
        return Anchor(h: h, up: p.y < s.midY)
    }
}

final class Store: ObservableObject {
    @Published var sessions: [Session] = []
    @Published var attention: Set<String> = []
    @Published var hovering = false
    @Published var now = Date()
    @Published var peekUntil: Date?
    @Published var anchor = Anchor()

    var expanded: Bool { hovering || (peekUntil.map { now < $0 } ?? false) }

    private var lastStatus: [String: String] = [:]
    private var attentionAt: [String: Date] = [:]
    private let dir: URL

    init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions")) {
        self.dir = dir
    }

    func refresh(at date: Date = Date()) {
        now = date
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        var list: [Session] = []
        for file in files where file.pathExtension == "json" {
            guard let data = try? Data(contentsOf: file),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let pid = (o["pid"] as? NSNumber)?.int32Value,
                  kill(pid, 0) == 0 || errno == EPERM,
                  (o["spare"] as? Bool) != true
            else { continue }
            let ms = (o["statusUpdatedAt"] as? NSNumber)?.doubleValue
            list.append(Session(
                id: o["sessionId"] as? String ?? String(pid),
                name: o["name"] as? String ?? "pid \(pid)",
                cwd: o["cwd"] as? String ?? "",
                status: o["status"] as? String ?? "",
                waitingFor: o["waitingFor"] as? String,
                since: ms.map { Date(timeIntervalSince1970: $0 / 1000) }
            ))
        }

        for s in list {
            if lastStatus[s.id] == "busy" && (s.status == "idle" || s.status == "waiting") {
                attentionAt[s.id] = now
                peekUntil = now.addingTimeInterval(5)
            }
            if s.status == "busy" { attentionAt[s.id] = nil }
            lastStatus[s.id] = s.status
        }
        let live = Set(list.map(\.id))
        attentionAt = attentionAt.filter { live.contains($0.key) && now.timeIntervalSince($0.value) < 20 }
        let newAttention = Set(attentionAt.keys)
        if newAttention != attention { attention = newAttention }

        func rank(_ s: Session) -> Int {
            switch s.status { case "waiting": 0; case "idle": 1; case "busy": 2; default: 3 }
        }
        list.sort { (rank($0), $0.name) < (rank($1), $1.name) }
        if list != sessions { sessions = list }
    }
}

enum Layout {
    static let margin: CGFloat = 14
    static let canvas = CGSize(width: 420 + margin * 2, height: 440)
    static let rowHeight: CGFloat = 36
    static let headerHeight: CGFloat = 32
    static let collapsed = CGSize(width: 190, height: headerHeight)

    static func expanded(rows: Int) -> CGSize {
        CGSize(width: 420, height: headerHeight + 4 + CGFloat(max(rows, 1)) * rowHeight + 10)
    }

    static func size(_ store: Store) -> CGSize {
        store.expanded ? expanded(rows: store.sessions.count) : collapsed
    }

    static func pill(in f: NSRect, size: CGSize, anchor: Anchor) -> NSRect {
        let x: CGFloat
        switch anchor.h {
        case .leading: x = f.minX + margin
        case .center: x = f.midX - size.width / 2
        case .trailing: x = f.maxX - margin - size.width
        }
        let y = anchor.up ? f.minY + margin : f.maxY - margin - size.height
        return NSRect(x: x, y: y, width: size.width, height: size.height)
    }

    static func canvasFrame(pillOrigin o: CGPoint, anchor: Anchor) -> NSRect {
        let c = collapsed
        let x: CGFloat
        switch anchor.h {
        case .leading: x = o.x - margin
        case .center: x = o.x + c.width / 2 - canvas.width / 2
        case .trailing: x = o.x + c.width + margin - canvas.width
        }
        let y = anchor.up ? o.y - margin : o.y + c.height + margin - canvas.height
        return NSRect(x: x, y: y, width: canvas.width, height: canvas.height)
    }

    static func clamp(pillOrigin o: CGPoint, to s: NSRect) -> CGPoint {
        CGPoint(x: min(max(o.x, s.minX), s.maxX - collapsed.width),
                y: min(max(o.y, s.minY), s.maxY - collapsed.height))
    }
}

func elapsed(_ since: Date?, _ now: Date) -> String {
    guard let since else { return "" }
    let s = max(0, Int(now.timeIntervalSince(since)))
    if s < 60 { return "\(s)s" }
    if s < 3600 { return "\(s / 60)m" }
    return String(format: "%dh%02d", s / 3600, (s % 3600) / 60)
}

func color(_ status: String) -> Color {
    switch status {
    case "waiting": .red
    case "idle": .orange
    case "busy": Color(red: 0.35, green: 0.6, blue: 1)
    default: .gray
    }
}

func label(_ s: Session) -> String {
    switch s.status {
    case "waiting": s.waitingFor == "permission prompt" ? "chờ duyệt" : "chờ trả lời"
    case "idle": "xong"
    case "busy": "đang chạy"
    default: s.status.isEmpty ? "?" : s.status
    }
}

struct PulseDot: View {
    let color: Color
    let pulsing: Bool

    var body: some View {
        TimelineView(.animation(paused: !pulsing)) { ctx in
            let phase = pulsing ? ctx.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1.2) / 1.2 : 1
            Circle().fill(color).frame(width: 8, height: 8)
                .overlay(Circle().stroke(color.opacity(0.8 * (1 - phase)), lineWidth: 2).scaleEffect(1 + 1.4 * phase))
        }
    }
}

struct WidgetView: View {
    @ObservedObject var store: Store

    var body: some View {
        let expanded = store.expanded
        let size = Layout.size(store)
        let radius: CGFloat = expanded ? 20 : Layout.headerHeight / 2
        let hot = !store.attention.isEmpty

        VStack(spacing: 0) {
            if expanded && store.anchor.up { list }
            header(expanded: expanded).frame(height: Layout.headerHeight)
            if expanded && !store.anchor.up { list }
        }
        .frame(width: size.width, height: size.height, alignment: store.anchor.up ? .bottom : .top)
        .background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Color.black.opacity(0.92)))
        .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous)
            .stroke(hot ? Color.orange : Color.white.opacity(0.14), lineWidth: hot ? 2 : 1))
        .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
        .shadow(color: .black.opacity(0.4), radius: expanded ? 12 : 6, y: 3)
        .contextMenu { Button("Thoát") { NSApp.terminate(nil) } }
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: expanded)
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: store.sessions.count)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: store.anchor.alignment)
        .padding(Layout.margin)
        .environment(\.colorScheme, .dark)
    }

    private var list: some View {
        VStack(spacing: 0) {
            ForEach(store.sessions) { row($0) }
            if store.sessions.isEmpty {
                Text("Không có session nào").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
                    .frame(height: Layout.rowHeight)
            }
        }
        .padding(.horizontal, 10)
        .padding(store.anchor.up ? .top : .bottom, 10)
        .padding(store.anchor.up ? .bottom : .top, 4)
        .transition(.opacity)
    }

    private var urgent: (color: Color, pulsing: Bool) {
        let st = Set(store.sessions.map(\.status))
        if st.contains("waiting") { return (color("waiting"), true) }
        if !store.attention.isEmpty { return (color("idle"), true) }
        if st.contains("busy") { return (color("busy"), true) }
        if st.contains("idle") { return (color("idle"), false) }
        return (.gray, false)
    }

    private func header(expanded: Bool) -> some View {
        let counts = Dictionary(grouping: store.sessions, by: \.status).mapValues(\.count)
        let u = urgent
        return HStack(spacing: 7) {
            PulseDot(color: u.color, pulsing: u.pulsing)
            Text("Claude").font(.system(size: 12.5, weight: .semibold)).foregroundStyle(.white)
            Spacer(minLength: 6)
            if store.sessions.isEmpty {
                Text("–").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
            }
            ForEach(["waiting", "idle", "busy"], id: \.self) { st in
                if let n = counts[st] {
                    HStack(spacing: 3) {
                        Circle().fill(color(st)).frame(width: 6, height: 6)
                        Text("\(n)").font(.system(size: 12, weight: .semibold).monospacedDigit()).foregroundStyle(.white)
                    }
                }
            }
        }
        .padding(.horizontal, expanded ? 18 : 13)
    }

    private func row(_ s: Session) -> some View {
        let hot = store.attention.contains(s.id)
        return HStack(spacing: 10) {
            Circle().fill(color(s.status)).frame(width: 8, height: 8)
            VStack(alignment: .leading, spacing: 1) {
                Text(s.name).font(.system(size: 12.5, weight: hot ? .bold : .medium)).foregroundStyle(.white).lineLimit(1)
                Text((s.cwd as NSString).lastPathComponent).font(.system(size: 10.5))
                    .foregroundStyle(.white.opacity(0.45)).lineLimit(1)
            }
            Spacer(minLength: 12)
            Text(label(s)).font(.system(size: 11.5, weight: .semibold)).foregroundStyle(color(s.status))
            Text(elapsed(s.since, store.now)).font(.system(size: 11.5).monospacedDigit())
                .foregroundStyle(.white.opacity(0.55)).frame(minWidth: 36, alignment: .trailing)
        }
        .padding(.horizontal, 8)
        .frame(height: Layout.rowHeight)
        .background(RoundedRectangle(cornerRadius: 9).fill(hot ? color(s.status).opacity(0.18) : .clear))
    }
}

final class DragHostingView<V: View>: NSHostingView<V> {
    var onDragEnd: (() -> Void)?
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {
        window?.performDrag(with: event)
        onDragEnd?()
    }
}

final class WidgetPanel: NSPanel {
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
    override var canBecomeKey: Bool { false }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    let store = Store()
    var panel: WidgetPanel!
    var timer: Timer?
    var monitors: [Any] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        panel = WidgetPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.acceptsMouseMovedEvents = true
        let host = DragHostingView(rootView: WidgetView(store: store))
        host.onDragEnd = { [weak self] in self?.settle() }
        panel.contentView = host

        let d = UserDefaults.standard
        if d.object(forKey: "pillX") != nil {
            place(pillOrigin: CGPoint(x: d.double(forKey: "pillX"), y: d.double(forKey: "pillY")))
        } else if let s = NSScreen.main?.visibleFrame {
            place(pillOrigin: CGPoint(x: s.maxX - Layout.collapsed.width - 16, y: s.maxY - Layout.collapsed.height - 10))
        }

        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main) { [weak self] _ in
            self?.settle()
        }
        let track: (NSEvent) -> Void = { [weak self] _ in self?.trackMouse() }
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: track) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved, .leftMouseDragged], handler: { track($0); return $0 }) { monitors.append(m) }

        store.refresh()
        panel.orderFrontRegardless()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            self?.store.refresh()
            self?.trackMouse()
        }
    }

    func place(pillOrigin raw: CGPoint) {
        let center = CGPoint(x: raw.x + Layout.collapsed.width / 2, y: raw.y + Layout.collapsed.height / 2)
        let screen = NSScreen.screens.first { $0.frame.contains(center) } ?? NSScreen.main
        guard let s = screen?.frame else { return }
        let o = Layout.clamp(pillOrigin: raw, to: s)
        store.anchor = Anchor.near(CGPoint(x: o.x + Layout.collapsed.width / 2, y: o.y + Layout.collapsed.height / 2), in: s)
        panel.setFrame(Layout.canvasFrame(pillOrigin: o, anchor: store.anchor), display: true)
        UserDefaults.standard.set(Double(o.x), forKey: "pillX")
        UserDefaults.standard.set(Double(o.y), forKey: "pillY")
    }

    func settle() {
        let size = Layout.size(store)
        let pill = Layout.pill(in: panel.frame, size: size, anchor: store.anchor)
        let x: CGFloat
        switch store.anchor.h {
        case .leading: x = pill.minX
        case .center: x = pill.midX - Layout.collapsed.width / 2
        case .trailing: x = pill.maxX - Layout.collapsed.width
        }
        let y = store.anchor.up ? pill.minY : pill.maxY - Layout.collapsed.height
        place(pillOrigin: CGPoint(x: x, y: y))
        trackMouse()
    }

    func trackMouse() {
        let zone = Layout.pill(in: panel.frame, size: Layout.size(store), anchor: store.anchor)
        let inside = zone.contains(NSEvent.mouseLocation)
        if inside != store.hovering {
            store.hovering = inside
            if inside { store.peekUntil = nil }
        }
        panel.ignoresMouseEvents = !inside
    }
}

#if SNAPSHOT
MainActor.assumeIsolated { runSnapshots() }
#else
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
#endif
