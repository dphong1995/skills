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

func write(_ url: URL, _ text: String) {
    try! FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try! text.write(to: url, atomically: true, encoding: .utf8)
}

func repo(_ head: String) -> URL {
    let d = makeTempDir()
    write(d.appendingPathComponent(".git/HEAD"), head)
    return d
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

/// Store whose transcript lookups never touch the real ~/.claude/projects and whose branch reader never touches a real repo.
func makeStore(dir: URL, projects: URL = makeTempDir(), tasks: URL = makeTempDir(), startHidden: Bool = false, prs: PRMonitor? = nil, jira: JiraMonitor? = nil, branch: @escaping (String) -> String? = { _ in nil }) -> Store {
    Store(dir: dir, projects: projects, tasks: tasks, startHidden: startHidden, prs: prs, jira: jira, scanner: TranscriptScanner(queue: nil), branch: branch)
}

func append(_ url: URL, _ s: String) {
    let h = try! FileHandle(forWritingTo: url)
    try! h.seekToEnd()
    try! h.write(contentsOf: Data(s.utf8))
    try! h.close()
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

func assistantLine(_ text: String?, sidechain: Bool = false) -> String {
    // One content block per line, like Claude Code writes them.
    let content: [[String: Any]] = [text.map { ["type": "text", "text": $0] } ?? ["type": "tool_use", "name": "Bash"]]
    return jsonLine(["type": "assistant", "isSidechain": sidechain, "message": ["content": content]])
}

/// The main-turn end marker Claude Code appends; counts left nil are omitted.
func turnDurationLine(background: Int? = nil, workflows: Int? = nil, sidechain: Bool = false) -> String {
    var o: [String: Any] = ["type": "system", "subtype": "turn_duration", "isSidechain": sidechain, "durationMs": 10]
    if let background { o["pendingBackgroundAgentCount"] = background }
    if let workflows { o["pendingWorkflowCount"] = workflows }
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

func userLine(_ text: String, isMeta: Bool = false, origin: String? = nil) -> String {
    var o: [String: Any] = ["type": "user", "message": ["content": text]]
    if isMeta { o["isMeta"] = true }
    if let origin { o["origin"] = ["kind": origin] }
    return jsonLine(o)
}

/// A background agent's report as Claude Code records it; `origin: false` is the older format without the field.
func notificationLine(_ result: String, toolUseId: String? = nil, origin: Bool = true) -> String {
    let id = toolUseId.map { "<tool-use-id>\($0)</tool-use-id>\n" } ?? ""
    return userLine("<task-notification>\n\(id)<status>completed</status>\n<result>\(result)</result>\n</task-notification>",
             origin: origin ? "task-notification" : nil)
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
func prJSON(id: Int, repo: String = "t6engineering/talo6-dct-backend", branch: String = "bugfix/T6IR-14744", state: String = "OPEN", comments: Int = 0, reviewers: Int = 0, rabbitApproved: Bool? = nil) -> [String: Any] {
    var participants: [[String: Any]] = []
    for i in 0..<reviewers {
        var reviewerState: Any = NSNull()
        if i == 0 { reviewerState = "changes_requested" } else if i == 1 { reviewerState = "approved" }
        let user: [String: Any] = ["uuid": "{r\(i)}", "display_name": "Reviewer \(i)"]
        participants.append(["role": "REVIEWER", "approved": i == 1, "state": reviewerState, "user": user])
    }
    if let rabbitApproved {
        participants.append(["role": "REVIEWER", "approved": rabbitApproved, "state": rabbitApproved ? "approved" : NSNull(), "user": ["uuid": "{tin}", "display_name": "Tin Tran"]])
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
    private let lock = NSLock()
    private var _requests: [String] = []
    private var _posts: [(path: String, json: [String: Any])] = []
    var requests: [String] { lock.lock(); defer { lock.unlock() }; return _requests }
    var posts: [(path: String, json: [String: Any])] { lock.lock(); defer { lock.unlock() }; return _posts }
    var postResult: Result<Data, BitbucketError> = .success(Data("{}".utf8))
    /// When set, `post` waits on it before returning.
    var postGate: DispatchSemaphore?

    func set(_ key: String, json: Any) { routes[key] = .success(try! JSONSerialization.data(withJSONObject: json)) }

    func requests(containing s: String) -> Int { requests.filter { $0.contains(s) }.count }

    func get(_ path: String) throws -> Data {
        lock.lock(); _requests.append(path); lock.unlock()
        if let failAll { throw failAll }
        guard let key = routes.keys.filter({ path.contains($0) }).max(by: { $0.count < $1.count }) else { throw BitbucketError.http(404) }
        return try routes[key]!.get()
    }

    func post(_ path: String, json: [String: Any]) throws -> Data {
        lock.lock(); _posts.append((path, json)); lock.unlock()
        postGate?.wait()
        return try postResult.get()
    }
}

/// Routes for the user, an open list and default passing CI status / no comments.
func fake(open: [[String: Any]] = []) -> FakeBitbucket {
    let api = FakeBitbucket()
    api.set("/2.0/user", json: ["uuid": "{me}"])
    api.set("/2.0/workspaces/t6engineering/pullrequests/", json: ["values": open])
    api.set("/statuses", json: ["values": [["state": "SUCCESSFUL"]]])
    api.set("/comments", json: ["values": []])
    return api
}

func comment(_ uuid: String, _ name: String, _ at: String) -> [String: Any] {
    ["user": ["uuid": uuid, "display_name": name], "created_on": at, "deleted": false]
}

/// A CodeRabbit build status as Bitbucket returns it.
func rabbitStatus(state: String = "SUCCESSFUL", description: String, commit: String = "abc123", created: String = "2026-11-03T03:33:15.111541+00:00", updated: String? = nil) -> [String: Any] {
    ["key": "coderabbitai/6867", "name": "CodeRabbit", "state": state, "description": description,
     "created_on": created, "updated_on": updated ?? created, "commit": ["hash": commit]]
}

/// A comment carrying CodeRabbit's marker, posted by a human account like the real ones.
func rabbitComment(_ raw: String, at: String) -> [String: Any] {
    ["user": ["uuid": "{tin}", "display_name": "Tin Tran"], "created_on": at, "updated_on": at, "deleted": false,
     "content": ["raw": "[coderabbit-html-comment-v1-0]: https://coderabbit.ai/#html-comment-v1:abc\n\(raw)"]]
}

/// An inline CodeRabbit comment: a finding, or a reply when `parent` is given.
func rabbitInline(id: Int, _ raw: String, at: String, parent: Int? = nil) -> [String: Any] {
    var c = rabbitComment(raw, at: at)
    c["id"] = id
    c["inline"] = ["path": "a.ts"]
    if let parent { c["parent"] = ["id": parent] }
    return c
}

/// A CodeRabbit summary with `count` and one top-level finding nobody answered, so the review stays in
/// `.reviewed(findings:)` instead of moving on to `.awaitingApproval`.
func pendingFinding(count: Int, at: String) -> [[String: Any]] {
    [rabbitComment("**Actionable comments posted: \(count)**", at: at), rabbitInline(id: 1, "nit", at: at)]
}

/// A comment without the CodeRabbit marker, e.g. a human reply to a finding.
func humanReply(id: Int, parent: Int?, uuid: String = "{me}", _ raw: String, at: String, inline: Bool = true) -> [String: Any] {
    var c: [String: Any] = ["id": id, "user": ["uuid": uuid, "display_name": "Human"], "created_on": at, "updated_on": at, "deleted": false,
                            "content": ["raw": raw]]
    if inline { c["inline"] = ["path": "a.ts"] }
    if let parent { c["parent"] = ["id": parent] }
    return c
}

final class FakeJira: JiraAPI {
    /// `(host, keys)` per `bulkFetch` call.
    var calls: [(host: String, keys: [String])] = []
    var issues: [String: (title: String, status: String?)] = [:]
    var types: [String: (name: String, icon: String)] = [:]
    /// Key to `statusCategory.key`.
    var categories: [String: String] = [:]
    var iconData: [URL: Data] = [:]
    var iconFail: JiraError?
    var iconCalls: [URL] = []
    var fail: JiraError?
    var myselfResult = Data("{}".utf8)

    func bulkFetch(host: String, keys: [String]) throws -> Data {
        calls.append((host, keys))
        if let fail { throw fail }
        let found: [[String: Any]] = keys.compactMap { k in
            guard let i = issues[k] else { return nil }
            var fields: [String: Any] = ["summary": i.title]
            if i.status != nil || categories[k] != nil {
                var st: [String: Any] = [:]
                if let s = i.status { st["name"] = s }
                if let c = categories[k] { st["statusCategory"] = ["key": c] }
                fields["status"] = st
            }
            if let t = types[k] { fields["issuetype"] = ["name": t.name, "iconUrl": t.icon] }
            return ["key": k, "fields": fields]
        }
        return try JSONSerialization.data(withJSONObject: ["issues": found])
    }

    func myself(host: String) throws -> Data {
        if let fail { throw fail }
        return myselfResult
    }

    func icon(url: URL) throws -> Data {
        iconCalls.append(url)
        if let iconFail { throw iconFail }
        guard let d = iconData[url] else { throw JiraError.http(404) }
        return d
    }
}

func prRef(_ id: Int, repo: String = "t6engineering/talo6-dct-backend") -> PRRef {
    let parts = repo.split(separator: "/")
    return PRRef(workspace: String(parts[0]), repo: String(parts[1]), id: id)
}

/// One `question` item as `feed.list` returns it (snake_case; no `ppid`; the session id is inside `workstream_id`).
func feedItem(requestId: String = "R1", sessionId: String = "a", created: Date = Date(), questions: [[String: Any]]? = nil) -> [String: Any] {
    let qs = questions ?? [feedQuestionJSON()]
    let b64 = { (s: String) in Data(s.utf8).base64EncodedString() }
    return ["kind": "question", "id": "ID-\(requestId)", "request_id": requestId, "status": "pending", "source": "claude",
            "cwd": "/x", "created_at": ISO8601DateFormatter().string(from: created),
            "workstream_id": "cmux-feed-v1:\(b64("claude")):\(b64(sessionId))", "questions": qs]
}

func feedQuestionJSON(prompt: String = "Pick a color", header: String = "Color", multi: Bool = false, labels: [String] = ["Red", "Blue", "Green"]) -> [String: Any] {
    ["id": "q0", "prompt": prompt, "header": header, "multi_select": multi,
     "options": labels.enumerated().map { ["id": "opt\($0.offset)", "label": $0.element, "description": "d-\($0.element)"] }]
}

func feedQuestion(requestId: String = "R1", sessionId: String? = "a", pid: Int32? = nil, created: Date = Date(), questions: [FeedQuestion.Item]? = nil) -> FeedQuestion {
    FeedQuestion(requestId: requestId, createdAt: created, pid: pid, sessionId: sessionId,
                 questions: questions ?? [FeedQuestion.Item(prompt: "Pick a color", header: "Color", multiSelect: false,
                                                            options: ["Red", "Blue"].map { FeedQuestion.Option(id: $0, label: $0, description: nil) })])
}

/// Canned `feed.list` / `feed.question.reply`; records every call. `failAll` makes every call throw.
final class FakeFeedTransport: FeedTransport {
    struct Failure: Error {}
    var items: [[String: Any]] = []
    var failAll = false
    var replyResult = "{\"ok\":true,\"result\":{\"delivered\":true}}"
    private let lock = NSLock()
    private var _calls: [(method: String, params: [String: Any])] = []
    var calls: [(method: String, params: [String: Any])] { lock.lock(); defer { lock.unlock() }; return _calls }

    func request(_ method: String, _ params: [String: Any], timeout: TimeInterval) throws -> Data {
        lock.lock(); _calls.append((method, params)); lock.unlock()
        if failAll { throw Failure() }
        if method == "feed.list" {
            return try JSONSerialization.data(withJSONObject: ["ok": true, "id": "x", "result": ["items": items]])
        }
        return Data(replyResult.utf8)
    }
}

/// A one-connection-at-a-time `AF_UNIX` listener: reads one line, answers `answer` (nothing when nil) and closes after the client does.
final class TestUnixServer {
    var answer: String?
    private let fd: Int32
    private let lock = NSLock()
    private var request = Data()

    init(path: String) throws {
        fd = socket(AF_UNIX, SOCK_STREAM, 0)
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &addr.sun_path) { buf in
            for (i, b) in path.utf8.enumerated() { buf[i] = b }
        }
        let bound = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard bound == 0, listen(fd, 4) == 0 else { throw CocoaError(.fileWriteUnknown) }
        Thread.detachNewThread { [self] in serve() }
    }

    private func serve() {
        while true {
            let c = accept(fd, nil, nil)
            if c < 0 { return }
            var line = Data()
            var byte: UInt8 = 0
            while read(c, &byte, 1) == 1 {
                line.append(byte)
                if byte == 10 { break }
            }
            lock.lock(); request = line; lock.unlock()
            if let answer {
                _ = answer.withCString { write(c, $0, strlen($0)) }
                Darwin.close(c)
            } else {
                // Stay silent until the client gives up.
                var b: UInt8 = 0
                _ = read(c, &b, 1)
                Darwin.close(c)
            }
        }
    }

    func lastRequest() -> Data { lock.lock(); defer { lock.unlock() }; return request }
    func close() { Darwin.close(fd) }
}
