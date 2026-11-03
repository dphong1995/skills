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
}
