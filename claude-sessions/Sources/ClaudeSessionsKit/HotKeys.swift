import Carbon.HIToolbox
import Foundation

public struct HotKeyCombo: Equatable, Codable {
    public let keyCode: UInt32
    public let modifiers: UInt32
    public let label: String

    public static let openChoices = [
        HotKeyCombo(keyCode: UInt32(kVK_Space), modifiers: UInt32(controlKey | optionKey), label: "⌃⌥Space"),
        HotKeyCombo(keyCode: UInt32(kVK_Space), modifiers: UInt32(controlKey | optionKey | cmdKey), label: "⌃⌥⌘Space"),
        HotKeyCombo(keyCode: UInt32(kVK_Space), modifiers: UInt32(optionKey), label: "⌥Space"),
    ]
    public static let nextChoices = [
        HotKeyCombo(keyCode: UInt32(kVK_RightArrow), modifiers: UInt32(controlKey | optionKey), label: "⌃⌥→"),
        HotKeyCombo(keyCode: UInt32(kVK_ANSI_J), modifiers: UInt32(controlKey | optionKey), label: "⌃⌥J"),
        HotKeyCombo(keyCode: UInt32(kVK_RightArrow), modifiers: UInt32(controlKey | optionKey | cmdKey), label: "⌃⌥⌘→"),
    ]
}

/// System-wide hotkeys through Carbon `RegisterEventHotKey`, which needs no Accessibility permission.
public final class HotKeys {
    private static let signature: OSType = 0x4353_4B59 // "CSKY"
    private var refs: [UInt32: EventHotKeyRef] = [:]
    private var handlers: [UInt32: () -> Void] = [:]
    private var installed = false

    public init() {}

    /// Registers `combo` under `id`, replacing the previous one; nil only unregisters. False when the system refuses (another app owns the combo).
    @discardableResult
    public func register(_ combo: HotKeyCombo?, id: UInt32, handler: @escaping () -> Void) -> Bool {
        if let old = refs.removeValue(forKey: id) { UnregisterEventHotKey(old) }
        handlers[id] = nil
        guard let combo else { return true }
        installHandlerOnce()
        var ref: EventHotKeyRef?
        let status = RegisterEventHotKey(combo.keyCode, combo.modifiers, EventHotKeyID(signature: Self.signature, id: id),
                                         GetApplicationEventTarget(), 0, &ref)
        guard status == noErr, let ref else { return false }
        refs[id] = ref
        handlers[id] = handler
        return true
    }

    private func installHandlerOnce() {
        guard !installed else { return }
        installed = true
        var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, event, userData in
            guard let event, let userData else { return OSStatus(eventNotHandledErr) }
            var hk = EventHotKeyID()
            GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil,
                              MemoryLayout<EventHotKeyID>.size, nil, &hk)
            let me = Unmanaged<HotKeys>.fromOpaque(userData).takeUnretainedValue()
            me.handlers[hk.id]?()
            return noErr
        }, 1, &spec, Unmanaged.passUnretained(self).toOpaque(), nil)
    }

    /// One run of presses: the order seen on its first press, the session jumped to last, and when.
    public struct Cycle: Equatable {
        public let order: [String]
        public let id: String
        public let at: Date
        public init(order: [String], id: String, at: Date) { self.order = order; self.id = id; self.at = at }
    }

    /// The order to walk and the session to jump to: within 5 s of the last press, the live one after `cycle.id` in the order saved
    /// on the first press (a finished session leaves `fresh` once it is seen, so `fresh` cannot be used); else (also when every saved session has died) the first of `fresh`.
    public static func advance(cycle: Cycle?, fresh: [String], isLive: (String) -> Bool, now: Date) -> (order: [String], first: String)? {
        if let cycle, now.timeIntervalSince(cycle.at) <= 5, let i = cycle.order.firstIndex(of: cycle.id) {
            let n = cycle.order.count
            if let id = (1...n).map({ cycle.order[(i + $0) % n] }).first(where: isLive) { return (cycle.order, id) }
        }
        return fresh.first.map { (fresh, $0) }
    }
}
