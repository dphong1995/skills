import Foundation

struct TicketMention: Equatable {
    let key: String
    var byUser: Bool
    let first: Int
    var last: Int
}

struct ScanResult: Equatable {
    /// Every KEY-123 seen (URL or bare), unfiltered.
    var tickets: [String: TicketMention] = [:]
    /// Jira project prefix -> host.
    var jiraHosts: [String: String] = [:]
    /// PR-shaped URLs, first appearance order, one per PR.
    var prLinks: [URL] = []
    /// Bitbucket refs, first appearance order, deduped.
    var prRefs: [PRRef] = []
    /// Bumped whenever `absorb` adds or changes anything, so callers can cache what they derive from it.
    var version = 0
}

struct ScanTarget: Equatable {
    let sessionId: String
    let transcript: URL?
    let size: UInt64?
}

/// Reads each session's transcript incrementally on a background queue and keeps the tickets and PR links it mentions.
public final class TranscriptScanner {
    static let chunkBytes: UInt64 = 1 << 20
    static let maxLineBytes = 4 << 20
    static let forgetAfter: TimeInterval = 30

    private struct State {
        var url: URL
        var offset: UInt64 = 0
        var carry = Data()
        var skipping = false
        var counter = 0
        var result = ScanResult()
        var lastSeen: Date
    }

    private let queue: DispatchQueue?
    private let lock = NSLock()
    private var pending: (targets: [ScanTarget], now: Date)?
    private var snapshot: [String: ScanResult] = [:]
    private var parsed = 0
    /// Only touched inside `tick`, which runs one at a time.
    private var states: [String: State] = [:]

    /// JSON parses done, for tests.
    var parsedLines: Int {
        lock.lock(); defer { lock.unlock() }
        return parsed
    }

    public init(queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.transcript-scanner", qos: .utility)) {
        self.queue = queue
    }

    /// Main thread, cheap: queues one `tick` with the latest targets.
    func update(_ targets: [ScanTarget], at now: Date = Date()) {
        guard let queue else { return tick(targets, at: now) }
        lock.lock()
        let queued = pending != nil
        pending = (targets, now)
        lock.unlock()
        if queued { return }
        queue.async { [self] in
            lock.lock()
            let p = pending
            pending = nil
            lock.unlock()
            if let p { tick(p.targets, at: p.now) }
        }
    }

    func takeSnapshot() -> [String: ScanResult] {
        lock.lock(); defer { lock.unlock() }
        return snapshot
    }

    private func tick(_ targets: [ScanTarget], at now: Date) {
        for t in targets {
            guard let url = t.transcript, let size = t.size else { continue }
            var s = states[t.sessionId] ?? State(url: url, lastSeen: now)
            if s.url != url || size < s.offset { s = State(url: url, lastSeen: now) }
            s.lastSeen = now
            let n = scan(&s, size: size)
            states[t.sessionId] = s
            lock.lock()
            snapshot[t.sessionId] = s.result
            parsed += n
            lock.unlock()
        }
        let live = Set(targets.map(\.sessionId))
        states = states.filter { live.contains($0.key) || now.timeIntervalSince($0.value.lastSeen) <= Self.forgetAfter }
        let results = states.mapValues(\.result)
        lock.lock()
        snapshot = results
        lock.unlock()
    }

    /// Reads `[offset, size)` in chunks; returns how many lines were parsed as JSON.
    private func scan(_ s: inout State, size: UInt64) -> Int {
        var parsedCount = 0
        while s.offset < size {
            let chunk = Transcript.read(s.url, from: s.offset, to: min(size, s.offset + Self.chunkBytes))
            if chunk.isEmpty { break }
            s.offset += UInt64(chunk.count)
            let buf: Data
            if s.skipping {
                guard let nl = chunk.firstIndex(of: 0x0A) else { continue }
                s.skipping = false
                buf = chunk[(nl + 1)...]
            } else {
                buf = s.carry.isEmpty ? chunk : s.carry + chunk
            }
            if let end = buf.lastIndex(of: 0x0A) {
                s.carry = Data(buf[(end + 1)...])
                for line in buf[buf.startIndex..<end].split(separator: 0x0A) where line.count <= Self.maxLineBytes {
                    guard Self.keep(line) else { continue }
                    parsedCount += 1
                    Self.absorb(line, into: &s)
                }
            } else {
                s.carry = buf
            }
            if s.carry.count > Self.maxLineBytes { s.carry = Data(); s.skipping = true }
        }
        return parsedCount
    }

    private static let want = ["\"type\":\"user\"", "\"type\":\"assistant\""].map { Data($0.utf8) }
    private static let reject = ["\"type\":\"tool_use\"", "\"type\":\"tool_result\"", "\"type\":\"thinking\"",
                                 "\"isSidechain\":true", "\"isMeta\":true"].map { Data($0.utf8) }

    private static func keep(_ line: Data.SubSequence) -> Bool {
        want.contains { line.range(of: $0) != nil } && !reject.contains { line.range(of: $0) != nil }
    }

    /// Parses one kept line and folds its text into `s.result`.
    private static func absorb(_ line: Data.SubSequence, into s: inout State) {
        guard let o = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
        guard (o["isSidechain"] as? Bool) != true, (o["isMeta"] as? Bool) != true,
              let type = o["type"] as? String, let content = (o["message"] as? [String: Any])?["content"]
        else { return }
        let isUser = type == "user"
        let text: String
        if let str = content as? String {
            if isUser && PullRequests.isShellOutput(str) { return }
            text = str
        } else if let blocks = content as? [[String: Any]] {
            let kept = isUser ? blocks.filter { !PullRequests.isShellOutput($0["text"] as? String ?? "") } : blocks
            text = Transcript.text(of: kept)
        } else { return }
        let clean = Transcript.stripReminders(text)
        let byUser = isUser && (o["isCompactSummary"] as? Bool) != true
        for key in Transcript.ticketKeys(in: clean) {
            s.counter += 1
            s.result.version += 1
            if var m = s.result.tickets[key] {
                m.byUser = m.byUser || byUser
                m.last = s.counter
                s.result.tickets[key] = m
            } else {
                s.result.tickets[key] = TicketMention(key: key, byUser: byUser, first: s.counter, last: s.counter)
            }
        }
        for (_, url) in Transcript.urlRanges(in: clean) {
            if let j = Transcript.jiraTicket(in: url), let prefix = Transcript.projectPrefix(of: j.key),
               s.result.jiraHosts[prefix] == nil {
                s.result.jiraHosts[prefix] = j.host
                s.result.version += 1
            }
            guard Transcript.isPullRequest(url) else { continue }
            for r in PullRequests.refs(in: url.absoluteString) where !s.result.prRefs.contains(r) {
                s.result.prRefs.append(r)
                s.result.version += 1
            }
            if let key = Transcript.pullRequestKey(url), !s.result.prLinks.contains(where: { Transcript.pullRequestKey($0) == key }) {
                s.result.prLinks.append(url)
                s.result.version += 1
            }
        }
    }
}
