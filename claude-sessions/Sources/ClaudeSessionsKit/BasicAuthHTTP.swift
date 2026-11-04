import Foundation

/// Shared by `BitbucketClient` and `JiraClient`: a cached Basic header for one Keychain service and a blocking request.
final class BasicAuthHTTP {
    enum Failure: Error { case http(Int), transport }

    private let service: Service
    private let lock = NSLock()
    private var auth: String?
    private let session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.timeoutIntervalForRequest = 15
        c.timeoutIntervalForResource = 15
        return URLSession(configuration: c)
    }()

    private final class Box: @unchecked Sendable {
        var data: Data?
        var response: URLResponse?
    }

    /// Nil when the Keychain item or its account is missing.
    init?(service: Service) {
        self.service = service
        guard let a = Self.readAuth(service) else { return nil }
        auth = a
    }

    /// Uses the given credentials until `reload()`; no Keychain read.
    init(service: Service, account: String, token: String) {
        self.service = service
        auth = Self.header(account: account, token: token)
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    func reload() {
        let a = Self.readAuth(service)
        lock.lock(); auth = a; lock.unlock()
    }

    private static func header(account: String, token: String) -> String {
        "Basic " + Data("\(account):\(token)".utf8).base64EncodedString()
    }

    private static func readAuth(_ service: Service) -> String? {
        Credentials.read(service).map { header(account: $0.account, token: $0.token) }
    }

    /// The cached header; re-read from the Keychain after a 401 cleared it.
    func currentAuth() -> String? {
        lock.lock(); defer { lock.unlock() }
        if auth == nil { auth = Self.readAuth(service) }
        return auth
    }

    /// A 401 forgets the cached header so the next call reads the Keychain again.
    func send(_ req: URLRequest) throws -> Data {
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
            throw Failure.transport
        }
        guard let http = box.response as? HTTPURLResponse, let data = box.data else { throw Failure.transport }
        if http.statusCode == 401 {
            lock.lock(); auth = nil; lock.unlock()
        }
        guard (200..<300).contains(http.statusCode) else { throw Failure.http(http.statusCode) }
        return data
    }
}
