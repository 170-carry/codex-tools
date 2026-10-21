//! 连接 Codex App 常驻 app-server 的最小 WebSocket-over-UDS 客户端。
//!
//! 为什么需要它：Codex App 运行时持有会话写锁，另起一个 `codex app-server`
//! 调用 `thread/resume` 会被拒绝（`already has an active writer`）。
//! App 自己监听 `~/.codex/app-server-control/app-server-control.sock`
//! （Windows 上是 AF_UNIX + WebSocket），通过它发起的 turn 才真正在 App 内执行。
//!
//! 这里手写协议是为了避免引入重量级依赖：
//! - Windows：WinSock `socket(AF_UNIX)` + `connect` + `SOCKADDR_UN`
//! - WebSocket：RFC 6455 握手 + 客户端掩码帧
//!
//! 连不上时调用方应回退到独立 app-server 方式。

use std::io::Read;
use std::io::Write;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// App 常驻实例的控制 socket 路径。
///
/// 不在这里检查文件是否存在：Windows 上的 AF_UNIX socket 是一个
/// 带 reparse point 的 0 字节文件，`Path::exists()` 可能返回 false。
/// 是否真的可用由 `app_control_reachable()` 通过实际握手判断。
pub(crate) fn control_socket_path() -> Option<std::path::PathBuf> {
    let codex_dir = crate::app_paths::codex_dir().ok()?;
    Some(
        codex_dir
            .join("app-server-control")
            .join("app-server-control.sock"),
    )
}

/// 控制 socket 是否存在（即 App 的常驻实例是否在监听）。
pub(crate) fn app_control_available() -> bool {
    control_socket_path()
        .map(|path| path.symlink_metadata().is_ok())
        .unwrap_or(false)
}

/// 实际探测控制通道能否握手成功，比只检查文件更可靠。
pub(crate) fn app_control_reachable() -> bool {
    ControlSocket::connect().is_ok()
}

/// 一个已建立的 WebSocket 连接，按 JSON-RPC 行文本收发。
pub(crate) struct ControlSocket {
    stream: SocketStream,
    buffer: Vec<u8>,
    next_id: u64,
}

impl ControlSocket {
    /// 连接 App 的控制 socket 并完成 WebSocket 握手。
    pub(crate) fn connect() -> Result<Self, String> {
        let path = control_socket_path()
            .ok_or_else(|| "Codex App 未运行（找不到 app-server-control.sock）".to_string())?;
        let stream = SocketStream::connect(&path, CONNECT_TIMEOUT)?;
        let mut socket = Self {
            stream,
            buffer: Vec::new(),
            next_id: 0,
        };
        socket.websocket_handshake()?;
        Ok(socket)
    }

    pub(crate) fn connect_or_start(_configured_path: Option<&str>) -> Result<Self, String> {
        Self::connect()
    }

    fn websocket_handshake(&mut self) -> Result<(), String> {
        let key = base64_encode(&handshake_nonce());
        let request = format!(
            "GET / HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        self.stream
            .write_all(request.as_bytes())
            .map_err(|error| format!("写入 WebSocket 握手失败: {error}"))?;
        let response = self
            .stream
            .read_until(b"\r\n\r\n", HANDSHAKE_TIMEOUT)
            .map_err(|error| format!("读取 WebSocket 握手响应失败: {error}"))?;
        let head = String::from_utf8_lossy(&response);
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!(
                "控制通道拒绝了 WebSocket 握手: {}",
                head.lines().next().unwrap_or("")
            ));
        }
        Ok(())
    }

    /// 发送一条 JSON-RPC 请求，等待同 id 的响应。
    pub(crate) fn call(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.send_text(&request.to_string())?;
        let deadline = Instant::now() + timeout;
        loop {
            let message = self.recv_text(deadline)?;
            let Ok(value) = serde_json::from_str::<Value>(&message) else {
                continue;
            };
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                let text = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("未知错误");
                return Err(text.to_string());
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    /// 发送一条通知（无 id）。
    pub(crate) fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.send_text(&message.to_string())
    }

    /// 读取下一条文本消息；期间跳过非文本帧。
    pub(crate) fn recv_text(&mut self, deadline: Instant) -> Result<String, String> {
        loop {
            let (opcode, payload) = self.recv_frame(deadline)?;
            match opcode {
                0x1 => {
                    return String::from_utf8(payload)
                        .map_err(|error| format!("控制通道返回了非 UTF-8 文本: {error}"))
                }
                0x8 => return Err("控制通道已关闭".to_string()),
                0x9 => {
                    // ping → pong
                    self.send_frame(0xA, &payload)?;
                }
                _ => continue,
            }
        }
    }

    fn send_text(&mut self, text: &str) -> Result<(), String> {
        self.send_frame(0x1, text.as_bytes())
    }

    fn send_frame(&mut self, opcode: u8, payload: &[u8]) -> Result<(), String> {
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x80 | opcode);
        let length = payload.len();
        if length < 126 {
            frame.push(0x80 | length as u8);
        } else if length < 65_536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
        // 客户端必须掩码；用时间戳做简单随机源即可，掩码只用于协议要求。
        let mask = mask_key();
        frame.extend_from_slice(&mask);
        for (index, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[index % 4]);
        }
        self.stream
            .write_all(&frame)
            .map_err(|error| format!("写入控制通道失败: {error}"))
    }

    fn recv_frame(&mut self, deadline: Instant) -> Result<(u8, Vec<u8>), String> {
        let header = self.read_exact_deadline(2, deadline)?;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let mut length = u64::from(header[1] & 0x7f);
        if length == 126 {
            let extended = self.read_exact_deadline(2, deadline)?;
            length = u64::from(u16::from_be_bytes([extended[0], extended[1]]));
        } else if length == 127 {
            let extended = self.read_exact_deadline(8, deadline)?;
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&extended);
            length = u64::from_be_bytes(bytes);
        }
        if length as usize > MAX_FRAME_BYTES {
            return Err("控制通道返回的帧过大".to_string());
        }
        let mask = if masked {
            Some(self.read_exact_deadline(4, deadline)?)
        } else {
            None
        };
        let mut payload = self.read_exact_deadline(length as usize, deadline)?;
        if let Some(mask) = mask {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        Ok((opcode, payload))
    }

    fn read_exact_deadline(
        &mut self,
        count: usize,
        deadline: Instant,
    ) -> Result<Vec<u8>, String> {
        let mut out = Vec::with_capacity(count);
        while out.len() < count {
            if !self.buffer.is_empty() {
                let take = (count - out.len()).min(self.buffer.len());
                out.extend(self.buffer.drain(..take));
                continue;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("等待控制通道数据超时".to_string());
            }
            self.stream
                .set_read_timeout(remaining)
                .map_err(|error| format!("设置读取超时失败: {error}"))?;
            let mut chunk = [0u8; 8192];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err("控制通道已关闭".to_string()),
                Ok(read) => self.buffer.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err("等待控制通道数据超时".to_string())
                }
                Err(error) => return Err(format!("读取控制通道失败: {error}")),
            }
        }
        Ok(out)
    }
}

fn mask_key() -> [u8; 4] {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.subsec_nanos())
        .unwrap_or(0);
    let mixed = nanos ^ (std::process::id().wrapping_mul(2_654_435_761));
    mixed.to_le_bytes()
}

fn handshake_nonce() -> [u8; 16] {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0) as u64;
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&nanos.to_le_bytes());
    bytes[8..].copy_from_slice(&(nanos ^ 0x9E37_79B9_7F4A_7C15).to_le_bytes());
    bytes
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// 跨平台的字节流封装；Windows 走 WinSock AF_UNIX。
struct SocketStream {
    inner: Inner,
}

enum Inner {
    #[cfg(windows)]
    Winsock(WinsockSocket),
}

impl SocketStream {
    fn connect(path: &std::path::Path, timeout: Duration) -> Result<Self, String> {
        #[cfg(windows)]
        {
            Ok(Self {
                inner: Inner::Winsock(WinsockSocket::connect(path, timeout)?),
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (path, timeout);
            Err("控制通道仅在 Windows 上实现".to_string())
        }
    }

    fn set_read_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        match &self.inner {
            #[cfg(windows)]
            Inner::Winsock(socket) => socket.set_read_timeout(timeout),
        }
    }

    fn read_until(&mut self, needle: &[u8], timeout: Duration) -> std::io::Result<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        let mut collected = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            if let Some(position) = find_subslice(&collected, needle) {
                return Ok(collected[..position + needle.len()].to_vec());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "握手超时",
                ));
            }
            self.set_read_timeout(remaining)?;
            let read = self.read(&mut chunk)?;
            if read == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "连接已关闭",
                ));
            }
            collected.extend_from_slice(&chunk[..read]);
        }
    }
}

impl Read for SocketStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            #[cfg(windows)]
            Inner::Winsock(socket) => socket.read(buf),
        }
    }
}

impl Write for SocketStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            #[cfg(windows)]
            Inner::Winsock(socket) => socket.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(windows)]
struct WinsockSocket {
    socket: windows::Win32::Networking::WinSock::SOCKET,
}

#[cfg(windows)]
impl WinsockSocket {
    fn connect(path: &std::path::Path, timeout: Duration) -> Result<Self, String> {
        use windows::Win32::Networking::WinSock::*;

        unsafe {
            let mut data = WSADATA::default();
            let startup = WSAStartup(0x0202, &mut data);
            if startup != 0 {
                return Err(format!("WSAStartup 失败: {startup}"));
            }
            // AF_UNIX 的 protocol 必须是 0；传 IPPROTO_TCP 会得到 WSAEPROTONOSUPPORT。
            let raw = socket(AF_UNIX as i32, SOCK_STREAM, 0)
            .map_err(|error| format!("创建 AF_UNIX socket 失败: {error}"))?;

            // SOCKADDR_UN: family(2) + path(108) + padding
            let path_text = path.to_string_lossy();
            let bytes = path_text.as_bytes();
            if bytes.len() >= 108 {
                let _ = closesocket(raw);
                return Err("控制 socket 路径过长".to_string());
            }
            let mut address = [0u8; 110];
            address[..2].copy_from_slice(&AF_UNIX.to_le_bytes());
            address[2..2 + bytes.len()].copy_from_slice(bytes);

            // 连接是阻塞的；用非阻塞 + 轮询实现超时。
            let mut mode: u32 = 1; // FIONBIO
            let _ = ioctlsocket(raw, mode as i32, &mut 1);
            mode = 0;
            let result = connect(
                raw,
                address.as_ptr() as *const SOCKADDR,
                address.len() as i32,
            );
            if result != 0 {
                let error = WSAGetLastError();
                if error != WSAEWOULDBLOCK {
                    let _ = closesocket(raw);
                    return Err(format!("连接控制 socket 失败: {error:?}"));
                }
                let deadline = Instant::now() + timeout;
                let mut connected = false;
                while Instant::now() < deadline {
                    let mut write_set = FD_SET {
                        fd_count: 1,
                        fd_array: [raw; 64],
                    };
                    let select_result = select(
                        0,
                        None,
                        Some(&mut write_set as *mut FD_SET),
                        None,
                        None,
                    );
                    if select_result > 0 {
                        connected = true;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                if !connected {
                    let _ = closesocket(raw);
                    return Err("连接控制 socket 超时".to_string());
                }
            }
            // 恢复阻塞模式，后续用 SO_RCVTIMEO 控制读超时。
            let mut block: u32 = 0;
            let _ = ioctlsocket(raw, 1, &mut block);
            Ok(Self { socket: raw })
        }
    }

    fn set_read_timeout(&self, timeout: Duration) -> std::io::Result<()> {
        use windows::Win32::Networking::WinSock::*;
        let millis = timeout.as_millis().min(u32::MAX as u128) as u32;
        let status = unsafe {
            setsockopt(
                self.socket,
                SOL_SOCKET,
                SO_RCVTIMEO,
                Some(&millis.to_le_bytes()),
            )
        };
        if status != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "设置接收超时失败",
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Read for WinsockSocket {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use windows::Win32::Networking::WinSock::*;
        let read = unsafe { recv(self.socket, buf, SEND_RECV_FLAGS(0)) };
        if read < 0 {
            let error = unsafe { WSAGetLastError() };
            if error == WSAETIMEDOUT {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "接收超时",
                ));
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("recv 失败: {error:?}"),
            ));
        }
        Ok(read as usize)
    }
}

#[cfg(windows)]
impl Write for WinsockSocket {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use windows::Win32::Networking::WinSock::*;
        let written = unsafe { send(self.socket, buf, SEND_RECV_FLAGS(0)) };
        if written < 0 {
            let error = unsafe { WSAGetLastError() };
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("send 失败: {error:?}"),
            ));
        }
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for WinsockSocket {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Networking::WinSock::closesocket(self.socket);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_reference_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn finds_subslice_for_handshake_terminator() {
        assert_eq!(find_subslice(b"HTTP/1.1 101\r\n\r\nrest", b"\r\n\r\n"), Some(12));
        assert_eq!(find_subslice(b"nope", b"\r\n\r\n"), None);
    }
}
