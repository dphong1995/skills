import Foundation

public enum BitbucketError: Error, Equatable { case http(Int), transport }

public protocol BitbucketAPI: AnyObject {
    /// `path` begins with `/2.0/` and includes the query. Blocking: called only on the monitor's queue.
    func get(_ path: String) throws -> Data
}

public final class BitbucketClient: BitbucketAPI {
    static let base = "https://api.bitbucket.org"

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

    /// Uses the given credentials until `reload()`; no Keychain read.
    public init(account: String, token: String) {
        auth = Self.header(account: account, token: token)
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    public func reload() {
        let a = Self.readAuth()
        lock.lock(); auth = a; lock.unlock()
    }

    private static func header(account: String, token: String) -> String {
        "Basic " + Data("\(account):\(token)".utf8).base64EncodedString()
    }

    private static func readAuth() -> String? {
        Credentials.read(.bitbucket).map { header(account: $0.account, token: $0.token) }
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
