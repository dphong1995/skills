import Foundation
import Testing
@testable import ClaudeSessionsKit

@Test func choicesDefaults() throws {
    #expect(HotKeyCombo.openChoices[0].label == "⌃⌥Space")
    #expect(HotKeyCombo.nextChoices[0].label == "⌃⌥→")
    let c = HotKeyCombo.nextChoices[1]
    let back = try JSONDecoder().decode(HotKeyCombo.self, from: JSONEncoder().encode(c))
    #expect(back == c)
}

@Test func cycleIndex() {
    let t = Date()
    let order = ["a", "b", "c"]
    let live = Set(order)
    func cycle(_ id: String, _ o: [String] = order) -> HotKeys.Cycle { HotKeys.Cycle(order: o, id: id, at: t) }
    #expect(HotKeys.advance(cycle: nil, fresh: order, isLive: live.contains, now: t)?.first == "a")
    #expect(HotKeys.advance(cycle: cycle("a"), fresh: order, isLive: live.contains, now: t.addingTimeInterval(4))?.first == "b")
    #expect(HotKeys.advance(cycle: cycle("c"), fresh: order, isLive: live.contains, now: t.addingTimeInterval(4))?.first == "a")
    #expect(HotKeys.advance(cycle: cycle("a"), fresh: order, isLive: live.contains, now: t.addingTimeInterval(6))?.first == "a")
    #expect(HotKeys.advance(cycle: cycle("gone"), fresh: order, isLive: live.contains, now: t.addingTimeInterval(1))?.first == "a")
    #expect(HotKeys.advance(cycle: cycle("a"), fresh: [], isLive: live.contains, now: t)?.first == "b")
    #expect(HotKeys.advance(cycle: nil, fresh: [], isLive: { _ in false }, now: t) == nil)
}

@Test func cycleWithEveryIdDeadFallsBackToFresh() {
    let t = Date()
    let c = HotKeys.Cycle(order: ["x", "y"], id: "x", at: t)
    let pick = HotKeys.advance(cycle: c, fresh: ["n1", "n2"], isLive: Set(["n1", "n2"]).contains, now: t.addingTimeInterval(2))
    #expect(pick?.first == "n1")
    #expect(pick?.order == ["n1", "n2"])
}

@Test func cycleSurvivesSessionLeavingFreshOrder() {
    // W1 waits, I1 and I2 finished; jumping to I1 marks it seen, so a fresh order is [W1, I2] and no longer holds I1.
    let t = Date()
    let saved = ["W1", "I1", "I2"]
    let live = Set(saved)
    let c = HotKeys.Cycle(order: saved, id: "I1", at: t)
    let pick = HotKeys.advance(cycle: c, fresh: ["W1", "I2"], isLive: live.contains, now: t.addingTimeInterval(2))
    #expect(pick?.first == "I2")
    #expect(pick?.order == saved)
    // A session that disappeared is skipped.
    #expect(HotKeys.advance(cycle: c, fresh: ["W1"], isLive: Set(["W1", "I1"]).contains, now: t.addingTimeInterval(2))?.first == "W1")
    // After 5 s the cycle starts over from a fresh order.
    #expect(HotKeys.advance(cycle: c, fresh: ["W1", "I2"], isLive: live.contains, now: t.addingTimeInterval(6))?.first == "W1")
}
