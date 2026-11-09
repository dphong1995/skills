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
                ForEach(item.options, id: \.id) { o in
                    let on = picked.contains(o.label)
                    PillButton(title: o.label,
                               fill: on ? Theme.Colors.selected : Theme.Colors.field,
                               symbol: item.multiSelect ? (on ? Theme.Symbols.checkOn : Theme.Symbols.checkOff) : nil) { panel.pick(q, i, o.label) }
                        .help(o.description ?? "")
                        .frame(height: Layout.optionRowHeight, alignment: .leading)
                }
                Group {
                    if (panel.editing == Store.Editing(id: s.id, kind: .answer) && panel.otherKey == key) || !(other[i] ?? "").isEmpty {
                        TextField("Khác…", text: Binding(get: { panel.others[q.requestId]?[i] ?? "" }, set: { panel.setOther(q, i, $0) }))
                            .textFieldStyle(.plain)
                            .font(Theme.Fonts.body)
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(RoundedRectangle(cornerRadius: 7).fill(Theme.Colors.field))
                            .focused($otherFocus, equals: key)
                            .onAppear { if panel.wantOther == key { otherFocus = key; panel.wantOther = nil } }
                            .onSubmit { sendAnswer(s, q) }
                            .onExitCommand { panel.editing = nil }
                    } else {
                        PillButton(title: "Khác…") {
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
            }
            .frame(height: Layout.questionFooterHeight, alignment: .leading)
        }
    }
}
