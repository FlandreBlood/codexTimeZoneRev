//! Recover only desktop-session metadata, never credential or profile variables.
use std::{collections::BTreeMap, env, ffi::OsString, fs, io::Read,
    os::unix::{ffi::OsStringExt, fs::MetadataExt}, path::Path, process::Command};

const KEYS: &[&str] = &[
    "DISPLAY", "WAYLAND_DISPLAY", "XAUTHORITY", "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS", "XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP",
    "DESKTOP_SESSION", "KDE_SESSION_VERSION", "KDE_FULL_SESSION",
    "GNOME_DESKTOP_SESSION_ID",
];
type Environment = BTreeMap<String, OsString>;

fn parse(bytes: &[u8]) -> Environment {
    let mut result = Environment::new();
    for entry in bytes.split(|b| *b == 0) {
        let Some(split) = entry.iter().position(|b| *b == b'=') else { continue };
        let Ok(key) = std::str::from_utf8(&entry[..split]) else { continue };
        if KEYS.contains(&key) && entry.len() > split + 1 {
            result.insert(key.into(), OsString::from_vec(entry[split + 1..].to_vec()));
        }
    }
    result
}

fn same_session(current: &Environment, candidate: &Environment) -> bool {
    ["DISPLAY", "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"]
        .iter().all(|key| match current.get(*key) {
            Some(value) => candidate.get(*key) == Some(value),
            None => true,
        })
}

fn session(proc_root: &Path, uid: u32, current: &Environment) -> Option<Environment> {
    let desktops = ["/usr/bin/plasmashell", "/usr/bin/gnome-shell",
        "/usr/bin/xfce4-session", "/usr/bin/cinnamon", "/usr/bin/mate-session",
        "/usr/bin/lxqt-session", "/usr/bin/sway", "/usr/bin/Hyprland"];
    for item in fs::read_dir(proc_root).ok()? {
        let Ok(item) = item else { continue };
        if !item.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) { continue; }
        if item.metadata().ok().map(|m| m.uid()) != Some(uid) { continue; }
        let Ok(executable) = fs::read_link(item.path().join("exe")) else { continue };
        if !desktops.iter().any(|path| executable == Path::new(path)) { continue; }
        let Ok(file) = fs::File::open(item.path().join("environ")) else { continue };
        let mut bytes = Vec::new();
        if file.take(128 * 1024 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > 128 * 1024 { continue; }
        let candidate = parse(&bytes);
        if candidate.contains_key("XDG_CURRENT_DESKTOP") && same_session(current, &candidate) {
            return Some(candidate);
        }
    }
    None
}

fn complete(mut current: Environment, recovered: Option<Environment>) -> Result<Environment, String> {
    if let Some(recovered) = recovered {
        for (key, value) in recovered { current.entry(key).or_insert(value); }
    }
    let desktop = current.get("XDG_CURRENT_DESKTOP").and_then(|v| v.to_str())
        .filter(|v| !v.trim().is_empty()).ok_or(
            "缺少桌面会话标识，可能导致登录凭据无法读取。请从应用菜单或正常桌面终端启动本启动器。")?;
    if desktop.split(':').any(|v| v.eq_ignore_ascii_case("KDE")) &&
        !current.get("KDE_SESSION_VERSION").and_then(|v| v.to_str()).is_some_and(|v| ["4", "5", "6"].contains(&v)) {
        return Err("缺少 KDE 会话版本，已停止启动以避免切换登录凭据存储。请从正常 KDE 桌面启动本启动器。".into());
    }
    if !current.contains_key("DBUS_SESSION_BUS_ADDRESS") {
        return Err("缺少桌面 D-Bus 会话，无法确保沿用登录凭据存储；已停止启动。".into());
    }
    Ok(current)
}

pub(super) fn apply(command: &mut Command) -> Result<(), String> {
    let current: Environment = KEYS.iter().filter_map(|key| env::var_os(key)
        .filter(|v| !v.is_empty()).map(|value| ((*key).into(), value))).collect();
    let current = command_environment(command, current);
    let uid = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
    let recovered = session(Path::new("/proc"), uid, &current);
    command.envs(complete(current, recovered)?);
    Ok(())
}

fn command_environment(command: &Command, mut current: Environment) -> Environment {
    // Desktop-entry assignments take precedence over the launcher's environment.
    for (key, value) in command.get_envs() {
        let Some(key) = key.to_str().filter(|key| KEYS.contains(key)) else { continue };
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            current.insert(key.into(), value.into());
        } else {
            current.remove(key);
        }
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    fn kde() -> Environment {
        parse(b"XDG_CURRENT_DESKTOP=KDE\0KDE_SESSION_VERSION=6\0DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\0DISPLAY=:0\0")
    }
    #[test]
    fn recovers_desktop_identity_without_copying_profile_or_credentials() {
        let source = parse(b"XDG_CURRENT_DESKTOP=KDE\0KDE_SESSION_VERSION=6\0DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\0HOME=/other\0CODEX_HOME=/other\0XDG_CONFIG_HOME=/other\0ACCESS_TOKEN=secret\0PATH=/other/bin\0");
        assert_eq!(source.len(), 3);
        let result = complete(Environment::new(), Some(source)).unwrap();
        assert_eq!(result["XDG_CURRENT_DESKTOP"], "KDE");
        assert_eq!(result["KDE_SESSION_VERSION"], "6");
        assert!(!result.contains_key("ACCESS_TOKEN"));
        assert!(!result.contains_key("XDG_CONFIG_HOME"));
    }
    #[test]
    fn preserves_explicit_environment_and_rejects_incomplete_context() {
        let source = kde();
        let mut command = Command::new("/fixture/client");
        command.env("DESKTOP_SESSION", "entry-session").env("HOME", "/other");
        let command_values = command_environment(&command, source.clone());
        assert_eq!(command_values["DESKTOP_SESSION"], "entry-session");
        assert!(!command_values.contains_key("HOME"));
        let mut current = source.clone();
        current.insert("DESKTOP_SESSION".into(), "custom-session".into());
        let result = complete(current, Some(source.clone())).unwrap();
        assert_eq!(result["DESKTOP_SESSION"], "custom-session");
        assert!(complete(Environment::new(), None).is_err());
        let mut incomplete = source.clone(); incomplete.remove("KDE_SESSION_VERSION");
        assert!(complete(incomplete, None).is_err());
        let mut other = source.clone(); other.insert("DISPLAY".into(), ":1".into());
        assert!(!same_session(&source, &other));
    }
    #[test]
    fn discovers_only_known_desktop_executables_of_current_user_and_session() {
        let dir = tempfile::tempdir().unwrap();
        let process = dir.path().join("123"); fs::create_dir(&process).unwrap();
        std::os::unix::fs::symlink("/usr/bin/plasmashell", process.join("exe")).unwrap();
        fs::write(process.join("environ"), b"XDG_CURRENT_DESKTOP=KDE\0KDE_SESSION_VERSION=6\0DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus\0DISPLAY=:0\0ACCESS_TOKEN=secret\0").unwrap();
        let uid = fs::metadata(dir.path()).unwrap().uid();
        assert!(session(dir.path(), uid, &Environment::new()).is_some());
        assert!(session(dir.path(), uid + 1, &Environment::new()).is_none());
        assert!(session(dir.path(), uid, &parse(b"DISPLAY=:1\0")).is_none());
        fs::remove_file(process.join("exe")).unwrap();
        std::os::unix::fs::symlink("/tmp/plasmashell", process.join("exe")).unwrap();
        assert!(session(dir.path(), uid, &Environment::new()).is_none());
    }
}
