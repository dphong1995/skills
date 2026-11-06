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
    #expect(HotKeys.next(after: nil, in: order, lastPress: nil, now: t) == "a")
    #expect(HotKeys.next(after: "a", in: order, lastPress: t, now: t.addingTimeInterval(4)) == "b")
    #expect(HotKeys.next(after: "c", in: order, lastPress: t, now: t.addingTimeInterval(4)) == "a")
    #expect(HotKeys.next(after: "a", in: order, lastPress: t, now: t.addingTimeInterval(6)) == "a")
    #expect(HotKeys.next(after: "gone", in: order, lastPress: t, now: t.addingTimeInterval(1)) == "a")
    #expect(HotKeys.next(after: "a", in: [], lastPress: t, now: t) == nil)
}
