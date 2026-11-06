import Foundation

/// A pending AskUserQuestion item of cmux Feed.
public struct FeedQuestion: Equatable {
    public struct Option: Equatable {
        public let id: String
        public let label: String
        public let description: String?
        public init(id: String, label: String, description: String?) { self.id = id; self.label = label; self.description = description }
    }
    public struct Item: Equatable {
        public let prompt: String
        public let header: String?
        public let multiSelect: Bool
        public let options: [Option]
        public init(prompt: String, header: String?, multiSelect: Bool, options: [Option]) {
            self.prompt = prompt; self.header = header; self.multiSelect = multiSelect; self.options = options
        }
    }
    public let requestId: String
    public let createdAt: Date
    public let pid: Int32?
    public let sessionId: String?
    public let questions: [Item]
    public init(requestId: String, createdAt: Date, pid: Int32?, sessionId: String?, questions: [Item]) {
        self.requestId = requestId; self.createdAt = createdAt; self.pid = pid; self.sessionId = sessionId; self.questions = questions
    }
}

public enum Feed {
    /// How long the Feed hook waits for an answer, from the item's `createdAt`.
    public static let window: TimeInterval = 120
    /// After a failed call the monitor waits this long before asking again.
    static let retryAfter: TimeInterval = 15

    public enum BlockState: Equatable { case form, sent, fallback }

    /// What a card shows for a question: the form, `Đã gửi`, or only `Trả lời trong cmux`.
    static func blockState(_ q: FeedQuestion, live: Bool, sent: Bool, failed: Bool, now: Date) -> BlockState {
        if sent { return .sent }
        guard live, !failed, supports(q), remaining(q, now: now) > 0 else { return .fallback }
        return .form
    }

    static func remaining(_ q: FeedQuestion, now: Date) -> TimeInterval { q.createdAt.addingTimeInterval(window).timeIntervalSince(now) }

    /// `còn m:ss`, never below 0:00.
    static func countdown(_ q: FeedQuestion, now: Date) -> String {
        let s = max(0, Int(remaining(q, now: now).rounded(.up)))
        return String(format: "còn %d:%02d", s / 60, s % 60)
    }

    /// Every question has options to pick from: the shapes the spike showed cmux takes (single, multi, several questions, Other text).
    static func supports(_ q: FeedQuestion) -> Bool {
        !q.questions.isEmpty && q.questions.allSatisfy { !$0.options.isEmpty }
    }

    /// Every question has a pick or non-blank Other text.
    static func complete(_ q: FeedQuestion, answers: [[String]], other: [Int: String]) -> Bool {
        q.questions.indices.allSatisfy { i in
            !(answers.indices.contains(i) ? answers[i] : []).isEmpty || !(other[i] ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
    }

    /// The newest question of a session: matched by pid, else by session id.
    static func match(_ items: [FeedQuestion], pid: Int32, sessionId: String) -> FeedQuestion? {
        items.filter { $0.pid == pid || $0.sessionId == sessionId }
            .max { $0.createdAt < $1.createdAt }
    }

    /// `feed.question.reply` params: one string per answered question, in order: the label, the labels joined by `", "`
    /// (multi-select), or the typed Other text; Other text follows the picks. Questions without an answer are left out.
    public static func replyParams(requestId: String, answers: [[String]], other: [Int: String]) -> [String: Any] {
        let count = max(answers.count, (other.keys.max() ?? -1) + 1)
        let selections: [String] = (0..<count).compactMap { i in
            let text = (other[i] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
            let parts = (answers.indices.contains(i) ? answers[i] : []) + (text.isEmpty ? [] : [text])
            return parts.isEmpty ? nil : parts.joined(separator: ", ")
        }
        return ["request_id": requestId, "selections": selections]
    }

    private static func date(_ v: Any?) -> Date? {
        guard let s = v as? String else { return nil }
        let f = ISO8601DateFormatter()
        if let d = f.date(from: s) { return d }
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f.date(from: s)
    }

    /// The session id inside `cmux-feed-v1:<b64 source>:<b64 session id>`.
    private static func sessionId(of workstream: String?) -> String? {
        guard let last = workstream?.split(separator: ":").last, workstream?.split(separator: ":").count == 3,
              let data = Data(base64Encoded: String(last)) else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// The pending questions of a `feed.list` response; anything else (or a protocol change) gives an empty list.
    static func parseList(_ data: Data) -> [FeedQuestion] {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any], o["ok"] as? Bool == true,
              let items = (o["result"] as? [String: Any])?["items"] as? [[String: Any]] else { return [] }
        return items.compactMap { item in
            guard item["kind"] as? String == "question", (item["status"] as? String ?? "pending") == "pending" else { return nil }
            let payload = (item["payload"] as? [String: Any])?["question"] as? [String: Any]
            guard let requestId = item["request_id"] as? String ?? payload?["requestId"] as? String else { return nil }
            let raw = item["questions"] as? [[String: Any]] ?? payload?["questions"] as? [[String: Any]] ?? []
            let questions = raw.compactMap { q -> FeedQuestion.Item? in
                guard let prompt = q["prompt"] as? String else { return nil }
                let options = (q["options"] as? [[String: Any]] ?? []).compactMap { o -> FeedQuestion.Option? in
                    guard let id = o["id"] as? String, let label = o["label"] as? String else { return nil }
                    return FeedQuestion.Option(id: id, label: label, description: o["description"] as? String)
                }
                return FeedQuestion.Item(prompt: prompt, header: q["header"] as? String,
                                         multiSelect: (q["multi_select"] ?? q["multiSelect"]) as? Bool ?? false, options: options)
            }
            return FeedQuestion(requestId: requestId, createdAt: date(item["created_at"] ?? item["createdAt"]) ?? Date(),
                                pid: (item["ppid"] as? NSNumber)?.int32Value,
                                sessionId: sessionId(of: (item["workstream_id"] ?? item["workstreamId"]) as? String), questions: questions)
        }
    }
}

public protocol FeedTransport {
    /// Sends one request line and returns the one response line.
    func request(_ method: String, _ params: [String: Any], timeout: TimeInterval) throws -> Data
}

/// cmux's control socket (`AF_UNIX` stream); no `auth` line is needed from outside cmux (spike).
public final class SocketFeedTransport: FeedTransport {
    struct Failure: Error {}
    private let pathFile: URL

    /// `pathFile` holds the socket path on its first line; nil when it does not exist.
    public init?(pathFile: URL) {
        guard FileManager.default.fileExists(atPath: pathFile.path) else { return nil }
        self.pathFile = pathFile
    }

    private func socketPath() -> String? {
        guard let text = try? String(contentsOf: pathFile, encoding: .utf8) else { return nil }
        let line = text.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: true).first.map(String.init)?
            .trimmingCharacters(in: .whitespaces)
        return line?.isEmpty == false ? line : nil
    }

    public func request(_ method: String, _ params: [String: Any], timeout: TimeInterval) throws -> Data {
        guard let path = socketPath(), path.utf8.count < MemoryLayout.size(ofValue: sockaddr_un().sun_path) else { throw Failure() }
        let payload = try JSONSerialization.data(withJSONObject: ["id": UUID().uuidString, "method": method, "params": params])
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw Failure() }
        defer { close(fd) }
        var tv = timeval(tv_sec: Int(timeout), tv_usec: Int32((timeout - Double(Int(timeout))) * 1_000_000))
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        var one: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &one, socklen_t(MemoryLayout<Int32>.size))
        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &addr.sun_path) { buf in
            for (i, b) in path.utf8.enumerated() { buf[i] = b }
        }
        let connected = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
        }
        guard connected == 0 else { throw Failure() }
        var out = payload
        out.append(10)
        let sent = out.withUnsafeBytes { write(fd, $0.baseAddress, out.count) }
        guard sent == out.count else { throw Failure() }
        var line = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while !line.contains(10) {
            let n = read(fd, &chunk, chunk.count)
            guard n > 0 else { throw Failure() }
            line.append(contentsOf: chunk[0..<n])
            guard line.count < 4_000_000 else { throw Failure() }
        }
        return line.prefix { $0 != 10 }
    }
}

/// Polls `feed.list` off the main thread while a session may be asking, and sends answers. Any transport or parse
/// error empties the snapshot and pauses polling for `Feed.retryAfter`, so a cmux protocol change only turns the feature off.
public final class FeedMonitor {
    private let transport: FeedTransport
    private let queue: DispatchQueue?
    private let lock = NSLock()
    private var snapshot: [FeedQuestion] = []
    private var pending: (active: Bool, now: Date)?
    private var pausedUntil: Date?

    public init(transport: FeedTransport, queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.feed-monitor")) {
        self.transport = transport
        self.queue = queue
    }

    /// Main thread, cheap: queues one poll when `active`; otherwise forgets the questions.
    public func update(active: Bool, at now: Date = Date()) {
        guard let queue else { return tick(active: active, at: now) }
        lock.lock()
        let queued = pending != nil
        pending = (active, now)
        lock.unlock()
        if queued { return }
        queue.async { [self] in
            lock.lock()
            let p = pending
            pending = nil
            lock.unlock()
            if let p { tick(active: p.active, at: p.now) }
        }
    }

    public func takeSnapshot() -> [FeedQuestion] {
        lock.lock(); defer { lock.unlock() }
        return snapshot
    }

    private func set(_ items: [FeedQuestion]) {
        lock.lock(); snapshot = items; lock.unlock()
    }

    private func tick(active: Bool, at now: Date) {
        guard active else { return set([]) }
        lock.lock()
        let paused = pausedUntil.map { now < $0 } ?? false
        lock.unlock()
        if paused { return }
        do {
            let data = try transport.request("feed.list", ["pending_only": true], timeout: 2)
            set(Feed.parseList(data))
        } catch {
            set([])
            lock.lock(); pausedUntil = now.addingTimeInterval(Feed.retryAfter); lock.unlock()
        }
    }

    /// Answers `q` if it is still in the last list; `completion` runs on the monitor's queue (the calling thread without one).
    public func reply(_ q: FeedQuestion, answers: [[String]], other: [Int: String], completion: @escaping (Bool) -> Void) {
        let run = { [self] in
            guard takeSnapshot().contains(where: { $0.requestId == q.requestId }) else { return completion(false) }
            let params = Feed.replyParams(requestId: q.requestId, answers: answers, other: other)
            guard let data = try? transport.request("feed.question.reply", params, timeout: 5),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any], o["ok"] as? Bool == true else { return completion(false) }
            completion(true)
        }
        if let queue { queue.async(execute: run) } else { run() }
    }
}
