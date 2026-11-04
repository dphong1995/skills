import Foundation

public struct PRTarget: Equatable {
    public let sessionId: String
    public let transcript: URL?
    public let cwd: String
    public let branch: String?
    public init(sessionId: String, transcript: URL?, cwd: String, branch: String?) {
        self.sessionId = sessionId; self.transcript = transcript; self.cwd = cwd; self.branch = branch
    }
}

public struct PRSnapshot: Equatable {
    /// Shown PRs per session in display order: link refs by first appearance, then branch refs. Not capped.
    public var prs: [String: [PRStatus]]
    public var events: [PREvent]
    public var lastSuccess: Date?
    public init(prs: [String: [PRStatus]] = [:], events: [PREvent] = [], lastSuccess: Date? = nil) {
        self.prs = prs; self.events = events; self.lastSuccess = lastSuccess
    }
}

public final class PRMonitor {
    private struct Repo: Equatable { let workspace: String; let name: String }

    private struct SessionState {
        var url: URL?
        var offset: UInt64 = 0
        var carry = Data()
        var linkRefs: [PRRef] = []
        var branchRefs: [PRRef] = []
        var branch: String?
    }

    private static let chunkBytes: UInt64 = 1 << 20
    private static let maxPages = 20
    private static let openFields = "values.id,values.title,values.state,values.comment_count,values.updated_on,values.source.branch.name,values.source.repository.full_name,values.source.commit.hash,values.participants.approved,values.participants.state,values.participants.role,values.participants.user.uuid,values.links.html.href,next"
    private static let singleFields = "id,title,state,comment_count,updated_on,source.branch.name,source.repository.full_name,source.commit.hash,participants.approved,participants.state,participants.role,participants.user.uuid,links.html.href"
    private static let commentFields = "values.user.uuid,values.user.display_name,values.created_on,values.deleted"

    private let api: BitbucketAPI
    private let queue: DispatchQueue?
    private let remote: (String) -> String?
    private let interval: TimeInterval

    // Shared between the main thread and the monitor queue.
    private let lock = NSLock()
    private var snapshot = PRSnapshot()
    private var pending: (targets: [PRTarget], now: Date)?

    // Touched only by `tick` (the monitor queue, or inline when `queue` is nil).
    private var sessions: [String: SessionState] = [:]
    private var remotes: [String: Repo?] = [:]
    private var statuses: [PRRef: PRStatus] = [:]
    private var hidden = Set<PRRef>()
    private var closed = Set<PRRef>()
    private var me: String?
    private var lastCycle: Date?

    public init(api: BitbucketAPI, queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.pr-monitor"),
                remote: @escaping (String) -> String? = PRMonitor.gitRemote, interval: TimeInterval = 60) {
        self.api = api; self.queue = queue; self.remote = remote; self.interval = interval
    }

    public static func gitRemote(_ cwd: String) -> String? {
        runProcess("/usr/bin/git", ["-C", cwd, "remote", "get-url", "origin"], timeout: 2)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Main thread, cheap: queues one `tick` with the latest targets.
    public func update(_ targets: [PRTarget], at now: Date = Date()) {
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

    public func takeSnapshot() -> PRSnapshot {
        lock.lock(); defer { lock.unlock() }
        let s = snapshot
        snapshot.events = []
        return s
    }

    func tick(_ targets: [PRTarget], at now: Date) {
        let live = Set(targets.map(\.sessionId))
        sessions = sessions.filter { live.contains($0.key) }
        var needCycle = lastCycle.map { now.timeIntervalSince($0) >= interval } ?? true
        for t in targets {
            let existing = sessions[t.sessionId]
            var s = existing ?? SessionState()
            if existing == nil ? t.branch != nil : s.branch != t.branch { needCycle = true }
            s.branch = t.branch
            if scan(&s, t.transcript) { needCycle = true }
            sessions[t.sessionId] = s
            if !remotes.keys.contains(t.cwd) {
                remotes[t.cwd] = remote(t.cwd).flatMap(PullRequests.repo(fromRemote:)).map { Repo(workspace: $0.workspace, name: $0.repo) }
            }
        }
        let used = Set(sessions.values.flatMap { $0.linkRefs + $0.branchRefs })
        statuses = statuses.filter { used.contains($0.key) }
        hidden.formIntersection(used)
        closed.formIntersection(used)

        var events: [PREvent] = []
        var succeeded = false
        if needCycle {
            lastCycle = now
            (events, succeeded) = cycle(targets)
        }
        var prs: [String: [PRStatus]] = [:]
        for (id, s) in sessions {
            var seen = Set<PRRef>()
            let list = (s.linkRefs + s.branchRefs).filter { seen.insert($0).inserted }.compactMap { statuses[$0] }
            if !list.isEmpty { prs[id] = list }
        }
        lock.lock()
        snapshot.prs = prs
        snapshot.events += events
        if succeeded { snapshot.lastSuccess = now }
        lock.unlock()
    }

    /// Reads the transcript bytes appended since the last scan, whole lines only. True when a new ref was found.
    private func scan(_ s: inout SessionState, _ url: URL?) -> Bool {
        guard let url else { return false }
        if s.url != url { s.url = url; s.offset = 0; s.carry = Data() }
        guard let size = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.size] as? UInt64 else { return false }
        if size < s.offset { s.offset = 0; s.carry = Data() }
        var gained = false
        while s.offset < size {
            let to = min(size, s.offset + Self.chunkBytes)
            var buf = s.carry
            buf.append(Transcript.read(url, from: s.offset, to: to))
            s.offset = to
            if let nl = buf.lastIndex(of: 0x0A) {
                s.carry = Data(buf[(nl + 1)...])
                for r in PullRequests.refs(inTranscript: Data(buf[...nl])) where !s.linkRefs.contains(r) {
                    s.linkRefs.append(r)
                    gained = true
                }
            } else {
                s.carry = buf
            }
        }
        return gained
    }

    private func decode(_ d: Data) -> [String: Any]? {
        try? JSONSerialization.jsonObject(with: d) as? [String: Any]
    }

    private func openList(_ workspace: String, me: String) throws -> [PRStatus] {
        let uuid = me.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? me
        var path: String? = "/2.0/workspaces/\(workspace)/pullrequests/\(uuid)?state=OPEN&pagelen=50&fields=\(Self.openFields)"
        var out: [PRStatus] = []
        var pages = 0
        while let p = path, pages < Self.maxPages {
            guard let page = PullRequests.page(try api.get(p)) else { throw BitbucketError.transport }
            out += page.values
            path = page.next.map { $0.hasPrefix(BitbucketClient.base) ? String($0.dropFirst(BitbucketClient.base.count)) : $0 }
            pages += 1
        }
        return out
    }

    private func cycle(_ targets: [PRTarget]) -> (events: [PREvent], succeeded: Bool) {
        var workspaces = Set(sessions.values.flatMap { $0.linkRefs + $0.branchRefs }.map(\.workspace))
        for t in targets { if let repo = remotes[t.cwd] ?? nil { workspaces.insert(repo.workspace) } }
        guard !workspaces.isEmpty else { return ([], false) }

        var open: [PRRef: PRStatus] = [:]
        do {
            if me == nil {
                guard let uuid = decode(try api.get("/2.0/user?fields=uuid"))?["uuid"] as? String else { throw BitbucketError.transport }
                me = uuid
            }
            for ws in workspaces.sorted() {
                for pr in try openList(ws, me: me!) { open[pr.ref] = pr }
            }
        } catch {
            return ([], false)
        }
        let me = self.me!

        for t in targets {
            guard let branch = t.branch, let repo = remotes[t.cwd] ?? nil else { continue }
            let full = "\(repo.workspace)/\(repo.name)"
            for pr in open.values.sorted(by: { $0.ref.id < $1.ref.id }) where pr.sourceRepo == full && pr.branch == branch {
                var s = sessions[t.sessionId]!
                if !s.linkRefs.contains(pr.ref) && !s.branchRefs.contains(pr.ref) {
                    s.branchRefs.append(pr.ref)
                    sessions[t.sessionId] = s
                }
            }
        }

        var refs: [PRRef] = []
        for key in sessions.keys.sorted() {
            for r in sessions[key]!.linkRefs + sessions[key]!.branchRefs where !refs.contains(r) { refs.append(r) }
        }

        var events: [PREvent] = []
        for ref in refs where !hidden.contains(ref) && !closed.contains(ref) {
            let prev = statuses[ref]
            var pr: PRStatus
            if let o = open[ref] {
                pr = o
            } else {
                let path = "/2.0/repositories/\(ref.workspace)/\(ref.repo)/pullrequests/\(ref.id)"
                guard let d = try? api.get("\(path)?fields=\(Self.singleFields)"),
                      let o = decode(d), let fetched = PullRequests.status(from: o) else { continue }
                pr = fetched
                if !pr.isOpen {
                    if prev == nil { hidden.insert(ref); continue }
                    closed.insert(ref)
                }
            }
            pr.pipeline = prev?.pipeline
            pr.lastOther = prev?.lastOther
            if pr.isOpen {
                let path = "/2.0/repositories/\(ref.workspace)/\(ref.repo)/pullrequests/\(ref.id)"
                if prev == nil || prev?.commit != pr.commit || pr.pipeline == nil || pr.pipeline == .running,
                   let d = try? api.get("\(path)/statuses?fields=values.state") {
                    pr.pipeline = PullRequests.pipeline(fromStatuses: d)
                }
                if prev == nil || prev?.comments != pr.comments {
                    guard let d = try? api.get("\(path)/comments?sort=-created_on&pagelen=20&fields=\(Self.commentFields)") else {
                        if prev != nil { continue }
                        statuses[ref] = pr
                        continue
                    }
                    if let c = PullRequests.lastOther(fromComments: d, me: me) { pr.lastOther = c }
                }
            }
            if let prev { events += PullRequests.events(old: prev, new: pr) }
            statuses[ref] = pr
        }
        return (events, true)
    }
}
