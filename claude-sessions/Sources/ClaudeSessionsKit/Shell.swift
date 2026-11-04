import Foundation

/// Runs a program and returns its stdout, or nil when it cannot start or times out.
/// With `requireSuccess` (the default) a non-zero exit status also gives nil.
/// `input`, when given, is written to the process's stdin, which is then closed.
func runProcess(_ path: String, _ args: [String], timeout: TimeInterval, requireSuccess: Bool = true, input: Data? = nil) -> String? {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: path)
    p.arguments = args
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = FileHandle.nullDevice
    let stdin = input == nil ? nil : Pipe()
    if let stdin { p.standardInput = stdin }
    guard (try? p.run()) != nil else { return nil }
    DispatchQueue.global().asyncAfter(deadline: .now() + timeout) { if p.isRunning { p.terminate() } }
    if let stdin, let input {
        try? stdin.fileHandleForWriting.write(contentsOf: input)
        try? stdin.fileHandleForWriting.close()
    }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    p.waitUntilExit()
    if requireSuccess, p.terminationStatus != 0 { return nil }
    return String(decoding: data, as: UTF8.self)
}
