import AppKit
import SwiftUI

extension WidgetView {
    func sendAnswer(_ s: Session, _ q: FeedQuestion) {
        let answers = panel.answers[q.requestId] ?? [], other = panel.others[q.requestId] ?? [:]
        guard Feed.complete(q, answers: answers, other: other) else { return }
        onAction(.answer(s.id, requestId: q.requestId, answers: answers, other: other))
    }

    /// The Feed question of a session: the form while cmux can still take the answer, else `Trả lời trong cmux`.
    func questionBlock(_ s: Session, _ q: FeedQuestion) -> some View {
        let state = store.blockState(of: s) ?? .fallback
        return VStack(alignment: .leading, spacing: 0) {
            switch state {
            case .sent:
                Text("Đã gửi").font(Theme.Fonts.small).foregroundStyle(Theme.Colors.primaryText)
                    .frame(height: Layout.questionFallbackHeight, alignment: .leading)
            case .fallback:
                PillButton(title: "Trả lời trong cmux") { onAction(.jump(s.id)) }
                    .frame(height: Layout.questionFallbackHeight, alignment: .leading)
            case .form:
                questionForm(s, q)
            }
        }
    }

    static func optionSymbol(multi: Bool, on: Bool) -> String {
        multi ? (on ? Theme.Symbols.checkOn : Theme.Symbols.checkOff) : (on ? Theme.Symbols.radioOn : Theme.Symbols.radioOff)
    }

    func questionForm(_ s: Session, _ q: FeedQuestion) -> some View {
        let answers = panel.answers[q.requestId] ?? [], other = panel.others[q.requestId] ?? [:]
        let canSend = Feed.complete(q, answers: answers, other: other)
        return VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(q.questions.enumerated()), id: \.offset) { i, item in
                let picked = Feed.picks(answers, i)
                let key = "\(s.id)#\(i)"
                HStack(spacing: 6) {
                    if let h = item.header {
                        Text(h).font(Theme.Fonts.captionStrong).foregroundStyle(Theme.Colors.secondaryText)
                            .padding(.horizontal, 6).padding(.vertical, 1)
                            .background(Capsule().fill(Theme.Colors.field))
                    }
                    Text(item.prompt).font(Theme.Fonts.small).foregroundStyle(Theme.Colors.primaryText).lineLimit(1)
                }
                .frame(height: Layout.questionLineHeight, alignment: .leading)
                ForEach(Array(item.options.enumerated()), id: \.element.id) { j, o in
                    OptionRow(symbol: Self.optionSymbol(multi: item.multiSelect, on: picked.contains(o.label)),
                              number: Feed.optionNumber(q, question: i, option: j), label: o.label, detail: o.description) {
                        panel.pick(q, i, o.label)
                    }
                    .frame(height: Layout.optionRowHeight)
                }
                Group {
                    if (panel.editing == Store.Editing(id: s.id, kind: .answer) && panel.otherKey == key) || !(other[i] ?? "").isEmpty {
                        HStack(spacing: Theme.Space.gap) {
                            Image(systemName: Theme.Symbols.other).font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText)
                                .frame(width: 14)
                            TextField("Khác…", text: Binding(get: { panel.others[q.requestId]?[i] ?? "" }, set: { panel.setOther(q, i, $0) }))
                                .textFieldStyle(.plain)
                                .font(Theme.Fonts.body)
                                .padding(.horizontal, 6)
                                .frame(height: Layout.otherFieldHeight)
                                .background(RoundedRectangle(cornerRadius: 5).fill(Theme.Colors.field))
                                .focused($otherFocus, equals: key)
                                .onAppear { if panel.wantOther == key { otherFocus = key; panel.wantOther = nil } }
                                .onSubmit { sendAnswer(s, q) }
                                .onExitCommand { panel.editing = nil }
                        }
                    } else {
                        OptionRow(symbol: Theme.Symbols.other, number: nil, label: "Khác…", detail: nil) {
                            panel.wantOther = key
                            panel.otherKey = key
                            panel.editing = Store.Editing(id: s.id, kind: .answer)
                        }
                    }
                }
                .frame(height: Layout.otherFieldHeight, alignment: .leading)
            }
            HStack(spacing: 8) {
                PillButton(title: "Gửi", fill: Theme.Colors.primaryAction, textColor: Theme.Colors.onPrimary) { sendAnswer(s, q) }
                    .disabled(!canSend).opacity(canSend ? 1 : Theme.Opacity.disabled)
                Text(Feed.countdown(q, now: store.now)).font(Theme.Fonts.small.monospacedDigit()).foregroundStyle(Theme.Colors.secondaryText)
                Text("⌘↩").font(Theme.Fonts.meta).foregroundStyle(Theme.Colors.tertiaryText)
            }
            .frame(height: Layout.questionFooterHeight, alignment: .leading)
        }
    }
}

/// One option of a question: symbol, number, label and a one-line description; the whole row is the tap target.
struct OptionRow: View {
    let symbol: String
    let number: Int?
    let label: String
    let detail: String?
    let action: () -> Void
    @StateObject private var hover = RowHover()

    var body: some View {
        Button(action: action) {
            HStack(spacing: Theme.Space.gap) {
                Image(systemName: symbol).font(Theme.Fonts.small).foregroundStyle(Theme.Colors.secondaryText).frame(width: 14)
                Text(number.map(String.init) ?? "").font(Theme.Fonts.mono).foregroundStyle(Theme.Colors.tertiaryText).frame(width: 8)
                Text(label).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.primaryText).lineLimit(1).fixedSize()
                if let d = detail, !d.isEmpty {
                    Text("— " + d).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.secondaryText)
                        .lineLimit(1).truncationMode(.tail)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 4)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 5).fill(hover.on ? Theme.Colors.cardHover : Color.clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover.on = $0 }
        .help(detail ?? "")
    }
}
