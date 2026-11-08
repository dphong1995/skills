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
                Text("Đã gửi").font(.system(size: 11)).foregroundStyle(.white.opacity(0.8))
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
                        Text(h).font(.system(size: 10, weight: .semibold)).foregroundStyle(.white.opacity(0.7))
                            .padding(.horizontal, 6).padding(.vertical, 1)
                            .background(Capsule().fill(.white.opacity(0.12)))
                    }
                    Text(item.prompt).font(.system(size: 11)).foregroundStyle(.white.opacity(0.85)).lineLimit(1)
                }
                .frame(height: Layout.questionLineHeight, alignment: .leading)
                ForEach(item.options, id: \.id) { o in
                    let on = picked.contains(o.label)
                    PillButton(title: (item.multiSelect ? (on ? "☑ " : "☐ ") : "") + o.label,
                               fill: on ? .white.opacity(0.32) : .white.opacity(0.12)) { panel.pick(q, i, o.label) }
                        .help(o.description ?? "")
                        .frame(height: Layout.optionRowHeight, alignment: .leading)
                }
                Group {
                    if (panel.editing == Store.Editing(id: s.id, kind: .answer) && panel.otherKey == key) || !(other[i] ?? "").isEmpty {
                        TextField("Khác…", text: Binding(get: { panel.others[q.requestId]?[i] ?? "" }, set: { panel.setOther(q, i, $0) }))
                            .textFieldStyle(.plain)
                            .font(.system(size: 11.5))
                            .padding(.horizontal, 8).padding(.vertical, 4)
                            .background(RoundedRectangle(cornerRadius: 7).fill(.white.opacity(0.1)))
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
                PillButton(title: "Gửi", fill: color(s.shownStatus), textColor: .black) { sendAnswer(s, q) }
                    .disabled(!canSend).opacity(canSend ? 1 : 0.4)
                Text(Feed.countdown(q, now: store.now)).font(.system(size: 11).monospacedDigit()).foregroundStyle(.white.opacity(0.55))
            }
            .frame(height: Layout.questionFooterHeight, alignment: .leading)
        }
    }
}
