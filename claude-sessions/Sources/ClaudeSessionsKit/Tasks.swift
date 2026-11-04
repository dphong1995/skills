import Foundation

struct Todo: Equatable {
    let done: Int
    let total: Int
    let current: String?
}

enum Tasks {
    private struct Item {
        let id: Int
        let subject: String
        let activeForm: String?
        let status: String
    }

    /// Progress of the task list in `dir` (`~/.claude/tasks/<sessionId>`), or nil when it is missing, empty or all done.
    static func read(_ dir: URL) -> Todo? {
        let files = (try? FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)) ?? []
        var items: [Item] = []
        for f in files where f.pathExtension == "json" && !f.lastPathComponent.hasPrefix(".") {
            // A file can be mid-write: skip what fails to decode, the next refresh reads it again.
            guard let data = try? Data(contentsOf: f),
                  let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let id = (o["id"] as? String).flatMap({ Int($0) }),
                  let status = o["status"] as? String,
                  ["pending", "in_progress", "completed"].contains(status)
            else { continue }
            items.append(Item(id: id, subject: o["subject"] as? String ?? "", activeForm: o["activeForm"] as? String, status: status))
        }
        items.sort { $0.id < $1.id }
        let done = items.filter { $0.status == "completed" }.count
        guard done < items.count else { return nil }
        let current = items.first { $0.status == "in_progress" }.map { $0.activeForm ?? $0.subject }
            ?? items.first { $0.status == "pending" }?.subject
        return Todo(done: done, total: items.count, current: current)
    }
}
