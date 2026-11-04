import Foundation

public enum JiraError: Error, Equatable { case http(Int), transport }

public protocol JiraAPI: AnyObject {
    /// Blocking: called only on the monitor's queue or a background queue.
    func bulkFetch(host: String, keys: [String]) throws -> Data
    /// Blocking: called only on a background queue.
    func myself(host: String) throws -> Data
}

public struct JiraIssue: Equatable {
    public let title: String?
    public let status: String?
    public init(title: String?, status: String?) { self.title = title; self.status = status }
}

enum Jira {
    /// Every requested key is in the result; one `bulkfetch` did not return gets no title and no status. Nil when `data` is not JSON.
    static func issues(from data: Data, requested: [String]) -> [String: JiraIssue]? {
        guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        var out = Dictionary(uniqueKeysWithValues: Set(requested).map { ($0, JiraIssue(title: nil, status: nil)) })
        for issue in o["issues"] as? [[String: Any]] ?? [] {
            guard let key = issue["key"] as? String, out[key] != nil else { continue }
            let fields = issue["fields"] as? [String: Any]
            out[key] = JiraIssue(title: fields?["summary"] as? String,
                                 status: (fields?["status"] as? [String: Any])?["name"] as? String)
        }
        return out
    }
}

public final class JiraClient: JiraAPI {
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
        Credentials.read(.jira).map { header(account: $0.account, token: $0.token) }
    }

    private func currentAuth() -> String? {
        lock.lock(); defer { lock.unlock() }
        if auth == nil { auth = Self.readAuth() }
        return auth
    }

    static func request(host: String, keys: [String], auth: String) -> URLRequest? {
        guard var req = request(host: host, path: "/rest/api/3/issue/bulkfetch", auth: auth),
              let body = try? JSONSerialization.data(withJSONObject: ["issueIdsOrKeys": keys, "fields": ["summary", "status"]])
        else { return nil }
        req.httpMethod = "POST"
        req.httpBody = body
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return req
    }

    private static func request(host: String, path: String, auth: String) -> URLRequest? {
        guard let url = URL(string: "https://\(host)\(path)"), url.host == host else { return nil }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.setValue(auth, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        return req
    }

    private final class Box: @unchecked Sendable {
        var data: Data?
        var response: URLResponse?
    }

    public func bulkFetch(host: String, keys: [String]) throws -> Data {
        guard let header = currentAuth(), let req = Self.request(host: host, keys: keys, auth: header) else { throw JiraError.transport }
        return try send(req)
    }

    public func myself(host: String) throws -> Data {
        guard let header = currentAuth(), let req = Self.request(host: host, path: "/rest/api/3/myself", auth: header) else { throw JiraError.transport }
        return try send(req)
    }

    private func send(_ req: URLRequest) throws -> Data {
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
            throw JiraError.transport
        }
        guard let http = box.response as? HTTPURLResponse, let data = box.data else { throw JiraError.transport }
        if http.statusCode == 401 {
            lock.lock(); auth = nil; lock.unlock()
        }
        guard (200..<300).contains(http.statusCode) else { throw JiraError.http(http.statusCode) }
        return data
    }
}
