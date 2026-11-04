import Foundation

public enum Service: CaseIterable {
    case bitbucket, jira

    public var keychainService: String {
        switch self {
        case .bitbucket: "bitbucket-api-token"
        case .jira: "jira-api-token"
        }
    }

    public var displayName: String {
        switch self {
        case .bitbucket: "Bitbucket"
        case .jira: "Jira"
        }
    }

    public static let tokenPage = URL(string: "https://id.atlassian.com/manage-profile/security/api-tokens")!
}

public enum ConnectionState: Equatable {
    case checking, ok, rejected, unreachable

    /// The HTTP status carried by a client error; nil for a transport error or any other error.
    public static func httpCode(of error: Error) -> Int? {
        switch error {
        case let e as BitbucketError: if case .http(let c) = e { return c }
        case let e as JiraError: if case .http(let c) = e { return c }
        default: break
        }
        return nil
    }

    /// The state after one call; `error` is nil on success. Errors that say nothing about the connection keep `previous`.
    public static func after(_ error: Error?, previous: ConnectionState) -> ConnectionState {
        guard let error else { return .ok }
        guard let code = httpCode(of: error) else { return .unreachable }
        switch code {
        case 401: return .rejected
        case let c where c == 429 || c >= 500: return .unreachable
        default: return previous
        }
    }
}

public enum Credentials {
    public static func read(_ s: Service) -> (account: String, token: String)? {
        guard let user = account(s),
              let pw = runProcess("/usr/bin/security", ["find-generic-password", "-s", s.keychainService, "-w"], timeout: 2)
        else { return nil }
        let token = pw.trimmingCharacters(in: .whitespacesAndNewlines)
        return token.isEmpty ? nil : (user, token)
    }

    /// The account of the Keychain item, never the token.
    public static func account(_ s: Service) -> String? {
        runProcess("/usr/bin/security", ["find-generic-password", "-s", s.keychainService], timeout: 2)
            .flatMap(account(fromAttributes:))
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

    /// False when a value holds a quote, backslash or newline, which `security -i` cannot take on one line.
    public static func isSafe(_ v: String) -> Bool {
        !v.contains { $0 == "\"" || $0 == "\\" || $0.isNewline }
    }

    /// A `security -i` command line, or nil when a value is not safe. `-U` updates an item of the same account in place.
    static func addScript(service: String, account: String, token: String) -> String? {
        guard [service, account, token].allSatisfy(isSafe) else { return nil }
        return "add-generic-password -U -s \"\(service)\" -a \"\(account)\" -w \"\(token)\"\n"
    }

    /// Adds or updates the item first, checks it reads back with the same account and token, and only then removes items
    /// of other accounts, so a failed add keeps the working token. The token travels on stdin only, never in an argument list.
    public static func write(_ s: Service, account: String, token: String) -> Bool {
        guard let script = addScript(service: s.keychainService, account: account, token: token),
              runProcess("/usr/bin/security", ["-i"], timeout: 5, input: Data(script.utf8)) != nil,
              runProcess("/usr/bin/security", ["find-generic-password", "-s", s.keychainService, "-a", account, "-w"], timeout: 2)?
                  .trimmingCharacters(in: .whitespacesAndNewlines) == token
        else { return false }
        for _ in 0..<10 {
            guard let other = Self.account(s), other != account,
                  runProcess("/usr/bin/security", ["delete-generic-password", "-s", s.keychainService, "-a", other], timeout: 2) != nil
            else { break }
        }
        return true
    }
}
