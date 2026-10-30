//! Accès HID brut Linux (/dev/hidraw) sans libudev ni hidapi.
//! Recherche l'interface fournisseur 320F:5041 dans /sys/class/hidraw.

use anyhow::{Context, Result, anyhow};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct LinuxHid {
    file: File,
}

impl LinuxHid {
    pub fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .with_context(|| {
                format!(
                    "ouverture de {} (accès refusé ? installe la règle udev fournie)",
                    path.display()
                )
            })?;
        Ok(Self { file })
    }

    pub fn write(&self, data: &[u8]) -> std::io::Result<usize> {
        let mut file = &self.file;
        file.write_all(data)?;
        Ok(data.len())
    }

    pub fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> std::io::Result<usize> {
        let mut pfd = libc::pollfd {
            fd: self.file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout = timeout_ms.max(0);
        let rc = unsafe { libc::poll(&mut pfd, 1, timeout) };
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                return Ok(0);
            }
            return Err(err);
        }
        if rc == 0 || (pfd.revents & libc::POLLIN) == 0 {
            return Ok(0);
        }
        let mut file = &self.file;
        file.read(buf)
    }
}

/// Liste les /dev/hidraw* dont le VID/PID correspond au Berserker.
fn descriptor_is_vendor(entry: &fs::DirEntry) -> bool {
    let path = entry.path().join("device/report_descriptor");
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let has_page = bytes.windows(3).any(|w| w == [0x06, 0x1C, 0xFF]);
    let has_usage = bytes.windows(2).any(|w| w == [0x09, 0x92]);
    has_page && has_usage
}

pub fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let mut vendor_paths = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/hidraw") else {
        return paths;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let uevent = entry.path().join("device/uevent");
        let Ok(text) = fs::read_to_string(&uevent) else {
            continue;
        };
        let Some(id) = text.lines().find_map(|line| line.strip_prefix("HID_ID=")) else {
            continue;
        };
        let parts: Vec<&str> = id.split(':').collect();
        if parts.len() != 3 {
            continue;
        }
        let Ok(vid) = u16::from_str_radix(parts[1].trim_start_matches('0'), 16) else {
            continue;
        };
        let Ok(pid) = u16::from_str_radix(parts[2].trim_start_matches('0'), 16) else {
            continue;
        };
        if vid == 0x320F && pid == 0x5041 {
            let path = PathBuf::from(format!("/dev/{}", name.to_string_lossy()));
            if descriptor_is_vendor(&entry) {
                vendor_paths.push(path.clone());
            }
            paths.push(path);
        }
    }
    if vendor_paths.is_empty() {
        paths
    } else {
        vendor_paths
    }
}

pub fn open_first_responding() -> Result<LinuxHid> {
    let paths = candidate_paths();
    if paths.is_empty() {
        return Err(anyhow!(
            "aucune interface hidraw 320F:5041 trouvée (clavier branché ?)"
        ));
    }
    let mut last_error = None;
    for path in paths {
        match LinuxHid::open(&path) {
            Ok(dev) => return Ok(dev),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("aucune interface hidraw accessible")))
}
