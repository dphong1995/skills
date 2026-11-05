import Foundation

/// The branch checked out in a directory, read from `.git/HEAD` without spawning git.
enum GitBranch {
    /// Walks up from `cwd` to the first `.git` and returns its `HEAD` file; nil for no repo or an unreadable `.git` file.
    static func headFile(in cwd: String) -> URL? {
        guard cwd.hasPrefix("/") else { return nil }
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: cwd, isDirectory: true)
        while true {
            let git = dir.appendingPathComponent(".git")
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: git.path, isDirectory: &isDir) {
                var head = git
                if !isDir.boolValue {
                    guard let text = try? String(contentsOf: git, encoding: .utf8), text.hasPrefix("gitdir:") else { return nil }
                    let path = text.dropFirst("gitdir:".count).trimmingCharacters(in: .whitespacesAndNewlines)
                    head = URL(fileURLWithPath: path, relativeTo: dir)
                }
                return head.appendingPathComponent("HEAD")
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { return nil }
            dir = parent
        }
    }

    /// The branch named by a `HEAD` file; nil for a detached HEAD or an unreadable file.
    static func branch(inHead head: URL, onHeadRead: () -> Void = {}) -> String? {
        onHeadRead()
        guard let content = try? String(contentsOf: head, encoding: .utf8) else { return nil }
        let line = content.trimmingCharacters(in: .whitespacesAndNewlines)
        let prefix = "ref: refs/heads/"
        let name = line.dropFirst(prefix.count)
        return line.hasPrefix(prefix) && !name.isEmpty ? String(name) : nil
    }

    /// Branch of the repo containing `cwd`; nil for a detached HEAD, no repo or an unreadable file.
    static func current(in cwd: String, onHeadRead: () -> Void = {}) -> String? {
        headFile(in: cwd).flatMap { branch(inHead: $0, onHeadRead: onHeadRead) }
    }
}

/// Reads branches for a `Store`: per cwd it remembers the `HEAD` file and re-reads it only when its `FileStamp` changes.
/// A cwd with no repo is not cached, so a later `git init` is seen. Counts `HEAD` file reads for tests.
final class GitBranchReader {
    private(set) var headReads = 0
    private var cache: [String: (head: URL, stamp: FileStamp, branch: String?)] = [:]
    var cachedCwds: Int { cache.count }

    init() {}

    func current(in cwd: String) -> String? {
        if let c = cache[cwd], let stamp = FileStamp(c.head.path), stamp == c.stamp { return c.branch }
        cache[cwd] = nil
        guard let head = GitBranch.headFile(in: cwd) else { return nil }
        let stamp = FileStamp(head.path)
        let branch = GitBranch.branch(inHead: head, onHeadRead: { headReads += 1 })
        if let stamp { cache[cwd] = (head, stamp, branch) }
        return branch
    }

    func prune(keeping cwds: Set<String>) {
        for k in cache.keys where !cwds.contains(k) { cache[k] = nil }
    }
}
