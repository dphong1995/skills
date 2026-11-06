import Foundation
import AppKit
import Testing
@testable import ClaudeSessionsKit

@Test func noteRowHoverEnterAndExit() {
    let panel = PanelState()
    panel.noteRowHover("a", inside: true)
    #expect(panel.hoveredRow == "a")
    panel.noteRowHover("a", inside: false)
    #expect(panel.hoveredRow == nil)
}

@Test func noteRowHoverEnterOverridesMissedExit() {
    let panel = PanelState()
    panel.noteRowHover("a", inside: true)
    panel.noteRowHover("b", inside: true)
    #expect(panel.hoveredRow == "b")
}

@Test func noteRowHoverExitOfOtherRowIsIgnored() {
    let panel = PanelState()
    panel.noteRowHover("a", inside: true)
    panel.noteRowHover("b", inside: true)
    panel.noteRowHover("a", inside: false)
    #expect(panel.hoveredRow == "b")
    panel.noteRowHover("b", inside: false)
    #expect(panel.hoveredRow == nil)
}

@Test func orderChangeKeepsRowThatStayedInPlace() {
    let panel = PanelState()
    panel.hoveredRow = "a"
    panel.noteOrderChange(from: ["a", "b", "c"], to: ["a", "c", "b"])
    #expect(panel.hoveredRow == "a")
}

@Test func orderChangeClearsRowThatMoved() {
    let panel = PanelState()
    panel.hoveredRow = "b"
    panel.noteOrderChange(from: ["a", "b"], to: ["b", "a"])
    #expect(panel.hoveredRow == nil)
}

@Test func orderChangeClearsRowThatVanished() {
    let panel = PanelState()
    panel.hoveredRow = "b"
    panel.noteOrderChange(from: ["a", "b"], to: ["a"])
    #expect(panel.hoveredRow == nil)
}

@Test func dwellTargetOnlyForFreshRow() {
    let panel = PanelState()
    let store = makeStore(dir: makeTempDir())
    store.unseen = ["a": NewMark(claude: true)]
    #expect(panel.dwellTarget(in: store) == nil)
    panel.hoveredRow = "a"
    #expect(panel.dwellTarget(in: store) == "a")
    panel.hoveredRow = "b"
    #expect(panel.dwellTarget(in: store) == nil)
    store.unseen = [:]
    panel.hoveredRow = "a"
    #expect(panel.dwellTarget(in: store) == nil)
}

private func twoQuestions(multi: Bool = false) -> FeedQuestion {
    let opts = ["x", "y"].map { FeedQuestion.Option(id: $0, label: $0, description: nil) }
    return feedQuestion(questions: [FeedQuestion.Item(prompt: "A", header: nil, multiSelect: multi, options: opts),
                                    FeedQuestion.Item(prompt: "B", header: nil, multiSelect: false, options: opts)])
}

@Test func singleSelectPickReplacesAndClearsOther() {
    let panel = PanelState()
    let q = twoQuestions()
    panel.pick(q, 0, "x")
    panel.pick(q, 0, "y")
    #expect(panel.answers["R1"] == [["y"], []])
    panel.setOther(q, 0, "free")
    #expect(panel.answers["R1"]?[0] == [])
    #expect(panel.others["R1"]?[0] == "free")
    panel.pick(q, 0, "x")
    #expect(panel.others["R1"]?[0] == nil)
    #expect(panel.answers["R1"]?[0] == ["x"])
}

@Test func multiSelectPickToggles() {
    let panel = PanelState()
    let q = twoQuestions(multi: true)
    panel.pick(q, 0, "x")
    panel.pick(q, 0, "y")
    #expect(panel.answers["R1"]?[0] == ["x", "y"])
    panel.pick(q, 0, "x")
    #expect(panel.answers["R1"]?[0] == ["y"])
    panel.setOther(q, 0, "more")
    #expect(panel.answers["R1"]?[0] == ["y"])
    #expect(panel.others["R1"]?[0] == "more")
}
