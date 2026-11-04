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
    /// `lastReply` parsed as markdown, once per tail read.
    var replyText: AttributedString? = nil
    var pending: PendingState? = nil
    var pid: Int32 = 0
    var nameSource: String? = nil
    var aiTitle: String? = nil
    var firstPrompt: String? = nil
    var branch: String? = nil
    var todo: Todo? = nil
    var links: [SessionLink] = []
    var activity: String? = nil
    var prs: [PRStatus] = []
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

public enum Chime: Equatable { case done, needsYou }

public final class Store: ObservableObject {
    @Published var sessions: [Session] = []
    /// Session id -> cmux surface UUID, filled by the app. Pruned to live ids and dropped when the session's pid changes.
    public var surfaces: [String: String] = [:]
    /// Session ids with a NEW mark. In memory only.
    @Published public var unseen: Set<String> = []
    /// Session ids whose cmux surface lookup failed; dropped when the pid changes or the session dies.
    public var surfaceMisses: Set<String> = []
    private var lastFocused: String?
    var newCount: Int {
        let live = Set(sessions.map(\.id))
        return unseen.filter(live.contains).count
    }
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
    /// Tool ids whose long command the user expanded with Xem đủ.
    @Published var shownCommands: Set<String> = []
    @Published public var drafts: [String: String] = [:]
    @Published var now = Date()
    @Published public var peekUntil: Date?
    @Published public var muted = false
    @Published public var bgOpacity: Double = 0.25
    @Published public var prPrompt: String = PullRequests.defaultPrompt
    public private(set) var chime: Chime?

    public var peeking: Bool { peekUntil.map { now < $0 } ?? false }

    /// Whether the session `e.id` still exists and still allows what `e` is editing.
    public func allows(_ e: Editing) -> Bool {
        sessions.first(where: { $0.id == e.id })?.allows(e.kind, toolId: e.toolId) == true
    }

    private var lastStatus: [String: String] = [:]
    private let dir: URL
    private let projects: URL
    private let tasks: URL
    private var cache: [String: Cached] = [:]
    private struct ButtonKey: Equatable {
        let version: Int
        let projectsGeneration: Int
        let branch: String?
    }
    /// Buttons per session with the PR refs each link points at, rebuilt only when their `ButtonKey` changes.
    private var buttonCache: [String: (key: ButtonKey, buttons: [(link: SessionLink, refs: [PRRef])])] = [:]
    /// Bumped whenever `jiraProjects` changes, so `ButtonKey` need not compare the dictionary.
    private var projectsGeneration = 0
    private var urls: [String: URL] = [:]
    private var urlTried: [String: Date?] = [:]
    private var heads: [String: (prompt: String?, branch: String?, size: Int)] = [:]
    private var todos: [String: (key: TaskKey, todo: Todo?)] = [:]
    private var activities: [String: (offset: UInt64, text: String?)] = [:]
    /// File reads done for the running-tool line, for tests.
    private(set) var activityReads = 0

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
        var replyText: AttributedString?
        var branch: String?
        var aiTitle: String?
        var pending: PendingState?
        var prompt = false
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects"),
                tasks: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/tasks"),
                startHidden: Bool = false,
                prs: PRMonitor? = nil,
                scanner: TranscriptScanner = TranscriptScanner()) {
        self.prMonitor = prs
        self.scanner = scanner
        self.dir = dir
        self.projects = projects
        self.tasks = tasks
        emptySince = startHidden ? .distantPast : nil
    }

    public var prMonitor: PRMonitor?
    private let scanner: TranscriptScanner
    /// Jira project prefix -> host, learned from `/browse/` and `selectedIssue=` links; persisted by the app.
    @Published public var jiraProjects: [String: String] = [:]
    @Published public private(set) var prLastSuccess: Date?
    /// No successful open-list call for 300 s while the feature is live.
    var prStale: Bool { prLastSuccess.map { now.timeIntervalSince($0) > 300 } ?? false }

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
        let keys: [URLResourceKey] = [.contentModificationDateKey, .fileSizeKey]
        let files = Tasks.files(in: d)
        let values = files.map { try? $0.resourceValues(forKeys: Set(keys)) }
        let key = TaskKey(dirDate: (try? d.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate,
                          files: files.map(\.lastPathComponent),
                          dates: values.map { $0?.contentModificationDate },
                          sizes: values.map { $0?.fileSize ?? -1 })
        if let c = todos[s.id], c.key == key { return c.todo }
        let todo = Tasks.read(files)
        todos[s.id] = (key, todo)
        return todo
    }

    /// The head of an append-only transcript cannot change once it holds a prompt or the file outgrew the head window.
    private func headIsFinal(_ id: String) -> Bool {
        heads[id].map { $0.prompt != nil || $0.size > Transcript.headBytes } ?? false
    }

    private func readHead(for s: Session, url: URL?, size: Int) -> (prompt: String?, branch: String?) {
        guard let url else { return (nil, nil) }
        if let h = heads[s.id], headIsFinal(s.id) || h.size == size { return (h.prompt, h.branch) }
        let d = Transcript.head(url)
        let full = Transcript.firstPrompt(in: d)
        let h = (prompt: full.map { Transcript.displayText($0, limit: 140) }, branch: Transcript.gitBranch(in: d, last: false), size: size)
        heads[s.id] = h
        return (h.prompt, h.branch)
    }

    private func readTail(for s: Session, url: URL?, size: Int) -> (reply: String?, replyText: AttributedString?, pending: PendingState?, branch: String?) {
        // A busy row keeps the branch the last tail read saw.
        guard s.isFinished, let url else { return (nil, nil, nil, cache[s.id]?.branch) }
        var c = cache[s.id] ?? Cached(since: s.since, size: -1, reply: nil, branch: nil, pending: nil)
        let prompt = s.isPermissionPrompt
        if c.size == size && c.since == s.since && c.prompt == prompt { return (c.reply, c.replyText, c.pending, c.branch) }
        c.since = s.since
        c.size = size
        c.prompt = prompt
        let tail = Transcript.tail(url)
        let full = Transcript.lastReply(in: tail)
        c.reply = full.map { Transcript.displayText($0, limit: 600) }
        c.replyText = c.reply.map(markdown)
        c.branch = Transcript.gitBranch(in: tail, last: true)
        c.pending = prompt ? Transcript.pending(in: tail) : nil
        if let t = Transcript.lastAITitle(in: tail) { c.aiTitle = t }
        cache[s.id] = c
        return (c.reply, c.replyText, c.pending, c.branch)
    }

    /// The last tool call of a busy session, read from the bytes appended since the last refresh (at most `activityWindow`).
    private func readActivity(for s: Session, url: URL?, size: Int) -> String? {
        let cached = activities[s.id]
        guard s.status == "busy", let url else {
            if let cached { activities[s.id] = (cached.offset, nil) }
            return nil
        }
        let size = UInt64(size)
        let window = Transcript.activityWindow
        let tailStart = size > window ? size - window : 0
        let from: UInt64
        if let cached, size >= cached.offset {
            if size == cached.offset { return cached.text }
            from = max(cached.offset, tailStart)
        } else {
            from = tailStart
        }
        let atLineStart = from == 0 || from == cached?.offset
        activityReads += 1
        let d = Transcript.read(url, from: from, to: size)
        var body = d[...]
        var offset = from
        if !atLineStart {
            guard let nl = d.firstIndex(of: 0x0A) else {
                activities[s.id] = (size, cached?.text)
                return cached?.text
            }
            body = d[(nl + 1)...]
            offset = from + UInt64(nl + 1)
        }
        var text = cached?.text
        if let end = body.lastIndex(of: 0x0A) {
            offset += UInt64(end + 1 - body.startIndex)
            text = Transcript.lastToolUse(in: body[body.startIndex...end]) ?? text
        }
        activities[s.id] = (offset, text)
        return text
    }

    public func markSeen(_ id: String) {
        if unseen.contains(id) { unseen.remove(id) }
    }

    /// Called with the focused cmux surface (nil when unknown). Clears the marks of the session shown there once it is seen on two consecutive calls.
    public func noteFocused(_ surface: String?) {
        if let surface, let last = lastFocused, last.caseInsensitiveCompare(surface) == .orderedSame {
            let matches = unseen.filter { surfaces[$0]?.caseInsensitiveCompare(surface) == .orderedSame }
            if !matches.isEmpty { unseen.subtract(matches) }
        }
        lastFocused = surface
    }

    public func refresh(at date: Date = Date()) {
        now = date
        chime = nil
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        var list: [Session] = []
        var sizes: [String: UInt64] = [:]
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
            let size = url.map(fileSize) ?? 0
            if url != nil { sizes[s.id] = UInt64(size) }
            let t = readTail(for: s, url: url, size: size)
            let head = readHead(for: s, url: url, size: size)
            s.firstPrompt = head.prompt
            s.aiTitle = cache[s.id]?.aiTitle
            s.branch = t.branch ?? head.branch
            s.lastReply = t.reply
            s.replyText = t.replyText
            s.todo = readTodo(for: s)
            if s.isPermissionPrompt { s.pending = t.pending }
            list.append(s)
        }
        // One conversation open in two processes (resumed in another pane): keep only the newest, so rows and caches keyed by id stay unique.
        var newest: [String: Session] = [:]
        for s in list where (newest[s.id]?.since ?? .distantPast) <= (s.since ?? .distantPast) { newest[s.id] = s }
        list = list.filter { newest[$0.id]?.pid == $0.pid }
        // After the dedupe, so two processes sharing a sessionId cannot reset each other's cached activity.
        for i in list.indices {
            let url = transcriptURL(for: list[i])
            list[i].activity = readActivity(for: list[i], url: url, size: url.map(fileSize) ?? 0)
        }

        if list.isEmpty {
            if emptySince == nil { emptySince = now }
        } else {
            emptySince = nil
        }

        var newUnseen = unseen
        for s in list {
            if let last = lastStatus[s.id], last != "busy", s.status == "busy" { newUnseen.remove(s.id) }
            if lastStatus[s.id] == "busy" && s.isFinished {
                newUnseen.insert(s.id)
                peekUntil = now.addingTimeInterval(5)
                if s.status == "waiting" { chime = .needsYou } else if chime == nil { chime = .done }
            }
            lastStatus[s.id] = s.status
        }
        scanner.update(list.map { ScanTarget(sessionId: $0.id, transcript: transcriptURL(for: $0), size: sizes[$0.id]) }, at: now)
        let scans = scanner.takeSnapshot()
        let learned = scans.values.reduce(into: jiraProjects) { acc, r in
            for (prefix, host) in r.jiraHosts where acc[prefix] == nil { acc[prefix] = host }
        }
        if learned != jiraProjects {
            jiraProjects = learned
            projectsGeneration += 1
        }
        var buttonRefs: [String: [[PRRef]]] = [:]
        for i in list.indices {
            let id = list[i].id
            let key = ButtonKey(version: scans[id]?.version ?? 0, projectsGeneration: projectsGeneration, branch: list[i].branch)
            if buttonCache[id]?.key != key {
                buttonCache[id] = (key, Store.buttons(scans[id] ?? ScanResult(), projects: jiraProjects, branch: list[i].branch))
            }
            let buttons = buttonCache[id]?.buttons ?? []
            list[i].links = buttons.map(\.link)
            buttonRefs[id] = buttons.map(\.refs)
        }
        if let prMonitor {
            let targets = list.map { PRTarget(sessionId: $0.id, linkRefs: scans[$0.id]?.prRefs ?? [], cwd: $0.cwd, branch: $0.branch) }
            prMonitor.update(targets, at: now)
            let snap = prMonitor.takeSnapshot()
            for i in list.indices {
                let all = snap.prs[list[i].id] ?? []
                if all.contains(where: { snap.eventRefs.contains($0.ref) }) { newUnseen.insert(list[i].id) }
                list[i].prs = Array(all.prefix(3))
                let shown = Set(list[i].prs.map(\.ref))
                if !shown.isEmpty {
                    let refs = buttonRefs[list[i].id] ?? []
                    list[i].links = list[i].links.enumerated().filter { !refs[$0.offset].contains(where: shown.contains) }.map(\.element)
                }
            }
            if prLastSuccess != snap.lastSuccess { prLastSuccess = snap.lastSuccess }
            if !snap.events.isEmpty {
                peekUntil = now.addingTimeInterval(5)
                if let c = PullRequests.chime(for: snap.events), chime != .needsYou { chime = c }
            }
        }
        let live = Set(list.map(\.id))
        cache = cache.filter { live.contains($0.key) }
        buttonCache = buttonCache.filter { live.contains($0.key) }
        urls = urls.filter { live.contains($0.key) }
        urlTried = urlTried.filter { live.contains($0.key) }
        heads = heads.filter { live.contains($0.key) }
        todos = todos.filter { live.contains($0.key) }
        activities = activities.filter { live.contains($0.key) }
        for s in list where pids[s.id] != nil && pids[s.id] != s.pid {
            surfaces[s.id] = nil
            surfaceMisses.remove(s.id)
        }
        surfaceMisses.formIntersection(live)
        newUnseen.formIntersection(live)
        if newUnseen != unseen { unseen = newUnseen }
        pids = Dictionary(list.map { ($0.id, $0.pid) }, uniquingKeysWith: { $1 })
        surfaces = surfaces.filter { live.contains($0.key) }
        let liveDrafts = drafts.filter { live.contains($0.key) }
        if liveDrafts.count != drafts.count { drafts = liveDrafts }
        let openTools = Set(list.compactMap { $0.approvable?.toolId })
        if !shownCommands.isSubset(of: openTools) { shownCommands.formIntersection(openTools) }

        func rank(_ s: Session) -> Int {
            switch s.status { case "waiting": 0; case "idle": 1; case "busy": 2; default: 3 }
        }
        list.sort { (rank($0), $0.title) < (rank($1), $1.title) }
        if list != sessions { sessions = list }
    }

    /// Buttons for one session: the branch's ticket, tickets the user mentioned, tickets only Claude mentioned, then PR-shaped links.
    static func buttons(_ r: ScanResult, projects: [String: String], branch: String?) -> [(link: SessionLink, refs: [PRRef])] {
        func host(_ key: String) -> String? { Transcript.projectPrefix(of: key).flatMap { projects[$0] } }
        var keys: [String] = []
        for k in branch.map(Transcript.ticketKeys) ?? [] where host(k) != nil && !keys.contains(k) { keys.append(k) }
        let known = r.tickets.values.filter { host($0.key) != nil && !keys.contains($0.key) }
        keys += known.filter(\.byUser).sorted { $0.first < $1.first }.map(\.key)
        keys += known.filter { !$0.byUser }.sorted { $0.last > $1.last }.map(\.key)
        let tickets = keys.compactMap { k in
            host(k).flatMap { URL(string: "https://\($0)/browse/\(k)") }.map { (link: SessionLink(label: k, url: $0), refs: [PRRef]()) }
        }
        return tickets + r.prLinks.map {
            (link: SessionLink(label: Transcript.label(for: $0), url: $0), refs: PullRequests.refs(in: $0.absoluteString))
        }
    }

    public func stillValid(_ a: WidgetAction) -> Bool {
        switch a {
        case .jump, .openLink: return true
        case .approve(_, let toolId), .deny(_, let toolId, _): return allows(Editing(id: a.sessionId, kind: .deny, toolId: toolId))
        case .askClaude(let id, _): return sessions.first { $0.id == id }?.status == "idle"
        }
    }
}
