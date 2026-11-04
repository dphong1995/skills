import Foundation
@testable import ClaudeSessionsKit

// Own pid keeps fake sessions alive (kill(pid, 0)); 999_999 is a dead one.
let me = getpid()

/// Fresh directory per call: Swift Testing runs tests in parallel.
func makeTempDir() -> URL {
    let dir = FileManager.default.temporaryDirectory
        .appendingPathComponent("claude-sessions-tests-\(UUID().uuidString)", isDirectory: true)
    try! FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return dir
}

func writeSession(_ dir: URL, _ file: String, pid: Int32, status: String, waitingFor: String? = nil, ageSec: Double, name: String, cwd: String, updatedAt: Date? = nil, nameSource: String? = nil) {
    var o: [String: Any] = ["pid": pid, "sessionId": file, "name": name, "cwd": cwd, "status": status,
                            "statusUpdatedAt": (updatedAt?.timeIntervalSince1970 ?? Date().timeIntervalSince1970 - ageSec) * 1000]
    if let waitingFor { o["waitingFor"] = waitingFor }
    if let nameSource { o["nameSource"] = nameSource }
    try! JSONSerialization.data(withJSONObject: o).write(to: dir.appendingPathComponent("\(file).json"))
}

let frontend = "/Users/phong/talo6-dct-frontend"
let backend = "/Users/phong/talo6-dct-backend"

/// Store whose transcript lookups never touch the real ~/.claude/projects.
func makeStore(dir: URL, projects: URL = makeTempDir(), tasks: URL = makeTempDir(), startHidden: Bool = false) -> Store {
    Store(dir: dir, projects: projects, tasks: tasks, startHidden: startHidden)
}

/// Writes `<dir>/<session>/<id>.json` like Claude Code's TaskCreate; without a `session` it writes straight into `dir`.
func writeTask(_ dir: URL, session: String? = nil, id: Int, status: String, subject: String, activeForm: String? = nil) {
    let d = session.map { dir.appendingPathComponent($0, isDirectory: true) } ?? dir
    try! FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
    var o: [String: Any] = ["id": String(id), "subject": subject, "status": status, "blocks": [], "blockedBy": []]
    if let activeForm { o["activeForm"] = activeForm }
    try! JSONSerialization.data(withJSONObject: o).write(to: d.appendingPathComponent("\(id).json"))
}

func jsonLine(_ o: [String: Any]) -> String {
    String(data: try! JSONSerialization.data(withJSONObject: o), encoding: .utf8)!
}

func assistantLine(_ text: String?, sidechain: Bool = false, branch: String? = nil) -> String {
    var content: [[String: Any]] = [["type": "tool_use", "name": "Bash"]]
    if let text { content.insert(["type": "text", "text": text], at: 0) }
    var o: [String: Any] = ["type": "assistant", "isSidechain": sidechain, "message": ["content": content]]
    if let branch { o["gitBranch"] = branch }
    return jsonLine(o)
}

func writeTranscript(_ projects: URL, cwd: String, id: String, lines: [String]) -> URL {
    let d = projects.appendingPathComponent(Transcript.projectDirName(cwd))
    try! FileManager.default.createDirectory(at: d, withIntermediateDirectories: true)
    let f = d.appendingPathComponent("\(id).jsonl")
    try! Data(lines.joined(separator: "\n").utf8).write(to: f)
    return f
}

func toolUseLine(id: String, name: String, input: [String: Any], sidechain: Bool = false) -> String {
    let o: [String: Any] = ["type": "assistant", "isSidechain": sidechain,
                            "message": ["content": [["type": "tool_use", "id": id, "name": name, "input": input]]]]
    return jsonLine(o)
}

func toolResultLine(id: String) -> String {
    let o: [String: Any] = ["type": "user", "message": ["content": [["type": "tool_result", "tool_use_id": id]]]]
    return jsonLine(o)
}

func userLine(_ text: String, branch: String? = nil) -> String {
    var o: [String: Any] = ["type": "user", "message": ["content": text]]
    if let branch { o["gitBranch"] = branch }
    return jsonLine(o)
}

func commandLine(_ name: String, args: String) -> String {
    userLine("<command-name>\(name)</command-name>\n<command-message>\(name.dropFirst())</command-message>\n<command-args>\(args)</command-args>")
}

func aiTitleLine(_ title: String) -> String {
    jsonLine(["type": "ai-title", "aiTitle": title])
}
