use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

const DEV_USB: &str = "/dev/xbelite2";
const DEV_BT: &str = "/dev/xbelite2_bt";

static SEQ_SYS: AtomicU8 = AtomicU8::new(0x60);
static SEQ_VENDOR: AtomicU8 = AtomicU8::new(0x80);

/// Low-level GIP device handle over /dev/xbelite2.
/// Provides send/recv for GIP frames through the kernel ring buffer.
pub struct GipDevice {
    file: File,
}

impl GipDevice {
    /// Open the USB misc device.
    pub fn open_usb() -> io::Result<Self> {
        Self::open(DEV_USB)
    }

    /// Open the BT misc device.
    pub fn open_bt() -> io::Result<Self> {
        Self::open(DEV_BT)
    }

    fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        unsafe {
            let fd = file.as_raw_fd();
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let mut dev = Self { file };
        dev.drain();
        Ok(dev)
    }

    /// Write a raw GIP packet.
    pub fn send(&mut self, pkt: &[u8]) -> io::Result<()> {
        self.file.write_all(pkt)
    }

    /// Read one frame from the ring buffer (2-byte LE length prefix + payload).
    pub fn recv(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        loop {
            let mut len_buf = [0u8; 2];
            match self.file.read(&mut len_buf) {
                Ok(2) => {
                    let frame_len = u16::from_le_bytes(len_buf) as usize;
                    if frame_len == 0 || frame_len > 512 {
                        continue;
                    }
                    let mut buf = vec![0u8; frame_len];
                    let mut read_so_far = 0;
                    while read_so_far < frame_len {
                        match self.file.read(&mut buf[read_so_far..]) {
                            Ok(n) if n > 0 => read_so_far += n,
                            Ok(_) => break,
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                if Instant::now() > deadline {
                                    return None;
                                }
                                std::thread::sleep(Duration::from_millis(1));
                            }
                            Err(_) => return None,
                        }
                    }
                    if read_so_far == frame_len {
                        return Some(buf);
                    }
                }
                Ok(_) => {}
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(_) => return None,
            }
        }
    }

    /// Read a response matching a specific command byte, skipping input reports.
    pub fn recv_cmd(&mut self, want_cmd: u8, timeout: Duration) -> Option<Vec<u8>> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match self.recv(remaining) {
                Some(frame) if !frame.is_empty() => {
                    let cmd = frame[0];
                    // Skip input/status/ACK noise
                    if matches!(cmd, 0x20 | 0x02 | 0x01 | 0x0C | 0x03 | 0x07) {
                        continue;
                    }
                    if cmd == want_cmd {
                        return Some(frame);
                    }
                }
                _ => return None,
            }
        }
    }

    /// Drain all pending data from the ring buffer.
    pub fn drain(&mut self) {
        let mut buf = [0u8; 512];
        loop {
            match self.file.read(&mut buf) {
                Ok(n) if n > 0 => continue,
                _ => break,
            }
        }
    }

    /// Send a 0x4D vendor command and read the 0x4D response.
    pub fn vendor_cmd(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let seq = SEQ_VENDOR.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![0x4D, 0x10, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt).ok()?;
        self.recv_cmd(0x4D, Duration::from_millis(500))
    }

    /// Send a 0x1E system command and read the 0x1E response.
    pub fn system_cmd(&mut self, payload: &[u8]) -> Option<Vec<u8>> {
        let seq = SEQ_SYS.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![0x1E, 0x30, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt).ok()?;
        self.recv_cmd(0x1E, Duration::from_millis(500))
    }

    /// Send a fire-and-forget command.
    pub fn send_cmd(&mut self, cmd: u8, flags: u8, payload: &[u8]) -> io::Result<()> {
        let seq = SEQ_SYS.fetch_add(1, Ordering::Relaxed);
        let mut pkt = vec![cmd, flags, seq, payload.len() as u8];
        pkt.extend_from_slice(payload);
        self.send(&pkt)
    }

    /// Send the UNLOCK command (0x4D sub 0x03). Required before writes.
    pub fn unlock(&mut self) -> bool {
        self.vendor_cmd(&[0x03]).is_some()
    }

    /// Send the INIT EXTENDED REPORTS command (0x4D sub 0x07).
    pub fn init_extended(&mut self) -> bool {
        self.vendor_cmd(&[0x07, 0x00]).is_some()
    }

    /// Get a reference to the underlying file (for poll/select in daemon).
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Get a mutable reference to the underlying file.
    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}
