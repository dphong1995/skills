import AppKit

/// Virtual key codes the keyboard mode reads.
enum KeyCode {
    static let escape: UInt16 = 53
    static let up: UInt16 = 126
    static let down: UInt16 = 125
    static let returnKey: UInt16 = 36
    static let keypadEnter: UInt16 = 76
}

/// What one key press does in keyboard mode.
enum KeyResult: Equatable {
    case action(WidgetAction)
    case edit(Store.Editing)
    case pick(question: Int, label: String)
    case move(Int)
    case close
    /// A bound key that cannot apply now.
    case beep
    /// Not ours: the event goes on.
    case pass
}

enum KeyCommand {
    /// The decision for a key press; `flags` holds only the command, control and option modifiers.
    static func resolve(keyCode: UInt16, characters: String, flags: NSEvent.ModifierFlags, store: Store, panel: PanelState) -> KeyResult {
        guard panel.keyboard, panel.editing == nil else { return .pass }
        if keyCode == KeyCode.escape { return .close }
        if keyCode == KeyCode.up || keyCode == KeyCode.down {
            return flags.isEmpty ? .move(keyCode == KeyCode.up ? -1 : 1) : .pass
        }
        let isReturn = keyCode == KeyCode.returnKey || keyCode == KeyCode.keypadEnter
        let key = isReturn ? "\r" : characters.lowercased()
        guard isReturn || (flags.isEmpty && key.count == 1) else { return .pass }
        let digit = Int(key).flatMap { (1...9).contains($0) ? $0 : nil }
        let bound = isReturn ? flags.isEmpty || flags == .command : flags.isEmpty && (["a", "d", "r"].contains(key) || digit != nil)
        guard bound else { return .pass }
        guard let id = panel.selected, let s = store.sessions.first(where: { $0.id == id }) else { return .beep }
        let form: FeedQuestion? = s.question.flatMap { store.blockState(of: s) == .form ? $0 : nil }
        if isReturn {
            if flags.isEmpty { return .action(.jump(id)) }
            guard let q = form else { return .beep }
            let answers = panel.answers[q.requestId] ?? [], other = panel.others[q.requestId] ?? [:]
            return Feed.complete(q, answers: answers, other: other) ? .action(.answer(id, requestId: q.requestId, answers: answers, other: other)) : .beep
        }
        if let n = digit {
            guard let q = form, let hit = PanelState.optionAt(q, number: n) else { return .beep }
            return .pick(question: hit.question, label: hit.label)
        }
        switch key {
        case "a":
            guard let p = s.approvable, store.isCommandShown(p) else { return .beep }
            return .action(.approve(id, toolId: p.toolId))
        case "d":
            // The `Từ chối…` button shows under the same condition.
            guard let p = s.approvable, store.isCommandShown(p) else { return .beep }
            return .edit(Store.Editing(id: id, kind: .deny, toolId: p.toolId))
        default:
            return s.canAskClaude && !store.surfaceMisses.contains(id) ? .edit(Store.Editing(id: id, kind: .reply)) : .beep
        }
    }
}

extension WidgetView {
    /// Keyboard mode: runs the key table on the selected card; returns nil when the event is consumed.
    public static func handleKey(_ e: NSEvent, store: Store, panel: PanelState, onAction: (WidgetAction) -> Void) -> NSEvent? {
        guard (e.window as? WidgetPanel)?.state === panel else { return e }
        let result = KeyCommand.resolve(keyCode: e.keyCode, characters: e.charactersIgnoringModifiers ?? "",
                                        flags: e.modifierFlags.intersection([.command, .control, .option]), store: store, panel: panel)
        switch result {
        case .pass: return e
        case .close: panel.close()
        case .move(let delta): panel.moveSelection(delta, ids: store.sessions.map(\.id))
        case .action(let a): onAction(a)
        case .edit(let editing): panel.editing = editing
        case .pick(let i, let label):
            if let id = panel.selected, let q = store.sessions.first(where: { $0.id == id })?.question { panel.pick(q, i, label) }
        case .beep: NSSound.beep()
        }
        return nil
    }
}
