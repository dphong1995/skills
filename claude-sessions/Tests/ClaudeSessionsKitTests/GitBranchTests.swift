import Foundation
import Testing
@testable import ClaudeSessionsKit

private func write(_ url: URL, _ text: String) {
    try! FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try! text.write(to: url, atomically: true, encoding: .utf8)
}

private func repo(_ head: String) -> URL {
    let d = makeTempDir()
    write(d.appendingPathComponent(".git/HEAD"), head)
    return d
}

@Test func gitBranchPlainRepo() {
    #expect(GitBranch.current(in: repo("ref: refs/heads/bugfix/T6IR-31\n").path) == "bugfix/T6IR-31")
}

@Test func gitBranchFromSubdirectory() {
    let d = repo("ref: refs/heads/bugfix/T6IR-31\n")
    let sub = d.appendingPathComponent("a/b")
    try! FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
    #expect(GitBranch.current(in: sub.path) == "bugfix/T6IR-31")
}

@Test func gitBranchWorktreeRelativeGitdir() {
    let root = makeTempDir()
    write(root.appendingPathComponent("main/.git/worktrees/w/HEAD"), "ref: refs/heads/feature/x\n")
    write(root.appendingPathComponent("w/.git"), "gitdir: ../main/.git/worktrees/w\n")
    #expect(GitBranch.current(in: root.appendingPathComponent("w").path) == "feature/x")
}

@Test func gitBranchWorktreeSubdirRelativeGitdir() {
    let root = makeTempDir()
    write(root.appendingPathComponent("main/.git/worktrees/w/HEAD"), "ref: refs/heads/feature/x\n")
    write(root.appendingPathComponent("w/.git"), "gitdir: ../main/.git/worktrees/w\n")
    let sub = root.appendingPathComponent("w/src")
    try! FileManager.default.createDirectory(at: sub, withIntermediateDirectories: true)
    #expect(GitBranch.current(in: sub.path) == "feature/x")
}

@Test func gitBranchWorktreeAbsoluteGitdir() {
    let root = makeTempDir()
    let gitdir = root.appendingPathComponent("main/.git/worktrees/w")
    write(gitdir.appendingPathComponent("HEAD"), "ref: refs/heads/feature/x\n")
    write(root.appendingPathComponent("w/.git"), "gitdir: \(gitdir.path)\n")
    #expect(GitBranch.current(in: root.appendingPathComponent("w").path) == "feature/x")
}

@Test func gitBranchDetachedIsNil() {
    #expect(GitBranch.current(in: repo(String(repeating: "a", count: 40) + "\n").path) == nil)
}

@Test func gitBranchNoRepoIsNil() {
    #expect(GitBranch.current(in: makeTempDir().path) == nil)
}

@Test func cachedBranchSeesCheckout() throws {
    let d = repo("ref: refs/heads/main\n")
    let reader = GitBranchReader()
    #expect(reader.current(in: d.path) == "main")
    #expect(reader.current(in: d.path) == "main")
    #expect(reader.headReads == 1)
    let head = d.appendingPathComponent(".git/HEAD")
    try "ref: refs/heads/feat\n".write(to: head, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(5)], ofItemAtPath: head.path)
    #expect(reader.current(in: d.path) == "feat")
    try FileManager.default.removeItem(at: d.appendingPathComponent(".git"))
    #expect(reader.current(in: d.path) == nil)
}
