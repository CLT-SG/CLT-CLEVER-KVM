//! VNC Input Handling Module
//! 
//! Handles keyboard and mouse input from VNC clients,
//! converting VNC key codes to OS-level input events.

use anyhow::Result;
use enigo::{Enigo, MouseControllable, KeyboardControllable, MouseButton, Key};
use log::{debug, warn};
use std::sync::OnceLock;
use parking_lot::Mutex;

static ENIGO: OnceLock<Mutex<Enigo>> = OnceLock::new();
static LAST_MOUSE_POS: OnceLock<Mutex<(u16, u16)>> = OnceLock::new();

fn get_enigo() -> &'static Mutex<Enigo> {
    ENIGO.get_or_init(|| Mutex::new(Enigo::new()))
}

fn get_last_mouse_pos() -> &'static Mutex<(u16, u16)> {
    LAST_MOUSE_POS.get_or_init(|| Mutex::new((0, 0)))
}

/// Handle VNC keyboard event
pub fn handle_vnc_keyboard(key: u32, down: bool) -> Result<()> {
    let mut enigo = get_enigo().lock();
    
    debug!("Keyboard event: key={} down={}", key, down);

    // Convert VNC keysym to enigo Key
    let enigo_key = match key {
        0xff08 => Key::Backspace,
        0xff09 => Key::Tab,
        0xff0d => Key::Return,
        0xff1b => Key::Escape,
        0xffff => Key::Delete,
        0xff50 => Key::Home,
        0xff51 => Key::LeftArrow,
        0xff52 => Key::UpArrow,
        0xff53 => Key::RightArrow,
        0xff54 => Key::DownArrow,
        0xff55 => Key::PageUp,
        0xff56 => Key::PageDown,
        0xff57 => Key::End,
        0xffe1 => Key::Shift,
        0xffe2 => Key::Shift,
        0xffe3 => Key::Control,
        0xffe4 => Key::Control,
        0xffe5 => Key::CapsLock,
        0xffe7 => Key::Meta,
        0xffe8 => Key::Meta,
        0xffe9 => Key::Alt,
        0xffea => Key::Alt,
        0xffeb => Key::Meta, // Super_L
        0xffec => Key::Meta, // Super_R
        0xffbe..=0xffc9 => {
            // F1-F12
            let f_num = key - 0xffbe + 1;
            match f_num {
                1 => Key::F1,
                2 => Key::F2,
                3 => Key::F3,
                4 => Key::F4,
                5 => Key::F5,
                6 => Key::F6,
                7 => Key::F7,
                8 => Key::F8,
                9 => Key::F9,
                10 => Key::F10,
                11 => Key::F11,
                12 => Key::F12,
                _ => return Ok(()),
            }
        }
        // ASCII printable characters (0x20-0x7e)
        0x20..=0x7e => {
            let ch = key as u8 as char;
            if down {
                enigo.key_click(Key::Layout(ch));
            }
            return Ok(());
        }
        _ => {
            // Try to convert to character if in Latin-1 range
            if key >= 0x20 && key <= 0xff {
                let ch = key as u8 as char;
                if down {
                    enigo.key_click(Key::Layout(ch));
                }
                return Ok(());
            }
            // Unknown key
            debug!("Unknown VNC keysym: 0x{:x}", key);
            return Ok(());
        }
    };

    // Handle key down/up
    if down {
        enigo.key_down(enigo_key);
    } else {
        enigo.key_up(enigo_key);
    }

    Ok(())
}

/// Handle VNC mouse event
pub fn handle_vnc_mouse(button_mask: u8, x: u16, y: u16) -> Result<()> {
    let mut enigo = get_enigo().lock();
    let mut last_pos = get_last_mouse_pos().lock();
    
    debug!("Mouse event: buttons=0x{:x} x={} y={}", button_mask, x, y);

    // Move mouse if position changed
    if last_pos.0 != x || last_pos.1 != y {
        enigo.mouse_move_to(x as i32, y as i32);
        *last_pos = (x, y);
    }

    // Handle button states
    // VNC button mask: bit 0 = left, bit 1 = middle, bit 2 = right, 
    //                  bit 3 = scroll up, bit 4 = scroll down
    
    // Left button (bit 0)
    if button_mask & 0x01 != 0 {
        enigo.mouse_down(MouseButton::Left);
    } else {
        enigo.mouse_up(MouseButton::Left);
    }

    // Middle button (bit 1)
    if button_mask & 0x02 != 0 {
        enigo.mouse_down(MouseButton::Middle);
    } else {
        enigo.mouse_up(MouseButton::Middle);
    }

    // Right button (bit 2)
    if button_mask & 0x04 != 0 {
        enigo.mouse_down(MouseButton::Right);
    } else {
        enigo.mouse_up(MouseButton::Right);
    }

    // Scroll up (bit 3)
    if button_mask & 0x08 != 0 {
        enigo.mouse_scroll_y(1);
    }

    // Scroll down (bit 4)
    if button_mask & 0x10 != 0 {
        enigo.mouse_scroll_y(-1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vnc_keyboard_ascii() {
        // Test ASCII character conversion
        let result = handle_vnc_keyboard(0x61, true); // 'a'
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_keyboard_special() {
        // Test special keys
        let result = handle_vnc_keyboard(0xff0d, true); // Enter
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_movement() {
        // Test mouse movement
        let result = handle_vnc_mouse(0x00, 100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_buttons() {
        // Test mouse button clicks
        let result = handle_vnc_mouse(0x01, 100, 100); // Left button
        assert!(result.is_ok());
    }
}
