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

private func statusText(_ s: Session) -> String {
    switch s.status {
    case "busy": "đang chạy"
    case "idle": "xong"
    case "waiting": s.isPermissionPrompt ? "chờ duyệt" : "chờ trả lời"
    default: s.status.isEmpty ? "?" : s.status
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

/// The parts of a PR line in order; `prLine` joins them and the row colours the red and green ones.
private func prParts(_ pr: PRStatus, now: Date, stale: Date?) -> [(text: String, tint: Color?)] {
    var parts: [(String, Color?)] = [("PR #\(pr.ref.id)", nil)]
    if !pr.isOpen {
        parts.append((pr.stateWord, nil))
        return parts
    }
    if pr.changesRequested > 0 { parts.append(("✗ \(pr.changesRequested) yêu cầu sửa", .red)) }
    if pr.reviewers > 0 { parts.append(("✓ \(pr.approved)/\(pr.reviewers)", pr.approved > 0 ? .green : nil)) }
    if let rabbit = pr.rabbit { parts.append((CodeRabbit.label(rabbit), nil)) }
    if pr.comments > 0 { parts.append(("💬 \(pr.comments)", nil)) }
    if let c = pr.lastOther { parts.append(("\(c.name) \(elapsed(c.at, now))", nil)) }
    if let stale { parts.append(("cũ \(elapsed(stale, now))", nil)) }
    return parts
}

func prLine(_ pr: PRStatus, now: Date, stale: Date?) -> String {
    prParts(pr, now: now, stale: stale).map(\.text).joined(separator: " · ")
}

private func prAttributed(_ pr: PRStatus, now: Date, stale: Date?) -> AttributedString {
    var out = AttributedString()
    for (i, p) in prParts(pr, now: now, stale: stale).enumerated() {
        if i > 0 { out += AttributedString(" · ") }
        var a = AttributedString(p.text)
        if let tint = p.tint { a.foregroundColor = tint }
        out += a
    }
    return out
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
    case askClaude(String, prompt: String)
    case nudgeRabbit(String, PRRef)
    /// Opens the token dialog of a service; belongs to no session.
    case connect(Service)

    public var sessionId: String {
        switch self {
        case .jump(let id), .approve(let id, _), .deny(let id, _, _), .openLink(let id, _), .askClaude(let id, _), .nudgeRabbit(let id, _): id
        case .connect: ""
        }
    }
}

/// Tone to a tint: doing blue, done/open green, merged purple; `nil` for grey (todo, closed, no tone).
func toneColor(_ t: ItemTone?) -> Color? {
    switch t {
    case .doing?: .blue
    case .done?, .open?: .green
    case .merged?: .purple
    case .todo?, .closed?, nil: nil
    }
}

struct PillLabel: View {
    let title: String
    var fill: Color = .white.opacity(0.12)
    var textColor: Color = .white
    var icon: NSImage? = nil
    /// Reserves a 14x14 slot for the icon even while `icon` is nil, so labels line up.
    var showsIcon = false
    var dot: Color? = nil

    var body: some View {
        HStack(spacing: 5) {
            if showsIcon {
                Group {
                    if let icon { Image(nsImage: icon).resizable().interpolation(.high) } else { Color.clear }
                }
                .frame(width: 14, height: 14)
            }
            Text(title)
            if let dot { Circle().fill(dot).frame(width: 6, height: 6) }
        }
        .font(.system(size: 11, weight: .semibold)).foregroundStyle(textColor)
        .padding(.horizontal, 9).padding(.vertical, 3)
        .background(Capsule().fill(fill))
    }
}

struct PillButton: View {
    let title: String
    var fill: Color = .white.opacity(0.12)
    var textColor: Color = .white
    var icon: NSImage? = nil
    var showsIcon = false
    var dot: Color? = nil
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            PillLabel(title: title, fill: fill, textColor: textColor, icon: icon, showsIcon: showsIcon, dot: dot)
        }
        .buttonStyle(.plain)
    }
}

/// Status of a ticket or PR: a capsule tinted by its tone.
struct StatusTag: View {
    let text: String
    let tone: ItemTone?

    var body: some View {
        let tint = toneColor(tone)
        let textColor = tint.map { Color(nsColor: NSColor($0).blended(withFraction: 0.35, of: .white) ?? .white) } ?? .white.opacity(0.55)
        Text(text).font(.system(size: 10.5, weight: .semibold))
            .foregroundStyle(textColor)
            .lineLimit(1).truncationMode(.tail)
            .frame(maxWidth: 130 - 14).fixedSize(horizontal: true, vertical: false)
            .padding(.horizontal, 7).padding(.vertical, 2)
            .background(Capsule().fill(tint?.opacity(0.25) ?? .white.opacity(0.12)))
    }
}

/// One line of the inline `☰ N` list.
final class RowHover: ObservableObject { @Published var on = false }

struct LinkRow: View {
    let item: LinkItem
    let icon: NSImage?
    let onOpen: () -> Void
    @StateObject private var hover = RowHover()

    var body: some View {
        HStack(spacing: 6) {
            leadingIcon.frame(width: 14, height: 14)
            Text(item.code).font(.system(size: 10.5).monospaced()).foregroundStyle(.white.opacity(0.5)).lineLimit(1).fixedSize()
            if let title = item.title {
                Text(title).font(.system(size: 11.5)).foregroundStyle(.white.opacity(0.85)).lineLimit(1).truncationMode(.tail)
                    .layoutPriority(1)
            }
            Spacer(minLength: 6)
            if let status = item.status { StatusTag(text: status, tone: item.tone) }
        }
        .padding(.horizontal, 4)
        .frame(height: Layout.linkRowHeight)
        .background(RoundedRectangle(cornerRadius: 5).fill(.white.opacity(hover.on ? 0.08 : 0)))
        .contentShape(Rectangle())
        .onHover { hover.on = $0 }
        .onTapGesture(perform: onOpen)
        .help(item.help ?? item.text)
    }

    @ViewBuilder private var leadingIcon: some View {
        switch item.kind {
        case .ticket:
            if let icon { Image(nsImage: icon).resizable().interpolation(.high) } else { Color.clear }
        case .pr: Image(systemName: "arrow.triangle.pull").font(.system(size: 10)).foregroundStyle(.white.opacity(0.6))
        case .link: Image(systemName: "link").font(.system(size: 10)).foregroundStyle(.white.opacity(0.6))
        }
    }
}

public struct WidgetView: View {
    @ObservedObject var store: Store
    @ObservedObject var panel: PanelState
    @FocusState private var focused: Bool
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
        .frame(width: expanded ? Layout.expandedWidth : Layout.collapsed.width, alignment: panel.anchor.up ? .bottom : .top)
        .frame(height: expanded ? nil : Layout.collapsed.height)
        .fixedSize(horizontal: false, vertical: expanded)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            guard panel.expanded(store) else { return }
            if abs(h - (panel.measuredHeight ?? 0)) > 0.5 { panel.measuredHeight = h }
        }
        .frame(maxHeight: expanded ? Layout.maxExpandedHeight : nil, alignment: panel.anchor.up ? .bottom : .top)
        .fixedSize(horizontal: false, vertical: expanded)
        .background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Color.black.opacity(o)))
        .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous)
            .stroke(Color.white.opacity(0.14 * o / Layout.cardOpacity), lineWidth: 1))
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
            Button("Prompt Nhờ Claude…") { editPrompt(store) }
            Divider()
            ForEach(Service.allCases, id: \.self) { svc in
                Button(store.connectionText(svc)) { onAction(.connect(svc)) }
            }
            Divider()
            Button("Thoát") { NSApp.terminate(nil) }
        }
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: expanded)
        .onChange(of: expanded) { _, now in if !now { panel.openMenus = [] } }
        .animation(.spring(response: 0.36, dampingFraction: 0.8), value: store.sessions.count)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: panel.anchor.alignment)
        .padding(Layout.margin)
        .environment(\.colorScheme, .dark)
    }

    private var list: some View {
        let rows = VStack(spacing: Layout.cardGap) {
            ForEach(store.sessions) { row($0) }
            if store.sessions.isEmpty {
                Text("Không có session nào").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
                    .frame(height: Layout.rowHeight)
            }
        }
        .padding(.horizontal, 10)
        .padding(panel.anchor.up ? .top : .bottom, Layout.listPaddingFar)
        .padding(panel.anchor.up ? .bottom : .top, Layout.listPaddingNear)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { h in
            if abs(h - (panel.listHeight ?? 0)) > 0.5 { panel.listHeight = h }
        }
        let height = min(panel.listHeight ?? (Layout.expanded(store.sessions, open: panel.openMenus).height - Layout.headerHeight), Layout.maxListHeight)
        return Group {
            if snapshotRendering {
                rows.fixedSize(horizontal: false, vertical: true)
                    .frame(maxHeight: Layout.maxListHeight, alignment: panel.anchor.up ? .bottom : .top).clipped()
            } else {
                ScrollView(.vertical) { rows }.frame(height: height)
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
        }
        .transition(.opacity)
    }

    private func header(expanded: Bool) -> some View {
        Group {
            if expanded {
                groups(expanded: true, dot: 3, gap: 7)
                    .frame(maxWidth: .infinity, alignment: .trailing)
            } else {
                // Two-digit counts fall back to the tight spacing.
                ViewThatFits(in: .horizontal) {
                    groups(expanded: false, dot: 6, gap: 10)
                    groups(expanded: false, dot: 3, gap: 6)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .shadow(color: expanded ? .clear : Color.black.opacity(0.6), radius: 1.5)
        .padding(.horizontal, expanded ? 18 : 12)
    }

    private func groups(expanded: Bool, dot: CGFloat, gap: CGFloat) -> some View {
        let counts = Dictionary(grouping: store.sessions, by: \.status).mapValues(\.count)
        let newCount = store.newCount
        return HStack(spacing: gap) {
            if newCount > 0 {
                HStack(spacing: dot) {
                    Text("✦").font(.system(size: 11, weight: .bold)).fixedSize()
                    Text("\(newCount)").font(.system(size: 12, weight: .semibold).monospacedDigit()).fixedSize()
                }
                .foregroundStyle(.yellow)
            }
            if store.sessions.isEmpty {
                HStack(spacing: 6) {
                    if !expanded { Circle().fill(.gray).frame(width: 6, height: 6) }
                    Text("–").font(.system(size: 12)).foregroundStyle(.white.opacity(0.5))
                }
            }
            ForEach(["waiting", "idle", "busy"], id: \.self) { st in
                if let n = counts[st] {
                    HStack(spacing: dot) {
                        Circle().fill(color(st)).frame(width: 6, height: 6)
                        Text("\(n)").font(.system(size: 12, weight: .semibold).monospacedDigit()).foregroundStyle(.white).fixedSize()
                    }
                }
            }
        }
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
                            PillButton(title: "Từ chối…") { panel.editing = Store.Editing(id: s.id, kind: .deny, toolId: p.toolId) }
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
            .onExitCommand { panel.editing = nil }
    }

    /// `☰ N`: toggles the inline list of every ticket and PR of the session.
    private func menuButton(_ s: Session) -> some View {
        let open = panel.openMenus.contains(s.id)
        let hiddenMark = store.unseen[s.id]?.prs.subtracting(s.prs.map(\.ref)).isEmpty == false
        return PillButton(title: "☰ \(s.menuCount)", fill: .white.opacity(open ? 0.22 : 0.12), dot: hiddenMark ? .purple : nil) {
            if open { panel.openMenus.remove(s.id) } else { panel.openMenus.insert(s.id) }
        }
    }

    private func linkList(_ s: Session) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(s.menu, id: \.title) { section in
                Text(section.title).font(.system(size: 10)).foregroundStyle(.white.opacity(0.45))
                    .frame(height: Layout.linkSectionHeight, alignment: .bottomLeading)
                ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in
                    LinkRow(item: item, icon: item.iconURL.flatMap { store.issueIcons[$0] }) { onAction(.openLink(s.id, item.url)) }
                }
            }
            if s.menuNeedsJira {
                PillButton(title: "Kết nối Jira…") { onAction(.connect(.jira)) }
                    .frame(height: Layout.linkRowHeight, alignment: .leading)
            }
        }
    }

    private func row(_ s: Session) -> some View {
        let editing = panel.editing?.id == s.id
        let mark = store.unseen[s.id]
        let claudeNew = mark?.claude == true
        var newLabel = claudeNew ? [", mới: Claude"] : []
        newLabel += (mark?.prs ?? []).sorted { $0.id < $1.id }.map { ", mới: PR #\($0.id)" }
        let markedShown = Set(s.prs.map(\.ref)).intersection(mark?.prs ?? [])
        return VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 10) {
                Circle().fill(color(s.status)).frame(width: 8, height: 8)
                    .accessibilityLabel(statusText(s) + newLabel.joined())
                VStack(alignment: .leading, spacing: 1) {
                    Text(s.title).font(.system(size: 12.5, weight: .medium)).foregroundStyle(.white).lineLimit(2)
                    Text(s.subtitle).font(.system(size: 10.5))
                        .foregroundStyle(.white.opacity(0.45)).lineLimit(1)
                    if let todo = s.todo {
                        Text(todo.label)
                            .font(.system(size: 11)).foregroundStyle(.white.opacity(0.7))
                            .lineLimit(1).truncationMode(.tail)
                    }
                    if let a = s.activity {
                        Text("▸ \(a)")
                            .font(.system(size: 11)).foregroundStyle(.white.opacity(0.6))
                            .lineLimit(1).truncationMode(.tail)
                    }
                    if s.isFinished, let reply = s.replyText {
                        Text(reply).font(.system(size: 11)).foregroundStyle(.white.opacity(0.75))
                            .lineLimit(4).truncationMode(.tail)
                    }
                }
                Spacer(minLength: 12)
                Text(elapsed(s.since, store.now)).font(.system(size: 11.5).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.55)).frame(minWidth: 36, alignment: .trailing)
            }
            .padding(.horizontal, 6).padding(.vertical, 3)
            .background(claudeNew ? Color.yellow.opacity(0.08) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .overlay(alignment: .leading) {
                if claudeNew { Capsule().fill(.yellow).frame(width: 3).padding(.vertical, 3) }
            }
            .padding(.horizontal, -6).padding(.vertical, -3)
            .contentShape(Rectangle())
            .onTapGesture { onAction(.jump(s.id)) }
            if !s.menu.isEmpty {
                HStack(spacing: 6) {
                    ViewThatFits(in: .horizontal) {
                        ForEach(Array(stride(from: min(s.links.count, 5), through: 1, by: -1)), id: \.self) { k in
                            HStack(spacing: 6) {
                                ForEach(s.links.prefix(k), id: \.url) { link in
                                    let titled = link.help != nil
                                    PillButton(title: titled ? link.label : "↗ \(link.label)",
                                               icon: titled ? link.iconURL.flatMap { store.issueIcons[$0] } : nil, showsIcon: titled,
                                               dot: titled ? toneColor(link.tone) ?? .white.opacity(0.55) : nil) { onAction(.openLink(s.id, link.url)) }
                                        .help(link.help ?? "")
                                }
                            }
                            .fixedSize(horizontal: true, vertical: false)
                        }
                        Color.clear.frame(width: 0, height: 0)
                    }
                    menuButton(s)
                }
                .padding(.leading, 18)
                if panel.openMenus.contains(s.id) { linkList(s).padding(.leading, 18) }
            }
            ForEach(s.prs, id: \.ref) { pr in
                let prNew = markedShown.contains(pr.ref)
                let prText = Text(prAttributed(pr, now: store.now, stale: store.prStale ? store.prLastSuccess : nil))
                HStack(spacing: 6) {
                    (prNew ? Text("✦ ").foregroundStyle(.purple) + prText : prText)
                        .font(.system(size: 11)).foregroundStyle(.white.opacity(0.75))
                        .lineLimit(1).truncationMode(.tail)
                        .contentShape(Rectangle())
                        .onTapGesture { onAction(.openLink(s.id, pr.url)) }
                        .help(pr.title ?? "")
                    Spacer(minLength: 0)
                    if pr.isOpen {
                        if pr.rabbit == .awaitingApproval {
                            PillButton(title: "🐰 Nhắc duyệt") { onAction(.nudgeRabbit(s.id, pr.ref)) }
                        }
                        PillButton(title: "Nhờ Claude") {
                            onAction(.askClaude(s.id, prompt: PullRequests.prompt(store.prPrompt, url: pr.url, id: pr.ref.id)))
                        }
                    }
                }
                .padding(.horizontal, 4)
                .background(prNew ? Color.purple.opacity(0.14) : .clear, in: RoundedRectangle(cornerRadius: 5))
                .padding(.horizontal, -4)
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
        .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 8))
        .onHover { panel.noteRowHover(s.id, inside: $0) }
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
    public override var canBecomeKey: Bool { state?.editing != nil }
}
