import AppKit

/// A session whose process ended, kept so it can be resumed with `claude --resume`.
public struct ClosedSession: Equatable, Codable {
    public let id: String
    public let title: String
    public let cwd: String
    public let branch: String?
    public let closedAt: Date

    public init(id: String, title: String, cwd: String, branch: String?, closedAt: Date) {
        self.id = id
        self.title = title
        self.cwd = cwd
        self.branch = branch
        self.closedAt = closedAt
    }

    init(_ s: Session, closedAt: Date) {
        self.init(id: s.id, title: s.title, cwd: s.cwd, branch: s.branch, closedAt: closedAt)
    }

    var folder: String { Self.folder(cwd) }
    var subtitle: String { folder }

    /// The folder name of `cwd`; shared with `Session`.
    static func folder(_ cwd: String) -> String { (cwd as NSString).lastPathComponent }

    /// What the user can paste into a terminal when cmux cannot open the workspace.
    public var shellCommand: String {
        "cd '" + cwd.replacingOccurrences(of: "'", with: "'\\''") + "' && claude --resume \(id)"
    }

    /// Blocking: call off the main thread. Opens a cmux workspace in `cwd` running `claude --resume <id>`; false on any failure.
    public func resume() -> Bool {
        CmuxJump.newWorkspace(cwd: cwd, command: "claude --resume \(id)")
    }
}

/// The recently closed sessions: newest first, at most `limit`, none older than `maxAge`, one per id.
public struct ClosedSessions: Equatable {
    static let limit = 5
    static let maxAge: TimeInterval = 24 * 3600

    public private(set) var items: [ClosedSession]

    public init(_ items: [ClosedSession] = []) { self.items = items }

    public mutating func add(_ s: ClosedSession, now: Date) {
        items.removeAll { $0.id == s.id }
        items.insert(s, at: 0)
        prune(now: now)
    }

    public mutating func revive(_ ids: Set<String>) {
        items.removeAll { ids.contains($0.id) }
    }

    public mutating func prune(now: Date) {
        items.removeAll { now.timeIntervalSince($0.closedAt) > Self.maxAge }
        if items.count > Self.limit { items.removeLast(items.count - Self.limit) }
    }
}
