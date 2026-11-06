import Foundation

/// Changed files and unpushed commits of a repo; `ahead` is nil without an upstream.
struct GitCounts: Equatable {
    let dirty: Int
    let ahead: Int?
}

enum GitStatus {
    /// Parses `git status --porcelain=v2 --branch`: every non-`#` line is a changed file (untracked included).
    static func parse(_ porcelain: String) -> GitCounts {
        var dirty = 0
        var ahead: Int?
        for line in porcelain.split(separator: "\n") {
            if line.hasPrefix("# branch.ab ") {
                ahead = line.split(separator: " ").dropFirst(2).first.flatMap { Int($0.dropFirst()) }
            } else if !line.hasPrefix("#") {
                dirty += 1
            }
        }
        return GitCounts(dirty: dirty, ahead: ahead)
    }
}

/// Runs `git status` per repo root, at most once per `interval` (at once when the root's HEAD changed), off the main thread.
public final class GitStatusMonitor {
    private let queue: DispatchQueue?
    private let interval: TimeInterval
    private let run: (String) -> String?
    private let lock = NSLock()
    private var counts: [String: GitCounts] = [:]
    private var lastRun: [String: Date] = [:]
    private var running = Set<String>()

    public init(queue: DispatchQueue? = DispatchQueue(label: "claude-sessions.git-status"), interval: TimeInterval = 30,
                run: ((String) -> String?)? = nil) {
        self.queue = queue
        self.interval = interval
        self.run = run ?? { root in
            runProcess("/usr/bin/git", ["-C", root, "status", "--porcelain=v2", "--branch"], timeout: 3)
        }
    }

    /// Main thread, cheap: drops unused roots and queues a run for each root that is due.
    func update(roots: Set<String>, headChanged: Set<String>, at now: Date) {
        lock.lock()
        counts = counts.filter { roots.contains($0.key) }
        lastRun = lastRun.filter { roots.contains($0.key) }
        var due: [String] = []
        for root in roots where !running.contains(root) {
            if headChanged.contains(root) || lastRun[root].map({ now.timeIntervalSince($0) >= interval }) ?? true {
                running.insert(root)
                lastRun[root] = now
                due.append(root)
            }
        }
        lock.unlock()
        for root in due {
            if let queue { queue.async { [self] in finish(root) } } else { finish(root) }
        }
    }

    private func finish(_ root: String) {
        let out = run(root)
        lock.lock(); defer { lock.unlock() }
        running.remove(root)
        if let out, lastRun[root] != nil { counts[root] = GitStatus.parse(out) }
    }

    func takeSnapshot() -> [String: GitCounts] {
        lock.lock(); defer { lock.unlock() }
        return counts
    }
}
