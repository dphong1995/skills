import AppKit
import SwiftUI

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

public struct WidgetView: View {
    @ObservedObject var store: Store

    let onSelect: (String) -> Void

    public init(store: Store, onSelect: @escaping (String) -> Void = { _ in }) {
        self.store = store
        self.onSelect = onSelect
    }

    public var body: some View {
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
        .contentShape(Rectangle())
        .onTapGesture { onSelect(s.id) }
    }
}

public final class DragHostingView<V: View>: NSHostingView<V> {
    public var onDragEnd: (() -> Void)?
    private var replaying = false
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    public override func mouseDown(with down: NSEvent) {
        if replaying {
            replaying = false
            return super.mouseDown(with: down)
        }
        guard let window else { return super.mouseDown(with: down) }
        while let e = window.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
            if e.type == .leftMouseUp {
                // A click: replay the press and release so SwiftUI sees them through the normal event path.
                replaying = true
                window.sendEvent(down)
                window.sendEvent(e)
                return
            }
            if Layout.isDrag(from: down.locationInWindow, to: e.locationInWindow) {
                window.performDrag(with: down)
                onDragEnd?()
                return
            }
        }
    }
}

public final class WidgetPanel: NSPanel {
    public override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
    public override var canBecomeKey: Bool { false }
}
