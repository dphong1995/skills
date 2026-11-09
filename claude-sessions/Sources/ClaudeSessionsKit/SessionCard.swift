import AppKit
import SwiftUI

func statusText(_ s: Session) -> String {
    if let n = s.backgroundAgents { return n > 0 ? "chờ \(n) agent nền" : "xong" }
    return switch s.status {
    case "busy": "đang chạy"
    case "idle": "xong"
    case "waiting": s.isPermissionPrompt ? "chờ duyệt" : "chờ trả lời"
    default: s.status.isEmpty ? "?" : s.status
    }
}

/// The parts of a PR line in order; `prLine` joins them and the row colours the red and green ones.
func prParts(_ pr: PRStatus, now: Date, stale: Date?) -> [(text: String, tint: Color?)] {
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

func prAttributed(_ pr: PRStatus, now: Date, stale: Date?) -> AttributedString {
    var out = AttributedString()
    for (i, p) in prParts(pr, now: now, stale: stale).enumerated() {
        if i > 0 { out += AttributedString(" · ") }
        var a = AttributedString(p.text)
        if let tint = p.tint { a.foregroundColor = tint }
        out += a
    }
    return out
}

extension WidgetView {
    func approvalBlock(_ s: Session, _ pending: PendingState) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            switch pending {
            case .unclear:
                jumpLine("Cần duyệt trong cmux", s)
            case .one(let p):
                let fit = Layout.commandFit(p.text)
                let full = fit == .short || (fit == .long && store.shownCommands.contains(p.toolId))
                Text(p.text).font(Theme.Fonts.mono)
                    .foregroundStyle(Theme.Colors.primaryText)
                    .lineLimit(full ? nil : 4).truncationMode(.tail)
                if s.approvable != nil {
                    if fit == .tooLong {
                        jumpLine("Lệnh dài, bấm để duyệt trong cmux", s)
                    } else if full {
                        HStack(spacing: 6) {
                            PillButton(title: "Cho phép", fill: Theme.Colors.primaryAction, textColor: Theme.Colors.onPrimary) {
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

    func jumpLine(_ text: String, _ s: Session) -> some View {
        Text(text).font(Theme.Fonts.small).foregroundStyle(Theme.Colors.primaryText)
            .contentShape(Rectangle())
            .onTapGesture { onAction(.jump(s.id)) }
    }

    func editor(_ s: Session, _ kind: Store.Editing.Kind) -> some View {
        let draftKey = Store.draftKey(s.id, kind)
        let draft = Binding(get: { store.drafts[draftKey, default: ""] }, set: { store.drafts[draftKey] = $0 })
        return TextField(kind == .reply ? "Trả lời…" : "Lý do (để trống = chỉ từ chối)", text: draft, axis: .vertical)
            .textFieldStyle(.plain)
            .font(Theme.Fonts.body)
            .lineLimit(1...4)
            .padding(.horizontal, 8).padding(.vertical, 5)
            .background(RoundedRectangle(cornerRadius: 7).fill(Theme.Colors.field))
            .focused($focused)
            .onAppear { focused = true }
            .onSubmit {
                let text = draft.wrappedValue
                if kind == .reply {
                    if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { onAction(.reply(s.id, text: text)) }
                } else {
                    onAction(.deny(s.id, toolId: s.approvable?.toolId ?? "", reason: text))
                }
            }
            .onExitCommand { panel.editing = nil }
    }

    /// `Trả lời…` on a finished session, `Dừng` (two clicks within 3 s) on a busy one; both need the session to be in cmux.
    @ViewBuilder
    func quickButton(_ s: Session) -> some View {
        let inCmux = !store.surfaceMisses.contains(s.id)
        Group {
            if s.canAskClaude {
                PillButton(title: "Trả lời…") { panel.editing = Store.Editing(id: s.id, kind: .reply) }
            } else if s.shownStatus == "busy" {
                let armed = panel.stopArmed[s.id].map { store.now.timeIntervalSince($0) < 3 } ?? false
                PillButton(title: armed ? "Bấm lần nữa để dừng" : "Dừng") {
                    // The click compares with the real time: `store.now` only ticks once a second.
                    if panel.stopArmed[s.id].map({ Date().timeIntervalSince($0) < 3 }) ?? false {
                        panel.stopArmed[s.id] = nil
                        onAction(.stop(s.id))
                    } else {
                        panel.stopArmed[s.id] = Date()
                    }
                }
            }
        }
        .disabled(!inCmux).opacity(inCmux ? 1 : Theme.Opacity.disabled)
        .help(inCmux ? "" : "Chỉ dùng được trong cmux")
    }

    /// Menu count: toggles the inline list of every ticket and PR of the session.
    func menuButton(_ s: Session) -> some View {
        let open = panel.openMenus.contains(s.id)
        let hiddenMark = store.unseen[s.id]?.prs.subtracting(s.prs.map(\.ref)).isEmpty == false
        return PillButton(title: "\(s.menuCount)", fill: open ? Theme.Colors.active : Theme.Colors.field, symbol: Theme.Symbols.menu, dot: hiddenMark ? Theme.Colors.freshPR : nil) {
            if open { panel.openMenus.remove(s.id) } else { panel.openMenus.insert(s.id) }
        }
    }

    func linkList(_ s: Session) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(s.menu, id: \.title) { section in
                Text(section.title).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.tertiaryText)
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

    func row(_ s: Session) -> some View {
        let editing = panel.editing?.id == s.id ? panel.editing?.kind : nil
        let mark = store.unseen[s.id]
        let claudeNew = mark?.claude == true
        var newLabel = claudeNew ? [", mới: Claude"] : []
        newLabel += (mark?.prs ?? []).sorted { $0.id < $1.id }.map { ", mới: PR #\($0.id)" }
        let markedShown = Set(s.prs.map(\.ref)).intersection(mark?.prs ?? [])
        return VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 10) {
                Circle().fill(color(s.colorStatus)).frame(width: 8, height: 8)
                    .accessibilityLabel(statusText(s) + newLabel.joined())
                VStack(alignment: .leading, spacing: 1) {
                    Text(s.title).font(Theme.Fonts.title).foregroundStyle(Theme.Colors.primaryText).lineLimit(2)
                    HStack(spacing: 0) {
                        Text(s.subtitle).lineLimit(1).truncationMode(.tail)
                        if s.agentCount > 0 {
                            HStack(spacing: 2) {
                                Text(" ·")
                                Image(systemName: Theme.Symbols.agents).imageScale(.small)
                                Text("\(s.agentCount)")
                            }
                            .lineLimit(1).fixedSize()
                                .help(s.agents.joined(separator: "\n"))
                        }
                    }
                    .font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText)
                    if let todo = s.todo {
                        Text(todo.label)
                            .font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText)
                            .lineLimit(1).truncationMode(.tail)
                    }
                    if let a = s.activity {
                        HStack(spacing: 4) {
                            Image(systemName: Theme.Symbols.activity).font(Theme.Fonts.micro)
                            Text(a).lineLimit(1).truncationMode(.tail)
                        }
                        .font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText)
                    }
                    if s.question == nil, s.isFinished, let reply = s.replyText {
                        Text(reply).font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText)
                            .lineLimit(4).truncationMode(.tail)
                    }
                }
                Spacer(minLength: 12)
                Text(elapsed(s.since, store.now)).font(Theme.Fonts.body.monospacedDigit())
                    .foregroundStyle(Theme.Colors.secondaryText).frame(minWidth: 36, alignment: .trailing)
                quickButton(s)
            }
            .padding(.horizontal, 6).padding(.vertical, 3)
            .background(claudeNew ? Theme.Colors.fresh.opacity(0.08) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .overlay(alignment: .leading) {
                if claudeNew { Capsule().fill(Theme.Colors.fresh).frame(width: 3).padding(.vertical, 3) }
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
                                    PillButton(title: link.label,
                                               symbol: titled ? nil : Theme.Symbols.link,
                                               icon: titled ? link.iconURL.flatMap { store.issueIcons[$0] } : nil, showsIcon: titled,
                                               dot: titled ? toneColor(link.tone) ?? Theme.Colors.secondaryText : nil) { onAction(.openLink(s.id, link.url)) }
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
                    (prNew ? Text(Image(systemName: Theme.Symbols.fresh)).foregroundStyle(Theme.Colors.freshPR) + Text(" ") + prText : prText)
                        .font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText)
                        .lineLimit(1).truncationMode(.tail)
                        .contentShape(Rectangle())
                        .onTapGesture { onAction(.openLink(s.id, pr.url)) }
                        .help(pr.title ?? "")
                    Spacer(minLength: 0)
                    if pr.isOpen {
                        if pr.rabbit == .awaitingApproval {
                            PillButton(title: "Nhắc duyệt", symbol: Theme.Symbols.rabbit) { onAction(.nudgeRabbit(s.id, pr.ref)) }
                        }
                        let canAsk = s.canAskClaude
                        PillButton(title: "Nhờ Claude") {
                            onAction(.askClaude(s.id, prompt: PullRequests.prompt(store.prPrompt, url: pr.url, id: pr.ref.id)))
                        }
                        .disabled(!canAsk)
                        .opacity(canAsk ? 1 : Theme.Opacity.disabled)
                        .help(canAsk ? "" : "Chỉ gửi được khi Claude đã xong lượt")
                    }
                }
                .padding(.horizontal, 4)
                .background(prNew ? Theme.Colors.freshPR.opacity(0.14) : .clear, in: RoundedRectangle(cornerRadius: 5))
                .padding(.horizontal, -4)
                .padding(.leading, 18)
            }
            if let pending = s.pending {
                approvalBlock(s, pending).padding(.leading, 18)
            }
            if let q = s.question {
                questionBlock(s, q).padding(.leading, 18)
            }
            if let editing, editing != .answer {
                editor(s, editing).padding(.leading, 18)
            }
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 5)
        .frame(minHeight: Layout.rowHeight)
        .background(Theme.Colors.card, in: RoundedRectangle(cornerRadius: 8))
        .onHover { panel.noteRowHover(s.id, inside: $0) }
    }
}
