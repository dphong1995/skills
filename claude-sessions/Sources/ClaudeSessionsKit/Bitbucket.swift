import Foundation

public enum BitbucketError: Error, Equatable { case http(Int), transport }

public protocol BitbucketAPI: AnyObject {
    /// `path` begins with `/2.0/` and includes the query. Blocking: called only on the monitor's queue.
    func get(_ path: String) throws -> Data
}

/// Runs a program and returns its stdout, or nil when it cannot start, times out, or exits non-zero.
func runProcess(_ path: String, _ args: [String], timeout: TimeInterval) -> String? {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: path)
    p.arguments = args
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = FileHandle.nullDevice
    guard (try? p.run()) != nil else { return nil }
    DispatchQueue.global().asyncAfter(deadline: .now() + timeout) { if p.isRunning { p.terminate() } }
    let data = pipe.fileHandleForReading.readDataToEndOfFile()
    p.waitUntilExit()
    guard p.terminationStatus == 0 else { return nil }
    return String(decoding: data, as: UTF8.self)
}

public final class BitbucketClient: BitbucketAPI {
    static let base = "https://api.bitbucket.org"
    private static let service = "bitbucket-api-token"

    private let lock = NSLock()
    private var auth: String?
    private let session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.timeoutIntervalForRequest = 15
        c.timeoutIntervalForResource = 15
        return URLSession(configuration: c)
    }()

    /// Nil when the Keychain item or its account is missing.
    public init?() {
        guard let a = Self.readAuth() else { return nil }
        auth = a
    }

    /// The value of the `"acct"` attribute line of `security find-generic-password` output.
    static func account(fromAttributes out: String) -> String? {
        for raw in out.split(separator: "\n") where raw.contains("\"acct\"<blob>=") {
            let line = raw.trimmingCharacters(in: .whitespaces)
            guard line.hasSuffix("\""), let eq = line.range(of: "<blob>=\"") else { return nil }
            return String(line[eq.upperBound..<line.index(before: line.endIndex)])
        }
        return nil
    }

    private static func readAuth() -> String? {
        guard let attrs = runProcess("/usr/bin/security", ["find-generic-password", "-s", service], timeout: 2),
              let user = account(fromAttributes: attrs),
              let pw = runProcess("/usr/bin/security", ["find-generic-password", "-s", service, "-w"], timeout: 2)
        else { return nil }
        let token = pw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !token.isEmpty else { return nil }
        return "Basic " + Data("\(user):\(token)".utf8).base64EncodedString()
    }

    private func currentAuth() -> String? {
        lock.lock(); defer { lock.unlock() }
        if auth == nil { auth = Self.readAuth() }
        return auth
    }

    private final class Box: @unchecked Sendable {
        var data: Data?
        var response: URLResponse?
    }

    public func get(_ path: String) throws -> Data {
        guard let header = currentAuth(), let url = URL(string: Self.base + path) else { throw BitbucketError.transport }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.setValue(header, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        let box = Box()
        let sem = DispatchSemaphore(value: 0)
        let task = session.dataTask(with: req) { data, response, _ in
            box.data = data
            box.response = response
            sem.signal()
        }
        task.resume()
        if sem.wait(timeout: .now() + 20) == .timedOut {
            task.cancel()
            throw BitbucketError.transport
        }
        guard let http = box.response as? HTTPURLResponse, let data = box.data else { throw BitbucketError.transport }
        if http.statusCode == 401 {
            lock.lock(); auth = nil; lock.unlock()
        }
        guard (200..<300).contains(http.statusCode) else { throw BitbucketError.http(http.statusCode) }
        return data
    }
}
