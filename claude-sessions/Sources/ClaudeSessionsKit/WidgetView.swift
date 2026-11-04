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

/// Inline markdown for a reply; links are dropped (phase buttons open them) and a parse failure shows the text as is.
func markdown(_ s: String) -> AttributedString {
    guard var a = try? AttributedString(markdown: s, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) else {
        return AttributedString(s)
    }
    for run in a.runs {
        if run.link != nil { a[run.range].link = nil }
        if run.inlinePresentationIntent?.contains(.code) == true {
            let bold = run.inlinePresentationIntent?.contains(.stronglyEmphasized) == true
            a[run.range].font = Font.system(size: 11, weight: bold ? .bold : .regular).monospaced()
        }
    }
    return a
}

func label(_ s: Session) -> String {
    switch s.status {
    case "waiting": s.waitingFor == "permission prompt" ? "chờ duyệt" : "chờ trả lời"
    case "idle": "xong"
    case "busy": "đang chạy"
    default: s.status.isEmpty ? "?" : s.status
    }
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

struct PulseDot: View {
    let color: Color
    let pulsing: Bool
    @Environment(\.snapshotRendering) private var snapshotRendering

    var body: some View {
        Circle().fill(color).frame(width: 8, height: 8)
            .overlay {
                if !snapshotRendering { PulseRing(color: color, pulsing: pulsing) }
            }
    }
}

/// The expanding ring is a Core Animation layer animation, which the render server runs; a SwiftUI animation would cost the app a redraw per frame.
struct PulseRing: NSViewRepresentable {
    let color: Color
    let pulsing: Bool
    private static let animationKey = "pulse"

    func makeNSView(context: Context) -> NSView {
        let view = NSView(frame: NSRect(x: 0, y: 0, width: 8, height: 8))
        view.wantsLayer = true
        view.layer?.masksToBounds = false
        let ring = CAShapeLayer()
        ring.frame = view.bounds
        ring.path = CGPath(ellipseIn: view.bounds, transform: nil)
        ring.fillColor = nil
        ring.lineWidth = 2
        view.layer?.addSublayer(ring)
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        guard let ring = view.layer?.sublayers?.first as? CAShapeLayer else { return }
        ring.strokeColor = NSColor(color).cgColor
        if pulsing {
            ring.isHidden = false
            if ring.animation(forKey: Self.animationKey) == nil {
                let scale = CABasicAnimation(keyPath: "transform.scale")
                scale.fromValue = 1
                scale.toValue = 2.4
                let fade = CABasicAnimation(keyPath: "opacity")
                fade.fromValue = 0.8
                fade.toValue = 0
                let group = CAAnimationGroup()
                group.animations = [scale, fade]
                group.duration = 1.2
                group.timingFunction = CAMediaTimingFunction(name: .linear)
                group.repeatCount = .infinity
                group.isRemovedOnCompletion = false
                ring.add(group, forKey: Self.animationKey)
            }
        } else {
            ring.removeAnimation(forKey: Self.animationKey)
            ring.isHidden = true
        }
    }
}

public enum WidgetAction: Equatable {
    case jump(String)
    case approve(String, toolId: String)
    case deny(String, toolId: String, reason: String)
    case openLink(String, URL)

    public var sessionId: String {
        switch self {
        case .jump(let id), .approve(let id, _), .deny(let id, _, _), .openLink(let id, _): id
        }
    }
}

struct PillButton: View {
    let title: String
    var fill: Color = .white.opacity(0.12)
    var textColor: Color = .white
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(title).font(.system(size: 11, weight: .semibold)).foregroundStyle(textColor)
                .padding(.horizontal, 9).padding(.vertical, 3)
                .background(Capsule().fill(fill))
        }
        .buttonStyle(.plain)
    }
}

public struct WidgetView: View {
    @ObservedObject var store: Store
    @FocusState private var focused: Bool
    @Environment(\.snapshotRendering) private var snapshotRendering

    let onAction: (WidgetAction) -> Void

    public init(store: Store, onAction: @escaping (WidgetAction) -> Void = { _ in }) {
        self.store = store
        self.onAction = onAction
    }

    public var body: some View {
        let expanded = store.expanded
        let radius: CGFloat = expanded ? 20 : Layout.headerHeight / 2
        let hot = !store.unseen.isEmpty

        VStack(spacing: 0) {
            if expanded && store.anchor.up { list }
            header(expanded: expanded).frame(height: Layout.headerHeight)
            if expanded && !store.anchor.up { list }
        }
        .frame(width: expanded ? Layout.expandedWidth : Layout.collapsed.width, alignment: store.anchor.up ? .bottom : .top)
        .frame(height: expanded ? nil : Layout.collapsed.height)
        .fixedSize(horizontal: false, vertical: expanded)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            guard store.expanded else { return }
            if abs(h - (store.measuredHeight ?? 0)) > 0.5 { store.measuredHeight = h }
        }
        .frame(maxHeight: expanded ? Layout.maxExpandedHeight : nil, alignment: store.anchor.up ? .bottom : .top)
        .fixedSize(horizontal: false, vertical: expanded)
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
        let rows = VStack(spacing: 0) {
            ForEach(store.sessions) { row($0) }
            if store.sessions.isEmpty {
                Text("Không có session nào").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
                    .frame(height: Layout.rowHeight)
            }
        }
        .padding(.horizontal, 10)
        .padding(store.anchor.up ? .top : .bottom, 10)
        .padding(store.anchor.up ? .bottom : .top, 4)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            if abs(h - (store.listHeight ?? 0)) > 0.5 { store.listHeight = h }
        }
        let height = min(store.listHeight ?? (Layout.expanded(store.sessions).height - Layout.headerHeight - 14), Layout.maxListHeight)
        return Group {
            if snapshotRendering {
                rows.fixedSize(horizontal: false, vertical: true)
                    .frame(maxHeight: Layout.maxListHeight, alignment: store.anchor.up ? .bottom : .top).clipped()
            } else {
                ScrollView(.vertical) { rows }.frame(height: height)
            }
        }
        .transition(.opacity)
    }

    private var urgent: (color: Color, pulsing: Bool) {
        let st = Set(store.sessions.map(\.status))
        if st.contains("waiting") { return (color("waiting"), true) }
        if !store.unseen.isEmpty { return (color("idle"), true) }
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
            if !store.unseen.isEmpty {
                Text("\(store.unseen.count) mới").font(.system(size: 11)).foregroundStyle(.orange)
            }
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

    private func approvalBlock(_ s: Session, _ pending: PendingState) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            switch pending {
            case .unclear:
                jumpLine("Cần duyệt trong cmux", s)
            case .one(let p):
                let fit = Layout.commandFit(p.text)
                let full = fit == .short || (fit == .long && store.shownCommands.contains(p.toolId))
                Text(p.text).font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(.white.opacity(0.8))
                    .lineLimit(full ? nil : 4).truncationMode(.tail)
                if s.approvable != nil {
                    if fit == .tooLong {
                        jumpLine("Lệnh dài, bấm để duyệt trong cmux", s)
                    } else if full {
                        HStack(spacing: 6) {
                            PillButton(title: "Cho phép", fill: color(s.status), textColor: .black) {
                                onAction(.approve(s.id, toolId: p.toolId))
                            }
                            PillButton(title: "Từ chối…") { store.editing = Store.Editing(id: s.id, kind: .deny, toolId: p.toolId) }
                        }
                    } else {
                        PillButton(title: "Xem đủ") { store.shownCommands.insert(p.toolId) }
                    }
                }
            }
        }
    }

    private func jumpLine(_ text: String, _ s: Session) -> some View {
        Text(text).font(.system(size: 11)).foregroundStyle(.white.opacity(0.8))
            .contentShape(Rectangle())
            .onTapGesture { onAction(.jump(s.id)) }
    }

    private func editor(_ s: Session) -> some View {
        let draft = Binding(get: { store.drafts[s.id, default: ""] }, set: { store.drafts[s.id] = $0 })
        return TextField("Lý do (để trống = chỉ từ chối)", text: draft, axis: .vertical)
            .textFieldStyle(.plain)
            .font(.system(size: 11.5))
            .lineLimit(1...4)
            .padding(.horizontal, 8).padding(.vertical, 5)
            .background(RoundedRectangle(cornerRadius: 7).fill(.white.opacity(0.1)))
            .focused($focused)
            .onAppear { focused = true }
            .onSubmit {
                let text = draft.wrappedValue
                onAction(.deny(s.id, toolId: s.approvable?.toolId ?? "", reason: text))
            }
            .onExitCommand { store.editing = nil }
    }

    private func row(_ s: Session) -> some View {
        let hot = store.unseen.contains(s.id)
        let editing = store.editing?.id == s.id
        return VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 10) {
                Circle().fill(color(s.status)).frame(width: 8, height: 8)
                VStack(alignment: .leading, spacing: 1) {
                    Text(s.title).font(.system(size: 12.5, weight: hot ? .bold : .medium)).foregroundStyle(.white).lineLimit(2)
                    Text(s.subtitle).font(.system(size: 10.5))
                        .foregroundStyle(.white.opacity(0.45)).lineLimit(1)
                    if let todo = s.todo {
                        Text(todo.label)
                            .font(.system(size: 11)).foregroundStyle(.white.opacity(0.7))
                            .lineLimit(1).truncationMode(.tail)
                    }
                    if s.isFinished, let reply = s.lastReply {
                        Text(markdown(reply)).font(.system(size: 11)).foregroundStyle(.white.opacity(0.75))
                            .lineLimit(4).truncationMode(.tail)
                    }
                }
                Spacer(minLength: 12)
                Text(label(s)).font(.system(size: 11.5, weight: .semibold)).foregroundStyle(color(s.status))
                Text(elapsed(s.since, store.now)).font(.system(size: 11.5).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.55)).frame(minWidth: 36, alignment: .trailing)
            }
            .contentShape(Rectangle())
            .onTapGesture { onAction(.jump(s.id)) }
            if !s.links.isEmpty {
                HStack(spacing: 6) {
                    ForEach(s.links, id: \.url) { link in
                        PillButton(title: "↗ \(link.label)") { onAction(.openLink(s.id, link.url)) }
                    }
                }
                .padding(.leading, 18)
            }
            if let pending = s.pending {
                approvalBlock(s, pending).padding(.leading, 18)
            }
            if editing {
                editor(s).padding(.leading, 18)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .frame(minHeight: Layout.rowHeight)
        .background(RoundedRectangle(cornerRadius: 9).fill(hot ? color(s.status).opacity(0.18) : .clear))
    }
}

public final class DragHostingView<V: View>: NSHostingView<V> {
    public var onDragEnd: (() -> Void)?
    private var replaying = false
    public override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    public override func mouseDown(with down: NSEvent) {
        guard !replaying, let window else { return super.mouseDown(with: down) }
        while let e = window.nextEvent(matching: [.leftMouseDragged, .leftMouseUp]) {
            if e.type == .leftMouseUp {
                // A click: replay the press and release so SwiftUI sees them through the normal event path.
                replaying = true
                window.sendEvent(down)
                replaying = false
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
    public weak var store: Store?
    public override var canBecomeKey: Bool { store?.editing != nil }
}
