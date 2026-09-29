//! Windows startup serialization only; no shell, environment mutation or process control.
use std::io;

pub(super) const MAX_ENV_UNITS: usize = 1024 * 1024;
const MAX_COMMAND_UNITS: usize = 32767; // CreateProcessW includes the terminating NUL.

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

/// argv[0] uses the executable-name rule; later arguments use the MS C runtime rule.
pub(super) fn command_line(program: &[u16], args: &[Vec<u16>]) -> io::Result<Vec<u16>> {
    if program.is_empty() || program.contains(&0) || program.contains(&34) {
        return Err(invalid("invalid Windows engine executable name"));
    }
    if program.len() >= MAX_COMMAND_UNITS {
        return Err(invalid(
            "Windows engine command line exceeds 32767 UTF-16 units",
        ));
    }
    let mut out = vec![34];
    out.extend_from_slice(program);
    out.push(34);
    for arg in args {
        if arg.contains(&0) || arg.len() >= MAX_COMMAND_UNITS {
            return Err(invalid("invalid or oversized Windows engine argument"));
        }
        out.extend([32, 34]);
        let mut slashes = 0;
        for &unit in arg {
            match unit {
                92 => slashes += 1,
                34 => {
                    out.extend(std::iter::repeat_n(92, slashes * 2 + 1));
                    out.push(34);
                    slashes = 0;
                }
                _ => {
                    out.extend(std::iter::repeat_n(92, slashes));
                    out.push(unit);
                    slashes = 0;
                }
            }
        }
        out.extend(std::iter::repeat_n(92, slashes * 2));
        out.push(34);
        if out.len() >= MAX_COMMAND_UNITS {
            return Err(invalid(
                "Windows engine command line exceeds 32767 UTF-16 units",
            ));
        }
    }
    out.push(0);
    if out.len() > MAX_COMMAND_UNITS {
        return Err(invalid(
            "Windows engine command line exceeds 32767 UTF-16 units",
        ));
    }
    Ok(out)
}

fn ascii_name_eq(name: &[u16], expected: &[u8]) -> bool {
    name.len() == expected.len()
        && name
            .iter()
            .zip(expected)
            .all(|(&unit, &byte)| unit <= 127 && (unit as u8).eq_ignore_ascii_case(&byte))
}

/// Preserve inherited ordering, UTF-16 and hidden drive entries; remove all Node overrides.
pub(super) fn filtered_environment(block: &[u16]) -> io::Result<Vec<u16>> {
    if block.len() > MAX_ENV_UNITS || !block.ends_with(&[0, 0]) {
        return Err(invalid("invalid or oversized Windows environment block"));
    }
    if block == [0, 0] {
        return Ok(vec![0, 0]);
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < block.len() - 1 {
        let end = start + block[start..].iter().position(|&u| u == 0).unwrap();
        if end == start {
            return Err(invalid(
                "unexpected data after Windows environment terminator",
            ));
        }
        let entry = &block[start..end];
        let separator = entry
            .iter()
            .position(|&u| u == 61)
            .ok_or_else(|| invalid("malformed Windows environment entry"))?;
        let name = &entry[..separator];
        if !ascii_name_eq(name, b"NODE_OPTIONS") && !ascii_name_eq(name, b"NODE_PATH") {
            out.extend_from_slice(entry);
            out.push(0);
        }
        start = end + 1;
    }
    out.push(0);
    if out.len() == 1 {
        out.push(0);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    #[test]
    fn crt_arguments_preserve_empty_quotes_slashes_and_shell_metacharacters() {
        let args = [
            "",
            "two words",
            "a\"b",
            "tail\\",
            "tab\tvalue",
            "中文",
            "%PATH% & ! ^",
        ]
        .map(wide);
        let line = command_line(&wide(r"C:\Program Files\node.exe"), &args).unwrap();
        assert_eq!(
            String::from_utf16(&line[..line.len() - 1]).unwrap(),
            "\"C:\\Program Files\\node.exe\" \"\" \"two words\" \"a\\\"b\" \"tail\\\\\" \"tab\tvalue\" \"中文\" \"%PATH% & ! ^\""
        );
    }

    #[test]
    fn command_rejects_nuls_and_oversize_before_any_process_creation() {
        assert!(command_line(&wide("node.exe"), &[vec![0]]).is_err());
        assert!(command_line(&wide("bad\"name.exe"), &[]).is_err());
        assert!(command_line(&wide("node.exe"), &[vec![65; MAX_COMMAND_UNITS]]).is_err());
        assert!(command_line(&vec![65; MAX_COMMAND_UNITS - 2], &[]).is_err());
    }

    #[test]
    fn environment_removes_case_variants_without_losing_drive_or_unicode_entries() {
        let input = wide("=C:=C:\\work\0Node_Options=bad\0NODE_PATH=bad\0Path=\0USER=中文\0\0");
        assert_eq!(
            filtered_environment(&input).unwrap(),
            wide("=C:=C:\\work\0Path=\0USER=中文\0\0")
        );
        assert_eq!(
            filtered_environment(&wide("NODE_OPTIONS=bad\0node_path=bad\0\0")).unwrap(),
            [0, 0]
        );
        assert_eq!(filtered_environment(&[0, 0]).unwrap(), [0, 0]);
    }

    #[test]
    fn environment_refuses_invalid_terminators_and_excessive_blocks() {
        assert!(filtered_environment(&wide("A=B\0")).is_err());
        assert!(filtered_environment(&wide("A=B\0\0C=D\0\0")).is_err());
        assert!(filtered_environment(&vec![0; MAX_ENV_UNITS + 1]).is_err());
    }
}
