import Foundation

/// Runs a program and returns its stdout, or nil when it cannot start or times out.
/// With `requireSuccess` (the default) a non-zero exit status also gives nil.
func runProcess(_ path: String, _ args: [String], timeout: TimeInterval, requireSuccess: Bool = true) -> String? {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: path)
    p.arguments = args
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = FileHandle.nullDevice
    guard (try? p.run()) != nil else { return nil }
    DispatchQueue.global().asyncAfter(deadline: .now() + timeout) { if p.isRunning { p.terminate() } }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    p.waitUntilExit()
    if requireSuccess, p.terminationStatus != 0 { return nil }
    return String(decoding: data, as: UTF8.self)
}
