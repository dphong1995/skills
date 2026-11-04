import Foundation

struct JiraTarget: Hashable {
    let key: String
    let host: String
}

struct JiraSnapshot: Equatable {
    var issues: [String: JiraIssue] = [:]
    var connection: ConnectionState = .checking
}

public final class JiraMonitor {
    static let newKeyGap: TimeInterval = 5
    static let refreshEvery: TimeInterval = 900
    static let retryAfter: TimeInterval = 60
    static let forgetAfter: TimeInterval = 30
    private static let chunk = 100

    private let api: JiraAPI
    private let queue: DispatchQueue?

    // Shared between the main thread and the monitor queue.
    private let lock = NSLock()
    private var snapshot = JiraSnapshot()
    private var pending: (targets: [JiraTarget], now: Date)?

    // Touched only by `tick` (the monitor queue, or inline when `queue` is nil).
    private var known = Set<JiraTarget>()
    private var cache: [JiraTarget: JiraIssue] = [:]
    private var absentSince: [JiraTarget: Date] = [:]
    /// Keys that failed or were never asked wait here until their retry time.
    private var retryAt: [JiraTarget: Date] = [:]
    private var lastRequest: Date?
    private var lastRefresh: Date?
    private var connection: ConnectionState = .checking

    public init(api: JiraAPI, queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.jira-monitor", qos: .utility)) {
        self.api = api; self.queue = queue
    }

    /// Main thread, cheap: queues one `tick` with the latest targets.
    func update(_ targets: [JiraTarget], at now: Date = Date()) {
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

    func takeSnapshot() -> JiraSnapshot {
        lock.lock(); defer { lock.unlock() }
        return snapshot
    }

    func tick(_ targets: [JiraTarget], at now: Date) {
        let live = Set(targets)
        for t in known.union(cache.keys) where !live.contains(t) {
            if absentSince[t] == nil { absentSince[t] = now }
        }
        for t in live { absentSince[t] = nil }
        for (t, since) in absentSince where now.timeIntervalSince(since) > Self.forgetAfter {
            known.remove(t); cache[t] = nil; retryAt[t] = nil; absentSince[t] = nil
        }
        known.formUnion(live)

        let gapOpen = lastRequest.map { now.timeIntervalSince($0) >= Self.newKeyGap } ?? true
        let refreshDue = lastRefresh.map { now.timeIntervalSince($0) >= Self.refreshEvery } ?? false
        var due: [JiraTarget] = []
        if refreshDue {
            due = Array(live)
        } else if gapOpen {
            due = live.filter { cache[$0] == nil && retryAt[$0].map { now >= $0 } ?? true }
        }
        if !due.isEmpty {
            lastRequest = now
            if refreshDue || lastRefresh == nil { lastRefresh = now }
            fetch(due.sorted { ($0.host, $0.key) < ($1.host, $1.key) }, at: now)
        }

        lock.lock()
        snapshot.issues = Dictionary(cache.map { ($0.key.key, $0.value) }, uniquingKeysWith: { a, _ in a })
        snapshot.connection = connection
        lock.unlock()
    }

    private func fetch(_ due: [JiraTarget], at now: Date) {
        let byHost = Dictionary(grouping: due, by: \.host)
        for host in byHost.keys.sorted() {
            let all = byHost[host]!
            for start in stride(from: 0, to: all.count, by: Self.chunk) {
                let part = Array(all[start..<min(start + Self.chunk, all.count)])
                let keys = part.map(\.key)
                do {
                    let data = try api.bulkFetch(host: host, keys: keys)
                    guard let issues = Jira.issues(from: data, requested: keys) else { throw JiraError.transport }
                    connection = ConnectionState.after(nil, previous: connection)
                    for t in part {
                        cache[t] = issues[t.key]
                        retryAt[t] = nil
                    }
                } catch {
                    connection = ConnectionState.after(error, previous: connection)
                    for t in part { retryAt[t] = now.addingTimeInterval(Self.retryAfter) }
                }
            }
        }
    }
}
