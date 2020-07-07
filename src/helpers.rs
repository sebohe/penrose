use crate::data_types::{CodeMap, Direction, KeyCode};
use std::process;
use xcb;

/// Cycle through a set of indices, wrapping at either end
pub fn cycle_index(ix: usize, max: usize, direction: Direction) -> usize {
    match direction {
        Direction::Forward => return if ix == max { 0 } else { ix + 1 },
        Direction::Backward => return if ix == 0 { max } else { ix - 1 },
    }
}

/**
 * Run an external command, redirecting the process stdout and stderr to /dev/null
 * Logs a warning if there were any errors in kicking off the process.
 */
pub fn spawn<S: Into<String>>(cmd: S) {
    let s = cmd.into();
    let parts: Vec<&str> = s.split_whitespace().collect();
    let result = if parts.len() > 1 {
        process::Command::new(parts[0])
            .args(&parts[1..])
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .spawn()
    } else {
        process::Command::new(parts[0])
            .stdout(process::Stdio::null())
            .stderr(process::Stdio::null())
            .spawn()
    };

    if let Err(e) = result {
        warn!("error spawning external program: {}", e);
    };
}

/**
 * Run the xmodmap command to dump the system keymap table in a form
 * that we can load in and convert back to key codes. This lets the user
 * define key bindings in the way that they would expect while also
 * ensuring that it is east to debug any odd issues with bindings by
 * referring the user to the xmodmap output.
 */
pub fn keycodes_from_xmodmap() -> CodeMap {
    match process::Command::new("xmodmap").arg("-pke").output() {
        Err(e) => die!("unable to fetch keycodes via xmodmap: {}", e),
        Ok(o) => match String::from_utf8(o.stdout) {
            Err(e) => die!("invalid utf8 from xmodmap: {}", e),
            Ok(s) => s
                .lines()
                .flat_map(|l| {
                    let mut words = l.split_whitespace(); // keycode <code> = <names ...>
                    let key_code: u8 = words.nth(1).unwrap().parse().unwrap();
                    words.skip(1).map(move |name| (name.into(), key_code))
                })
                .collect::<CodeMap>(),
        },
    }
}

/**
 * Allow the user to define their keybindings using the gen_keybindings macro
 * which calls through to this. Bindings are of the form '<MOD>-<key name>'
 * with multipple modifiers being allowed, and keynames being taken from the
 * output of 'xmodmap -pke'.
 *
 * Allowed modifiers are:
 *   M - Super
 *   A - Alt
 *   C - Ctrl
 *   S - Shift
 *
 * The user friendly patterns are parsed into a modifier mask and X key code
 * pair that is then grabbed by penrose to trigger the bound action.
 */
pub fn parse_key_binding<S>(pattern: S, known_codes: &CodeMap) -> Option<KeyCode>
where
    S: Into<String>,
{
    let s = pattern.into();
    let mut parts: Vec<&str> = s.split("-").collect();
    match known_codes.get(parts.remove(parts.len() - 1)) {
        Some(code) => {
            let mask = parts
                .iter()
                .map(|s| match s {
                    &"A" => xcb::MOD_MASK_1,
                    &"M" => xcb::MOD_MASK_4,
                    &"S" => xcb::MOD_MASK_SHIFT,
                    &"C" => xcb::MOD_MASK_CONTROL,
                    &_ => die!("invalid key binding prefix: {}", s),
                })
                .fold(0, |acc, v| acc | v);

            // log!("binding '{}' as [{}, {}]", s, mask, code);
            Some(KeyCode {
                mask: mask as u16,
                code: *code,
            })
        }
        None => None,
    }
}