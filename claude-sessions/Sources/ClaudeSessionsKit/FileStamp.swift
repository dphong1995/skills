import Foundation

/// What identifies a file's content between two refreshes: inode (rename-based writes), size and nanosecond mtime.
struct FileStamp: Equatable {
    let inode: UInt64
    let size: Int
    let sec: Int
    let nsec: Int

    init?(_ path: String) {
        var st = stat()
        guard stat(path, &st) == 0 else { return nil }
        inode = UInt64(st.st_ino)
        size = Int(st.st_size)
        sec = Int(st.st_mtimespec.tv_sec)
        nsec = Int(st.st_mtimespec.tv_nsec)
    }
}
