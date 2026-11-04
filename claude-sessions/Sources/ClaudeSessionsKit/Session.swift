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
    var nameSource: String? = nil
    var aiTitle: String? = nil
    var firstPrompt: String? = nil
    var branch: String? = nil
    var todo: Todo? = nil
}

extension Session {
    var title: String { nameSource == "user" ? name : (aiTitle ?? firstPrompt ?? name) }
    var folder: String { (cwd as NSString).lastPathComponent }
    var subtitle: String { branch.map { "\(folder) · \($0)" } ?? folder }
    var isFinished: Bool { status == "idle" || status == "waiting" }
    var isPermissionPrompt: Bool { status == "waiting" && waitingFor == "permission prompt" }
    var approvable: Pending? {
        guard case .one(let p)? = pending else { return nil }
        return p
    }

    /// Whether an editor of this kind, or an approve/deny of `toolId` (approve and deny are both `.deny`), still applies.
    func allows(_ kind: Store.Editing.Kind, toolId: String?) -> Bool {
        switch kind {
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
        public enum Kind { case deny }
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
    private let tasks: URL
    private var cache: [String: Cached] = [:]
    private var urls: [String: URL] = [:]
    private var urlTried: [String: Date?] = [:]
    private var heads: [String: (prompt: String?, branch: String?, size: Int)] = [:]
    private var aiTitles: [String: String] = [:]
    private var todos: [String: (key: TaskKey, todo: Todo?)] = [:]

    /// Changes whenever a task file is added, removed or rewritten; sizes cover writes within one timestamp tick.
    private struct TaskKey: Equatable {
        var dirDate: Date?
        var files: [String]
        var dates: [Date?]
        var sizes: [Int]
    }

    private struct Cached {
        var since: Date?
        var size: Int
        var reply: String?
        var branch: String?
        var pending: PendingState?
        var prompt = false
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects"),
                tasks: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/tasks"),
                startHidden: Bool = false) {
        self.dir = dir
        self.projects = projects
        self.tasks = tasks
        emptySince = startHidden ? .distantPast : nil
    }

    public private(set) var emptySince: Date?
    public var hidden: Bool { emptySince.map { now.timeIntervalSince($0) >= 30 } ?? false }

    private func transcriptURL(for s: Session) -> URL? {
        if let u = urls[s.id] { return u }
        if let tried = urlTried[s.id], tried == s.since { return nil }
        urlTried[s.id] = s.since
        let u = Transcript.find(sessionId: s.id, cwd: s.cwd, projects: projects)
        urls[s.id] = u
        return u
    }

    private func fileSize(_ url: URL) -> Int {
        (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
    }

    private func readTodo(for s: Session) -> Todo? {
        let d = tasks.appendingPathComponent(s.id, isDirectory: true)
        let fm = FileManager.default
        let keys: [URLResourceKey] = [.contentModificationDateKey, .fileSizeKey]
        let files = ((try? fm.contentsOfDirectory(at: d, includingPropertiesForKeys: keys)) ?? [])
            .filter { $0.pathExtension == "json" && !$0.lastPathComponent.hasPrefix(".") }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
        let values = files.map { try? $0.resourceValues(forKeys: Set(keys)) }
        let key = TaskKey(dirDate: (try? d.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate,
                          files: files.map(\.lastPathComponent),
                          dates: values.map { $0?.contentModificationDate },
                          sizes: values.map { $0?.fileSize ?? -1 })
        if let c = todos[s.id], c.key == key { return c.todo }
        let todo = Tasks.read(d)
        todos[s.id] = (key, todo)
        return todo
    }

    private func readHead(for s: Session, url: URL?) -> (prompt: String?, branch: String?) {
        guard let url else { return (nil, nil) }
        let size = fileSize(url)
        if let h = heads[s.id], h.prompt != nil || h.size == size { return (h.prompt, h.branch) }
        let d = Transcript.head(url)
        let h = (prompt: Transcript.firstPrompt(in: d), branch: Transcript.gitBranch(in: d, last: false), size: size)
        heads[s.id] = h
        return (h.prompt, h.branch)
    }

    private func readTail(for s: Session, url: URL?) -> (reply: String?, pending: PendingState?, branch: String?) {
        guard s.isFinished, let url else { return (nil, nil, nil) }
        var c = cache[s.id] ?? Cached(since: s.since, size: -1, reply: nil, branch: nil, pending: nil)
        let size = fileSize(url)
        let prompt = s.isPermissionPrompt
        if c.size == size && c.since == s.since && c.prompt == prompt { return (c.reply, c.pending, c.branch) }
        c.since = s.since
        c.size = size
        c.prompt = prompt
        let tail = Transcript.tail(url)
        c.reply = Transcript.lastReply(in: tail)
        c.branch = Transcript.gitBranch(in: tail, last: true)
        c.pending = prompt ? Transcript.pending(in: tail) : nil
        if let t = Transcript.lastAITitle(in: tail) { aiTitles[s.id] = t }
        cache[s.id] = c
        return (c.reply, c.pending, c.branch)
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
            s.nameSource = o["nameSource"] as? String
            let url = transcriptURL(for: s)
            let t = readTail(for: s, url: url)
            let head = readHead(for: s, url: url)
            s.firstPrompt = head.prompt
            s.aiTitle = aiTitles[s.id]
            s.branch = t.branch ?? head.branch
            s.lastReply = t.reply
            s.todo = readTodo(for: s)
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
        urls = urls.filter { live.contains($0.key) }
        urlTried = urlTried.filter { live.contains($0.key) }
        heads = heads.filter { live.contains($0.key) }
        aiTitles = aiTitles.filter { live.contains($0.key) }
        todos = todos.filter { live.contains($0.key) }
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
        list.sort { (rank($0), $0.title) < (rank($1), $1.title) }
        if list != sessions { sessions = list }
    }

    public func stillValid(_ a: WidgetAction) -> Bool {
        let live = sessions.first { $0.id == a.sessionId }
        switch a {
        case .jump: return true
        case .approve(_, let toolId), .deny(_, let toolId, _): return live?.allows(.deny, toolId: toolId) == true
        }
    }

    public func markSeen(_ id: String) {
        if unseen.contains(id) { unseen.remove(id) }
    }
}
