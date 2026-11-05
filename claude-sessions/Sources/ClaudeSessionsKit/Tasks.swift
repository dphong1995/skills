import Foundation

struct Todo: Equatable {
    let done: Int
    let total: Int
    let current: String?

    var label: String {
        let n = "☑ \(done)/\(total)"
        return current.map { "\(n) · \($0)" } ?? n
    }
}

enum Tasks {
    private struct Item {
        let id: Int
        let name: String
        let subject: String?
        let activeForm: String?
        let status: String
    }

    /// The task files in `dir` (`~/.claude/tasks/<sessionId>`), sorted by name; dot-files are bookkeeping.
    static func files(in dir: URL) -> [URL] {
        ((try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? [])
            .filter { $0.pathExtension == "json" && !$0.lastPathComponent.hasPrefix(".") }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    static func read(_ dir: URL) -> Todo? { read(files(in: dir)) }

    /// Progress of the task list in `files`, or nil when it is empty or all done.
    static func read(_ files: [URL]) -> Todo? {
        var items: [Item] = []
        for f in files {
            // A file can be mid-write: skip what fails to decode, the next refresh reads it again.
            guard let data = try? Data(contentsOf: f),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let status = o["status"] as? String,
                  ["pending", "in_progress", "completed"].contains(status)
            else { continue }
            let id = (o["id"] as? String).flatMap { Int($0) } ?? Int.max
            items.append(Item(id: id, name: f.lastPathComponent, subject: nonEmpty(o["subject"] as? String), activeForm: nonEmpty(o["activeForm"] as? String), status: status))
        }
        items.sort { ($0.id, $0.name) < ($1.id, $1.name) }
        let done = items.filter { $0.status == "completed" }.count
        guard done < items.count else { return nil }
        let current = items.first { $0.status == "in_progress" }.map { $0.activeForm ?? $0.subject }
            ?? items.first { $0.status == "pending" }?.subject
        return Todo(done: done, total: items.count, current: current)
    }

    private static func nonEmpty(_ s: String?) -> String? { s?.isEmpty == false ? s : nil }
}
