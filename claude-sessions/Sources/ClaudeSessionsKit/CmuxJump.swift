import AppKit

public enum CmuxJump {
    static let bundleID = "com.cmuxterm.app"

    static func surface(fromSessionsOutput out: String) -> String? {
        guard let r = out.range(of: "surface=[0-9A-Fa-f-]{36}", options: .regularExpression) else { return nil }
        return String(out[r].dropFirst("surface=".count))
    }

    public static func jump(sessionId: String) {
        guard let app = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else {
            return NSSound.beep()
        }
        let cli = app.appendingPathComponent("Contents/Resources/bin/cmux")
        DispatchQueue.global().async {
            let p = Process()
            p.executableURL = cli
            p.arguments = ["sessions", "--session", sessionId, "--agent", "claude"]
            let pipe = Pipe()
            p.standardOutput = pipe
            p.standardError = FileHandle.nullDevice
            var surface: String?
            if (try? p.run()) != nil {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                surface = CmuxJump.surface(fromSessionsOutput: String(decoding: data, as: UTF8.self))
            }
            DispatchQueue.main.async {
                guard let surface else { return NSSound.beep() }
                let src = """
                tell application id "\(bundleID)"
                    activate
                    focus (first terminal whose id is "\(surface)")
                end tell
                """
                var err: NSDictionary?
                NSAppleScript(source: src)?.executeAndReturnError(&err)
                if err != nil { NSSound.beep() }
            }
        }
    }
}
