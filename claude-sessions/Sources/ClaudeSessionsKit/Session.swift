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
}

extension Session {
    var isFinished: Bool { status == "idle" || status == "waiting" }
}

public final class Store: ObservableObject {
    @Published var sessions: [Session] = []
    @Published var attention: Set<String> = []
    @Published public var hovering = false
    @Published var now = Date()
    @Published public var peekUntil: Date?
    @Published public var anchor = Anchor()

    var expanded: Bool { hovering || (peekUntil.map { now < $0 } ?? false) }

    private var lastStatus: [String: String] = [:]
    private var attentionAt: [String: Date] = [:]
    private let dir: URL
    private let projects: URL
    private var cache: [String: Cached] = [:]

    private struct Cached {
        var since: Date?
        var url: URL?
        var size: Int
        var reply: String?
    }

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects")) {
        self.dir = dir
        self.projects = projects
    }

    private func reply(for s: Session) -> String? {
        guard s.isFinished else { return nil }
        var c: Cached
        if let cached = cache[s.id], cached.url != nil || cached.since == s.since {
            c = cached
        } else {
            c = Cached(since: s.since, url: Transcript.find(sessionId: s.id, cwd: s.cwd, projects: projects), size: -1, reply: nil)
        }
        guard let url = c.url else {
            cache[s.id] = c
            return nil
        }
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        if c.size == size && c.since == s.since { return c.reply }
        c.since = s.since
        c.size = size
        c.reply = Transcript.lastReply(in: Transcript.tail(url))
        cache[s.id] = c
        return c.reply
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
            s.lastReply = reply(for: s)
            list.append(s)
        }

        for s in list {
            if lastStatus[s.id] == "busy" && s.isFinished {
                attentionAt[s.id] = now
                peekUntil = now.addingTimeInterval(5)
            }
            if s.status == "busy" { attentionAt[s.id] = nil }
            lastStatus[s.id] = s.status
        }
        let live = Set(list.map(\.id))
        cache = cache.filter { live.contains($0.key) }
        attentionAt = attentionAt.filter { live.contains($0.key) && now.timeIntervalSince($0.value) < 20 }
        let newAttention = Set(attentionAt.keys)
        if newAttention != attention { attention = newAttention }

        func rank(_ s: Session) -> Int {
            switch s.status { case "waiting": 0; case "idle": 1; case "busy": 2; default: 3 }
        }
        list.sort { (rank($0), $0.name) < (rank($1), $1.name) }
        if list != sessions { sessions = list }
    }
}
