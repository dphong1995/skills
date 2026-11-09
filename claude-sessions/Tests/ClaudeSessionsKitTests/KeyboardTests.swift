import AppKit
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

// MARK: key table

private let none = NSEvent.ModifierFlags()

@MainActor
private func keyboardSetup(_ sessions: [Session]) -> (Store, PanelState) {
    let store = makeStore(dir: makeTempDir())
    store.sessions = sessions
    let panel = PanelState()
    panel.keyboard = true
    panel.selected = sessions.first?.id
    return (store, panel)
}

private func idle(_ id: String = "a") -> Session {
    Session(id: id, name: id, cwd: "/x", status: "idle", waitingFor: nil, since: nil)
}

private func waiting(command: String = "ls", toolId: String = "t1") -> Session {
    var s = Session(id: "a", name: "a", cwd: "/x", status: "waiting", waitingFor: "permission prompt", since: nil)
    s.pending = .one(Pending(toolId: toolId, tool: "Bash", text: command))
    return s
}

private func asking() -> Session {
    var s = Session(id: "a", name: "a", cwd: "/x", status: "waiting", waitingFor: nil, since: nil)
    s.question = feedQuestion(sessionId: "a")
    s.questionLive = true
    return s
}

@MainActor
private func press(_ key: String, _ store: Store, _ panel: PanelState, code: UInt16 = 0, flags: NSEvent.ModifierFlags = none) -> KeyResult {
    KeyCommand.resolve(keyCode: code, characters: key, flags: flags, store: store, panel: panel)
}

@MainActor @Test func arrowsMoveSelection() {
    let (store, panel) = keyboardSetup([idle("a")])
    #expect(press("", store, panel, code: KeyCode.up) == .move(-1))
    #expect(press("", store, panel, code: KeyCode.down) == .move(1))
    #expect(press("", store, panel, code: KeyCode.down, flags: .command) == .pass)
}

@MainActor @Test func returnJumps() {
    let (store, panel) = keyboardSetup([idle("a")])
    #expect(press("", store, panel, code: KeyCode.returnKey) == .action(.jump("a")))
    #expect(press("", store, panel, code: KeyCode.keypadEnter) == .action(.jump("a")))
    #expect(press("", store, panel, code: KeyCode.returnKey, flags: .option) == .pass)
}

@MainActor @Test func approveNeedsApprovableAndFullCommand() {
    var (store, panel) = keyboardSetup([waiting()])
    #expect(press("a", store, panel) == .action(.approve("a", toolId: "t1")))
    #expect(press("A", store, panel) == .action(.approve("a", toolId: "t1")))
    (store, panel) = keyboardSetup([idle()])
    #expect(press("a", store, panel) == .beep)
    let long = (1...6).map { "line \($0)" }.joined(separator: "\n")
    (store, panel) = keyboardSetup([waiting(command: long)])
    #expect(press("a", store, panel) == .beep)
    store.shownCommands.insert("t1")
    #expect(press("a", store, panel) == .action(.approve("a", toolId: "t1")))
    (store, panel) = keyboardSetup([waiting(command: String(repeating: "x\n", count: 20))])
    store.shownCommands.insert("t1")
    #expect(press("a", store, panel) == .beep)
}

@MainActor @Test func denyFollowsTheDenyButton() {
    var (store, panel) = keyboardSetup([waiting()])
    #expect(press("d", store, panel) == .edit(Store.Editing(id: "a", kind: .deny, toolId: "t1")))
    (store, panel) = keyboardSetup([idle()])
    #expect(press("d", store, panel) == .beep)
    // Only `duyệt trong cmux` is offered for a long command that is not expanded, or too long: D beeps too.
    let long = (1...6).map { "line \($0)" }.joined(separator: "\n")
    (store, panel) = keyboardSetup([waiting(command: long)])
    #expect(press("d", store, panel) == .beep)
    store.shownCommands.insert("t1")
    #expect(press("d", store, panel) == .edit(Store.Editing(id: "a", kind: .deny, toolId: "t1")))
    (store, panel) = keyboardSetup([waiting(command: String(repeating: "x\n", count: 20))])
    #expect(press("d", store, panel) == .beep)
}

@MainActor @Test func replyNeedsIdleSessionInCmux() {
    var (store, panel) = keyboardSetup([idle()])
    #expect(press("r", store, panel) == .edit(Store.Editing(id: "a", kind: .reply)))
    store.surfaceMisses.insert("a")
    #expect(press("r", store, panel) == .beep)
    let busy = Session(id: "a", name: "a", cwd: "/x", status: "busy", waitingFor: nil, since: nil)
    (store, panel) = keyboardSetup([busy])
    #expect(press("r", store, panel) == .beep)
}

@MainActor @Test func digitsPickOptionsAndBeepBeyondCount() {
    let (store, panel) = keyboardSetup([asking()])
    #expect(press("1", store, panel) == .pick(question: 0, label: "Red"))
    #expect(press("2", store, panel) == .pick(question: 0, label: "Blue"))
    #expect(press("3", store, panel) == .beep)
    #expect(press("9", store, panel) == .beep)
    #expect(press("0", store, panel) == .pass)
    let (plain, plainPanel) = keyboardSetup([idle()])
    #expect(press("1", plain, plainPanel) == .beep)
}

@MainActor @Test func commandReturnSubmitsOnlyACompleteForm() {
    let (store, panel) = keyboardSetup([asking()])
    #expect(press("", store, panel, code: KeyCode.returnKey, flags: .command) == .beep)
    panel.pick(feedQuestion(sessionId: "a"), 0, "Blue")
    #expect(press("", store, panel, code: KeyCode.returnKey, flags: .command)
            == .action(.answer("a", requestId: "R1", answers: [["Blue"]], other: [:])))
    let (plain, plainPanel) = keyboardSetup([idle()])
    #expect(press("", plain, plainPanel, code: KeyCode.returnKey, flags: .command) == .beep)
}

@MainActor @Test func escapeCloses() {
    let (store, panel) = keyboardSetup([idle()])
    #expect(press("", store, panel, code: KeyCode.escape) == .close)
    panel.editing = Store.Editing(id: "a", kind: .reply)
    #expect(press("", store, panel, code: KeyCode.escape) == .pass)
}

@MainActor @Test func anyKeyPassesWhileEditing() {
    let (store, panel) = keyboardSetup([waiting()])
    panel.editing = Store.Editing(id: "a", kind: .deny, toolId: "t1")
    for key in ["a", "d", "r", "1", "x"] { #expect(press(key, store, panel) == .pass) }
    #expect(press("", store, panel, code: KeyCode.returnKey) == .pass)
    #expect(press("", store, panel, code: KeyCode.up) == .pass)
}

@MainActor @Test func unboundKeysPassWithoutSelection() {
    let (store, panel) = keyboardSetup([])
    #expect(press("x", store, panel) == .pass)
    #expect(press("0", store, panel) == .pass)
    #expect(press("a", store, panel, flags: .command) == .pass)
    #expect(press("ab", store, panel) == .pass)
    for key in ["a", "d", "r", "1"] { #expect(press(key, store, panel) == .beep) }
    #expect(press("", store, panel, code: KeyCode.returnKey) == .beep)
    let (full, fullPanel) = keyboardSetup([idle()])
    #expect(press("x", full, fullPanel) == .pass)
}

@MainActor @Test func notInKeyboardModePasses() {
    let (store, panel) = keyboardSetup([idle()])
    panel.keyboard = false
    #expect(press("a", store, panel) == .pass)
    #expect(press("", store, panel, code: KeyCode.escape) == .pass)
}

// MARK: hotkey and key status

@Test func hotkeyOpensWhenNothingIsOpen() {
    #expect(PanelState.hotkeyToggle(open: []) == .open)
}

@Test func hotkeyClosesAPinnedOrKeyboardCard() {
    let pinned = PanelState()
    pinned.hovering = true; pinned.pinnedOpen = true
    #expect(PanelState.hotkeyToggle(open: [pinned]) == .close)
    let typing = PanelState()
    typing.hovering = true; typing.keyboard = true
    #expect(PanelState.hotkeyToggle(open: [typing]) == .close)
}

@Test func hotkeyOnAHoverCardTakesTheKeyboardAndPins() {
    let hover = PanelState()
    hover.hovering = true
    #expect(PanelState.hotkeyToggle(open: [hover]) == .keyboard)
    hover.pinWithKeyboard()
    #expect(hover.keyboard && hover.pinnedOpen && !hover.pinnedEntered)
}

@Test func closeClearsEverything() {
    let panel = PanelState()
    panel.hovering = true; panel.pinnedOpen = true; panel.pinnedEntered = true; panel.keyboard = true; panel.selected = "a"
    panel.editing = Store.Editing(id: "a", kind: .reply)
    panel.close()
    #expect(!panel.hovering && !panel.pinnedOpen && !panel.pinnedEntered && !panel.keyboard && panel.editing == nil && panel.selected == nil)
    #expect(!panel.wantsKey)
}

@Test func editorCloseKeepsKeyboard() {
    let panel = PanelState()
    panel.keyboard = true
    panel.editing = Store.Editing(id: "a", kind: .reply)
    panel.editing = nil
    #expect(panel.wantsKey)
    #expect(!panel.shouldHandBackKey(isKey: true, hidden: false))
}

@Test func keyIsHandedBackOnlyWhenNothingWantsIt() {
    let panel = PanelState()
    panel.editing = Store.Editing(id: "a", kind: .reply)
    #expect(panel.wantsKey)
    panel.editing = nil
    #expect(panel.shouldHandBackKey(isKey: true, hidden: false))
    #expect(!panel.shouldHandBackKey(isKey: false, hidden: false))
    #expect(!panel.shouldHandBackKey(isKey: true, hidden: true))
}
