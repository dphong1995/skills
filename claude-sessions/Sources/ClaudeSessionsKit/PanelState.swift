import SwiftUI

/// State that belongs to one pill (one display); everything shared lives in `Store`.
public final class PanelState: ObservableObject {
    @Published public var hovering = false
    /// Opened by the hotkey: `trackMouse` keeps `hovering` true until the hotkey is pressed again or the mouse has entered and then left the card.
    public var pinnedOpen = false
    public var pinnedEntered = false
    @Published public var editing: Store.Editing?
    @Published public var anchor = Anchor()
    @Published public var measuredHeight: CGFloat?
    /// Height of the session rows alone, so the list can scroll when the panel is full.
    @Published public var listHeight: CGFloat?

    /// Session id -> when its `Dừng` button was first clicked; a second click within 3 s sends the stop.
    @Published public var stopArmed: [String: Date] = [:]

    /// Session ids whose `☰ N` list is expanded; cleared when the widget collapses.
    @Published public var openMenus: Set<String> = []

    /// The `Vừa đóng (N)` group is expanded; cleared when the widget collapses.
    @Published public var closedOpen = false

    /// Session id the mouse is over in the expanded card; drives the hover-dwell that clears a fresh mark.
    @Published public var hoveredRow: String?

    /// Request id -> the options picked per question index, and the `Khác…` text per question index.
    @Published public var answers: [String: [[String]]] = [:]
    @Published public var others: [String: [Int: String]] = [:]
    /// The `Khác…` field to focus once it appears, as `"<session id>#<question index>"`.
    public var wantOther: String?
    /// The `Khác…` field that was opened, same key form: only that question shows its field.
    @Published public var otherKey: String?

    public init() {}

    /// Picks `label` for question `i`: replaces the pick of a single-select question (and drops its Other text), toggles it in a multi-select one.
    public func pick(_ q: FeedQuestion, _ i: Int, _ label: String) {
        guard q.questions.indices.contains(i) else { return }
        var all = answers[q.requestId] ?? []
        while all.count < q.questions.count { all.append([]) }
        if q.questions[i].multiSelect {
            if let at = all[i].firstIndex(of: label) { all[i].remove(at: at) } else { all[i].append(label) }
        } else {
            all[i] = [label]
            others[q.requestId]?[i] = nil
        }
        answers[q.requestId] = all
    }

    /// Sets the `Khác…` text of question `i`; typing in a single-select question drops its pick.
    public func setOther(_ q: FeedQuestion, _ i: Int, _ text: String) {
        guard q.questions.indices.contains(i) else { return }
        others[q.requestId, default: [:]][i] = text
        if !q.questions[i].multiSelect, !text.isEmpty, var all = answers[q.requestId], all.indices.contains(i) {
            all[i] = []
            answers[q.requestId] = all
        }
    }

    /// Drops the answers and the open `Khác…` field of questions that are gone.
    func pruneQuestions(live: [FeedQuestion]) {
        let ids = Set(live.map(\.requestId))
        if !answers.keys.allSatisfy(ids.contains) { answers = answers.filter { ids.contains($0.key) } }
        if !others.keys.allSatisfy(ids.contains) { others = others.filter { ids.contains($0.key) } }
        let sessions = Set(live.compactMap(\.sessionId))
        if let key = otherKey, !sessions.contains(String(key.split(separator: "#").first ?? "")) { otherKey = nil }
    }

    public func noteRowHover(_ id: String, inside: Bool) {
        if inside {
            if hoveredRow != id { hoveredRow = id }
        } else if hoveredRow == id {
            hoveredRow = nil
        }
    }

    /// Clears `hoveredRow` when its position in the id order changed or it vanished.
    public func noteOrderChange(from old: [String], to new: [String]) {
        guard let id = hoveredRow, old.firstIndex(of: id) != new.firstIndex(of: id) else { return }
        hoveredRow = nil
    }

    /// The hovered row when it carries a fresh mark.
    public func dwellTarget(in store: Store) -> String? {
        hoveredRow.flatMap { store.unseen[$0] != nil ? $0 : nil }
    }

    public func expanded(_ store: Store) -> Bool { hovering || editing != nil || store.peeking }
}
