use std::collections::VecDeque;
use std::ffi::CStr;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};
use std::sync::Mutex;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
struct EncodedPacket {
    codec: String,
    data: Vec<u8>,
    key: bool,
}

const STATUS_IDLE: i32 = 0;
const STATUS_READY: i32 = 1;
const STATUS_CONNECTING: i32 = 2;
const STATUS_FAILED: i32 = 3;

static STATUS: AtomicI32 = AtomicI32::new(STATUS_IDLE);
static RUNNING: AtomicBool = AtomicBool::new(false);
static CONNECT_GEN: AtomicI32 = AtomicI32::new(0);
static CHECK_GEN: AtomicI32 = AtomicI32::new(0);
static CHECK_RESULT: Mutex<String> = Mutex::new(String::new());
static FRAME_COUNT: AtomicI32 = AtomicI32::new(0);
static SERVER: Mutex<String> = Mutex::new(String::new());
static RELAY: Mutex<String> = Mutex::new(String::new());
static KEY: Mutex<String> = Mutex::new(String::new());
static FORCE_RELAY: AtomicBool = AtomicBool::new(false);
static DIRECT_IP_SESSION: AtomicBool = AtomicBool::new(false);
/// High-quality profile (same knobs as IP direct). Set by direct-ip or UI「高清」.
static HD_PROFILE: AtomicBool = AtomicBool::new(false);
static LOGS: Mutex<String> = Mutex::new(String::new());
static ERROR: Mutex<String> = Mutex::new(String::new());
static LAST_CODEC: Mutex<String> = Mutex::new(String::new());
static LAST_PACKET: Mutex<Option<EncodedPacket>> = Mutex::new(None);
static LAST_KEY_PACKET: Mutex<Option<EncodedPacket>> = Mutex::new(None);
static PACKET_Q: Mutex<VecDeque<EncodedPacket>> = Mutex::new(VecDeque::new());
static LAST_COPIED_KEY: AtomicI32 = AtomicI32::new(0);
static DISCARD_Q: AtomicBool = AtomicBool::new(false);
const MAX_PACKET_Q: usize = 120;
const MAX_PACKET_Q_HD: usize = 240;
static PACKET_SEQ: AtomicI32 = AtomicI32::new(0);
static KEY_SEQ: AtomicI32 = AtomicI32::new(0);
static GOT_KEYFRAME: AtomicBool = AtomicBool::new(false);
static KEYFRAME_ASKS: AtomicI32 = AtomicI32::new(0);
static DISPLAY_W: AtomicI32 = AtomicI32::new(0);
static DISPLAY_H: AtomicI32 = AtomicI32::new(0);
static SESSION_MSG_LOGGED: AtomicI32 = AtomicI32::new(0);
static LAST_VIDEO_MS: AtomicI64 = AtomicI64::new(0);
static LAST_REFRESH_ASK_MS: AtomicI64 = AtomicI64::new(0);
static LAST_AUTO_FPS: AtomicI32 = AtomicI32::new(0);
static TEST_DELAY_ECHOED: AtomicI32 = AtomicI32::new(0);
static OUTGOING: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
static LAST_MOUSE: Mutex<(i32, i32)> = Mutex::new((0, 0));

fn append_log(line: &str) {
    if let Ok(mut logs) = LOGS.lock() {
        logs.push_str(line);
        logs.push('\n');
        if logs.len() > 4000 {
            let drop_len = logs.len() - 3000;
            logs.drain(..drop_len);
        }
    }
}

fn set_error(message: &str) {
    if let Ok(mut err) = ERROR.lock() {
        *err = message.to_string();
    }
}

fn copy_to_buf(src: &str, buf: *mut c_char, len: c_int) -> c_int {
    if buf.is_null() || len <= 0 {
        return 0;
    }
    let max = (len as usize).saturating_sub(1);
    let bytes = src.as_bytes();
    let n = bytes.len().min(max);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n);
        *buf.add(n) = 0;
    }
    n as c_int
}

fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

fn is_official_placeholder(raw: &str) -> bool {
    let lower = raw.trim().to_ascii_lowercase();
    lower.is_empty()
        || lower.contains("rustdesk.com/api")
        || lower == "router.rustdesk.com"
        || lower == "router.rustdesk.com:21116"
        || lower == "rs-ny.rustdesk.com"
        || lower == "rs-ny.rustdesk.com:21116"
        || lower == "rs-sg.rustdesk.com"
        || lower == "rs-sg.rustdesk.com:21116"
}

fn b64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 256] = &{
        let mut t = [0xffu8; 256];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = 26 + i;
            i += 1;
        }
        i = 0;
        while i < 10 {
            t[(b'0' + i) as usize] = 52 + i;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    if cleaned.is_empty() || cleaned.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        let mut vals = [0u8; 4];
        for (i, b) in chunk.iter().enumerate() {
            let v = TABLE[*b as usize];
            if v == 0xff {
                return None;
            }
            vals[i] = v;
        }
        out.push((vals[0] << 2) | (vals[1] >> 4));
        if chunk.len() > 2 {
            out.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if chunk.len() > 3 {
            out.push((vals[2] << 6) | vals[3]);
        }
    }
    Some(out)
}

fn b64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn normalize_licence_key(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        s = s[1..s.len() - 1].to_string();
    }
    if let Some(bytes) = b64_decode(&s) {
        if bytes.len() == 64 {
            return b64_encode(&bytes[32..]);
        }
    }
    s
}

fn key_debug(key: &str) -> String {
    if key.is_empty() {
        return "empty".to_string();
    }
    let tail = if key.len() >= 4 { &key[key.len() - 4..] } else { key };
    format!("len={} tail={}", key.len(), tail)
}

fn normalize_server(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if is_official_placeholder(trimmed) {
        return Err("missing self-hosted id server".to_string());
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let hostport = without_scheme.split('/').next().unwrap_or("").trim();
    if hostport.is_empty() || is_official_placeholder(hostport) {
        return Err("missing self-hosted id server".to_string());
    }
    if hostport.contains(':') {
        Ok(hostport.to_string())
    } else {
        Ok(format!("{hostport}:21116"))
    }
}

/// True when peer id is an IP / host:port for RustDesk direct IP access (default port 21118).
fn is_direct_ip_peer(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() || s.starts_with("DEMO") || s.starts_with("TEST") {
        return false;
    }
    // [ipv6]:port
    if s.starts_with('[') {
        return s.contains("]:");
    }
    // ipv4 or ipv4:port
    let host = s.split(':').next().unwrap_or(s);
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok()) {
        if s.contains(':') {
            let port = s.rsplit(':').next().unwrap_or("");
            return port.parse::<u16>().is_ok();
        }
        return true;
    }
    // hostname:port (not a pure numeric RustDesk id)
    if let Some((h, p)) = s.rsplit_once(':') {
        if !h.is_empty() && p.parse::<u16>().is_ok() && !h.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn normalize_direct_addr(raw: &str) -> String {
    let s = raw.trim();
    if s.starts_with('[') {
        return s.to_string();
    }
    if s.contains(':') {
        // ipv4:port or host:port
        return s.to_string();
    }
    // bare ipv4 → default direct-access port (RENDEZVOUS 21116 + 2)
    format!("{s}:21118")
}

fn encode_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn encode_tag(out: &mut Vec<u8>, field: u32, wire: u32) {
    encode_varint(out, ((field << 3) | wire) as u64);
}

fn encode_bytes(out: &mut Vec<u8>, field: u32, data: &[u8]) {
    encode_tag(out, field, 2);
    encode_varint(out, data.len() as u64);
    out.extend_from_slice(data);
}

fn encode_string(out: &mut Vec<u8>, field: u32, value: &str) {
    encode_bytes(out, field, value.as_bytes());
}

fn encode_varint_field(out: &mut Vec<u8>, field: u32, value: u64) {
    encode_tag(out, field, 0);
    encode_varint(out, value);
}

fn encode_sint32_field(out: &mut Vec<u8>, field: u32, value: i32) {
    let zigzag = ((value << 1) ^ (value >> 31)) as u32 as u64;
    encode_varint_field(out, field, zigzag);
}

fn enqueue_msg_front(payload: Vec<u8>) {
    if let Ok(mut queue) = OUTGOING.lock() {
        queue.insert(0, frame_message(&payload));
    }
}

fn enqueue_msg(payload: Vec<u8>) {
    if let Ok(mut queue) = OUTGOING.lock() {
        // 输入事件优先：队列过长时丢掉旧的刷新类大包，避免密码/按键被冲掉
        if queue.len() > 160 {
            let drop_n = queue.len() - 80;
            queue.drain(..drop_n);
        }
        queue.push(frame_message(&payload));
    }
}

fn flush_outgoing(stream: &mut TcpStream) {
    let messages = match OUTGOING.lock() {
        Ok(mut queue) => std::mem::take(&mut *queue),
        Err(_) => return,
    };
    for message in messages {
        if stream.write_all(&message).is_err() {
            break;
        }
    }
}

fn build_mouse_event(mask: i32, x: i32, y: i32) -> Vec<u8> {
    let mut inner = Vec::new();
    encode_varint_field(&mut inner, 1, mask as u64);
    encode_sint32_field(&mut inner, 2, x);
    encode_sint32_field(&mut inner, 3, y);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 10, &inner);
    msg
}

fn build_key_chr(chr: u32, down: bool, press: bool) -> Vec<u8> {
    build_key_chr_mod(chr, &[], down, press)
}

fn build_key_chr_mod(chr: u32, modifiers: &[u32], down: bool, press: bool) -> Vec<u8> {
    let mut inner = Vec::new();
    if down {
        encode_varint_field(&mut inner, 1, 1);
    }
    if press {
        encode_varint_field(&mut inner, 2, 1);
    }
    encode_varint_field(&mut inner, 4, chr as u64);
    for &m in modifiers {
        encode_varint_field(&mut inner, 8, m as u64);
    }
    encode_varint_field(&mut inner, 9, 0);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 15, &inner);
    msg
}

/// Legacy + seq：Windows 密码框/安全桌面可用（与 RustDesk input_os_password 一致）
fn build_key_seq(text: &str) -> Vec<u8> {
    let mut inner = Vec::new();
    encode_varint_field(&mut inner, 2, 1); // press
    encode_string(&mut inner, 6, text);
    encode_varint_field(&mut inner, 9, 0); // KeyboardMode::Legacy
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 15, &inner);
    msg
}

fn build_key_control(code: u32, down: bool, press: bool) -> Vec<u8> {
    build_key_control_mod(code, &[], down, press)
}

fn build_key_control_mod(code: u32, modifiers: &[u32], down: bool, press: bool) -> Vec<u8> {
    let mut inner = Vec::new();
    if down {
        encode_varint_field(&mut inner, 1, 1);
    }
    if press {
        encode_varint_field(&mut inner, 2, 1);
    }
    encode_varint_field(&mut inner, 3, code as u64);
    for &m in modifiers {
        encode_varint_field(&mut inner, 8, m as u64);
    }
    encode_varint_field(&mut inner, 9, 0);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 15, &inner);
    msg
}

/// 松开 Ctrl/Shift/Alt/Meta，避免修饰键卡住
fn release_modifiers() {
    for code in [4u32, 29, 1, 23] {
        // Control=4, Shift=29, Alt=1, Meta=23
        enqueue_msg(build_key_control(code, false, false));
    }
}

fn read_varint(data: &[u8], index: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0;
    while *index < data.len() {
        let byte = data[*index];
        *index += 1;
        result |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

enum ProtoValue<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn is_timeout(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::TimedOut || err.kind() == ErrorKind::WouldBlock
}

fn read_all(stream: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> Result<(), String> {
    let mut got = 0;
    while got < buf.len() {
        if Instant::now() >= deadline {
            return Err(format!("payload timeout got={got} need={}", buf.len()));
        }
        match stream.read(&mut buf[got..]) {
            Ok(0) => return Err("eof".into()),
            Ok(n) => got += n,
            Err(e) if is_timeout(&e) => {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn next_field<'a>(data: &'a [u8], index: &mut usize) -> Option<(u32, ProtoValue<'a>)> {
    let tag = read_varint(data, index)?;
    let field = (tag >> 3) as u32;
    let wire = (tag & 7) as u32;
    match wire {
        0 => Some((field, ProtoValue::Varint(read_varint(data, index)?))),
        1 => {
            if *index + 8 > data.len() {
                return None;
            }
            *index += 8;
            Some((field, ProtoValue::Bytes(&data[*index - 8..*index])))
        }
        2 => {
            let len = read_varint(data, index)? as usize;
            if *index + len > data.len() {
                return None;
            }
            let start = *index;
            *index += len;
            Some((field, ProtoValue::Bytes(&data[start..*index])))
        }
        5 => {
            if *index + 4 > data.len() {
                return None;
            }
            *index += 4;
            Some((field, ProtoValue::Bytes(&data[*index - 4..*index])))
        }
        _ => None,
    }
}

fn frame_message(payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut out = Vec::with_capacity(len + 4);
    if len <= 0x3F {
        out.push((len << 2) as u8);
    } else if len <= 0x3FFF {
        out.extend_from_slice(&(((len << 2) as u16) | 0x1).to_le_bytes());
    } else {
        let encoded = ((len as u32) << 2) | 0x2;
        out.extend_from_slice(&encoded.to_le_bytes()[..3]);
    }
    out.extend_from_slice(payload);
    out
}

fn read_framed(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut first = [0u8; 1];
    match stream.read(&mut first) {
        Ok(0) => return Err("eof".into()),
        Ok(_) => {}
        Err(e) if is_timeout(&e) => return Err("idle".into()),
        Err(e) => return Err(format!("read header: {e}")),
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(8)));
    let deadline = Instant::now() + Duration::from_secs(8);
    let head_len = ((first[0] & 0x3) + 1) as usize;
    let mut header = vec![first[0]];
    if head_len > 1 {
        header.resize(head_len, 0);
        read_all(stream, &mut header[1..], deadline).map_err(|e| format!("read header rest: {e}"))?;
    }
    let mut packed = header[0] as usize;
    if head_len > 1 {
        packed |= (header[1] as usize) << 8;
    }
    if head_len > 2 {
        packed |= (header[2] as usize) << 16;
    }
    if head_len > 3 {
        packed |= (header[3] as usize) << 24;
    }
    let msg_len = packed >> 2;
    if msg_len == 0 || msg_len > 64 * 1024 * 1024 {
        return Err(format!("bad frame len {msg_len}"));
    }
    if msg_len > 200_000 {
        append_log(&format!("big frame {msg_len}"));
    }
    if msg_len > 8 * 1024 * 1024 {
        let mut left = msg_len;
        let mut buf = [0u8; 8192];
        while left > 0 {
            let n = left.min(buf.len());
            read_all(stream, &mut buf[..n], deadline).map_err(|e| format!("drain payload: {e}"))?;
            left -= n;
        }
        append_log(&format!("skip large frame {msg_len}"));
        return Ok(Vec::new());
    }
    let mut payload = vec![0u8; msg_len];
    read_all(stream, &mut payload, deadline).map_err(|e| format!("read payload: {e}"))?;
    Ok(payload)
}

fn build_punch_request(peer_id: &str, licence_key: &str, force_relay: bool) -> Vec<u8> {
    let mut inner = Vec::new();
    encode_string(&mut inner, 1, peer_id);
    // NatType: 0=UNKNOWN, 1=ASYMMETRIC, 2=SYMMETRIC。
    // 以前写死 SYMMETRIC 会让 hbbs/对端直接走中继，移动网永远无法打洞，延迟远高于直连。
    // force_relay 时才声明 SYMMETRIC；否则 UNKNOWN，尽量先打洞。
    encode_varint_field(&mut inner, 2, if force_relay { 2 } else { 0 });
    if !licence_key.is_empty() {
        encode_string(&mut inner, 3, licence_key);
    }
    encode_string(&mut inner, 6, "1.4.0");
    if force_relay {
        encode_varint_field(&mut inner, 8, 1);
    }
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 8, &inner);
    msg
}

fn payload_fields(payload: &[u8]) -> Vec<u32> {
    let mut index = 0;
    let mut fields = Vec::new();
    while let Some((field, _)) = next_field(payload, &mut index) {
        fields.push(field);
    }
    fields
}

fn hex_preview(data: &[u8], max: usize) -> String {
    data.iter()
        .take(max)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

/// Read rendezvous frames until PunchHoleResponse / RelayResponse.
/// Newer hbbs may send KeyExchange (field 25) first — official client skips it.
fn read_punch_result(stream: &mut TcpStream) -> Result<String, String> {
    for attempt in 0..8 {
        let payload = match read_framed(stream) {
            Ok(p) => p,
            Err(e) if e == "idle" && attempt > 0 => continue,
            Err(e) => return Err(e),
        };
        let fields = payload_fields(&payload);
        append_log(&format!(
            "rendezvous msg {} bytes fields={fields:?}",
            payload.len()
        ));
        // KeyExchange = 25
        if fields.contains(&25) {
            append_log("skip KeyExchange");
            continue;
        }
        // ConfigUpdate = 14
        if fields.contains(&14) && !fields.contains(&11) && !fields.contains(&19) {
            append_log("skip ConfigUpdate");
            continue;
        }
        if let Some(detail) = parse_punch_response(&payload) {
            return Ok(detail);
        }
        if let Ok(info) = parse_relay_response(&payload) {
            // uuid [relay] — treat as online via relay
            let relay = info.split_whitespace().nth(1).unwrap_or("");
            if relay.is_empty() {
                return Ok("ONLINE_RELAY".to_string());
            }
            return Ok(format!("ONLINE_RELAY {relay}"));
        }
        append_log(&format!(
            "unrecognized rendezvous hex={}",
            hex_preview(&payload, 96)
        ));
        return Err("unrecognized rendezvous response".to_string());
    }
    Err("no punch response".to_string())
}

fn parse_punch_response(payload: &[u8]) -> Option<String> {
    let mut index = 0;
    while let Some((field, value)) = next_field(payload, &mut index) {
        if field != 11 {
            continue;
        }
        let ProtoValue::Bytes(inner) = value else {
            continue;
        };
        let mut inner_index = 0;
        let mut failure: Option<u64> = None;
        let mut relay = String::new();
        let mut other = String::new();
        let mut peer_addr = String::new();
        while let Some((inner_field, inner_value)) = next_field(inner, &mut inner_index) {
            match (inner_field, inner_value) {
                (1, ProtoValue::Bytes(addr)) if !addr.is_empty() => {
                    if let Some(sa) = decode_addr_mangle(addr) {
                        peer_addr = sa;
                    }
                }
                (3, ProtoValue::Varint(code)) => failure = Some(code),
                (4, ProtoValue::Bytes(text)) => relay = String::from_utf8_lossy(text).into_owned(),
                (7, ProtoValue::Bytes(text)) => other = String::from_utf8_lossy(text).into_owned(),
                _ => {}
            }
        }
        if !other.is_empty() {
            return Some(format!("other_failure:{other}"));
        }
        if let Some(code) = failure {
            let name = match code {
                0 => "ID_NOT_EXIST",
                2 => "OFFLINE",
                3 => "LICENSE_MISMATCH",
                4 => "LICENSE_OVERUSE",
                _ => "UNKNOWN_FAILURE",
            };
            return Some(name.to_string());
        }
        // Prefer direct peer addr when present; keep relay as fallback hint.
        if !peer_addr.is_empty() {
            if relay.is_empty() {
                return Some(format!("ONLINE_DIRECT {peer_addr}"));
            }
            return Some(format!("ONLINE_DIRECT {peer_addr}|{relay}"));
        }
        if !relay.is_empty() {
            return Some(format!("ONLINE_RELAY {relay}"));
        }
        return Some("EMPTY_RESPONSE".to_string());
    }
    None
}

/// RustDesk AddrMangle::decode (IPv4 path).
fn decode_addr_mangle(bytes: &[u8]) -> Option<String> {
    use std::net::{Ipv4Addr, SocketAddrV4};
    if bytes.is_empty() || bytes.len() > 16 {
        // IPv6 path unused for now
        return None;
    }
    let mut padded = [0u8; 16];
    padded[..bytes.len()].copy_from_slice(bytes);
    let number = u128::from_le_bytes(padded);
    let tm = (number >> 17) & (u32::MAX as u128);
    let ip = (((number >> 49) - tm) as u32).to_le_bytes();
    let port = (number & 0xFFFFFF) - (tm & 0xFFFF);
    if port == 0 || port > 65535 {
        return None;
    }
    let addr = SocketAddrV4::new(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port as u16);
    Some(addr.to_string())
}

fn connect_tcp(target: &str, read_secs: u64) -> Result<TcpStream, String> {
    append_log(&format!("dns lookup {target}"));
    let addrs: Vec<_> = target
        .to_socket_addrs()
        .map_err(|e| format!("dns {target}: {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("no address for {target}"));
    }
    let mut last = String::new();
    for addr in addrs {
        append_log(&format!("tcp connect {addr}"));
        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(read_secs)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                append_log(&format!("tcp ok {addr}"));
                return Ok(stream);
            }
            Err(e) => {
                last = format!("{addr}: {e}");
                append_log(&format!("tcp fail {last}"));
            }
        }
    }
    Err(last)
}

fn uuid_v4() -> String {
    let mut bytes = [0u8; 16];
    if File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .is_err()
    {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = (nanos >> ((index % 8) * 8)) as u8 ^ (index as u8).wrapping_mul(31);
        }
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

fn normalize_relay_addr(raw: &str, id_server: &str) -> String {
    let trimmed = raw.trim();
    let host = if trimmed.is_empty() {
        id_server.split(':').next().unwrap_or(id_server)
    } else {
        let without_scheme = trimmed
            .strip_prefix("https://")
            .or_else(|| trimmed.strip_prefix("http://"))
            .unwrap_or(trimmed);
        without_scheme.split('/').next().unwrap_or(without_scheme)
    };
    if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:21117")
    }
}

fn build_request_relay(
    peer_id: &str,
    uuid: &str,
    relay_server: &str,
    licence_key: &str,
    secure: bool,
) -> Vec<u8> {
    let mut inner = Vec::new();
    encode_string(&mut inner, 1, peer_id);
    encode_string(&mut inner, 2, uuid);
    if !relay_server.is_empty() {
        encode_string(&mut inner, 4, relay_server);
    }
    if secure {
        encode_varint_field(&mut inner, 5, 1);
    }
    if !licence_key.is_empty() {
        encode_string(&mut inner, 6, licence_key);
    }
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 18, &inner);
    msg
}

fn parse_relay_response(payload: &[u8]) -> Result<String, String> {
    let mut index = 0;
    while let Some((field, value)) = next_field(payload, &mut index) {
        if field != 19 {
            continue;
        }
        let ProtoValue::Bytes(inner) = value else {
            continue;
        };
        let mut inner_index = 0;
        let mut refuse = String::new();
        let mut uuid = String::new();
        let mut relay = String::new();
        while let Some((inner_field, inner_value)) = next_field(inner, &mut inner_index) {
            match (inner_field, inner_value) {
                (2, ProtoValue::Bytes(text)) => uuid = String::from_utf8_lossy(text).into_owned(),
                (3, ProtoValue::Bytes(text)) => relay = String::from_utf8_lossy(text).into_owned(),
                (6, ProtoValue::Bytes(text)) => refuse = String::from_utf8_lossy(text).into_owned(),
                _ => {}
            }
        }
        if !refuse.is_empty() {
            return Err(format!("relay refused: {refuse}"));
        }
        return Ok(if relay.is_empty() {
            uuid
        } else {
            format!("{uuid} {relay}")
        });
    }
    Err("no RelayResponse".to_string())
}

fn password_hash(password: &[u8], salt: &str, challenge: &str) -> Vec<u8> {
    let mut first = Sha256::new();
    first.update(password);
    first.update(salt.as_bytes());
    let hashed = first.finalize();
    let mut second = Sha256::new();
    second.update(&hashed);
    second.update(challenge.as_bytes());
    second.finalize().to_vec()
}

fn build_empty_public_key() -> Vec<u8> {
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 4, &[]);
    msg
}

/// Unified profile: NotSet + custom q=100 + 60fps (former「高清」).
fn profile_knobs() -> (u32, u32, u32) {
    (0, 100, 60)
}

fn use_hd_profile() -> bool {
    true
}

fn is_direct_session() -> bool {
    DIRECT_IP_SESSION.load(Ordering::SeqCst)
}

fn build_option_message() -> Vec<u8> {
    let (iq, quality, fps) = profile_knobs();
    let mut decoding = Vec::new();
    // HarmonyOS hardware decoder: h264/h265 only — do NOT advertise vp9/vp8/av1.
    encode_varint_field(&mut decoding, 1, 0); // ability_vp9 = no
    encode_varint_field(&mut decoding, 2, 1); // ability_h264
    encode_varint_field(&mut decoding, 3, 1); // ability_h265
    // PreferCodec: H265=3
    encode_varint_field(&mut decoding, 4, 3);
    encode_varint_field(&mut decoding, 5, 0); // ability_vp8 = no
    encode_varint_field(&mut decoding, 6, 0); // ability_av1 = no
    let mut option = Vec::new();
    encode_varint_field(&mut option, 1, iq as u64);
    encode_varint_field(&mut option, 3, 1); // show_remote_cursor = No
    if iq == 0 && quality > 0 {
        // Official client stores slider as value << 8
        encode_varint_field(&mut option, 6, (quality as u64) << 8);
    }
    encode_varint_field(&mut option, 7, 2); // disable_audio = Yes
    encode_varint_field(&mut option, 8, 2); // disable_clipboard = Yes
    encode_bytes(&mut option, 10, &decoding);
    encode_varint_field(&mut option, 11, fps as u64);
    encode_varint_field(&mut option, 12, 1); // disable_keyboard = No
    option
}

fn build_custom_image_quality(quality_0_100: u32) -> Vec<u8> {
    let mut option = Vec::new();
    encode_varint_field(&mut option, 6, (quality_0_100 as u64) << 8);
    let mut misc = Vec::new();
    encode_bytes(&mut misc, 7, &option);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn build_login_request(peer_id: &str, password_hash: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    encode_string(&mut inner, 1, peer_id);
    encode_bytes(&mut inner, 2, password_hash);
    encode_string(&mut inner, 4, "harmonydesk-ohos");
    encode_string(&mut inner, 5, "HarmonyDesk");
    encode_bytes(&mut inner, 6, &build_option_message());
    // video_ack_required：官方主要用于 web。移动网 RTT 高时逐帧等 ACK ≈ 1/RTT fps（常见卡死在 ~2fps）。
    // 弱网改靠软丢队列，不要用 ACK 限吞吐。
    encode_varint_field(&mut inner, 9, 0);
    encode_varint_field(&mut inner, 10, session_id());
    encode_string(&mut inner, 11, "1.4.0");
    encode_string(&mut inner, 13, "Android");
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 7, &inner);
    msg
}

fn session_id() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn build_refresh_video() -> Vec<u8> {
    let mut misc = Vec::new();
    encode_varint_field(&mut misc, 10, 1);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn build_video_received() -> Vec<u8> {
    let mut misc = Vec::new();
    encode_varint_field(&mut misc, 12, 1);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn ask_refresh_video(reason: &str) {
    let now = now_ms();
    let last = LAST_REFRESH_ASK_MS.load(Ordering::SeqCst);
    if last != 0 && now - last < 1500 {
        return;
    }
    LAST_REFRESH_ASK_MS.store(now, Ordering::SeqCst);
    enqueue_msg(build_refresh_video());
    append_log(reason);
}

fn build_capture_displays() -> Vec<u8> {
    let mut capture = Vec::new();
    encode_varint_field(&mut capture, 3, 0);
    let mut misc = Vec::new();
    encode_bytes(&mut misc, 30, &capture);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn build_option_misc() -> Vec<u8> {
    let mut misc = Vec::new();
    encode_bytes(&mut misc, 7, &build_option_message());
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn build_test_delay_reply(inner: &[u8]) -> Vec<u8> {
    // Official client echoes TestDelay bytes as-is (from_client stays false).
    // Peer measures RTT only on from_client=false replies and clears last_test_delay.
    // If we set from_client=true, peer never clears the timer → after ~2s
    // user_delay_response_elapsed marks response_delayed → FPS locked at MIN_FPS+1 (=2).
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 5, inner);
    msg
}

fn build_auto_fps(fps: u32) -> Vec<u8> {
    let mut misc = Vec::new();
    encode_varint_field(&mut misc, 28, fps as u64); // Misc.auto_adjust_fps
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn build_custom_fps(fps: u32) -> Vec<u8> {
    let mut option = Vec::new();
    encode_varint_field(&mut option, 11, fps as u64); // OptionMessage.custom_fps
    let mut misc = Vec::new();
    encode_bytes(&mut misc, 7, &option);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

/// ImageQuality: Low=2, Balanced=3, Best=4
fn build_image_quality(quality: u32) -> Vec<u8> {
    let mut option = Vec::new();
    encode_varint_field(&mut option, 1, quality as u64);
    let mut misc = Vec::new();
    encode_bytes(&mut misc, 7, &option);
    let mut msg = Vec::new();
    encode_bytes(&mut msg, 19, &misc);
    msg
}

fn request_lower_fps(fps: u32, reason: &str) {
    let prev = LAST_AUTO_FPS.load(Ordering::SeqCst);
    if prev == fps as i32 {
        return;
    }
    LAST_AUTO_FPS.store(fps as i32, Ordering::SeqCst);
    // Only Misc.auto_adjust_fps — do NOT overwrite OptionMessage.custom_fps,
    // or peer permanently caps highest_fps at the lowered value.
    enqueue_msg(build_auto_fps(fps));
    append_log(reason);
}

fn maybe_adjust_fps(_queue_len: usize) {
    // 不再根据队列改 peer FPS：会覆盖自定义档，移动网上还容易和 ABR 互相打架。
}

fn enqueue_video_packet(codec: &str, data: Vec<u8>, key: bool) {
    let packet = EncodedPacket {
        codec: codec.to_string(),
        data,
        key,
    };
    let hd = use_hd_profile();
    let max_q = if hd { MAX_PACKET_Q_HD } else { MAX_PACKET_Q };
    if key {
        DISCARD_Q.store(false, Ordering::SeqCst);
        GOT_KEYFRAME.store(true, Ordering::SeqCst);
        KEYFRAME_ASKS.store(0, Ordering::SeqCst);
        if let Ok(mut stored) = LAST_KEY_PACKET.lock() {
            *stored = Some(packet.clone());
        }
        KEY_SEQ.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut q) = PACKET_Q.lock() {
            q.clear();
            q.push_back(packet);
        }
        return;
    }
    if DISCARD_Q.load(Ordering::SeqCst) || !GOT_KEYFRAME.load(Ordering::SeqCst) {
        let asks = KEYFRAME_ASKS.fetch_add(1, Ordering::SeqCst);
        if asks == 0 || asks % 20 == 0 {
            ask_refresh_video("request keyframe");
        }
        return;
    }
    let mut qlen_after = 0usize;
    if let Ok(mut q) = PACKET_Q.lock() {
        if q.len() >= max_q {
            // 软丢旧帧，不断流（移动网/直连都适用）
            let drop_n = (q.len() / 3).max(8);
            for _ in 0..drop_n {
                q.pop_front();
            }
        }
        q.push_back(packet);
        qlen_after = q.len();
    }
    maybe_adjust_fps(qlen_after);
}

fn record_encoded_frames(codec: &str, frames: &[u8]) {
    let mut index = 0;
    let mut count = 0;
    let mut bytes = 0usize;
    let mut last_data: Option<Vec<u8>> = None;
    let mut last_key = false;
    while let Some((field, value)) = next_field(frames, &mut index) {
        if field != 1 {
            continue;
        }
        let ProtoValue::Bytes(frame) = value else {
            continue;
        };
        let mut frame_index = 0;
        let mut data: Option<Vec<u8>> = None;
        let mut key = false;
        while let Some((inner_field, inner_value)) = next_field(frame, &mut frame_index) {
            match (inner_field, inner_value) {
                (1, ProtoValue::Bytes(payload)) => data = Some(payload.to_vec()),
                (2, ProtoValue::Varint(flag)) => key = flag != 0,
                _ => {}
            }
        }
        if let Some(payload) = data {
            count += 1;
            bytes += payload.len();
            last_key = key;
            last_data = Some(payload.clone());
            enqueue_video_packet(codec, payload, key);
        }
    }
    if count == 0 {
        return;
    }
    if let Some(payload) = last_data {
        let packet = EncodedPacket {
            codec: codec.to_string(),
            data: payload,
            key: last_key,
        };
        if let Ok(mut stored) = LAST_PACKET.lock() {
            *stored = Some(packet);
        }
        PACKET_SEQ.fetch_add(count, Ordering::SeqCst);
        LAST_VIDEO_MS.store(now_ms(), Ordering::SeqCst);
    }
    let total = FRAME_COUNT.fetch_add(count, Ordering::SeqCst) + count;
    if let Ok(mut stored) = LAST_CODEC.lock() {
        *stored = codec.to_string();
    }
    if total <= 5 || total % 30 == 0 {
        append_log(&format!(
            "video {codec} frames={total} last_bytes={bytes} key={}",
            if last_key { 1 } else { 0 }
        ));
    }
}

fn log_session_message(field: u32, payload_len: usize) {
    let n = SESSION_MSG_LOGGED.fetch_add(1, Ordering::SeqCst);
    if n < 24 {
        append_log(&format!("session msg field={field} bytes={payload_len}"));
    }
}

fn handle_misc(inner: &[u8]) {
    let mut index = 0;
    while let Some((field, value)) = next_field(inner, &mut index) {
        match (field, value) {
            (5, ProtoValue::Bytes(display)) => {
                let mut display_index = 0;
                let mut width = 0i32;
                let mut height = 0i32;
                while let Some((inner_field, inner_value)) = next_field(display, &mut display_index)
                {
                    match (inner_field, inner_value) {
                        (4, ProtoValue::Varint(v)) => width = v as i32,
                        (5, ProtoValue::Varint(v)) => height = v as i32,
                        _ => {}
                    }
                }
                if width > 0 && height > 0 {
                    DISPLAY_W.store(width, Ordering::SeqCst);
                    DISPLAY_H.store(height, Ordering::SeqCst);
                    append_log(&format!("switch display {width}x{height}"));
                }
            }
            (6, ProtoValue::Bytes(perm)) => {
                let mut perm_index = 0;
                let mut kind = 0u64;
                let mut enabled = 0u64;
                while let Some((inner_field, inner_value)) = next_field(perm, &mut perm_index) {
                    match (inner_field, inner_value) {
                        (1, ProtoValue::Varint(v)) => kind = v,
                        (2, ProtoValue::Varint(v)) => enabled = v,
                        _ => {}
                    }
                }
                append_log(&format!("permission kind={kind} enabled={enabled}"));
            }
            (9, ProtoValue::Bytes(reason)) => {
                append_log(&format!(
                    "peer close: {}",
                    String::from_utf8_lossy(reason)
                ));
            }
            _ => {}
        }
    }
}

fn handle_session_message(payload: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if payload.is_empty() {
        return Ok(None);
    }
    let mut index = 0;
    let Some((field, value)) = next_field(payload, &mut index) else {
        return Ok(None);
    };
    log_session_message(field, payload.len());
    match (field, value) {
        (5, ProtoValue::Bytes(inner)) => {
            // 同步回显：移动网上行拥堵时，走 OUTGOING 队列易超时触发 response_delayed→2fps
            let n = TEST_DELAY_ECHOED.fetch_add(1, Ordering::SeqCst) + 1;
            if n <= 3 || n % 30 == 0 {
                append_log(&format!("echo TestDelay n={n} bytes={}", inner.len()));
            }
            return Ok(Some(frame_message(&build_test_delay_reply(inner))));
        }
        (6, ProtoValue::Bytes(inner)) => {
            let mut video_index = 0;
            let mut encoded = false;
            while let Some((video_field, video_value)) = next_field(inner, &mut video_index) {
                match (video_field, video_value) {
                    (7, _) => append_log("video rgb (uncompressed)"),
                    (8, _) => append_log("video yuv (uncompressed)"),
                    (_, ProtoValue::Bytes(frames)) => {
                        let codec = match video_field {
                            6 => "vp9",
                            10 => "h264",
                            11 => "h265",
                            12 => "vp8",
                            13 => "av1",
                            _ => "",
                        };
                        if !codec.is_empty() {
                            record_encoded_frames(codec, frames);
                            encoded = true;
                        } else {
                            append_log(&format!("video other field={video_field} bytes={}", frames.len()));
                        }
                    }
                    _ => {}
                }
            }
            if encoded {
                // 即便未开 video_ack，也尽快回执，避免对端 response_delayed 把 FPS 砸到 MIN
                enqueue_msg_front(build_video_received());
            } else {
                ask_refresh_video(&format!("empty video_frame bytes={}", inner.len()));
            }
        }
        (19, ProtoValue::Bytes(inner)) => handle_misc(inner),
        (21, ProtoValue::Bytes(inner)) => {
            let mut box_index = 0;
            let mut title = String::new();
            let mut text = String::new();
            while let Some((inner_field, inner_value)) = next_field(inner, &mut box_index) {
                match (inner_field, inner_value) {
                    (2, ProtoValue::Bytes(v)) => title = String::from_utf8_lossy(v).into_owned(),
                    (3, ProtoValue::Bytes(v)) => text = String::from_utf8_lossy(v).into_owned(),
                    _ => {}
                }
            }
            append_log(&format!("message box {title}: {text}"));
        }
        (12, ProtoValue::Bytes(inner)) => {
            append_log(&format!("cursor data bytes={}", inner.len()));
        }
        (16, ProtoValue::Bytes(inner)) => {
            append_log(&format!("clipboard bytes={}", inner.len()));
        }
        (20, ProtoValue::Bytes(inner)) => {
            append_log(&format!("cliprdr bytes={}", inner.len()));
        }
        (28, ProtoValue::Bytes(inner)) => {
            append_log(&format!("multi-clipboard bytes={}", inner.len()));
        }
        other => {
            if other.0 != 8 && other.0 != 25 && other.0 != 13 && other.0 != 11 && other.0 != 14 {
                append_log(&format!("session msg field={}", other.0));
            }
        }
    }
    Ok(None)
}

fn is_idle_read(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("timed out")
        || lower.contains("wouldblock")
        || lower.contains("would block")
        || lower.contains("temporarily unavailable")
        || lower.contains("os error 11")
        || lower.contains("os error 110")
        || lower.contains("10060")
}

fn spawn_writer(stream: &TcpStream) {
    let gen = CONNECT_GEN.load(Ordering::SeqCst);
    match stream.try_clone() {
        Ok(mut writer) => {
            let _ = writer.set_nodelay(true);
            let _ = writer.set_write_timeout(Some(Duration::from_secs(2)));
            let _ = std::thread::Builder::new()
                .name("hd-write".into())
                .spawn(move || {
                    while RUNNING.load(Ordering::SeqCst) && CONNECT_GEN.load(Ordering::SeqCst) == gen {
                        flush_outgoing(&mut writer);
                        let now = now_ms();
                        let last = LAST_VIDEO_MS.load(Ordering::SeqCst);
                        let refresh_ms = if use_hd_profile() { 8000 } else { 6000 };
                        if last == 0 || now - last >= refresh_ms {
                            ask_refresh_video("writer refresh_video");
                        }
                        std::thread::sleep(Duration::from_millis(if use_hd_profile() {
                            2
                        } else {
                            4
                        }));
                    }
                });
            append_log("input writer started");
        }
        Err(e) => append_log(&format!("input writer {e}")),
    }
}

fn run_session(stream: &mut TcpStream) {
    let gen = CONNECT_GEN.load(Ordering::SeqCst);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    spawn_writer(stream);
    if stream.write_all(&frame_message(&build_option_misc())).is_ok() {
        let (iq, q, fps) = profile_knobs();
        enqueue_msg(build_image_quality(iq));
        if iq == 0 && q > 0 {
            enqueue_msg(build_custom_image_quality(q));
        }
        enqueue_msg(build_custom_fps(fps));
        enqueue_msg(build_auto_fps(fps));
        LAST_AUTO_FPS.store(fps as i32, Ordering::SeqCst);
        let path = if is_direct_session() { "direct" } else { "relay" };
        append_log(&format!(
            "sent OptionMessage {path} iq={iq} q={q} fps={fps} H265"
        ));
    }
    if stream
        .write_all(&frame_message(&build_capture_displays()))
        .is_ok()
    {
        append_log("sent CaptureDisplays");
    }
    if stream.write_all(&frame_message(&build_refresh_video())).is_ok() {
        append_log("sent refresh_video");
    }
    while RUNNING.load(Ordering::SeqCst) && CONNECT_GEN.load(Ordering::SeqCst) == gen {
        match read_framed(stream) {
            Ok(payload) => {
                match handle_session_message(&payload) {
                    Ok(Some(reply)) => {
                        if stream.write_all(&reply).is_err() {
                            append_log("TestDelay write fail");
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        append_log(&format!("session handle {e}"));
                        break;
                    }
                }
            }
            Err(e) => {
                if e == "idle" {
                    let last = LAST_VIDEO_MS.load(Ordering::SeqCst);
                    if last == 0 || now_ms() - last > 2500 {
                        ask_refresh_video("idle refresh_video");
                    }
                    continue;
                }
                if is_idle_read(&e) {
                    continue;
                }
                append_log(&format!("session read {e}"));
                break;
            }
        }
    }
    append_log("session loop end");
    if RUNNING.load(Ordering::SeqCst) && CONNECT_GEN.load(Ordering::SeqCst) == gen {
        set_error("会话中断");
        STATUS.store(STATUS_FAILED, Ordering::SeqCst);
    }
}

fn parse_hash(inner: &[u8]) -> (String, String) {
    let mut salt = String::new();
    let mut challenge = String::new();
    let mut index = 0;
    while let Some((field, value)) = next_field(inner, &mut index) {
        match (field, value) {
            (1, ProtoValue::Bytes(text)) => salt = String::from_utf8_lossy(text).into_owned(),
            (2, ProtoValue::Bytes(text)) => challenge = String::from_utf8_lossy(text).into_owned(),
            _ => {}
        }
    }
    (salt, challenge)
}

fn parse_login_response(inner: &[u8]) -> Result<String, String> {
    let mut error = String::new();
    let mut username = String::new();
    let mut hostname = String::new();
    let mut platform = String::new();
    let mut index = 0;
    while let Some((field, value)) = next_field(inner, &mut index) {
        match (field, value) {
            (1, ProtoValue::Bytes(text)) => error = String::from_utf8_lossy(text).into_owned(),
            (2, ProtoValue::Bytes(peer)) => {
                let mut peer_index = 0;
                while let Some((peer_field, peer_value)) = next_field(peer, &mut peer_index) {
                    match (peer_field, peer_value) {
                        (1, ProtoValue::Bytes(text)) => {
                            username = String::from_utf8_lossy(text).into_owned()
                        }
                        (2, ProtoValue::Bytes(text)) => {
                            hostname = String::from_utf8_lossy(text).into_owned()
                        }
                        (3, ProtoValue::Bytes(text)) => {
                            platform = String::from_utf8_lossy(text).into_owned()
                        }
                        (4, ProtoValue::Bytes(display)) => {
                            let mut display_index = 0;
                            let mut width = 0i32;
                            let mut height = 0i32;
                            while let Some((disp_field, disp_value)) =
                                next_field(display, &mut display_index)
                            {
                                match (disp_field, disp_value) {
                                    (3, ProtoValue::Varint(v)) => width = v as i32,
                                    (4, ProtoValue::Varint(v)) => height = v as i32,
                                    _ => {}
                                }
                            }
                            if width > 0 && height > 0 {
                                DISPLAY_W.store(width, Ordering::SeqCst);
                                DISPLAY_H.store(height, Ordering::SeqCst);
                                append_log(&format!("peer display {width}x{height}"));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    if !error.is_empty() {
        Err(error)
    } else {
        Ok(format!("{username}@{hostname} {platform}"))
    }
}

fn handshake_after_relay(
    stream: &mut TcpStream,
    peer_id: &str,
    password: &str,
) -> Result<String, String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let mut got_hash = false;
    for _ in 0..8 {
        let payload = match read_framed(stream) {
            Ok(payload) => payload,
            Err(e) => {
                if got_hash {
                    return Ok(format!("RESULT: LOGIN_SENT {peer_id}"));
                }
                return Err(e);
            }
        };
        append_log(&format!("peer msg {} bytes", payload.len()));
        let mut index = 0;
        let Some((field, value)) = next_field(&payload, &mut index) else {
            append_log("empty peer message");
            continue;
        };
        match field {
            3 => {
                append_log("got SignedId, send empty PublicKey");
                stream
                    .write_all(&frame_message(&build_empty_public_key()))
                    .map_err(|e| format!("send PublicKey: {e}"))?;
            }
            4 => append_log("got PublicKey"),
            9 => {
                let ProtoValue::Bytes(inner) = value else {
                    continue;
                };
                let (salt, challenge) = parse_hash(inner);
                append_log(&format!(
                    "got Hash salt_len={} challenge_len={}",
                    salt.len(),
                    challenge.len()
                ));
                let hashed = password_hash(password.as_bytes(), &salt, &challenge);
                stream
                    .write_all(&frame_message(&build_login_request(peer_id, &hashed)))
                    .map_err(|e| format!("send LoginRequest: {e}"))?;
                append_log("sent LoginRequest");
                got_hash = true;
            }
            8 => {
                let ProtoValue::Bytes(inner) = value else {
                    continue;
                };
                return match parse_login_response(inner) {
                    Ok(info) => {
                        let result = format!("RESULT: LOGIN_OK {peer_id} {info}");
                        append_log(&result);
                        STATUS.store(STATUS_READY, Ordering::SeqCst);
                        run_session(stream);
                        Ok(result)
                    }
                    Err(e) => Ok(format!("RESULT: LOGIN_ERROR {peer_id} {e}")),
                };
            }
            other => append_log(&format!("peer msg field={other}")),
        }
    }
    if got_hash {
        Ok(format!("RESULT: LOGIN_SENT {peer_id}"))
    } else {
        Ok(format!("RESULT: RELAY_PAIRED {peer_id}"))
    }
}

fn request_and_join_relay(
    id_server: &str,
    relay_hint: &str,
    configured_relay: &str,
    peer_id: &str,
    licence_key: &str,
    password: &str,
) -> Result<String, String> {
    let relay = if !relay_hint.is_empty() {
        normalize_relay_addr(relay_hint, id_server)
    } else {
        normalize_relay_addr(configured_relay, id_server)
    };
    let uuid = uuid_v4();
    append_log(&format!("relay request uuid={uuid} via {relay}"));

    let mut hbbs = connect_tcp(id_server, 15)?;
    hbbs.write_all(&frame_message(&build_request_relay(
        peer_id,
        &uuid,
        &relay,
        licence_key,
        true,
    )))
    .map_err(|e| format!("send RequestRelay: {e}"))?;
    append_log("sent RequestRelay to hbbs");
    let payload = read_framed(&mut hbbs).map_err(|e| format!("read RelayResponse: {e}"))?;
    append_log(&format!("relay response {} bytes", payload.len()));
    let info = parse_relay_response(&payload)?;
    append_log(&format!("RelayResponse ok {info}"));
    drop(hbbs);

    let mut hbbr = connect_tcp(&relay, 12)?;
    hbbr.write_all(&frame_message(&build_request_relay(
        peer_id,
        &uuid,
        "",
        licence_key,
        false,
    )))
    .map_err(|e| format!("send hbbr RequestRelay: {e}"))?;
    append_log("sent RequestRelay to hbbr");
    match handshake_after_relay(&mut hbbr, peer_id, password) {
        Ok(msg) => Ok(msg),
        Err(e) if e.contains("read header") || e.contains("timed out") => {
            append_log(&format!("handshake wait {e}"));
            Ok(format!("RESULT: RELAY_WAITING {peer_id} {relay}"))
        }
        Err(e) => Err(e),
    }
}

/// Punch-hole only: ask hbbs whether the peer is online, without joining relay.
fn check_peer_online(server: &str, peer_id: &str, licence_key: &str) -> Result<String, String> {
    if is_direct_ip_peer(peer_id) {
        let addr = normalize_direct_addr(peer_id);
        append_log(&format!("check direct-ip {addr}"));
        return match connect_tcp(&addr, 3) {
            Ok(_) => Ok("ONLINE".to_string()),
            Err(e) => {
                append_log(&format!("direct-ip probe fail {e}"));
                Ok("OFFLINE".to_string())
            }
        };
    }
    let force_relay = FORCE_RELAY.load(Ordering::SeqCst);
    let mut stream = connect_tcp(server, 8)?;
    stream
        .write_all(&frame_message(&build_punch_request(
            peer_id,
            licence_key,
            force_relay,
        )))
        .map_err(|e| format!("send punch: {e}"))?;
    append_log(&format!("check PunchHoleRequest {peer_id}"));
    let detail = read_punch_result(&mut stream)?;
    match detail.as_str() {
        "OFFLINE" => Ok("OFFLINE".to_string()),
        "ID_NOT_EXIST" => Ok("NOT_EXIST".to_string()),
        "LICENSE_MISMATCH" => Ok("LICENSE_MISMATCH".to_string()),
        "LICENSE_OVERUSE" => Ok("LICENSE_OVERUSE".to_string()),
        other if other.starts_with("ONLINE") => Ok("ONLINE".to_string()),
        other => Ok(other.to_string()),
    }
}

fn connect_direct_ip(peer: &str, password: &str) -> Result<String, String> {
    DIRECT_IP_SESSION.store(true, Ordering::SeqCst);
    HD_PROFILE.store(true, Ordering::SeqCst);
    LAST_AUTO_FPS.store(60, Ordering::SeqCst);
    let addr = normalize_direct_addr(peer);
    append_log(&format!("direct-ip connect {addr} (HD profile)"));
    let mut stream = connect_tcp(&addr, 15)?;
    append_log("direct-ip tcp ok, start handshake");
    handshake_after_relay(&mut stream, peer, password)
}

fn lookup_peer(
    server: &str,
    configured_relay: &str,
    peer_id: &str,
    licence_key: &str,
    password: &str,
) -> Result<String, String> {
    if is_direct_ip_peer(peer_id) {
        return connect_direct_ip(peer_id, password);
    }
    let force_relay = FORCE_RELAY.load(Ordering::SeqCst);
    let mut stream = connect_tcp(server, 8)?;
    stream
        .write_all(&frame_message(&build_punch_request(
            peer_id,
            licence_key,
            force_relay,
        )))
        .map_err(|e| format!("send punch: {e}"))?;
    append_log("sent PunchHoleRequest");
    let detail = read_punch_result(&mut stream)?;
    match detail.as_str() {
        "OFFLINE" => Ok(format!("RESULT: OFFLINE {peer_id}")),
        "ID_NOT_EXIST" => Ok(format!("RESULT: NOT_EXIST {peer_id}")),
        "LICENSE_MISMATCH" => Ok(format!("RESULT: LICENSE_MISMATCH {peer_id}")),
        "LICENSE_OVERUSE" => Ok(format!("RESULT: LICENSE_OVERUSE {peer_id}")),
        other if other.starts_with("ONLINE") => {
            append_log(&format!("RESULT: ONLINE {peer_id} {other}"));
            // ONLINE_DIRECT <addr>[|<relay>] — try punch TCP first, then relay
            if let Some(rest) = other.strip_prefix("ONLINE_DIRECT ") {
                let (peer_addr, relay_hint) = match rest.split_once('|') {
                    Some((a, r)) => (a, r),
                    None => (rest, ""),
                };
                append_log(&format!("try punch-direct {peer_addr}"));
                match connect_tcp(peer_addr, 8) {
                    Ok(mut stream) => {
                        append_log("punch-direct tcp ok, start handshake");
                        DIRECT_IP_SESSION.store(false, Ordering::SeqCst); // not LAN IP mode, but low-latency path
                        return handshake_after_relay(&mut stream, peer_id, password);
                    }
                    Err(e) => {
                        append_log(&format!("punch-direct fail {e}, fallback relay"));
                        let hint = if relay_hint.is_empty() {
                            ""
                        } else {
                            relay_hint
                        };
                        return match request_and_join_relay(
                            server,
                            hint,
                            configured_relay,
                            peer_id,
                            licence_key,
                            password,
                        ) {
                            Ok(msg) => Ok(msg),
                            Err(e2) => Ok(format!(
                                "RESULT: ONLINE {peer_id} PUNCH_FAIL {e} RELAY_FAIL {e2}"
                            )),
                        };
                    }
                }
            }
            let relay_hint = other.strip_prefix("ONLINE_RELAY ").unwrap_or("");
            match request_and_join_relay(
                server,
                relay_hint,
                configured_relay,
                peer_id,
                licence_key,
                password,
            )
            {
                Ok(msg) => Ok(msg),
                Err(e) => {
                    append_log(&format!("relay failed {e}"));
                    Ok(format!("RESULT: ONLINE {peer_id} {other} RELAY_FAIL {e}"))
                }
            }
        }
        other => Ok(format!("RESULT: {other} {peer_id}")),
    }
}

#[no_mangle]
pub extern "C" fn hd_init() -> c_int {
    append_log("hdcore init ok");
    STATUS.store(STATUS_IDLE, Ordering::SeqCst);
    0
}

#[no_mangle]
pub extern "C" fn hd_set_server(
    id_server: *const c_char,
    relay: *const c_char,
    force_relay: c_int,
    key: *const c_char,
) -> c_int {
    let licence = normalize_licence_key(&cstr_to_string(key));
    let relay_host = cstr_to_string(relay);
    FORCE_RELAY.store(force_relay != 0, Ordering::SeqCst);
    match normalize_server(&cstr_to_string(id_server)) {
        Ok(server) => {
            if let Ok(mut stored) = SERVER.lock() {
                *stored = server.clone();
            }
            if let Ok(mut stored) = RELAY.lock() {
                *stored = relay_host.clone();
            }
            if let Ok(mut stored) = KEY.lock() {
                *stored = licence.clone();
            }
            append_log(&format!(
                "server set {server} relay={} key={}",
                if relay_host.is_empty() { "empty" } else { &relay_host },
                key_debug(&licence)
            ));
            0
        }
        Err(e) => {
            if let Ok(mut stored) = SERVER.lock() {
                stored.clear();
            }
            if let Ok(mut stored) = KEY.lock() {
                stored.clear();
            }
            append_log(&format!("server rejected: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn hd_connect_start(desk_id: *const c_char, password: *const c_char) -> c_int {
    let desk = cstr_to_string(desk_id);
    let password = cstr_to_string(password);
    let server = SERVER.lock().map(|s| s.clone()).unwrap_or_default();
    let configured_relay = RELAY.lock().map(|s| s.clone()).unwrap_or_default();
    let licence = KEY.lock().map(|s| s.clone()).unwrap_or_default();
    if server.is_empty() && !is_direct_ip_peer(&desk) {
        set_error("请先在设置填写自建 ID 服务器");
        append_log("lookup aborted: missing self-hosted id server");
        STATUS.store(STATUS_FAILED, Ordering::SeqCst);
        return -1;
    }

    CONNECT_GEN.fetch_add(1, Ordering::SeqCst);
    let gen = CONNECT_GEN.load(Ordering::SeqCst);
    let direct = is_direct_ip_peer(&desk);
    DIRECT_IP_SESSION.store(direct, Ordering::SeqCst);
    // 默认与直连相同：高清档（custom 100/60）；UI 仍可切流畅
    HD_PROFILE.store(true, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);
    FRAME_COUNT.store(0, Ordering::SeqCst);
    PACKET_SEQ.store(0, Ordering::SeqCst);
    KEY_SEQ.store(0, Ordering::SeqCst);
    GOT_KEYFRAME.store(false, Ordering::SeqCst);
    KEYFRAME_ASKS.store(0, Ordering::SeqCst);
    SESSION_MSG_LOGGED.store(0, Ordering::SeqCst);
    TEST_DELAY_ECHOED.store(0, Ordering::SeqCst);
    LAST_VIDEO_MS.store(0, Ordering::SeqCst);
    LAST_REFRESH_ASK_MS.store(0, Ordering::SeqCst);
    DISPLAY_W.store(0, Ordering::SeqCst);
    DISPLAY_H.store(0, Ordering::SeqCst);
    LAST_COPIED_KEY.store(0, Ordering::SeqCst);
    DISCARD_Q.store(false, Ordering::SeqCst);
    LAST_AUTO_FPS.store(if direct { 60 } else { 25 }, Ordering::SeqCst);
    if let Ok(mut q) = PACKET_Q.lock() {
        q.clear();
    }
    if let Ok(mut packet) = LAST_PACKET.lock() {
        *packet = None;
    }
    if let Ok(mut packet) = LAST_KEY_PACKET.lock() {
        *packet = None;
    }
    if let Ok(mut queue) = OUTGOING.lock() {
        queue.clear();
    }
    STATUS.store(STATUS_CONNECTING, Ordering::SeqCst);
    set_error("");
    if is_direct_ip_peer(&desk) {
        append_log(&format!(
            "lookup start direct-ip={} key={}",
            normalize_direct_addr(&desk),
            key_debug(&licence)
        ));
    } else {
        append_log(&format!(
            "lookup start id={desk} server={server} key={}",
            key_debug(&licence)
        ));
    }

    let _ = std::thread::Builder::new()
        .name("hd-lookup".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            if CONNECT_GEN.load(Ordering::SeqCst) != gen {
                return;
            }
            STATUS.store(STATUS_CONNECTING, Ordering::SeqCst);
            match lookup_peer(&server, &configured_relay, &desk, &licence, &password) {
                Ok(msg) => {
                    if CONNECT_GEN.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    append_log(&format!("lookup ok {msg}"));
                    if msg.contains("LOGIN_OK") {
                        return;
                    }
                    set_error("");
                    STATUS.store(STATUS_READY, Ordering::SeqCst);
                }
                Err(e) => {
                    if CONNECT_GEN.load(Ordering::SeqCst) != gen {
                        return;
                    }
                    set_error(&e);
                    append_log(&format!("lookup failed {e}"));
                    STATUS.store(STATUS_FAILED, Ordering::SeqCst);
                }
            }
        });
    0
}

#[no_mangle]
pub extern "C" fn hd_status() -> c_int {
    STATUS.load(Ordering::SeqCst)
}

/// Non-blocking online probe. Poll `hd_copy_check_result` for ONLINE / OFFLINE / …
#[no_mangle]
pub extern "C" fn hd_check_peer(peer_id: *const c_char) -> c_int {
    let desk = cstr_to_string(peer_id);
    if desk.is_empty() {
        return -1;
    }
    let server = SERVER.lock().map(|s| s.clone()).unwrap_or_default();
    let licence = KEY.lock().map(|s| s.clone()).unwrap_or_default();
    if server.is_empty() && !is_direct_ip_peer(&desk) {
        if let Ok(mut result) = CHECK_RESULT.lock() {
            *result = "NO_SERVER".to_string();
        }
        return -2;
    }
    let gen = CHECK_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    if let Ok(mut result) = CHECK_RESULT.lock() {
        *result = "PENDING".to_string();
    }
    let _ = std::thread::Builder::new()
        .name("hd-check".into())
        .spawn(move || {
            let outcome = match check_peer_online(&server, &desk, &licence) {
                Ok(s) => s,
                Err(e) => format!("ERROR:{e}"),
            };
            if CHECK_GEN.load(Ordering::SeqCst) != gen {
                return;
            }
            append_log(&format!("check {desk} -> {outcome}"));
            if let Ok(mut result) = CHECK_RESULT.lock() {
                *result = outcome;
            }
        });
    0
}

#[no_mangle]
pub extern "C" fn hd_copy_check_result(buf: *mut c_char, len: c_int) -> c_int {
    let result = CHECK_RESULT.lock().map(|s| s.clone()).unwrap_or_default();
    copy_to_buf(&result, buf, len)
}

#[no_mangle]
pub extern "C" fn hd_disconnect() {
    CONNECT_GEN.fetch_add(1, Ordering::SeqCst);
    RUNNING.store(false, Ordering::SeqCst);
    DIRECT_IP_SESSION.store(false, Ordering::SeqCst);
    HD_PROFILE.store(false, Ordering::SeqCst);
    STATUS.store(STATUS_IDLE, Ordering::SeqCst);
    append_log("hdcore disconnect");
}

#[no_mangle]
pub extern "C" fn hd_frame_count() -> c_int {
    FRAME_COUNT.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_copy_codec(buf: *mut c_char, len: c_int) -> c_int {
    let codec = LAST_CODEC.lock().map(|s| s.clone()).unwrap_or_default();
    copy_to_buf(&codec, buf, len)
}

#[no_mangle]
pub extern "C" fn hd_copy_logs(buf: *mut c_char, len: c_int) -> c_int {
    let logs = LOGS.lock().map(|s| s.clone()).unwrap_or_default();
    copy_to_buf(&logs, buf, len)
}

#[no_mangle]
pub extern "C" fn hd_copy_error(buf: *mut c_char, len: c_int) -> c_int {
    let err = ERROR.lock().map(|s| s.clone()).unwrap_or_default();
    copy_to_buf(&err, buf, len)
}

#[no_mangle]
pub extern "C" fn hd_packet_seq() -> c_int {
    PACKET_SEQ.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_packet_key() -> c_int {
    LAST_COPIED_KEY.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_display_width() -> c_int {
    DISPLAY_W.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_display_height() -> c_int {
    DISPLAY_H.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_queue_len() -> c_int {
    PACKET_Q.lock().map(|q| q.len() as c_int).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn hd_key_seq() -> c_int {
    KEY_SEQ.load(Ordering::SeqCst)
}

#[no_mangle]
pub extern "C" fn hd_request_keyframe() {
    GOT_KEYFRAME.store(false, Ordering::SeqCst);
    DISCARD_Q.store(true, Ordering::SeqCst);
    KEYFRAME_ASKS.store(0, Ordering::SeqCst);
    if let Ok(mut q) = PACKET_Q.lock() {
        q.clear();
    }
    enqueue_msg(build_refresh_video());
    append_log("request keyframe");
}

#[no_mangle]
pub extern "C" fn hd_copy_key_packet(buf: *mut u8, len: c_int) -> c_int {
    copy_stored_packet(&LAST_KEY_PACKET, buf, len)
}

#[no_mangle]
pub extern "C" fn hd_copy_packet(buf: *mut u8, len: c_int) -> c_int {
    if buf.is_null() || len <= 0 {
        return 0;
    }
    let packet = match PACKET_Q.lock() {
        Ok(mut q) => q.pop_front(),
        Err(_) => None,
    };
    let Some(packet) = packet else {
        return 0;
    };
    LAST_COPIED_KEY.store(if packet.key { 1 } else { 0 }, Ordering::SeqCst);
    if let Ok(mut codec) = LAST_CODEC.lock() {
        *codec = packet.codec.clone();
    }
    let n = packet.data.len().min(len as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(packet.data.as_ptr(), buf, n);
    }
    n as c_int
}

fn copy_stored_packet(slot: &Mutex<Option<EncodedPacket>>, buf: *mut u8, len: c_int) -> c_int {
    if buf.is_null() || len <= 0 {
        return 0;
    }
    let packet = match slot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };
    let Some(packet) = packet else {
        return 0;
    };
    let n = packet.data.len().min(len as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(packet.data.as_ptr(), buf, n);
    }
    n as c_int
}

#[no_mangle]
pub extern "C" fn hd_send_mouse(mask: c_int, x: c_int, y: c_int) {
    let evt = mask & 0x7;
    // Wheel / trackpad: x,y are scroll deltas — never overwrite last cursor.
    if evt == 3 || evt == 4 {
        enqueue_msg(build_mouse_event(mask, x, y));
        static WHEEL_LOGS: AtomicI32 = AtomicI32::new(0);
        let n = WHEEL_LOGS.fetch_add(1, Ordering::SeqCst);
        if n < 24 {
            append_log(&format!("wheel mask={mask} dx={x} dy={y}"));
        }
        return;
    }
    let mut px = x;
    let mut py = y;
    if let Ok(mut pos) = LAST_MOUSE.lock() {
        if evt == 0 || x != 0 || y != 0 {
            *pos = (x, y);
        } else {
            px = pos.0;
            py = pos.1;
        }
    }
    enqueue_msg(build_mouse_event(mask, px, py));
    static MOUSE_LOGS: AtomicI32 = AtomicI32::new(0);
    let n = MOUSE_LOGS.fetch_add(1, Ordering::SeqCst);
    if n < 24 {
        append_log(&format!("mouse mask={mask} {px},{py}"));
    }
}

#[no_mangle]
pub extern "C" fn hd_send_key(chr: c_int, down: c_int, press: c_int) {
    if chr <= 0 {
        return;
    }
    enqueue_msg(build_key_chr(chr as u32, down != 0, press != 0));
}

#[no_mangle]
pub extern "C" fn hd_send_control(code: c_int, down: c_int, press: c_int) {
    if code <= 0 {
        return;
    }
    let key = code as u32;
    if press != 0 {
        // 完整点按：down + up（对方服务端看的是 down，不是 press）
        enqueue_msg(build_key_control(key, true, false));
        enqueue_msg(build_key_control(key, false, false));
    } else {
        enqueue_msg(build_key_control(key, down != 0, false));
    }
}

/// Send Ctrl/Alt/Shift/Meta + key chord.
/// `modifier` is RustDesk ControlKey (Control=4, Alt=1, Shift=29, Meta=23).
/// `chr`:
/// - 32 → Space (IME 中英文切换)
/// - 1..=255 → ASCII letter/digit (e.g. 'd'=100 for Win+D)
/// - >=1000 → ControlKey (chr - 1000), e.g. 1015 = F4 for Alt+F4
#[no_mangle]
pub extern "C" fn hd_send_chord(chr: c_int, modifier: c_int) {
    if chr <= 0 || modifier <= 0 {
        return;
    }
    let mod_code = modifier as u32;
    let ch = chr as u32;
    enqueue_msg(build_key_control(mod_code, true, false));
    if ch >= 1000 {
        let key = ch - 1000;
        enqueue_msg(build_key_control_mod(key, &[mod_code], true, false));
        enqueue_msg(build_key_control_mod(key, &[mod_code], false, false));
    } else if ch == 32 {
        // ControlKey::Space = 30 — Windows 输入法 Ctrl+空格
        enqueue_msg(build_key_control_mod(30, &[mod_code], true, false));
        enqueue_msg(build_key_control_mod(30, &[mod_code], false, false));
    } else {
        enqueue_msg(build_key_chr_mod(ch, &[mod_code], true, false));
        enqueue_msg(build_key_chr_mod(ch, &[mod_code], false, false));
    }
    enqueue_msg(build_key_control(mod_code, false, false));
    // 再松一次，防止对方修饰键卡住
    enqueue_msg(build_key_control(mod_code, false, false));
    append_log(&format!("chord mod={mod_code} key={ch}"));
}

/// Re-apply unified image quality (UI no longer offers smooth/HD toggle).
#[no_mangle]
pub extern "C" fn hd_set_image_quality(_quality: c_int) {
    let (iq, bitrate, fps) = profile_knobs();
    LAST_AUTO_FPS.store(fps as i32, Ordering::SeqCst);
    enqueue_msg(build_option_misc());
    enqueue_msg(build_image_quality(iq));
    enqueue_msg(build_custom_image_quality(bitrate));
    enqueue_msg(build_custom_fps(fps));
    enqueue_msg(build_auto_fps(fps));
    let path = if is_direct_session() { "direct" } else { "relay" };
    ask_refresh_video(&format!("profile unified {path} q={bitrate} fps={fps}"));
    append_log(&format!("profile -> unified {path} q={bitrate} fps={fps}"));
}

#[no_mangle]
pub extern "C" fn hd_send_text(text: *const c_char) {
    let text = cstr_to_string(text);
    if text.is_empty() {
        return;
    }
    // 先松开修饰键（中英切换后 Ctrl 常会卡住，密码框就收不到字）
    release_modifiers();
    // Windows 密码框：逐字 Legacy chr(down)，比整段 seq 稳
    for ch in text.chars() {
        match ch {
            '\n' | '\r' => {
                enqueue_msg(build_key_control(27, true, false));
                enqueue_msg(build_key_control(27, false, false));
            }
            '\t' => {
                enqueue_msg(build_key_control(31, true, false));
                enqueue_msg(build_key_control(31, false, false));
            }
            _ => {
                let code = ch as u32;
                enqueue_msg(build_key_chr(code, true, false));
                enqueue_msg(build_key_chr(code, false, false));
            }
        }
    }
    append_log(&format!("send text len={} legacy-chr", text.chars().count()));
}
