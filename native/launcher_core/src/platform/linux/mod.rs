//! Linux desktop integration. No elevation, shell execution or debugger port.
mod desktop_session;
mod desktop_entry;
use crate::common::{load, save, settings, tz, zones, Settings};
use serde_json::{json, Value};
use std::{env, fs, io::{Read, Write}, os::unix::fs::{DirBuilderExt, MetadataExt}, path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};

const APP_ID: &str = "com.fei-away.codextimezone";

fn home() -> Result<PathBuf, String> {
    let path = env::var_os("HOME").map(PathBuf::from).ok_or("无法确定用户主目录。")?;
    if !path.is_absolute() { return Err("用户主目录必须是绝对路径。".into()); }
    Ok(path)
}

fn config_directory() -> Result<PathBuf, String> {
    let root = match env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => home()?.join(".config"),
    };
    if !root.is_absolute() { return Err("XDG_CONFIG_HOME 必须是绝对路径。".into()); }
    Ok(root.join(APP_ID))
}

fn current_uid() -> Result<u32, String> {
    fs::metadata("/proc/self").map(|m| m.uid()).map_err(|e| format!("无法检查当前用户：{e}"))
}

fn private_directory(dir: &Path) -> Result<(), String> {
    if !dir.exists() {
        fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|e| e.to_string())?;
    }
    let metadata = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.uid() != current_uid()? || metadata.mode() & 0o077 != 0 {
        return Err(format!("配置目录必须属于当前用户、不是符号链接，并使用 700 权限：{}", dir.display()));
    }
    for name in ["settings.json", "launch.log"] {
        let path = dir.join(name);
        match fs::symlink_metadata(&path) {
            Ok(m) if !m.is_file() || m.uid() != current_uid()? || m.nlink() != 1 || m.mode() & 0o077 != 0 =>
                return Err(format!("配置文件必须是当前用户的私有普通文件：{}", path.display())),
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

fn validate(path: &str) -> Result<PathBuf, String> {
    let selected = PathBuf::from(path.trim());
    if !selected.is_absolute() { return Err("请选择客户端的绝对路径。".into()); }
    let mut executable = fs::canonicalize(selected).map_err(|_| "客户端路径不存在。")?;
    // Only unwrap this known packaged entrypoint; never execute arbitrary shell wrappers.
    if executable == Path::new("/usr/lib/chatgpt/codex-launcher") {
        executable = fs::canonicalize("/usr/lib/chatgpt/ChatGPT").map_err(|e| e.to_string())?;
    }
    let name = executable.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !["ChatGPT", "Codex", "codex"].contains(&name) {
        return Err("请选择 Codex 桌面客户端，不要选择命令行程序或脚本。".into());
    }
    let parent = executable.parent().ok_or("客户端目录无效。")?;
    let resource = parent.join("resources/app.asar");
    if !resource.is_file() {
        return Err("缺少桌面客户端 resources/app.asar；不支持 Codex CLI 或任意脚本。".into());
    }
    let uid = current_uid()?;
    let resource = fs::canonicalize(resource).map_err(|e| e.to_string())?;
    for entry in executable.ancestors().chain(resource.ancestors()) {
        let m = fs::metadata(entry).map_err(|e| e.to_string())?;
        let sticky_directory = m.is_dir() && m.mode() & 0o1000 != 0;
        if (m.uid() != 0 && m.uid() != uid) || (m.mode() & 0o022 != 0 && !sticky_directory) {
            return Err(format!("客户端及上级目录不能由其他用户修改：{}", entry.display()));
        }
    }
    let metadata = fs::metadata(&executable).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.mode() & 0o111 == 0 || metadata.mode() & 0o6000 != 0 {
        return Err("客户端必须是普通可执行文件，不能使用 setuid/setgid。".into());
    }
    let mut magic = [0; 4];
    fs::File::open(&executable).and_then(|mut f| f.read_exact(&mut magic)).map_err(|e| e.to_string())?;
    if magic != *b"\x7fELF" { return Err("客户端必须是 Linux ELF 桌面程序；不支持脚本。".into()); }
    Ok(executable)
}

fn discover() -> String {
    for candidate in ["/usr/bin/chatgpt", "/usr/lib/chatgpt/ChatGPT", "/opt/Codex/Codex", "/opt/codex/codex"] {
        if let Ok(path) = validate(candidate) { return path.to_string_lossy().into_owned(); }
    }
    String::new()
}

fn is_running(executable: &Path) -> Result<bool, String> {
    let uid = current_uid()?;
    for item in fs::read_dir("/proc").map_err(|e| e.to_string())? {
        let Ok(item) = item else { continue };
        if !item.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()) { continue; }
        let Ok(m) = item.metadata() else { continue };
        if m.uid() != uid { continue; }
        match fs::read_link(item.path().join("exe")) {
            Ok(path) if path == executable => return Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                // Non-dumpable user services can deny exe access. Fail closed only
                // when the process name could identify the selected client.
                let name = fs::read_to_string(item.path().join("comm")).unwrap_or_default();
                if executable.file_name().and_then(|n| n.to_str()).is_some_and(|n| name.trim() == n) {
                    return Err("无法检查当前用户的客户端进程；已停止启动。".into());
                }
            },
            _ => {},
        }
    }
    Ok(false)
}

fn target(s: &Settings) -> Result<PathBuf, String> {
    let selected = if s.executable.trim().is_empty() { discover() } else { s.executable.clone() };
    validate(&selected)
}

fn without_skin(mut s: Settings) -> Settings { s.dream_skin_compatible = false; s }

fn isolate_packaged_input_cache(command: &mut Command, cache: Option<std::ffi::OsString>, input: Option<std::ffi::OsString>) {
    if let (Some(cache), Some(input)) = (cache, input) {
        let cache = PathBuf::from(cache);
        if cache.is_absolute() && Path::new(&input).starts_with(&cache) {
            command.env_remove("GTK_IM_MODULE_FILE");
        }
    }
    command.env_remove("CODEX_TZ_RUNTIME_CACHE");
}

fn launch_command(executable: &Path, s: &Settings) -> Result<Command, String> {
    let timezone = tz(s)?;
    if !Path::new("/usr/share/zoneinfo").join(&timezone).is_file() {
        return Err(format!("系统缺少时区数据：{timezone}，请安装 tzdata。"));
    }
    let mut command = Command::new(executable);
    command.current_dir(executable.parent().ok_or("客户端目录无效。")?)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    isolate_packaged_input_cache(&mut command, env::var_os("CODEX_TZ_RUNTIME_CACHE"), env::var_os("GTK_IM_MODULE_FILE"));
    if let Some(entry) = desktop_entry::load(executable)? { entry.apply(&mut command); }
    // Original-entry overrides are applied first; TZ and loader isolation remain ours.
    command.env("TZ", timezone).env_remove("ELECTRON_RUN_AS_NODE")
        .env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
    Ok(command)
}

fn write_log(dir: &Path, value: &Value) -> Result<(), String> {
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    temp.write_all(serde_json::to_string_pretty(value).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(dir.join("launch.log")).map_err(|e| e.to_string())?;
    Ok(())
}

fn launch(dir: &Path, s: Settings) -> Result<Value, String> {
    let s = without_skin(s);
    let executable = target(&s)?;
    if is_running(&executable)? { return Err("客户端正在运行。请保存工作并完全退出客户端后重试；启动器不会关闭现有进程。".into()); }
    if env::var_os("DISPLAY").filter(|v| !v.is_empty()).is_none() && env::var_os("WAYLAND_DISPLAY").filter(|v| !v.is_empty()).is_none() {
        return Err("当前没有图形会话，请在桌面终端中运行启动器。".into());
    }
    let mut command = launch_command(&executable, &s)?;
    // Chromium selects the Linux cookie/key storage backend from the desktop
    // identity. A headless caller must not silently switch KDE to basic storage.
    desktop_session::apply(&mut command)?;
    save(dir, &s)?;
    write_log(dir, &json!({"stage":"requested","timezone":tz(&s)?,"executable":executable}))?;
    let mut child = command.spawn().map_err(|e| format!("无法启动客户端：{e}"))?;
    let pid = child.id();
    std::thread::sleep(Duration::from_millis(800));
    let status = child.try_wait().map_err(|e| e.to_string())?;
    // A logging failure must not abandon child reaping or claim a verified window.
    if let Some(status) = status {
        let _ = write_log(dir, &json!({"stage":"early_exit","pid":pid,"status":status.to_string()}));
        return Err(format!("客户端启动后提前退出：{status}。请检查桌面环境和是否存在旧实例。"));
    }
    std::thread::spawn(move || { let _ = child.wait(); });
    let warning = write_log(dir, &json!({"stage":"process_started","pid":pid,"timezone":tz(&s)?,"executable":executable,"windowReadyVerified":false})).err();
    Ok(json!({"launched":true,"pid":pid,"settings":s,"target":{"executable":executable},"windowReadyVerified":false,"warning":warning,"message":"客户端进程已按所选时区启动；窗口就绪和实际时间显示仍需确认。"}))
}

fn desktop_quote(path: &Path) -> Result<String, String> {
    let value = path.to_str().ok_or("快捷方式路径必须是 UTF-8。")?;
    if value.contains(['\n', '\r', '\0']) { return Err("快捷方式路径包含无效字符。".into()); }
    // Exec parsing and desktop-entry string parsing each consume an escape layer.
    let escaped = value.replace('\\', "\\\\\\\\").replace('"', "\\\\\"").replace('`', "\\\\`").replace('$', "\\\\$").replace('%', "%%");
    Ok(format!("\"{escaped}\""))
}

fn create_shortcut_at(executable: &Path, applications: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(applications).map_err(|e| e.to_string())?;
    let path = applications.join(format!("{APP_ID}.desktop"));
    let body = format!("[Desktop Entry]\nType=Application\nName=Codex 时区启动器\nComment=为新客户端设置独立时区\nExec={}\nTerminal=false\nCategories=Utility;\n", desktop_quote(executable)?);
    let mut temp = tempfile::NamedTempFile::new_in(applications).map_err(|e| e.to_string())?;
    temp.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist_noclobber(&path).map_err(|_| format!("入口已存在，未覆盖：{}", path.display()))?;
    Ok(path)
}

pub fn execute(command: &str, payload: Value) -> Result<Value, String> {
    if ["repair_launch", "launch_dream_skin", "reapply_dream_skin"].contains(&command) {
        return Err("Linux 版不提供提权修复或皮肤功能。".into());
    }
    let dir = config_directory()?;
    private_directory(&dir)?;
    match command {
        "bootstrap" => Ok(json!({"settings":without_skin(load(&dir)?),"zones":zones(),"detected":discover(),"platform":"linux","settingsPath":dir.join("settings.json")})),
        "discover" => Ok(json!({"path":discover()})),
        "validate" => Ok(json!({"path":validate(payload["path"].as_str().unwrap_or_default())?})),
        "save" => { let s = without_skin(settings(&payload, &dir)?); save(&dir, &s)?; Ok(json!({"settings":s,"path":dir.join("settings.json"),"message":"设置已保存。"})) },
        "launch" => launch(&dir, settings(&payload, &dir)?),
        "check_launch" => {
            let s = settings(&payload, &dir)?;
            let executable = target(&s)?;
            let entry = desktop_entry::load(&executable)?;
            let command = launch_command(&executable, &s)?;
            let original = entry.as_ref().map(|entry| json!({
                "path":entry.source,
                "environmentKeys":entry.environment.keys().collect::<Vec<_>>(),
                "environmentPreserved":entry.environment.iter().filter(|(key,_)| !["TZ", "LD_LIBRARY_PATH", "LD_PRELOAD", "ELECTRON_RUN_AS_NODE"].contains(&key.as_str())).all(|(key,value)| command.get_envs().any(|(k,v)| k == std::ffi::OsStr::new(key) && v == Some(std::ffi::OsStr::new(value)))),
                "argumentsPreserved":command.get_args().eq(entry.arguments.iter().map(std::ffi::OsStr::new)),
                "workingDirectoryPreserved":command.get_current_dir() == Some(entry.working_directory.as_path())
            }));
            Ok(json!({"healthy":true,"running":is_running(&executable)?,"target":{"executable":executable},"originalDesktopEntry":original}))
        },
        "create_shortcut" => {
            let root = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or(home()?.join(".local/share"));
            if !root.is_absolute() { return Err("XDG_DATA_HOME 必须是绝对路径。".into()); }
            let path = create_shortcut_at(&env::current_exe().map_err(|e| e.to_string())?, &root.join("applications"))?;
            Ok(json!({"created":true,"path":path,"message":"已创建应用菜单入口，可从桌面应用菜单搜索 Codex 时区启动器。"}))
        },
        _ => Err(format!("未知命令：{command}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn packaged_input_cache_does_not_leak_into_original_client() {
        let mut cmd = Command::new("/fixture/client");
        isolate_packaged_input_cache(&mut cmd, Some("/fixture/cache".into()), Some("/fixture/cache/immodules".into()));
        assert!(cmd.get_envs().any(|(key,value)| key == "GTK_IM_MODULE_FILE" && value.is_none()));
        // A user desktop-entry override is applied afterwards and stays effective.
        cmd.env("GTK_IM_MODULE_FILE", "/original/immodules");
        assert!(cmd.get_envs().any(|(key,value)| key == "GTK_IM_MODULE_FILE" && value == Some(std::ffi::OsStr::new("/original/immodules"))));
        let mut other = Command::new("/fixture/client");
        isolate_packaged_input_cache(&mut other, Some("/fixture/cache".into()), Some("/original/immodules".into()));
        assert!(!other.get_envs().any(|(key,_)| key == "GTK_IM_MODULE_FILE"));
    }
    #[test]
    fn private_config_rejects_symlinks_and_foreign_access() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        private_directory(&config).unwrap();
        let outside = dir.path().join("outside");
        fs::write(&outside, "unchanged").unwrap();
        std::os::unix::fs::symlink(&outside, config.join("settings.json")).unwrap();
        assert!(private_directory(&config).is_err());
        fs::remove_file(config.join("settings.json")).unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(private_directory(&config).is_err());
        assert_eq!(fs::read_to_string(outside).unwrap(), "unchanged");
    }
    #[test]
    fn validates_desktop_binary_and_rejects_scripts_cli_and_writable_target() {
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let binary = dir.path().join("ChatGPT");
        fs::copy("/bin/sleep", &binary).unwrap();
        fs::create_dir(dir.path().join("resources")).unwrap();
        fs::write(dir.path().join("resources/app.asar"), "fixture").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(dir.path().join("resources"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(dir.path().join("resources/app.asar"), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(validate(binary.to_str().unwrap()).unwrap(), binary);
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(validate(binary.to_str().unwrap()).is_err());
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o1777)).unwrap();
        assert!(validate(binary.to_str().unwrap()).is_err());
        fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(validate(binary.to_str().unwrap()).is_err());
        assert!(validate("/usr/lib/chatgpt/resources/codex").is_err());
        assert!(validate("ChatGPT").is_err());
    }
    #[test]
    fn timezone_child_does_not_change_parent_or_open_debugger() {
        let before = env::var_os("TZ");
        for zone in ["Asia/Shanghai", "America/New_York", "Etc/GMT+5"] {
            let s = Settings { zone_id: zone.into(), ..Settings::default() };
            let mut s = s;
            if zone.starts_with("Etc/") { s.mode = "offset".into(); s.offset = -5; }
            let mut cmd = launch_command(Path::new("/bin/date"), &s).unwrap();
            assert!(cmd.get_args().next().is_none());
            assert!(cmd.get_envs().any(|(k,v)| k == "ELECTRON_RUN_AS_NODE" && v.is_none()));
            let out = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).args(["--date=@1791417600", "+%z"]).output().unwrap();
            assert!(out.status.success());
            let expected = match zone { "Asia/Shanghai" => "+0800", "America/New_York" => "-0400", _ => "-0500" };
            assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), expected);
        }
        assert_eq!(env::var_os("TZ"), before);
    }
    #[test]
    fn detects_running_process_without_terminating_it() {
        let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let exe = fs::canonicalize("/bin/sleep").unwrap();
        let result = is_running(&exe);
        assert!(child.try_wait().unwrap().is_none());
        child.kill().unwrap(); child.wait().unwrap();
        assert!(result.unwrap());
    }
    #[test]
    fn shortcut_escapes_exec_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let executable = Path::new("/tmp/a $b`c\\d\"e%f/app");
        let path = create_shortcut_at(executable, dir.path()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("%%f")); assert!(text.contains("\\\\$b"));
        assert!(create_shortcut_at(executable, dir.path()).is_err());
        assert_eq!(text, fs::read_to_string(path).unwrap());
        assert!(desktop_quote(Path::new("/tmp/new\nline")).is_err());
    }
}
