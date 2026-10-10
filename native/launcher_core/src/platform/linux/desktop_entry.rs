//! Reuse the installed client's effective desktop entry, including user overrides.
//! Exec is parsed as data; no shell, env program, or arbitrary wrapper is executed.
use std::{collections::BTreeMap, env, fs, io::Read, os::unix::fs::MetadataExt,
    path::{Path, PathBuf}, process::Command};

#[derive(Debug)]
pub(super) struct Entry {
    pub source: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
}

fn tokens(value: &str) -> Result<Vec<String>, String> {
    // Desktop-entry strings are unescaped before the Exec quoting layer.
    let mut unescaped = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' { unescaped.push(ch); continue; }
        match chars.next() {
            Some('\\') => unescaped.push('\\'), Some('s') => unescaped.push(' '),
            Some('n' | 'r' | 't') => return Err("客户端入口包含不支持的控制字符。".into()),
            _ => return Err("客户端入口包含不支持的转义；未降级为默认启动。".into()),
        }
    }
    let mut output = Vec::new(); let mut word = String::new();
    let mut quoted = false; let mut started = false; let mut chars = unescaped.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => { quoted = !quoted; started = true; },
            '\\' if quoted => match chars.next() {
                Some(ch @ ('"' | '`' | '$' | '\\')) => { word.push(ch); started = true; },
                _ => return Err("客户端入口包含不支持的 Exec 转义。".into()),
            },
            ch if ch.is_whitespace() && !quoted => {
                if started { output.push(std::mem::take(&mut word)); started = false; }
            },
            '\0' | '\n' | '\r' => return Err("客户端入口包含控制字符。".into()),
            ch @ ('\'' | '`' | '$' | ';' | '|' | '&' | '<' | '>' | '(' | ')' | '\\') if !quoted => {
                return Err(format!("客户端入口包含未引用的保留字符 {ch}；不会执行 shell。"));
            },
            ch => { word.push(ch); started = true; },
        }
    }
    if quoted { return Err("客户端入口的引号不完整。".into()); }
    if started { output.push(word); }
    Ok(output)
}

fn parse(text: &str, source: PathBuf, home: &Path) -> Result<Entry, String> {
    let mut fields = BTreeMap::new(); let mut in_entry = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') { in_entry = line == "[Desktop Entry]"; continue; }
        if !in_entry || line.starts_with('#') || line.is_empty() { continue; }
        if let Some((key,value)) = line.split_once('=') {
            if fields.insert(key, value).is_some() { return Err("客户端入口存在重复字段。".into()); }
        }
    }
    if fields.get("Type") != Some(&"Application") || fields.get("Hidden") == Some(&"true") ||
        fields.get("Terminal") == Some(&"true") || fields.get("DBusActivatable") == Some(&"true") {
        return Err("客户端入口不是受支持的直接桌面启动方式。".into());
    }
    let parts = tokens(fields.get("Exec").ok_or("客户端入口缺少 Exec。")?)?;
    let mut position = 0; let mut environment = BTreeMap::new();
    if parts.first().is_some_and(|p| p == "env" || p == "/usr/bin/env") {
        position += 1;
        while let Some(part) = parts.get(position) {
            let Some((key,value)) = part.split_once('=') else { break };
            let valid = !key.is_empty() && key.bytes().enumerate().all(|(i,b)|
                b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()));
            if !valid || value.contains('%') || environment.insert(key.into(), value.into()).is_some() {
                return Err("客户端入口的环境赋值不受支持。".into());
            }
            position += 1;
        }
    }
    let executable = parts.get(position).ok_or("客户端入口缺少程序路径。")?;
    if !["chatgpt", "/usr/bin/chatgpt", "/usr/lib/chatgpt/codex-launcher", "/usr/lib/chatgpt/ChatGPT"].contains(&executable.as_str()) {
        return Err("客户端入口指向未知程序；不会忽略原入口或执行任意脚本。".into());
    }
    let mut arguments = Vec::new();
    for argument in &parts[position + 1..] {
        if ["%u", "%U", "%f", "%F"].contains(&argument.as_str()) { continue; }
        if argument == "%k" { arguments.push(source.to_string_lossy().into_owned()); }
        else if argument == "%c" { arguments.push(fields.get("Name").unwrap_or(&"ChatGPT").to_string()); }
        else {
            let literal = argument.replace("%%", "\u{1}");
            if literal.contains('%') { return Err("客户端入口包含未支持的字段代码。".into()); }
            arguments.push(literal.replace('\u{1}', "%"));
        }
    }
    let directory = fields.get("Path").filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(*v)).unwrap_or_else(|| home.to_path_buf());
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("客户端入口的工作目录无效。".into());
    }
    Ok(Entry { source, environment, arguments, working_directory: directory })
}

fn read(source: PathBuf, home: &Path, uid: u32) -> Result<Entry, String> {
    let m = fs::metadata(&source).map_err(|e| e.to_string())?;
    if !m.is_file() || (m.uid() != uid && m.uid() != 0) || m.mode() & 0o022 != 0 {
        return Err("客户端入口必须是可信用户拥有、其他用户不可写的普通文件。".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&source).map_err(|e| e.to_string())?.take(64 * 1024 + 1)
        .read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 { return Err("客户端入口过大。".into()); }
    parse(std::str::from_utf8(&bytes).map_err(|_| "客户端入口不是 UTF-8。")?, source, home)
}

pub(super) fn load(executable: &Path) -> Result<Option<Entry>, String> {
    if executable != Path::new("/usr/lib/chatgpt/ChatGPT") { return Ok(None); }
    let home = super::home()?;
    let user_data = env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty())
        .map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    if !user_data.is_absolute() { return Err("XDG_DATA_HOME 必须是绝对路径。".into()); }
    let mut paths = vec![user_data.join("applications/chatgpt.desktop")];
    let dirs = env::var_os("XDG_DATA_DIRS").filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    paths.extend(env::split_paths(&dirs).filter(|p| p.is_absolute()).map(|p| p.join("applications/chatgpt.desktop")));
    let uid = super::current_uid()?;
    for path in paths {
        match fs::symlink_metadata(&path) {
            Ok(_) => return read(path, &home, uid).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("未找到已安装客户端的原桌面入口；已停止启动以免改变原登录环境。".into())
}

impl Entry {
    pub fn apply(&self, command: &mut Command) {
        command.envs(&self.environment).args(&self.arguments).current_dir(&self.working_directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reuses_original_env_arguments_and_directory_without_shell_or_profile() {
        let temp = tempfile::tempdir().unwrap();
        let entry = parse("[Desktop Entry]\nType=Application\nExec=env HTTPS_PROXY=socks5h://127.0.0.1:10808 NO_PROXY=localhost XMODIFIERS=@im=fcitx chatgpt %U\nPath=\n", temp.path().join("chatgpt.desktop"), temp.path()).unwrap();
        let mut command = Command::new("/usr/lib/chatgpt/ChatGPT"); entry.apply(&mut command);
        assert_eq!(command.get_program(), "/usr/lib/chatgpt/ChatGPT");
        assert!(command.get_args().next().is_none());
        assert_eq!(command.get_current_dir(), Some(temp.path()));
        assert!(command.get_envs().any(|(k,v)| k == "HTTPS_PROXY" && v == Some(std::ffi::OsStr::new("socks5h://127.0.0.1:10808"))));
        assert!(!entry.environment.contains_key("CODEX_ELECTRON_USER_DATA_PATH"));
    }
    #[test]
    fn rejects_shell_unknown_program_and_unhandled_desktop_activation() {
        let temp = tempfile::tempdir().unwrap();
        for exec in ["sh -c chatgpt", "env --ignore-environment chatgpt", "chatgpt ; rm", "chatgpt %Z", "chatgpt \"broken"] {
            assert!(parse(&format!("[Desktop Entry]\nType=Application\nExec={exec}\n"), temp.path().join("chatgpt.desktop"), temp.path()).is_err());
        }
        assert!(parse("[Desktop Entry]\nType=Application\nExec=chatgpt\nDBusActivatable=true\n", temp.path().join("chatgpt.desktop"), temp.path()).is_err());
    }
    #[test]
    fn quoted_literals_and_original_profile_options_are_not_reinterpreted() {
        let temp = tempfile::tempdir().unwrap();
        let entry = parse("[Desktop Entry]\nType=Application\nExec=env \"LABEL=two words\" chatgpt \"--title=literal $value\" %U\n", temp.path().join("chatgpt.desktop"), temp.path()).unwrap();
        assert_eq!(entry.environment["LABEL"], "two words");
        assert_eq!(entry.arguments, ["--title=literal $value"]);
        assert!(tokens("chatgpt \"bad\\q\"").is_err());
    }
}
