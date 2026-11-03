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
}

extension Session {
    var isFinished: Bool { status == "idle" || status == "waiting" }
}

public final class Store: ObservableObject {
    @Published var sessions: [Session] = []
    @Published public var unseen: Set<String> = []
    @Published public var measuredHeight: CGFloat?
    /// Session id -> cmux surface UUID, filled by the app and pruned to live ids.
    public var surfaces: [String: String] = [:]
    @Published public var hovering = false
    @Published var now = Date()
    @Published public var peekUntil: Date?
    @Published public var anchor = Anchor()

    var expanded: Bool { hovering || (peekUntil.map { now < $0 } ?? false) }

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
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects")) {
        self.dir = dir
        self.projects = projects
    }

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
        if c.size == size && c.since == s.since { return (c.reply, c.pending) }
        c.since = s.since
        c.size = size
        let tail = Transcript.tail(url)
        c.reply = Transcript.lastReply(in: tail)
        c.pending = Transcript.pending(in: tail)
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
                since: ms.map { Date(timeIntervalSince1970: $0 / 1000) }
            )
            let t = readTail(for: s)
            s.lastReply = t.reply
            if s.status == "waiting" && s.waitingFor == "permission prompt" { s.pending = t.pending }
            list.append(s)
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
        surfaces = surfaces.filter { live.contains($0.key) }
        newUnseen.formIntersection(live)
        if newUnseen != unseen { unseen = newUnseen }

        func rank(_ s: Session) -> Int {
            switch s.status { case "waiting": 0; case "idle": 1; case "busy": 2; default: 3 }
        }
        list.sort { (rank($0), $0.name) < (rank($1), $1.name) }
        if list != sessions { sessions = list }
    }

    public func markSeen(_ id: String) {
        if unseen.contains(id) { unseen.remove(id) }
    }
}
