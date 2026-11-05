import AppKit

public enum CmuxJump {
    public static let bundleID = "com.cmuxterm.app"

    static func surface(fromSessionsOutput out: String) -> String? {
        guard let r = out.range(of: "surface=[0-9A-Fa-f-]{36}", options: .regularExpression) else { return nil }
        return String(out[r].dropFirst("surface=".count))
    }

    private static var cliPath: String? {
        NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID)?
            .appendingPathComponent("Contents/Resources/bin/cmux").path
    }

    /// Blocking: call off the main thread. Nil when cmux is missing or the output has no surface.
    public static func surface(sessionId: String) -> String? {
        guard let cmux = cliPath else { return nil }
        guard let out = runProcess(cmux, ["sessions", "--session", sessionId, "--agent", "claude"], timeout: 2, requireSuccess: false)
        else { return nil }
        return surface(fromSessionsOutput: out)
    }

    static func focusedSurface(fromIdentify data: Data) -> String? {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let focused = o["focused"] as? [String: Any],
              focused["surface_type"] as? String == "terminal"
        else { return nil }
        return focused["surface_id"] as? String
    }

    /// Blocking: call off the main thread. The focused terminal surface of cmux, nil when cmux is missing or the focus is not a terminal.
    public static func focusedSurface() -> String? {
        guard let cmux = cliPath else { return nil }
        guard let out = runProcess(cmux, ["--id-format", "uuids", "identify", "--no-caller"], timeout: 2)
        else { return nil }
        return focusedSurface(fromIdentify: Data(out.utf8))
    }

    /// Main thread. Runs the AppleScript body under a timeout; nil on any error.
    static func run(_ body: String, timeout: Int) -> NSAppleEventDescriptor? {
        var err: NSDictionary?
        let r = NSAppleScript(source: "with timeout of \(timeout) seconds\n\(body)\nend timeout")?.executeAndReturnError(&err)
        return err == nil ? r : nil
    }

    static func target(_ surface: String) -> String {
        "(first terminal whose id is \(quote(surface)))"
    }

    /// Main thread. Activates cmux and focuses the terminal; false on any AppleScript error.
    public static func jump(surface: String) -> Bool {
        run("tell application id \"\(bundleID)\"\nactivate\nfocus \(target(surface))\nend tell", timeout: 5) != nil
    }

    public enum Key: Equatable {
        case raw(String)
        case paste(String)
        case pause(TimeInterval)
    }

    /// Empties Claude's input so a half-typed draft is not sent with a deny reason: Ctrl-E, then Ctrl-U + Backspace
    /// for up to 20 lines (Ctrl-U alone stops at the line start). Ctrl-Y in Claude brings the draft back.
    static let clearInput = Key.raw("\\x05" + String(repeating: "\\x15\\x7f", count: 20))

    public static func keys(for a: WidgetAction) -> [Key] {
        switch a {
        case .jump, .openLink, .connect, .nudgeRabbit: return []
        case .approve: return [.raw("1")]
        case .deny(_, _, let reason):
            let r = reason.trimmingCharacters(in: .whitespacesAndNewlines)
            return r.isEmpty ? [.raw("\\x1b")] : [.raw("\\x1b"), .pause(0.3), clearInput, .paste(r), .raw("\\r")]
        case .askClaude(_, let prompt): return [clearInput, .paste(prompt), .raw("\\r")]
        }
    }

    static func quote(_ s: String) -> String {
        "\"" + s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    /// The AppleScript body for one key. Pauses are handled by `send`, so they have no script.
    static func script(surface: String, key: Key) -> String {
        switch key {
        case .raw(let r): return "tell application id \"\(bundleID)\" to perform action \(quote("text:" + r)) on \(target(surface))"
        case .paste(let t): return "tell application id \"\(bundleID)\" to input text \(quote(t)) to \(target(surface))"
        case .pause: return ""
        }
    }

    /// Types the keys into the surface's terminal without activating cmux. `completion` runs on main: false on the first error.
    public static func send(surface: String, keys: [Key], completion: @escaping (Bool) -> Void) {
        send(surface: surface, keys: keys[...], completion: completion)
    }

    private static func send(surface: String, keys: ArraySlice<Key>, completion: @escaping (Bool) -> Void) {
        var rest = keys
        while let key = rest.popFirst() {
            if case .pause(let t) = key {
                let remaining = rest
                DispatchQueue.main.asyncAfter(deadline: .now() + t) { send(surface: surface, keys: remaining, completion: completion) }
                return
            }
            guard run(script(surface: surface, key: key), timeout: 5) != nil else { return completion(false) }
        }
        completion(true)
    }
}
