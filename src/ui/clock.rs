#[derive(Clone, Copy, Debug)]
pub struct ClockTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub day: u8,
    pub month: u8,
    pub weekday: u8,
    pub american_date: bool,
}
impl Default for ClockTime {
    fn default() -> Self {
        Self {
            hour: 0,
            minute: 0,
            second: 0,
            day: 1,
            month: 1,
            weekday: 0,
            american_date: false,
        }
    }
}
impl ClockTime {
    pub fn hour_text(self, twenty_four: bool) -> String {
        format!(
            "{:02}",
            if twenty_four {
                self.hour
            } else {
                match self.hour % 12 {
                    0 => 12,
                    h => h,
                }
            }
        )
    }
    pub fn minute_text(self) -> String {
        format!("{:02}", self.minute)
    }
    pub fn meridiem(self) -> &'static str {
        if self.hour >= 12 { "pm" } else { "am" }
    }
    pub fn weekday_text(self) -> &'static str {
        ["mon", "tue", "wed", "thu", "fri", "sat", "sun"][self.weekday.min(6) as usize]
    }
    pub fn date(self, spaces: bool) -> String {
        let (a, b) = if self.american_date {
            (self.month, self.day)
        } else {
            (self.day, self.month)
        };
        if spaces {
            format!("{a:02} / {b:02}")
        } else {
            format!("{a:02}/{b:02}")
        }
    }
}

/// `ClockHudFrame.formatDate`: "MM / dd" when the JVM's default zone id
/// contains "America", else "dd / MM". The id is resolved once, the way the
/// JVM resolves its default zone on this platform (`TimeZone_md.c`).
pub fn american_zone() -> bool {
    static AMERICAN: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AMERICAN.get_or_init(|| system_zone_id().is_some_and(|id| american(&id)))
}

/// `ZoneId.SHORT_IDS` names that `TimeZone.toZoneId` turns into America
/// zones; every other short id maps elsewhere or to a fixed offset.
const SHORT_AMERICA: &[&str] = &[
    "AGT", "AST", "BET", "CNT", "CST", "IET", "PNT", "PRT", "PST",
];

fn american(id: &str) -> bool {
    id.contains("America") || SHORT_AMERICA.contains(&id)
}

/// An id the JVM cannot load falls back to a "GMT+hh:mm" offset id, so only
/// names with a zoneinfo file (or a short id) survive.
#[cfg(not(windows))]
fn system_zone_id() -> Option<String> {
    let tz = std::env::var("TZ").ok();
    let zoneinfo = std::path::Path::new("/usr/share/zoneinfo");
    unix_zone_id(tz.as_deref(), std::path::Path::new("/etc"), zoneinfo)
        .filter(|id| SHORT_AMERICA.contains(&id.as_str()) || zoneinfo.join(id).is_file())
}

/// Windows: the JVM reads the registry's `TimeZoneKeyName`, which the
/// payload cannot (no registry API, and it may not spawn `reg`), so the
/// zone is unknown and the date stays "dd / MM".
#[cfg(windows)]
fn system_zone_id() -> Option<String> {
    None
}

/// `getSystemTimeZoneID` on Linux: `TZ`, else `/etc/timezone`, else the
/// `/etc/localtime` symlink's `zoneinfo/` part, else the zoneinfo file
/// whose bytes `/etc/localtime` copies. A leading ':' and "posix/" go.
#[cfg_attr(windows, allow(dead_code))]
fn unix_zone_id(
    tz: Option<&str>,
    etc: &std::path::Path,
    zoneinfo: &std::path::Path,
) -> Option<String> {
    let id = match tz.filter(|tz| !tz.is_empty()) {
        Some(tz) => tz.to_string(),
        None => platform_zone_id(etc, zoneinfo)?,
    };
    let id = id.strip_prefix(':').unwrap_or(&id);
    Some(id.strip_prefix("posix/").unwrap_or(id).to_string())
}

/// `getZoneName`: the part of a path after "zoneinfo/".
#[cfg_attr(windows, allow(dead_code))]
fn zone_name(path: &str) -> Option<&str> {
    path.find("zoneinfo/")
        .map(|at| &path[at + "zoneinfo/".len()..])
}

#[cfg_attr(windows, allow(dead_code))]
fn platform_zone_id(etc: &std::path::Path, zoneinfo: &std::path::Path) -> Option<String> {
    // Debian's /etc/timezone: its first line, if not empty.
    if let Ok(text) = std::fs::read(etc.join("timezone")) {
        let line = String::from_utf8_lossy(&text);
        let line = line.split('\n').next().unwrap_or_default();
        if !line.is_empty() {
            return Some(line.to_string());
        }
    }
    let localtime = etc.join("localtime");
    let meta = std::fs::symlink_metadata(&localtime).ok()?;
    if meta.file_type().is_symlink()
        && let Ok(link) = std::fs::read_link(&localtime)
        && let Some(name) = zone_name(&link.to_string_lossy())
    {
        return Some(name.to_string());
    }
    let bytes = std::fs::read(&localtime).ok()?;
    find_zoneinfo_file(&bytes, zoneinfo, true)
}

/// `findZoneinfoFile`: UTC and GMT first, then the directory in its own
/// order (skipping dot files, ROC, posixrules and localtime), recursing.
#[cfg_attr(windows, allow(dead_code))]
fn find_zoneinfo_file(bytes: &[u8], dir: &std::path::Path, top: bool) -> Option<String> {
    let popular = top.then_some(["UTC", "GMT"]).into_iter().flatten();
    let listed = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            !name.starts_with('.') && !matches!(name.as_str(), "ROC" | "posixrules" | "localtime")
        });
    for name in popular.map(String::from).chain(listed) {
        let path = dir.join(&name);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            if let Some(found) = find_zoneinfo_file(bytes, &path, false) {
                return Some(found);
            }
        } else if meta.len() == bytes.len() as u64
            && std::fs::read(&path).is_ok_and(|other| other == bytes)
        {
            return zone_name(&path.to_string_lossy()).map(String::from);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vape-rs-clock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("etc")).unwrap();
        std::fs::create_dir_all(dir.join("zoneinfo/America")).unwrap();
        std::fs::create_dir_all(dir.join("zoneinfo/Europe")).unwrap();
        std::fs::write(dir.join("zoneinfo/America/New_York"), b"TZif-ny").unwrap();
        std::fs::write(dir.join("zoneinfo/Europe/Berlin"), b"TZif-berlin").unwrap();
        std::fs::write(dir.join("zoneinfo/UTC"), b"TZif-utc").unwrap();
        dir
    }

    #[test]
    fn dates_follow_the_zone_order() {
        let time = ClockTime {
            day: 3,
            month: 11,
            ..ClockTime::default()
        };
        assert_eq!(time.date(true), "03 / 11");
        let us = ClockTime {
            american_date: true,
            ..time
        };
        assert_eq!(us.date(true), "11 / 03");
        assert_eq!(us.date(false), "11/03");
        assert_eq!(ClockTime { weekday: 4, ..time }.weekday_text(), "fri");
        assert!(american("America/Argentina/Buenos_Aires") && american("PST"));
        assert!(!american("Europe/Berlin") && !american("EST") && !american("GMT-05:00"));
    }

    #[cfg(unix)]
    #[test]
    fn linux_zone_follows_the_jdk_precedence() {
        let dir = temp("precedence");
        let (etc, zi) = (dir.join("etc"), dir.join("zoneinfo"));
        // No /etc/localtime at all: no zone.
        assert_eq!(unix_zone_id(None, &etc, &zi), None);
        // TZ wins, with ':' and "posix/" removed.
        assert_eq!(
            unix_zone_id(Some(":posix/America/Denver"), &etc, &zi).as_deref(),
            Some("America/Denver")
        );
        // A copied zoneinfo file is matched by content.
        std::fs::write(etc.join("localtime"), b"TZif-ny").unwrap();
        assert_eq!(
            unix_zone_id(Some(""), &etc, &zi).as_deref(),
            Some("America/New_York")
        );
        std::fs::write(etc.join("localtime"), b"TZif-utc").unwrap();
        assert_eq!(unix_zone_id(None, &etc, &zi).as_deref(), Some("UTC"));
        // A symlink names its zone after "zoneinfo/".
        std::fs::remove_file(etc.join("localtime")).unwrap();
        std::os::unix::fs::symlink("../zoneinfo/Europe/Berlin", etc.join("localtime")).unwrap();
        assert_eq!(
            unix_zone_id(None, &etc, &zi).as_deref(),
            Some("Europe/Berlin")
        );
        // /etc/timezone comes before /etc/localtime; an empty one is skipped.
        std::fs::write(etc.join("timezone"), "\n").unwrap();
        assert_eq!(
            unix_zone_id(None, &etc, &zi).as_deref(),
            Some("Europe/Berlin")
        );
        std::fs::write(etc.join("timezone"), "America/Chicago\n").unwrap();
        assert_eq!(
            unix_zone_id(None, &etc, &zi).as_deref(),
            Some("America/Chicago")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
