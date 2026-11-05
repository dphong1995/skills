import Foundation

/// The branch checked out in a directory, read from `.git/HEAD` without spawning git.
enum GitBranch {
    /// Walks up from `cwd` to the first `.git`; nil for a detached HEAD, no repo or an unreadable file.
    static func current(in cwd: String) -> String? {
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
                guard let content = try? String(contentsOf: head.appendingPathComponent("HEAD"), encoding: .utf8) else { return nil }
                let line = content.trimmingCharacters(in: .whitespacesAndNewlines)
                let prefix = "ref: refs/heads/"
                let name = line.dropFirst(prefix.count)
                return line.hasPrefix(prefix) && !name.isEmpty ? String(name) : nil
            }
            let parent = dir.deletingLastPathComponent()
            if parent.path == dir.path { return nil }
            dir = parent
        }
    }
}
