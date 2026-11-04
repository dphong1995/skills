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

func userLine(_ text: String, branch: String? = nil, isMeta: Bool = false) -> String {
    var o: [String: Any] = ["type": "user", "message": ["content": text]]
    if isMeta { o["isMeta"] = true }
    if let branch { o["gitBranch"] = branch }
    return jsonLine(o)
}

func commandLine(_ name: String, args: String) -> String {
    userLine("<command-name>\(name)</command-name>\n<command-message>\(name.dropFirst())</command-message>\n<command-args>\(args)</command-args>")
}

func aiTitleLine(_ title: String) -> String {
    jsonLine(["type": "ai-title", "aiTitle": title])
}

/// A `type: "user"` line whose content is a `tool_result` block carrying `text`.
func toolResultTextLine(_ text: String) -> String {
    jsonLine(["type": "user", "message": ["content": [["type": "tool_result", "tool_use_id": "t1", "content": [["type": "text", "text": text]], "text": text]]]])
}

/// Bitbucket PR object shaped like an open-list value. `repo` is `workspace/name`; the first reviewer requests changes, the second approves.
func prJSON(id: Int, repo: String = "t6engineering/talo6-dct-backend", branch: String = "bugfix/T6IR-14744", state: String = "OPEN", comments: Int = 0, reviewers: Int = 0) -> [String: Any] {
    var participants: [[String: Any]] = []
    for i in 0..<reviewers {
        var reviewerState: Any = NSNull()
        if i == 0 { reviewerState = "changes_requested" } else if i == 1 { reviewerState = "approved" }
        let user: [String: Any] = ["uuid": "{r\(i)}", "display_name": "Reviewer \(i)"]
        participants.append(["role": "REVIEWER", "approved": i == 1, "state": reviewerState, "user": user])
    }
    participants.append(["role": "PARTICIPANT", "approved": false, "state": NSNull(), "user": ["uuid": "{author}", "display_name": "Author"]])
    return ["id": id, "title": "t", "state": state, "comment_count": comments,
            "source": ["branch": ["name": branch], "repository": ["full_name": repo], "commit": ["hash": "abc123"]],
            "links": ["html": ["href": "https://bitbucket.org/\(repo)/pull-requests/\(id)"]],
            "participants": participants]
}

/// Canned Bitbucket. A route matches when the requested path contains its key; the longest matching key wins.
/// Every request is recorded; `failAll` makes every call throw.
final class FakeBitbucket: BitbucketAPI {
    var routes: [String: Result<Data, BitbucketError>] = [:]
    var failAll: BitbucketError?
    private(set) var requests: [String] = []

    func set(_ key: String, json: Any) { routes[key] = .success(try! JSONSerialization.data(withJSONObject: json)) }

    func requests(containing s: String) -> Int { requests.filter { $0.contains(s) }.count }

    func get(_ path: String) throws -> Data {
        requests.append(path)
        if let failAll { throw failAll }
        guard let key = routes.keys.filter({ path.contains($0) }).max(by: { $0.count < $1.count }) else { throw BitbucketError.http(404) }
        return try routes[key]!.get()
    }
}
