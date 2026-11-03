import Foundation
import Combine

struct Session: Identifiable, Equatable {
    let id: String
    let name: String
    let cwd: String
    let status: String
    let waitingFor: String?
    let since: Date?
    var lastReply: String? = nil
    var pending: PendingState? = nil
    var pid: Int32 = 0
}

extension Session {
    var isFinished: Bool { status == "idle" || status == "waiting" }
    var isPermissionPrompt: Bool { status == "waiting" && waitingFor == "permission prompt" }
    var approvable: Pending? {
        guard case .one(let p)? = pending else { return nil }
        return p
    }
    var canReply: Bool { status == "idle" }

    /// Whether an editor of this kind, or an approve/deny of `toolId` (approve and deny are both `.deny`), still applies.
    func allows(_ kind: Store.Editing.Kind, toolId: String?) -> Bool {
        switch kind {
        case .reply: canReply
        case .deny: toolId != nil && approvable?.toolId == toolId
        }
    }
}

public final class Store: ObservableObject {
    @Published var sessions: [Session] = []
    @Published public var unseen: Set<String> = []
    @Published public var measuredHeight: CGFloat?
    /// Session id -> cmux surface UUID, filled by the app. Pruned to live ids and dropped when the session's pid changes.
    public var surfaces: [String: String] = [:]
    /// Session ids whose cmux lookup found nothing. Same pruning as `surfaces`.
    public var surfaceMisses: Set<String> = []
    private var pids: [String: Int32] = [:]
    /// The pid of the live session with this id as of the last refresh.
    public func pid(of id: String) -> Int32? { pids[id] }
    public struct Editing: Equatable {
        public enum Kind { case reply, deny }
        public let id: String
        public let kind: Kind
        /// The tool a `.deny` editor is for.
        public let toolId: String?
        public init(id: String, kind: Kind, toolId: String? = nil) {
            self.id = id
            self.kind = kind
            self.toolId = toolId
        }
    }
    @Published public var editing: Editing?
    /// Tool ids whose long command the user expanded with Xem đủ.
    @Published var shownCommands: Set<String> = []
    @Published public var drafts: [String: String] = [:]
    @Published public var hovering = false
    @Published var now = Date()
    @Published public var peekUntil: Date?
    @Published public var anchor = Anchor()

    var expanded: Bool { hovering || editing != nil || (peekUntil.map { now < $0 } ?? false) }

    private var lastStatus: [String: String] = [:]
    private let dir: URL
    private let projects: URL
    private var cache: [String: Cached] = [:]

    private struct Cached {
        var since: Date?
        var url: URL?
        var size: Int
        var reply: String?
        var pending: PendingState?
        var prompt = false
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects"),
                startHidden: Bool = false) {
        self.dir = dir
        self.projects = projects
        emptySince = startHidden ? .distantPast : nil
    }

    public private(set) var emptySince: Date?
    public var hidden: Bool { emptySince.map { now.timeIntervalSince($0) >= 30 } ?? false }

    private func readTail(for s: Session) -> (reply: String?, pending: PendingState?) {
        guard s.isFinished else { return (nil, nil) }
        var c: Cached
        if let cached = cache[s.id], cached.url != nil || cached.since == s.since {
            c = cached
        } else {
            c = Cached(since: s.since, url: Transcript.find(sessionId: s.id, cwd: s.cwd, projects: projects), size: -1, reply: nil, pending: nil)
        }
        guard let url = c.url else {
            cache[s.id] = c
            return (nil, nil)
        }
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        let prompt = s.isPermissionPrompt
        if c.size == size && c.since == s.since && c.prompt == prompt { return (c.reply, c.pending) }
        c.since = s.since
        c.size = size
        c.prompt = prompt
        let tail = Transcript.tail(url)
        c.reply = Transcript.lastReply(in: tail)
        c.pending = prompt ? Transcript.pending(in: tail) : nil
        cache[s.id] = c
        return (c.reply, c.pending)
    }

    public func refresh(at date: Date = Date()) {
        now = date
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        var list: [Session] = []
        for file in files where file.pathExtension == "json" {
            guard let data = try? Data(contentsOf: file),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let pid = (o["pid"] as? NSNumber)?.int32Value,
                  kill(pid, 0) == 0 || errno == EPERM,
                  (o["spare"] as? Bool) != true
            else { continue }
            let ms = (o["statusUpdatedAt"] as? NSNumber)?.doubleValue
            var s = Session(
                id: o["sessionId"] as? String ?? String(pid),
                name: o["name"] as? String ?? "pid \(pid)",
                cwd: o["cwd"] as? String ?? "",
                status: o["status"] as? String ?? "",
                waitingFor: o["waitingFor"] as? String,
                since: ms.map { Date(timeIntervalSince1970: $0 / 1000) },
                pid: pid
            )
            let t = readTail(for: s)
            s.lastReply = t.reply
            if s.isPermissionPrompt { s.pending = t.pending }
            list.append(s)
        }
        // One conversation open in two processes (resumed in another pane): keep only the newest, so rows and caches keyed by id stay unique.
        var newest: [String: Session] = [:]
        for s in list where (newest[s.id]?.since ?? .distantPast) <= (s.since ?? .distantPast) { newest[s.id] = s }
        list = list.filter { newest[$0.id]?.pid == $0.pid }

        if list.isEmpty {
            if emptySince == nil { emptySince = now }
        } else {
            emptySince = nil
        }

        var newUnseen = unseen
        for s in list {
            if lastStatus[s.id] == "busy" && s.isFinished {
                newUnseen.insert(s.id)
                peekUntil = now.addingTimeInterval(5)
            }
            if s.status == "busy" { newUnseen.remove(s.id) }
            lastStatus[s.id] = s.status
        }
        let live = Set(list.map(\.id))
        cache = cache.filter { live.contains($0.key) }
        for s in list where pids[s.id] != nil && pids[s.id] != s.pid {
            surfaces[s.id] = nil
            surfaceMisses.remove(s.id)
        }
        pids = Dictionary(list.map { ($0.id, $0.pid) }, uniquingKeysWith: { $1 })
        surfaces = surfaces.filter { live.contains($0.key) }
        surfaceMisses.formIntersection(live)
        let liveDrafts = drafts.filter { live.contains($0.key) }
        if liveDrafts.count != drafts.count { drafts = liveDrafts }
        let openTools = Set(list.compactMap { $0.approvable?.toolId })
        if !shownCommands.isSubset(of: openTools) { shownCommands.formIntersection(openTools) }
        newUnseen.formIntersection(live)
        if newUnseen != unseen { unseen = newUnseen }

        func rank(_ s: Session) -> Int {
            switch s.status { case "waiting": 0; case "idle": 1; case "busy": 2; default: 3 }
        }
        if let e = editing {
            if list.first(where: { $0.id == e.id })?.allows(e.kind, toolId: e.toolId) != true { editing = nil }
        }
        list.sort { (rank($0), $0.name) < (rank($1), $1.name) }
        if list != sessions { sessions = list }
    }

    public func stillValid(_ a: WidgetAction) -> Bool {
        let live = sessions.first { $0.id == a.sessionId }
        switch a {
        case .jump: return true
        case .approve(_, let toolId), .deny(_, let toolId, _): return live?.allows(.deny, toolId: toolId) == true
        case .reply: return live?.allows(.reply, toolId: nil) == true
        }
    }

    public func markSeen(_ id: String) {
        if unseen.contains(id) { unseen.remove(id) }
    }
}
