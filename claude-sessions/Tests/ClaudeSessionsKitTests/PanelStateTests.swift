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
    store.unseen = ["a"]
    #expect(panel.dwellTarget(in: store) == nil)
    panel.hoveredRow = "a"
    #expect(panel.dwellTarget(in: store) == "a")
    panel.hoveredRow = "b"
    #expect(panel.dwellTarget(in: store) == nil)
    store.unseen = []
    panel.hoveredRow = "a"
    #expect(panel.dwellTarget(in: store) == nil)
}
