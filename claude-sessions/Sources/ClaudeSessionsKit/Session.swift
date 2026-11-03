import Foundation
import Combine

public struct Session: Identifiable, Equatable {
    public let id: String
    public let name: String
    public let cwd: String
    public let status: String
    public let waitingFor: String?
    public let since: Date?
    public var lastReply: String? = nil
}

public final class Store: ObservableObject {
    @Published public var sessions: [Session] = []
    @Published public var attention: Set<String> = []
    @Published public var hovering = false
    @Published public var now = Date()
    @Published public var peekUntil: Date?
    @Published public var anchor = Anchor()

    public var expanded: Bool { hovering || (peekUntil.map { now < $0 } ?? false) }

    private var lastStatus: [String: String] = [:]
    private var attentionAt: [String: Date] = [:]
    private let dir: URL
    private let projects: URL
    private var replyCache: [String: (since: Date?, size: Int, reply: String?)] = [:]
    private var urlCache: [String: (since: Date?, url: URL?)] = [:]

    public init(dir: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/sessions"),
                projects: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects")) {
        self.dir = dir
        self.projects = projects
    }

    private func reply(for s: Session) -> String? {
        guard s.status == "idle" || s.status == "waiting" else { return nil }
        if urlCache[s.id] == nil || (urlCache[s.id]!.url == nil && urlCache[s.id]!.since != s.since) {
            urlCache[s.id] = (s.since, Transcript.find(sessionId: s.id, cwd: s.cwd, projects: projects))
        }
        guard let url = urlCache[s.id]?.url else { return nil }
        let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
        if let c = replyCache[s.id], c.since == s.since, c.size == size { return c.reply }
        let reply = Transcript.lastReply(in: Transcript.tail(url))
        replyCache[s.id] = (s.since, size, reply)
        return reply
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
            list.append(Session(
                id: o["sessionId"] as? String ?? String(pid),
                name: o["name"] as? String ?? "pid \(pid)",
                cwd: o["cwd"] as? String ?? "",
                status: o["status"] as? String ?? "",
                waitingFor: o["waitingFor"] as? String,
                since: ms.map { Date(timeIntervalSince1970: $0 / 1000) }
            ))
        }

        list = list.map { var s = $0; s.lastReply = reply(for: s); return s }

        for s in list {
            if lastStatus[s.id] == "busy" && (s.status == "idle" || s.status == "waiting") {
                attentionAt[s.id] = now
                peekUntil = now.addingTimeInterval(5)
            }
            if s.status == "busy" { attentionAt[s.id] = nil }
            lastStatus[s.id] = s.status
        }
        let live = Set(list.map(\.id))
        replyCache = replyCache.filter { live.contains($0.key) }
        urlCache = urlCache.filter { live.contains($0.key) }
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
