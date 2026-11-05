import Foundation

/// The branch checked out in a directory, read from `.git/HEAD` without spawning git.
public enum GitBranch {
    /// Walks up from `cwd` to the first `.git`; nil for a detached HEAD, no repo or an unreadable file.
    public static func current(in cwd: String) -> String? {
        let fm = FileManager.default
        var dir = URL(fileURLWithPath: cwd)
        var isDir: ObjCBool = false
        while cwd.hasPrefix("/") {
            let git = dir.appendingPathComponent(".git")
            if fm.fileExists(atPath: git.path, isDirectory: &isDir) {
                var head = git
                if !isDir.boolValue {
                    guard let text = try? String(contentsOf: git, encoding: .utf8), text.hasPrefix("gitdir:") else { return nil }
                    let path = text.dropFirst("gitdir:".count).trimmingCharacters(in: .whitespacesAndNewlines)
                    head = URL(fileURLWithPath: path, relativeTo: dir)
                }
                guard let content = try? String(contentsOf: head.appendingPathComponent("HEAD"), encoding: .utf8) else { return nil }
                let line = content.trimmingCharacters(in: .whitespacesAndNewlines)
                let prefix = "ref: refs/heads/"
                return line.hasPrefix(prefix) && line.count > prefix.count ? String(line.dropFirst(prefix.count)) : nil
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { return nil }
            dir = parent
        }
        return nil
    }
}
