import Foundation

struct PRTarget: Equatable {
    let sessionId: String
    let transcript: URL?
    /// Size of `transcript` in bytes as measured by the caller; nil when there is no readable transcript.
    let size: UInt64?
    let cwd: String
    let branch: String?
    init(sessionId: String, transcript: URL?, size: UInt64?, cwd: String, branch: String?) {
        self.sessionId = sessionId; self.transcript = transcript; self.size = size; self.cwd = cwd; self.branch = branch
    }
}

struct PRSnapshot: Equatable {
    /// Shown PRs per session in display order: link refs by first appearance, then branch refs. Not capped.
    var prs: [String: [PRStatus]]
    var events: [PREvent]
    var lastSuccess: Date?
    init(prs: [String: [PRStatus]] = [:], events: [PREvent] = [], lastSuccess: Date? = nil) {
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

        /// Link refs first, then branch refs, without duplicates.
        var refs: [PRRef] {
            var seen = Set<PRRef>()
            return (linkRefs + branchRefs).filter { seen.insert($0).inserted }
        }
    }

    private static let chunkBytes: UInt64 = 1 << 20
    private static let maxPages = 20
    private static let forgetAfter: TimeInterval = 30
    private static let remoteRetry: TimeInterval = 300
    private static let singleFields = "id,title,state,comment_count,updated_on,source.branch.name,source.repository.full_name,source.commit.hash,participants.approved,participants.state,participants.role,participants.user.uuid,links.html.href"
    private static let openFields = singleFields.split(separator: ",").map { "values.\($0)" }.joined(separator: ",") + ",next"
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
    private var remotes: [String: (repo: Repo?, at: Date)] = [:]
    private var absentSince: [String: Date] = [:]
    private var statuses: [PRRef: PRStatus] = [:]
    private var hidden = Set<PRRef>()
    private var closed = Set<PRRef>()
    private var me: String?
    private var lastCycle: Date?

    public init(api: BitbucketAPI, queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.pr-monitor"),
                remote: ((String) -> String?)? = nil, interval: TimeInterval = 60) {
        self.api = api; self.queue = queue; self.remote = remote ?? Self.gitRemote; self.interval = interval
    }

    static func gitRemote(_ cwd: String) -> String? {
        runProcess("/usr/bin/git", ["-C", cwd, "remote", "get-url", "origin"], timeout: 2)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Main thread, cheap: queues one `tick` with the latest targets.
    func update(_ targets: [PRTarget], at now: Date = Date()) {
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

    func takeSnapshot() -> PRSnapshot {
        lock.lock(); defer { lock.unlock() }
        let s = snapshot
        snapshot.events = []
        return s
    }

    func tick(_ targets: [PRTarget], at now: Date) {
        let live = Set(targets.map(\.sessionId))
        for id in sessions.keys where !live.contains(id) {
            if absentSince[id] == nil { absentSince[id] = now }
        }
        for id in live { absentSince[id] = nil }
        for (id, since) in absentSince where now.timeIntervalSince(since) > Self.forgetAfter {
            sessions[id] = nil
            absentSince[id] = nil
        }
        var needCycle = lastCycle.map { now.timeIntervalSince($0) >= interval } ?? true
        for t in targets {
            let existing = sessions[t.sessionId]
            var s = existing ?? SessionState()
            if existing == nil ? t.branch != nil : s.branch != t.branch { needCycle = true }
            s.branch = t.branch
            if scan(&s, t.transcript, size: t.size) { needCycle = true }
            sessions[t.sessionId] = s
            if let known = remotes[t.cwd], known.repo != nil || now.timeIntervalSince(known.at) < Self.remoteRetry { continue }
            let repo = remote(t.cwd).flatMap(PullRequests.repo(fromRemote:)).map { Repo(workspace: $0.workspace, name: $0.repo) }
            remotes[t.cwd] = (repo, now)
        }
        let used = Set(sessions.values.flatMap(\.refs))
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
            let list = s.refs.compactMap { statuses[$0] }
            if !list.isEmpty { prs[id] = list }
        }
        lock.lock()
        snapshot.prs = prs
        snapshot.events += events
        if succeeded { snapshot.lastSuccess = now }
        lock.unlock()
    }

    /// Reads the transcript bytes appended since the last scan, whole lines only. True when a new ref was found.
    private func scan(_ s: inout SessionState, _ url: URL?, size: UInt64?) -> Bool {
        guard let url, let size else { return false }
        if s.url != url { s.url = url; s.offset = 0; s.carry = Data() }
        if size < s.offset { s.offset = 0; s.carry = Data() }
        var gained = false
        while s.offset < size {
            let to = min(size, s.offset + Self.chunkBytes)
            let chunk = Transcript.read(url, from: s.offset, to: to)
            if chunk.isEmpty { break }
            var buf = s.carry
            buf.append(chunk)
            s.offset += UInt64(chunk.count)
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
        var workspaces = Set(sessions.values.flatMap(\.refs).map(\.workspace))
        for t in targets { if let repo = remotes[t.cwd]?.repo { workspaces.insert(repo.workspace) } }
        guard !workspaces.isEmpty else { return ([], false) }

        var open: [PRRef: PRStatus] = [:]
        var listed = 0
        do {
            if me == nil {
                guard let uuid = decode(try api.get("/2.0/user?fields=uuid"))?["uuid"] as? String else { throw BitbucketError.transport }
                me = uuid
            }
            for ws in workspaces.sorted() {
                do {
                    for pr in try openList(ws, me: me!) { open[pr.ref] = pr }
                    listed += 1
                } catch BitbucketError.http(let code) where code == 403 || code == 404 {
                    continue // foreign or unknown workspace: its refs go through the single-PR path
                }
            }
        } catch {
            return ([], false)
        }
        guard listed > 0 else { return ([], false) }
        let me = self.me!

        for t in targets {
            guard let branch = t.branch, let repo = remotes[t.cwd]?.repo else { continue }
            let full = "\(repo.workspace)/\(repo.name)"
            for pr in open.values.sorted(by: { $0.ref.id < $1.ref.id }) where pr.sourceRepo == full && pr.branch == branch {
                var s = sessions[t.sessionId]!
                if !s.refs.contains(pr.ref) {
                    s.branchRefs.append(pr.ref)
                    sessions[t.sessionId] = s
                }
            }
        }

        var seenRefs = Set<PRRef>()
        let refs = sessions.keys.sorted().flatMap { sessions[$0]!.refs }.filter { seenRefs.insert($0).inserted }

        var events: [PREvent] = []
        for ref in refs where !hidden.contains(ref) && !closed.contains(ref) {
            let prev = statuses[ref]
            let path = "/2.0/repositories/\(ref.workspace)/\(ref.repo)/pullrequests/\(ref.id)"
            var pr: PRStatus
            if let o = open[ref] {
                pr = o
            } else {
                let d: Data
                do {
                    d = try api.get("\(path)?fields=\(Self.singleFields)")
                } catch BitbucketError.http(let code) where prev == nil && (code == 403 || code == 404) {
                    hidden.insert(ref) // foreign or placeholder workspace: never ask again
                    continue
                } catch {
                    continue
                }
                guard let o = decode(d), let fetched = PullRequests.status(from: o) else { continue }
                pr = fetched
                if !pr.isOpen {
                    if prev == nil { hidden.insert(ref); continue }
                    closed.insert(ref)
                }
            }
            pr.pipeline = prev?.pipeline
            pr.lastOther = prev?.lastOther
            if pr.isOpen {
                if prev == nil || prev?.commit != pr.commit || pr.pipeline == nil || pr.pipeline == .running {
                    if let d = try? api.get("\(path)/statuses?fields=values.state,values.commit.hash&pagelen=50&sort=-created_on") {
                        pr.pipeline = PullRequests.pipeline(fromStatuses: d, commit: pr.commit)
                    } else if prev != nil {
                        continue
                    } else {
                        pr.pipeline = nil
                    }
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
