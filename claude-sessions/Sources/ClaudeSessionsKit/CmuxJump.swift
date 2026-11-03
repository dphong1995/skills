import AppKit

public enum CmuxJump {
    static let bundleID = "com.cmuxterm.app"

    static func surface(fromSessionsOutput out: String) -> String? {
        guard let r = out.range(of: "surface=[0-9A-Fa-f-]{36}", options: .regularExpression) else { return nil }
        return String(out[r].dropFirst("surface=".count))
    }

    static let focusedSurfaceScript = """
    with timeout of 2 seconds
        tell application id "com.cmuxterm.app" to get id of focused terminal of selected tab of front window
    end timeout
    """

    /// Blocking: call off the main thread. Nil when cmux is missing or the output has no surface.
    public static func surface(sessionId: String) -> String? {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else { return nil }
        let p = Process()
        p.executableURL = app.appendingPathComponent("Contents/Resources/bin/cmux")
        p.arguments = ["sessions", "--session", sessionId, "--agent", "claude"]
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        guard (try? p.run()) != nil else { return nil }
        DispatchQueue.global().asyncAfter(deadline: .now() + 2) { if p.isRunning { p.terminate() } }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return surface(fromSessionsOutput: String(decoding: data, as: UTF8.self))
    }

    /// Main thread. Nil on any AppleScript error.
    public static func focusedSurface() -> String? {
        var err: NSDictionary?
        let r = NSAppleScript(source: focusedSurfaceScript)?.executeAndReturnError(&err)
        return err == nil ? r?.stringValue : nil
    }

    public static func jump(sessionId: String) {
        DispatchQueue.global().async {
            let surface = CmuxJump.surface(sessionId: sessionId)
            DispatchQueue.main.async {
                guard let surface else { return NSSound.beep() }
                let src = """
                with timeout of 5 seconds
                    tell application id "\(bundleID)"
                        activate
                        focus (first terminal whose id is "\(surface)")
                    end tell
                end timeout
                """
                var err: NSDictionary?
                NSAppleScript(source: src)?.executeAndReturnError(&err)
                if err != nil { NSSound.beep() }
            }
        }
    }

    public enum Key: Equatable {
        case raw(String)
        case paste(String)
        case pause(TimeInterval)
    }

    public static func keys(for a: WidgetAction) -> [Key] {
        switch a {
        case .jump: return []
        case .approve: return [.raw("1")]
        case .deny(_, _, let reason):
            let r = reason.trimmingCharacters(in: .whitespacesAndNewlines)
            return r.isEmpty ? [.raw("\\x1b")] : [.raw("\\x1b"), .pause(0.3), .paste(r), .raw("\\r")]
        case .reply(_, let text): return [.paste(text), .raw("\\r")]
        }
    }

    static func quote(_ s: String) -> String {
        "\"" + s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    static func script(surface: String, key: Key) -> String? {
        let target = "(first terminal whose id is \(quote(surface)))"
        switch key {
        case .raw(let r): return "tell application id \"\(bundleID)\" to perform action \(quote("text:" + r)) on \(target)"
        case .paste(let t): return "tell application id \"\(bundleID)\" to input text \(quote(t)) to \(target)"
        case .pause: return nil
        }
    }

    /// Types the keys into the session's terminal without activating cmux. Beeps on a missing surface or the first error.
    public static func send(sessionId: String, keys: [Key], knownSurface: String?) {
        func run(_ surface: String) {
            var i = 0
            func step() {
                guard i < keys.count else { return }
                let key = keys[i]
                i += 1
                if case .pause(let t) = key {
                    DispatchQueue.main.asyncAfter(deadline: .now() + t) { step() }
                    return
                }
                guard let body = script(surface: surface, key: key) else { return }
                var err: NSDictionary?
                NSAppleScript(source: "with timeout of 5 seconds\n\(body)\nend timeout")?.executeAndReturnError(&err)
                if err != nil { return NSSound.beep() }
                step()
            }
            step()
        }
        if let knownSurface { return run(knownSurface) }
        DispatchQueue.global().async {
            let surface = CmuxJump.surface(sessionId: sessionId)
            DispatchQueue.main.async {
                guard let surface else { return NSSound.beep() }
                run(surface)
            }
        }
    }
}
