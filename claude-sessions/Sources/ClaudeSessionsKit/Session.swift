import Foundation
import Combine
import AppKit

enum ItemTone: Equatable {
    case todo, doing, done, open, merged, closed

    init(_ t: JiraIssue.Tone) {
        switch t {
        case .todo: self = .todo
        case .doing: self = .doing
        case .done: self = .done
        }
    }
}

struct LinkItem: Equatable {
    enum Kind { case ticket, pr, link }
    let text: String
    let url: URL
    var kind: Kind
    var code: String
    var title: String?
    var status: String?
    var tone: ItemTone?
    var iconURL: URL?
    var help: String?

    init(text: String, url: URL, kind: Kind = .link, code: String? = nil, title: String? = nil, status: String? = nil,
         tone: ItemTone? = nil, iconURL: URL? = nil, help: String? = nil) {
        self.text = text; self.url = url; self.kind = kind; self.code = code ?? text; self.title = title; self.status = status
        self.tone = tone; self.iconURL = iconURL; self.help = help
    }
}

struct LinkSection: Equatable {
    let title: String
    let items: [LinkItem]
}

enum ConnectionLine: Equatable { case missing, checking(String), ok(String), rejected, unreachable, noSite }

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
    /// Every ticket and PR of the session, grouped for the `☰ N` menu.
    var menu: [LinkSection] = []
    /// The menu holds a ticket and Jira is not connected: the menu ends with a `Kết nối Jira…` item.
    var menuNeedsJira = false
}

extension Session {
    var title: String { nameSource == "user" ? name : (aiTitle ?? firstPrompt ?? name) }
    var folder: String { (cwd as NSString).lastPathComponent }
    var subtitle: String { branch.map { "\(folder) · \($0)" } ?? folder }
    var menuCount: Int { menu.reduce(0) { $0 + $1.items.count } }
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
    }
    /// Buttons per session with the PR refs each link points at, rebuilt only when their `ButtonKey` changes.
    private var buttonCache: [String: (key: ButtonKey, buttons: [ButtonEntry])] = [:]
    /// Bumped whenever `jiraProjects` changes, so `ButtonKey` need not compare the dictionary.
    private var projectsGeneration = 0
    private var urls: [String: URL] = [:]
    private var urlTried: [String: Date?] = [:]
    private var heads: [String: (prompt: String?, size: Int)] = [:]
    private var todos: [String: (key: TaskKey, files: [URL], todo: Todo?)] = [:]
    /// Parsed session files by URL, valid while the file's `FileStamp` is unchanged: the `Session` with the fields the file gives, and its `spare` flag.
    private var sessionJSON: [URL: (stamp: FileStamp, base: Session, spare: Bool)] = [:]
    private var activities: [String: (offset: UInt64, text: String?)] = [:]
    /// File reads done for the running-tool line, for tests.
    private(set) var activityReads = 0
    /// Transcript bytes read by the head, tail and activity reads, for tests.
    private(set) var transcriptBytesRead = 0
    /// Session `.json` files read and parsed, for tests.
    private(set) var sessionParses = 0
    /// Task directory listings, for tests.
    private(set) var taskListings = 0
    /// `HEAD` file reads of the store's own branch reader; 0 when a custom reader was injected.
    var headReads: Int { gitReader?.headReads ?? 0 }
    /// Entry count of every per-session cache, by property name, for tests.
    var cacheCounts: [String: Int] {
        ["cache": cache.count, "buttonCache": buttonCache.count, "urls": urls.count, "urlTried": urlTried.count,
         "heads": heads.count, "todos": todos.count, "activities": activities.count,
         "pids": pids.count, "surfaces": surfaces.count, "sessionJSON": sessionJSON.count,
         "branches": gitReader?.cachedCwds ?? 0]
    }

    /// Changes whenever a task file is added, removed or rewritten; each `FileStamp` (inode, size, ns mtime) covers an in-place rewrite.
    private struct TaskKey: Equatable {
        var dir: FileStamp?
        var stamps: [FileStamp?]
    }

    private struct Cached {
        var since: Date?
        var size: Int
        var reply: String?
        var replyText: AttributedString?
        var aiTitle: String?
        var pending: PendingState?
        var prompt = false
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects"),
                tasks: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/tasks"),
                startHidden: Bool = false,
                prs: PRMonitor? = nil,
                jira: JiraMonitor? = nil,
                scanner: TranscriptScanner = TranscriptScanner(),
                branch: ((String) -> String?)? = nil) {
        if let branch {
            self.gitReader = nil
            self.branchReader = branch
        } else {
            let reader = GitBranchReader()
            self.gitReader = reader
            self.branchReader = reader.current(in:)
        }
        self.prMonitor = prs
        self.jiraMonitor = jira
        self.scanner = scanner
        self.dir = dir
        self.projects = projects
        self.tasks = tasks
        emptySince = startHidden ? .distantPast : nil
    }

    /// The branch checked out in a cwd, read every refresh.
    private let branchReader: (String) -> String?
    private let gitReader: GitBranchReader?
    public var prMonitor: PRMonitor?
    public var jiraMonitor: JiraMonitor?
    /// Decoded issue type icons by URL, kept for the run.
    @Published var issueIcons: [URL: NSImage] = [:]
    private var iconsSeen = Set<URL>()

    /// Keychain account (email) per service, set by the app.
    @Published public var accounts: [Service: String] = [:]
    @Published var connections: [Service: ConnectionLine] = [:]
    private let scanner: TranscriptScanner
    /// Jira project prefix -> host, learned from `/browse/` and `selectedIssue=` links; persisted by the app.
    @Published public var jiraProjects: [String: String] = [:]
    /// The Jira site to check a token against: the first learned `*.atlassian.net` host.
    public var jiraHost: String? { jiraProjects.values.filter(Jira.isAtlassianHost).sorted().first }
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

    private func readTodo(for s: Session) -> Todo? {
        let d = tasks.appendingPathComponent(s.id, isDirectory: true)
        let dirStamp = FileStamp(d.path)
        let cached = todos[s.id]
        // An unchanged directory has the same files: re-stat them (an in-place rewrite does not touch the directory) instead of listing.
        let files: [URL]
        if let cached, cached.key.dir == dirStamp {
            files = cached.files
        } else {
            taskListings += 1
            files = Tasks.files(in: d)
        }
        let key = TaskKey(dir: dirStamp, stamps: files.map { FileStamp($0.path) })
        if let cached, cached.key == key { return cached.todo }
        let todo = Tasks.read(files)
        todos[s.id] = (key, files, todo)
        return todo
    }

    /// The head of an append-only transcript cannot change once it holds a prompt or the file outgrew the head window.
    private func headIsFinal(_ id: String) -> Bool {
        heads[id].map { $0.prompt != nil || $0.size > Transcript.headBytes } ?? false
    }

    private func readHead(for s: Session, url: URL?, size: Int) -> String? {
        guard let url else { return nil }
        if let h = heads[s.id], headIsFinal(s.id) || h.size == size { return h.prompt }
        let d = Transcript.head(url)
        transcriptBytesRead += d.count
        let full = Transcript.firstPrompt(in: d)
        let h = (prompt: full.map { Transcript.displayText($0, limit: 200) }, size: size)
        heads[s.id] = h
        return h.prompt
    }

    private func readTail(for s: Session, url: URL?, size: Int) -> (reply: String?, replyText: AttributedString?, pending: PendingState?) {
        guard s.isFinished, let url else { return (nil, nil, nil) }
        var c = cache[s.id] ?? Cached(since: s.since, size: -1, reply: nil, pending: nil)
        let prompt = s.isPermissionPrompt
        if c.size == size && c.since == s.since && c.prompt == prompt { return (c.reply, c.replyText, c.pending) }
        c.since = s.since
        c.size = size
        c.prompt = prompt
        let tail = Transcript.tail(url)
        transcriptBytesRead += tail.count
        let full = Transcript.lastReply(in: tail)
        c.reply = full.map { Transcript.displayText($0, limit: 600) }
        c.replyText = c.reply.map(markdown)
        c.pending = prompt ? Transcript.pending(in: tail) : nil
        if let t = Transcript.lastAITitle(in: tail) { c.aiTitle = t }
        cache[s.id] = c
        return (c.reply, c.replyText, c.pending)
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
        transcriptBytesRead += d.count
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
        var seenFiles = Set<URL>()
        for file in files where file.pathExtension == "json" {
            seenFiles.insert(file)
            guard let stamp = FileStamp(file.path) else { continue }
            let base: Session
            let spare: Bool
            if let c = sessionJSON[file], c.stamp == stamp {
                base = c.base
                spare = c.spare
            } else {
                sessionParses += 1
                guard let data = try? Data(contentsOf: file),
                      let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                      let pid = (o["pid"] as? NSNumber)?.int32Value
                else { continue }
                var parsed = Session(
                    id: (o["sessionId"] as? String) ?? String(pid),
                    name: (o["name"] as? String) ?? "pid \(pid)",
                    cwd: (o["cwd"] as? String) ?? "",
                    status: (o["status"] as? String) ?? "",
                    waitingFor: o["waitingFor"] as? String,
                    since: (o["statusUpdatedAt"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue / 1000) },
                    pid: pid
                )
                parsed.nameSource = o["nameSource"] as? String
                base = parsed
                spare = (o["spare"] as? Bool) == true
                sessionJSON[file] = (stamp, base, spare)
            }
            // Liveness is checked every refresh: a dead pid drops the row even when the file is unchanged.
            guard kill(base.pid, 0) == 0 || errno == EPERM, !spare else { continue }
            var s = base
            let url = transcriptURL(for: s)
            let size = url.flatMap { FileStamp($0.path)?.size } ?? 0
            if url != nil { sizes[s.id] = UInt64(size) }
            let t = readTail(for: s, url: url, size: size)
            let head = readHead(for: s, url: url, size: size)
            s.firstPrompt = head
            s.aiTitle = cache[s.id]?.aiTitle
            s.branch = branchReader(s.cwd)
            s.lastReply = t.reply
            s.replyText = t.replyText
            s.todo = readTodo(for: s)
            if s.isPermissionPrompt { s.pending = t.pending }
            list.append(s)
        }
        for k in sessionJSON.keys where !seenFiles.contains(k) { sessionJSON[k] = nil }
        // One conversation open in two processes (resumed in another pane): keep only the newest, so rows and caches keyed by id stay unique.
        var newest: [String: Session] = [:]
        for s in list where (newest[s.id]?.since ?? .distantPast) <= (s.since ?? .distantPast) { newest[s.id] = s }
        list = list.filter { newest[$0.id]?.pid == $0.pid }
        // After the dedupe, so two processes sharing a sessionId cannot reset each other's cached activity.
        for i in list.indices {
            let url = transcriptURL(for: list[i])
            list[i].activity = readActivity(for: list[i], url: url, size: Int(sizes[list[i].id] ?? 0))
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
            for (prefix, host) in r.jiraHosts where Jira.replaces(acc[prefix], with: host) { acc[prefix] = host }
        }
        if learned != jiraProjects {
            jiraProjects = learned
            projectsGeneration += 1
        }
        var entries: [String: [ButtonEntry]] = [:]
        for i in list.indices {
            let id = list[i].id
            let key = ButtonKey(version: scans[id]?.version ?? 0, projectsGeneration: projectsGeneration)
            if buttonCache[id]?.key != key {
                buttonCache[id] = (key, Store.buttons(scans[id] ?? ScanResult(), projects: jiraProjects))
            }
            entries[id] = buttonCache[id]?.buttons ?? []
        }
        var issues: [String: JiraIssue] = [:]
        var jiraState: ConnectionState?
        if let jiraMonitor {
            // Only `*.atlassian.net` hosts get the token; the monitor drops duplicates.
            let targets = list.flatMap { entries[$0.id] ?? [] }.compactMap(\.target).filter { Jira.isAtlassianHost($0.host) }
            jiraMonitor.update(targets, at: now)
            let snap = jiraMonitor.takeSnapshot()
            issues = snap.issues
            decodeIcons(snap.icons)
            jiraState = snap.connection
        }
        var prState: ConnectionState?
        var buttonRefs: [String: [[PRRef]]] = [:]
        var allPRs: [String: [PRStatus]] = [:]
        for i in list.indices {
            let buttons = entries[list[i].id] ?? []
            list[i].links = buttons.map { b in
                var link = b.link
                if let t = b.target {
                    let issue = issues[t.key]
                    link.help = Store.help(key: t.key, issue)
                    link.iconURL = issue?.iconURL
                    link.tone = issue?.tone.map(ItemTone.init)
                }
                return link
            }
            buttonRefs[list[i].id] = buttons.map(\.refs)
        }
        if let prMonitor {
            let targets = list.map { PRTarget(sessionId: $0.id, linkRefs: scans[$0.id]?.prRefs ?? [], cwd: $0.cwd, branch: $0.branch) }
            prMonitor.update(targets, at: now)
            let snap = prMonitor.takeSnapshot()
            prState = snap.connection
            for i in list.indices {
                let all = snap.prs[list[i].id] ?? []
                allPRs[list[i].id] = all
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
        for i in list.indices {
            let id = list[i].id
            list[i].menu = Store.menu(entries[id] ?? [], issues: issues, prs: prMonitor == nil ? nil : allPRs[id] ?? [])
            list[i].menuNeedsJira = jiraState != .ok && (entries[id] ?? []).contains { $0.target.map { Jira.isAtlassianHost($0.host) } ?? false }
        }
        let lines: [Service: ConnectionLine] = [
            .bitbucket: Store.line(present: prMonitor != nil, account: accounts[.bitbucket], state: prState ?? .checking),
            .jira: jiraHost == nil ? .noSite : Store.line(present: jiraMonitor != nil, account: accounts[.jira], state: jiraState ?? .checking),
        ]
        if lines != connections { connections = lines }
        let live = Set(list.map(\.id))
        cache = cache.filter { live.contains($0.key) }
        buttonCache = buttonCache.filter { live.contains($0.key) }
        urls = urls.filter { live.contains($0.key) }
        urlTried = urlTried.filter { live.contains($0.key) }
        heads = heads.filter { live.contains($0.key) }
        todos = todos.filter { live.contains($0.key) }
        gitReader?.prune(keeping: Set(list.map(\.cwd)))
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

    struct ButtonEntry {
        enum Source { case user, claude, pr }
        let link: SessionLink
        let refs: [PRRef]
        let source: Source
        /// The Jira issue of a ticket button; every ticket button has a known host.
        let target: JiraTarget?
    }

    /// Buttons for one session: tickets the user mentioned, tickets only Claude mentioned, then PR-shaped links.
    static func buttons(_ r: ScanResult, projects: [String: String]) -> [ButtonEntry] {
        func host(_ key: String) -> String? { Transcript.projectPrefix(of: key).flatMap { projects[$0] } }
        var keys: [(key: String, source: ButtonEntry.Source)] = []
        let known = r.tickets.values.filter { host($0.key) != nil }
        keys += known.filter(\.byUser).sorted { $0.first < $1.first }.map { ($0.key, .user) }
        keys += known.filter { !$0.byUser }.sorted { $0.last > $1.last }.map { ($0.key, .claude) }
        let tickets = keys.compactMap { k -> ButtonEntry? in
            guard let h = host(k.key), let url = URL(string: "https://\(h)/browse/\(k.key)") else { return nil }
            return ButtonEntry(link: SessionLink(label: k.key, url: url), refs: [], source: k.source, target: JiraTarget(key: k.key, host: h))
        }
        return tickets + r.prLinks.map {
            ButtonEntry(link: SessionLink(label: Transcript.label(for: $0), url: $0), refs: PullRequests.refs(in: $0.absoluteString), source: .pr, target: nil)
        }
    }

    /// `KEY · title · status` for a ticket button's hover text; nil until Jira answered with a title.
    static func help(key: String, _ issue: JiraIssue?) -> String? {
        guard let title = issue?.title else { return nil }
        return ([key, title] + [issue?.status].compactMap { $0 }).joined(separator: " · ")
    }

    /// Decodes each icon once; undecodable data is not retried. Internal for tests.
    func decodeIcons(_ data: [URL: Data]) {
        var next = issueIcons
        for (url, d) in data where iconsSeen.insert(url).inserted {
            if let img = NSImage(data: d) { next[url] = img }
        }
        if next.count != issueIcons.count { issueIcons = next }
    }

    /// `KEY  title  ·  status`, the title cut at 60 characters; the key alone without a title.
    static func ticketText(key: String, _ issue: JiraIssue?) -> String {
        guard let title = issue?.title else { return key }
        let cut = title.count > 60 ? String(title.prefix(60)) + "…" : title
        return "\(key)  \(cut)" + (issue?.status.map { "  ·  \($0)" } ?? "")
    }

    /// `PR #id  title  ·  state`.
    static func linkText(_ pr: PRStatus) -> String {
        "PR #\(pr.ref.id)  " + (pr.title.map { "\($0)  ·  " } ?? "·  ") + pr.stateWord
    }

    private static func prItem(_ pr: PRStatus) -> LinkItem {
        LinkItem(text: linkText(pr), url: pr.url, kind: .pr, code: "#\(pr.ref.id)", title: pr.title,
                 status: pr.stateWord, tone: pr.tone, help: pr.title)
    }

    /// The `☰ N` menu: tickets by origin, then every PR. `prs` is nil without a PR monitor, so every PR link is listed as it is.
    static func menu(_ entries: [ButtonEntry], issues: [String: JiraIssue], prs: [PRStatus]?) -> [LinkSection] {
        func tickets(_ source: ButtonEntry.Source, _ title: String) -> LinkSection {
            LinkSection(title: title, items: entries.filter { $0.source == source }.compactMap { e in
                e.target.map { t in
                    let issue = issues[t.key]
                    return LinkItem(text: ticketText(key: t.key, issue), url: e.link.url, kind: .ticket, code: t.key, title: issue?.title,
                                    status: issue?.status, tone: issue?.tone.map(ItemTone.init), iconURL: issue?.iconURL,
                                    help: help(key: t.key, issue))
                }
            })
        }
        let tracked = Set((prs ?? []).map(\.ref))
        let untracked = entries.filter { $0.source == .pr && !$0.refs.contains(where: tracked.contains) }
            .map { LinkItem(text: $0.link.label, url: $0.link.url, kind: .link, code: $0.link.label) }
        let pr = LinkSection(title: "PR", items: (prs ?? []).map(prItem) + untracked)
        return [tickets(.user, "Bạn nhắc"), tickets(.claude, "Claude nhắc"), pr].filter { !$0.items.isEmpty }
    }

    static func line(present: Bool, account: String?, state: ConnectionState) -> ConnectionLine {
        guard present, let account else { return .missing }
        switch state {
        case .checking: return .checking(account)
        case .ok: return .ok(account)
        case .rejected: return .rejected
        case .unreachable: return .unreachable
        }
    }

    /// The context-menu line of one service.
    public func connectionText(_ s: Service) -> String {
        let name = s.displayName
        switch connections[s] ?? .missing {
        case .missing: return "\(name): ✗ chưa có token"
        case .checking(let a): return "\(name): \(a) (đang kiểm tra)"
        case .ok(let a): return "\(name): ✓ \(a)"
        case .rejected: return "\(name): ✗ token bị từ chối (401)"
        case .unreachable: return "\(name): ✗ không kết nối được"
        case .noSite: return "\(name): ✗ chưa biết site Jira"
        }
    }

    public func stillValid(_ a: WidgetAction) -> Bool {
        switch a {
        case .jump, .openLink, .connect, .nudgeRabbit: return true
        case .approve(_, let toolId), .deny(_, let toolId, _): return allows(Editing(id: a.sessionId, kind: .deny, toolId: toolId))
        case .askClaude(let id, _): return sessions.first { $0.id == id }?.status == "idle"
        }
    }
}
