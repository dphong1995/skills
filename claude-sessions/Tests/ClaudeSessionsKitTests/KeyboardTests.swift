import Foundation
import Testing
@testable import ClaudeSessionsKit

private func question(_ counts: [Int]) -> FeedQuestion {
    FeedQuestion(requestId: "r", createdAt: Date(), sessionId: "s", questions: counts.enumerated().map { i, n in
        FeedQuestion.Item(prompt: "q\(i)", header: nil, multiSelect: false,
                          options: (0..<n).map { FeedQuestion.Option(id: "\(i)-\($0)", label: "o\(i)-\($0)", description: nil) })
    })
}

@Test func moveSelection() {
    let panel = PanelState()
    let ids = ["a", "b", "c"]
    panel.moveSelection(1, ids: ids)
    #expect(panel.selected == "a")
    panel.moveSelection(1, ids: ids)
    panel.moveSelection(1, ids: ids)
    panel.moveSelection(1, ids: ids)
    #expect(panel.selected == "c")
    panel.moveSelection(-1, ids: ids)
    panel.moveSelection(-1, ids: ids)
    panel.moveSelection(-1, ids: ids)
    #expect(panel.selected == "a")
}

@Test func selectionSurvivesRemoval() {
    let panel = PanelState()
    panel.selected = "b"
    panel.reconcileSelection(old: ["a", "b", "c"], new: ["a", "c"])
    #expect(panel.selected == "c")
    panel.selected = "c"
    panel.reconcileSelection(old: ["a", "c"], new: ["a"])
    #expect(panel.selected == "a")
    panel.reconcileSelection(old: ["a"], new: [])
    #expect(panel.selected == nil)
}

@Test func digitMapsAcrossQuestions() {
    let q = question([3, 3])
    let hit = PanelState.optionAt(q, number: 4)
    #expect(hit?.question == 1)
    #expect(hit?.label == "o1-0")
    #expect(PanelState.optionAt(q, number: 7) == nil)
}

@Test func editorCloseKeepsKeyboard() {
    let panel = PanelState()
    panel.keyboard = true
    panel.editing = Store.Editing(id: "a", kind: .reply)
    panel.editing = nil
    #expect(panel.keyboard)
}
