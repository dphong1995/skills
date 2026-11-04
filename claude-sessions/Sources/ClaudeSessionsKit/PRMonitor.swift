import Foundation

struct PRTarget: Equatable {
    let sessionId: String
    /// PR refs found in the transcript by `TranscriptScanner`, first appearance order.
    let linkRefs: [PRRef]
    let cwd: String
    let branch: String?
}

struct PRSnapshot: Equatable {
    /// Shown PRs per session in display order: link refs by first appearance, then branch refs. Not capped.
    var prs: [String: [PRStatus]]
    var events: [PREvent]
    /// The PRs that produced `events` since the last `takeSnapshot`.
    var eventRefs: Set<PRRef>
    var lastSuccess: Date?
    init(prs: [String: [PRStatus]] = [:], events: [PREvent] = [], eventRefs: Set<PRRef> = [], lastSuccess: Date? = nil) {
        self.prs = prs; self.events = events; self.eventRefs = eventRefs; self.lastSuccess = lastSuccess
    }
}

public final class PRMonitor {
    private struct Repo: Equatable { let workspace: String; let name: String }

    private struct SessionState {
        var linkRefs: [PRRef] = []
        var branchRefs: [PRRef] = []
        var branch: String?

        /// Link refs first, then branch refs, without duplicates.
        var refs: [PRRef] {
            var seen = Set<PRRef>()
            return (linkRefs + branchRefs).filter { seen.insert($0).inserted }
        }
    }

    private static let maxPages = 20
    private static let forgetAfter: TimeInterval = 30
    private static let remoteRetry: TimeInterval = 300
    /// How long a PR head without a CodeRabbit status keeps asking for one.
    private static let rabbitWait: TimeInterval = 600
    private static let singleFields = "id,title,state,comment_count,updated_on,source.branch.name,source.repository.full_name,source.commit.hash,participants.approved,participants.state,participants.role,participants.user.uuid,links.html.href"
    private static let openFields = singleFields.split(separator: ",").map { "values.\($0)" }.joined(separator: ",") + ",next"
    private static let statusFields = "values.state,values.commit.hash,values.key,values.name,values.description,values.created_on,values.updated_on"
    private static let commentFields = "values.user.uuid,values.user.display_name,values.created_on,values.updated_on,values.deleted,values.content.raw"

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
    private var rabbitSeen: [PRRef: (commit: String?, label: String)] = [:]
    /// PRs whose `/statuses` was fetched successfully at least once.
    private var statusesFetched = Set<PRRef>()
    /// PRs whose first `/comments` fetch failed and is still owed.
    private var commentsOwed = Set<PRRef>()
    private var headSeen: [PRRef: (commit: String?, at: Date)] = [:]
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
        snapshot.eventRefs = []
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
            for r in t.linkRefs where !s.linkRefs.contains(r) {
                s.linkRefs.append(r)
                needCycle = true
            }
            sessions[t.sessionId] = s
            if let known = remotes[t.cwd], known.repo != nil || now.timeIntervalSince(known.at) < Self.remoteRetry { continue }
            let repo = remote(t.cwd).flatMap(PullRequests.repo(fromRemote:)).map { Repo(workspace: $0.workspace, name: $0.repo) }
            remotes[t.cwd] = (repo, now)
        }
        let used = Set(sessions.values.flatMap(\.refs))
        statuses = statuses.filter { used.contains($0.key) }
        rabbitSeen = rabbitSeen.filter { used.contains($0.key) }
        headSeen = headSeen.filter { used.contains($0.key) }
        statusesFetched.formIntersection(used)
        commentsOwed.formIntersection(used)
        hidden.formIntersection(used)
        closed.formIntersection(used)

        var events: [PREvent] = []
        var eventRefs = Set<PRRef>()
        var succeeded = false
        if needCycle {
            lastCycle = now
            (events, eventRefs, succeeded) = cycle(targets, at: now)
        }
        var prs: [String: [PRStatus]] = [:]
        for (id, s) in sessions {
            let list = s.refs.compactMap { statuses[$0] }
            if !list.isEmpty { prs[id] = list }
        }
        lock.lock()
        snapshot.prs = prs
        snapshot.events += events
        snapshot.eventRefs.formUnion(eventRefs)
        if succeeded { snapshot.lastSuccess = now }
        lock.unlock()
    }

    /// Remembers the `(commit, label)` of a finished review so only a new one chimes.
    private func recordRabbit(_ ref: PRRef, _ pr: PRStatus) {
        if let r = pr.rabbit, CodeRabbit.isFinal(r) { rabbitSeen[ref] = (pr.commit, CodeRabbit.label(r)) }
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

    private func cycle(_ targets: [PRTarget], at now: Date) -> (events: [PREvent], refs: Set<PRRef>, succeeded: Bool) {
        var workspaces = Set(sessions.values.flatMap(\.refs).map(\.workspace))
        for t in targets { if let repo = remotes[t.cwd]?.repo { workspaces.insert(repo.workspace) } }
        guard !workspaces.isEmpty else { return ([], [], false) }

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
            return ([], [], false)
        }
        guard listed > 0 else { return ([], [], false) }
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
        var eventRefs = Set<PRRef>()
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
            pr.rabbit = prev?.rabbit
            pr.rabbitSince = prev?.rabbitSince
            var rabbitFinished = false
            var firstStatuses = false
            if pr.isOpen {
                if headSeen[ref].map({ $0.commit != pr.commit }) ?? true { headSeen[ref] = (pr.commit, now) }
                let inWindow = now.timeIntervalSince(headSeen[ref]!.at) < Self.rabbitWait
                let rabbitPending = prev?.rabbit == .reviewing || (prev?.rabbit == nil && inWindow)
                if prev == nil || prev?.commit != pr.commit || (pr.pipeline == nil && inWindow) || pr.pipeline == .running
                    || rabbitPending || prev?.comments != pr.comments {
                    if let d = try? api.get("\(path)/statuses?fields=\(Self.statusFields)&pagelen=50&sort=-created_on") {
                        firstStatuses = !statusesFetched.contains(ref)
                        pr.pipeline = PullRequests.pipeline(fromStatuses: d, commit: pr.commit)
                        let fresh = CodeRabbit.state(fromStatuses: d, commit: pr.commit)
                        let sameCommit = prev?.commit == pr.commit
                        if var r = fresh?.rabbit {
                            if case .reviewed = r, sameCommit, case .reviewed(let f)? = prev?.rabbit { r = .reviewed(findings: f) }
                            rabbitFinished = CodeRabbit.isFinal(r) && !(sameCommit && CodeRabbit.kind(prev?.rabbit) == CodeRabbit.kind(r))
                            pr.rabbit = r
                        } else {
                            pr.rabbit = nil
                        }
                        pr.rabbitSince = fresh?.since
                    } else if prev != nil {
                        continue
                    } else {
                        pr.pipeline = nil
                    }
                }
                let owed = commentsOwed.contains(ref)
                if prev == nil || prev?.comments != pr.comments || rabbitFinished || owed {
                    guard let d = try? api.get("\(path)/comments?sort=-created_on&pagelen=20&fields=\(Self.commentFields)") else {
                        if prev != nil { continue }
                        commentsOwed.insert(ref)
                        recordRabbit(ref, pr)
                        if firstStatuses { statusesFetched.insert(ref) }
                        statuses[ref] = pr
                        continue
                    }
                    commentsOwed.remove(ref)
                    if let c = PullRequests.lastOther(fromComments: d, me: me) { pr.lastOther = c }
                    if case .reviewed? = pr.rabbit, let since = pr.rabbitSince {
                        switch CodeRabbit.findings(fromComments: d, since: since) {
                        case .count(let n): pr.rabbit = .reviewed(findings: n)
                        case .stale: pr.rabbit = .reviewed(findings: nil)
                        case .notFound: break // the summary is beyond this page: keep the known count
                        }
                    }
                    // Re-record only a count filled in for the review already recorded; a new kind must still chime.
                    if owed, rabbitSeen[ref]?.commit == pr.commit, CodeRabbit.kind(prev?.rabbit) == CodeRabbit.kind(pr.rabbit) { recordRabbit(ref, pr) }
                }
            }
            if let prev {
                var found = PullRequests.events(old: prev, new: pr)
                if firstStatuses {
                    recordRabbit(ref, pr)
                } else if pr.isOpen, let r = pr.rabbit, CodeRabbit.isFinal(r), rabbitSeen[ref].map({ $0 != (pr.commit, CodeRabbit.label(r)) }) ?? true {
                    switch r {
                    case .reviewed(let n?) where n > 0: found.append(.rabbitNeedsYou)
                    case .failed: found.append(.rabbitNeedsYou)
                    default: found.append(.rabbitDone)
                    }
                    recordRabbit(ref, pr)
                }
                if !found.isEmpty { eventRefs.insert(ref) }
                events += found
            } else {
                recordRabbit(ref, pr)
            }
            if firstStatuses { statusesFetched.insert(ref) }
            statuses[ref] = pr
        }
        return (events, eventRefs, true)
    }
}
