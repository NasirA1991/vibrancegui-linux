use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Shortcut {
    pub appid: u32,
    pub name: String,
    pub exe: PathBuf,
    pub start_dir: Option<PathBuf>,
    pub icon: Option<PathBuf>,
    pub launch_options: String,
}

#[derive(Debug)]
enum Value {
    String(String),
    Int(i32),
    Object(BTreeMap<String, Value>),
}

pub fn parse_file(path: &Path) -> Result<Vec<Shortcut>> {
    let bytes = std::fs::read(path)?;
    let (root, _) = parse_object(&bytes, 0)?;
    let shortcuts = root
        .get("shortcuts")
        .and_then(|v| match v {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .context("missing shortcuts object")?;

    let mut out = Vec::new();
    for value in shortcuts.values() {
        let Value::Object(fields) = value else {
            continue;
        };
        let appid = match fields.get("appid") {
            Some(Value::Int(v)) => *v as u32,
            _ => continue,
        };
        let name = match fields.get("AppName").or_else(|| fields.get("appname")) {
            Some(Value::String(v)) => v.clone(),
            _ => continue,
        };
        let exe = match fields.get("exe").or_else(|| fields.get("Exe")) {
            Some(Value::String(v)) => PathBuf::from(unquote(v)),
            _ => continue,
        };
        if name.trim().is_empty() || exe.as_os_str().is_empty() {
            continue;
        }
        let start_dir = fields.get("StartDir").and_then(|v| match v {
            Value::String(s) if !s.is_empty() => Some(PathBuf::from(unquote(s))),
            _ => None,
        });
        let icon = fields.get("icon").and_then(|v| match v {
            Value::String(s) if !s.is_empty() => Some(PathBuf::from(unquote(s))),
            _ => None,
        });
        let launch_options = fields
            .get("LaunchOptions")
            .or_else(|| fields.get("launchoptions"))
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();
        out.push(Shortcut {
            appid,
            name,
            exe,
            start_dir,
            icon,
            launch_options,
        });
    }
    Ok(out)
}

fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

fn parse_object(data: &[u8], mut pos: usize) -> Result<(BTreeMap<String, Value>, usize)> {
    let mut map = BTreeMap::new();
    loop {
        let ty = *data.get(pos).context("unexpected EOF")?;
        pos += 1;
        if ty == 0x08 {
            return Ok((map, pos));
        }
        let key = read_cstr(data, &mut pos)?;
        let value = match ty {
            0x00 => {
                let (child, next) = parse_object(data, pos)?;
                pos = next;
                Value::Object(child)
            }
            0x01 => Value::String(read_cstr(data, &mut pos)?),
            0x02 => {
                let bytes = data.get(pos..pos + 4).context("short int")?;
                pos += 4;
                Value::Int(i32::from_le_bytes(bytes.try_into()?))
            }
            0x07 => {
                let bytes = data.get(pos..pos + 8).context("short uint64")?;
                pos += 8;
                let v = u64::from_le_bytes(bytes.try_into()?);
                Value::Int(v as i32)
            }
            0x03 => {
                let bytes = data.get(pos..pos + 4).context("short float")?;
                pos += 4;
                let v = f32::from_le_bytes(bytes.try_into()?);
                Value::Int(v as i32)
            }
            other => anyhow::bail!("unsupported binary VDF type {other:#x}"),
        };
        map.insert(key, value);
    }
}

fn read_cstr(data: &[u8], pos: &mut usize) -> Result<String> {
    let rest = data.get(*pos..).context("unexpected EOF")?;
    let end = rest
        .iter()
        .position(|b| *b == 0)
        .context("unterminated string")?;
    let s = String::from_utf8_lossy(&rest[..end]).to_string();
    *pos += end + 1;
    Ok(s)
}
