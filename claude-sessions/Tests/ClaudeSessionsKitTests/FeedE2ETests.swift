import Foundation
import Testing
@testable import ClaudeSessionsKit

/// Real cmux, real Claude (haiku, short prompts): by hand only, `CLAUDE_SESSIONS_E2E=1 swift test --filter FeedE2ETests`.
/// Each shape opens its own throwaway workspace and closes it again; only items of that workspace's own session id are touched.
private let e2eEnabled = ProcessInfo.processInfo.environment["CLAUDE_SESSIONS_E2E"] == "1"
private let cmux = "/Applications/cmux.app/Contents/Resources/bin/cmux"

private struct Shape {
    let name: String
    let prompt: String
    let answers: [[String]]
    let other: [Int: String]
    /// What the AskUserQuestion `tool_result` must contain: each answer value, not the prompt text (haiku rewords prompts).
    let expected: [String]
}

private let shapes = [
    Shape(name: "single", prompt: "Call AskUserQuestion once with exactly one question 'Pick a color' and options Red, Blue, Green, then reply with the answer only.",
          answers: [["Blue"]], other: [:], expected: ["=\"Blue\""]),
    Shape(name: "multi", prompt: "Call AskUserQuestion once with exactly one question 'Pick colors' with multiSelect true and options Red, Blue, Green, then reply with the answer only.",
          answers: [["Red", "Blue"]], other: [:], expected: ["=\"Red, Blue\""]),
    Shape(name: "two", prompt: "Call AskUserQuestion once with exactly two questions: 'Pick a color' (options Red, Blue) and 'Pick a size' (options Small, Large), then reply with both answers only.",
          answers: [["Red"], ["Large"]], other: [:], expected: ["=\"Red\"", "=\"Large\""]),
    Shape(name: "other", prompt: "Call AskUserQuestion once with exactly one question 'Pick a color' and options Red, Blue, Green, then reply with the answer only.",
          answers: [[]], other: [0: "Purple"], expected: ["=\"Purple\""]),
]

/// The text of the AskUserQuestion `tool_result` line (found by its `tool_use_id`); empty until Claude has it.
private func askResult(_ id: String, cwd: String) -> String {
    let projects = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/projects")
    guard let url = Transcript.find(sessionId: id, cwd: cwd, projects: projects),
          let text = try? String(contentsOf: url, encoding: .utf8) else { return "" }
    let lines = text.split(separator: "\n").map(String.init)
    let usePattern = #""id":"(toolu_[A-Za-z0-9]+)","name":"AskUserQuestion""#
    guard let use = lines.first(where: { $0.range(of: usePattern, options: .regularExpression) != nil }),
          let range = use.range(of: usePattern, options: .regularExpression),
          let toolId = use[range].split(separator: "\"").dropFirst(2).first.map(String.init) else { return "" }
    // Raw JSONL escapes the quotes inside the tool_result text.
    return lines.first { $0.contains("\"tool_use_id\":\"\(toolId)\"") }
        .map { $0.replacingOccurrences(of: "\\\"", with: "\"") } ?? ""
}

private func wait(timeout: TimeInterval, _ done: () -> Bool) -> Bool {
    let end = Date().addingTimeInterval(timeout)
    while Date() < end {
        if done() { return true }
        Thread.sleep(forTimeInterval: 1)
    }
    return done()
}

@Suite(.serialized, .enabled(if: e2eEnabled))
struct FeedE2ETests {
    @Test(arguments: shapes.map(\.name)) func answerReachesClaude(_ name: String) throws {
        let shape = shapes.first { $0.name == name }!
        let id = UUID().uuidString.lowercased()
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let cwd = root.appendingPathComponent(".build/e2e/\(name)")
        try FileManager.default.createDirectory(at: cwd, withIntermediateDirectories: true)
        let command = "claude --model haiku --session-id \(id) \"\(shape.prompt)\""
        let made = runProcess(cmux, ["new-workspace", "--name", "e2e-\(name)", "--cwd", cwd.path, "--focus", "false", "--command", command], timeout: 20) ?? ""
        let ref = made.split(separator: "\n").compactMap { l in l.hasPrefix("OK ") ? l.split(separator: " ").dropFirst().first.map(String.init) : nil }.first
        let workspace = try #require(ref, "cmux new-workspace gave: \(made)")
        defer { _ = runProcess(cmux, ["close-workspace", "--workspace", workspace, "--force"], timeout: 20) }

        let socket = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/state/cmux/last-socket-path")
        let transport = try #require(SocketFeedTransport(pathFile: socket))
        let monitor = FeedMonitor(transport: transport, queue: nil)
        var question: FeedQuestion?
        #expect(wait(timeout: 60) {
            monitor.update(active: true, at: Date())
            question = monitor.takeSnapshot().first { $0.sessionId == id }
            return question != nil
        }, "no Feed question for session \(id)")
        guard let question else { return }
        #expect(Feed.supports(question))

        var sent: Bool?
        monitor.reply(question, answers: shape.answers, other: shape.other) { sent = $0 }
        #expect(sent == true)
        #expect(wait(timeout: 60) { let r = askResult(id, cwd: cwd.path); return shape.expected.allSatisfy(r.contains) },
                "Claude never saw \(shape.expected)")
    }
}
