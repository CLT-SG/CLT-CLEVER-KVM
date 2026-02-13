use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use lazy_static::lazy_static;
use std::sync::Mutex;
use log::{debug, info, warn};
use std::thread; // Add missing thread import

lazy_static! {
    // Map for special keys that need more complex handling
    static ref KEY_MAP: HashMap<&'static str, Key> = {
        let mut m = HashMap::new();
        // Function keys
        m.insert("F1", Key::F1);
        m.insert("F2", Key::F2);
        m.insert("F3", Key::F3);
        m.insert("F4", Key::F4);
        m.insert("F5", Key::F5);
        m.insert("F6", Key::F6);
        m.insert("F7", Key::F7);
        m.insert("F8", Key::F8);
        m.insert("F9", Key::F9);
        m.insert("F10", Key::F10);
        m.insert("F11", Key::F11);
        m.insert("F12", Key::F12);
        
        // Navigation keys
        m.insert("Home", Key::Home);
        m.insert("End", Key::End);
        m.insert("PageUp", Key::PageUp);
        m.insert("PageDown", Key::PageDown);
        // m.insert("Insert", Key::Insert); // Not available in this enigo version
        m.insert("Delete", Key::Delete);
        
        // Arrow keys
        m.insert("ArrowUp", Key::UpArrow);
        m.insert("ArrowDown", Key::DownArrow);
        m.insert("ArrowLeft", Key::LeftArrow);
        m.insert("ArrowRight", Key::RightArrow);
        
        // Common keys
        m.insert("Backspace", Key::Backspace);
        m.insert("Tab", Key::Tab);
        m.insert("Enter", Key::Return);
        m.insert("Escape", Key::Escape);
        m.insert("Space", Key::Space);
        m.insert("CapsLock", Key::CapsLock);
        // m.insert("NumLock", Key::Numlock); // Not available in this enigo version
        // m.insert("ScrollLock", Key::ScrollLock); // Not available in this enigo version
        // m.insert("PrintScreen", Key::Print); // Not available in this enigo version
        
        // Modifier keys
        m.insert("Control", Key::Control);
        m.insert("Alt", Key::Alt);
        m.insert("Shift", Key::Shift);
        m.insert("Meta", Key::Meta);
        m.insert("Command", Key::Meta);
        m.insert("Windows", Key::Meta);
        
        m
    };
    
    // Keep track of pressed keys to handle key repeats correctly
    static ref PRESSED_KEYS: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());
}

// Add new input event types
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    #[serde(rename = "mousemove")]
    MouseMove { x: i32, y: i32, monitor_id: Option<String> },
    
    #[serde(rename = "mousedown")]
    MouseDown { button: String, x: i32, y: i32, monitor_id: Option<String> },
    
    #[serde(rename = "mouseup")]
    MouseUp { button: String, x: i32, y: i32, monitor_id: Option<String> },
    
    #[serde(rename = "wheel")]
    MouseWheel { delta_y: i32, delta_x: Option<i32>, monitor_id: Option<String> },
    
    #[serde(rename = "keydown")]
    KeyDown { key: String, code: Option<String>, modifiers: Vec<String>, repeat: Option<bool> },
    
    #[serde(rename = "keyup")]
    KeyUp { key: String, code: Option<String>, modifiers: Vec<String> },
    
    #[serde(rename = "gesture")]
    Gesture { 
        gesture_type: String, 
        scale: Option<f32>,
        rotation: Option<f32>,
        delta_x: Option<i32>,
        delta_y: Option<i32>,
        monitor_id: Option<String>
    },
    
    #[serde(rename = "mousemultitouch")]
    MouseMultiTouch { 
        touches: Vec<TouchPoint>,
        monitor_id: Option<String> 
    },
    
    #[serde(rename = "gamepad")]
    GamepadEvent {
        button: u8,
        value: f32,
        is_pressed: bool
    },
    
    #[serde(rename = "hotkey")]
    HotKey {
        combination: Vec<String>
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TouchPoint {
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub pressure: Option<f32>,
}

pub struct InputHandler {
    enigo: Enigo,
    // Monitor positions and dimensions for multi-monitor support
    monitors: Vec<(String, i32, i32, i32, i32)>, // (id, x, y, width, height)
    active_monitor: usize, // Index of the active monitor
    // Key repeat handling
    key_repeat_delay: Duration,
    key_repeat_interval: Duration,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            enigo: Enigo::new(),
            monitors: Vec::new(),
            active_monitor: 0,
            key_repeat_delay: Duration::from_millis(500),     // Initial delay before repeating
            key_repeat_interval: Duration::from_millis(30),  // Interval between repeats
        }
    }
    
    pub fn update_monitors(&mut self, monitors: Vec<(String, i32, i32, i32, i32)>) {
        self.monitors = monitors;
        info!("Updated monitor configuration with {} monitors", self.monitors.len());
        for (i, (id, x, y, width, height)) in self.monitors.iter().enumerate() {
            debug!("Monitor {}: ID={}, position=({},{}), size={}x{}", 
                   i, id, x, y, width, height);
        }
    }

    pub fn handle_event(&mut self, event: InputEvent) -> Result<(), String> {
        match event {
            InputEvent::MouseMove { x, y, monitor_id } => {
                // Translate coordinates to global screen space if monitor_id is provided
                let (global_x, global_y) = self.translate_coordinates(x, y, monitor_id)?;
                self.enigo.mouse_move_to(global_x, global_y);
                debug!("Mouse move to ({}, {})", global_x, global_y);
            }
            
            InputEvent::MouseDown { button, x, y, monitor_id } => {
                // Translate coordinates to global screen space if monitor_id is provided
                let (global_x, global_y) = self.translate_coordinates(x, y, monitor_id)?;
                self.enigo.mouse_move_to(global_x, global_y);
                let button = self.map_mouse_button(&button)?;
                self.enigo.mouse_down(button);
                debug!("Mouse down {:?} at ({}, {})", button, global_x, global_y);
            }
            
            InputEvent::MouseUp { button, x, y, monitor_id } => {
                // Translate coordinates to global screen space if monitor_id is provided
                let (global_x, global_y) = self.translate_coordinates(x, y, monitor_id)?;
                self.enigo.mouse_move_to(global_x, global_y);
                let button = self.map_mouse_button(&button)?;
                self.enigo.mouse_up(button);
                debug!("Mouse up {:?} at ({}, {})", button, global_x, global_y);
            }
            
            InputEvent::MouseWheel { delta_y, delta_x, monitor_id: _ } => {
                // Vertical scrolling
                let y_click_count = (delta_y / 120).abs() as usize;
                for _ in 0..y_click_count {
                    if delta_y > 0 {
                        self.enigo.mouse_scroll_y(-1);
                    } else {
                        self.enigo.mouse_scroll_y(1);
                    }
                }
                
                // Horizontal scrolling (if supported and provided)
                if let Some(delta_x) = delta_x {
                    let x_click_count = (delta_x / 120).abs() as usize;
                    for _ in 0..x_click_count {
                        if delta_x > 0 {
                            self.enigo.mouse_scroll_x(1);
                        } else {
                            self.enigo.mouse_scroll_x(-1);
                        }
                    }
                }
            }
            
            InputEvent::KeyDown { key, code, modifiers, repeat } => {
                // Handle key repeats
                let should_send = if repeat.unwrap_or(false) {
                    // Check if we should send a repeat based on timing
                    if let Some(pressed_time) = PRESSED_KEYS.lock().unwrap().get(&key) {
                        let elapsed = pressed_time.elapsed();
                        
                        if elapsed >= self.key_repeat_delay {
                            // After initial delay, send repeats at regular intervals
                            let repeat_count = ((elapsed - self.key_repeat_delay).as_millis() 
                                               / self.key_repeat_interval.as_millis()) as u64;
                            
                            // If time for a new repeat
                            let next_repeat_time = self.key_repeat_delay + 
                                                  self.key_repeat_interval.mul_f64(repeat_count as f64);
                            
                            elapsed >= next_repeat_time
                        } else {
                            // Still in initial delay period
                            false
                        }
                    } else {
                        // Key not in pressed keys map (shouldn't happen)
                        true
                    }
                } else {
                    // First press, not a repeat
                    // Store the key and current time
                    PRESSED_KEYS.lock().unwrap().insert(key.clone(), Instant::now());
                    true
                };
                
                if should_send {
                    self.handle_modifiers(&modifiers, true)?;
                    
                    // Try to use code first if available (more reliable for keyboard layouts)
                    if let Some(code_str) = code {
                        if let Ok(key) = self.map_key_code(&code_str) {
                            self.enigo.key_down(key);
                            return Ok(());
                        }
                    }
                    
                    // Fall back to key if code mapping failed
                    let key = self.map_key(&key)?;
                    self.enigo.key_down(key);
                }
            }
            
            InputEvent::KeyUp { key, code, modifiers } => {
                // Remove from pressed keys map
                PRESSED_KEYS.lock().unwrap().remove(&key);
                
                // Try to use code first if available
                if let Some(code_str) = code {
                    if let Ok(key) = self.map_key_code(&code_str) {
                        self.enigo.key_up(key);
                        self.handle_modifiers(&modifiers, false)?;
                        return Ok(());
                    }
                }
                
                // Fall back to key
                let key = self.map_key(&key)?;
                self.enigo.key_up(key);
                self.handle_modifiers(&modifiers, false)?;
            }
            
            InputEvent::Gesture { gesture_type, scale, rotation, delta_x, delta_y, monitor_id: _ } => {
                // Handle multi-touch gestures
                match gesture_type.as_str() {
                    "pinch" => {
                        if let Some(scale) = scale {
                            // Convert pinch to zoom in/out
                            if scale > 1.0 {
                                // Zoom in (Ctrl + '+')
                                self.enigo.key_down(Key::Control);
                                self.enigo.key_click(Key::Layout('+'));
                                self.enigo.key_up(Key::Control);
                            } else if scale < 1.0 {
                                // Zoom out (Ctrl + '-')
                                self.enigo.key_down(Key::Control);
                                self.enigo.key_click(Key::Layout('-'));
                                self.enigo.key_up(Key::Control);
                            }
                        }
                    },
                    "rotate" => {
                        if let Some(_rotation) = rotation {
                            // Implement rotation gesture
                            // Not commonly supported in desktop apps, but could map to specific actions
                        }
                    },
                    "pan" => {
                        if let (Some(dx), Some(dy)) = (delta_x, delta_y) {
                            // Implement panning
                            // Could map to arrow keys or scrolling
                            if dx.abs() > dy.abs() {
                                // Horizontal pan
                                if dx > 0 {
                                    self.enigo.key_click(Key::RightArrow);
                                } else {
                                    self.enigo.key_click(Key::LeftArrow);
                                }
                            } else {
                                // Vertical pan
                                if dy > 0 {
                                    self.enigo.key_click(Key::DownArrow);
                                } else {
                                    self.enigo.key_click(Key::UpArrow);
                                }
                            }
                        }
                    },
                    _ => {
                        warn!("Unsupported gesture type: {}", gesture_type);
                    }
                }
            }
            
            InputEvent::MouseMultiTouch { touches, monitor_id } => {
                // Implementation for multi-touch gestures
                // This is a placeholder - real implementation would depend on platform support
                if !touches.is_empty() {
                    // Just move to the first touch point for basic compatibility
                    let primary = &touches[0];
                    let (global_x, global_y) = self.translate_coordinates(primary.x, primary.y, monitor_id)?;
                    self.enigo.mouse_move_to(global_x, global_y);
                    debug!("Multi-touch primary point: ({}, {})", global_x, global_y);
                }
            }
            
            InputEvent::GamepadEvent { button, value, is_pressed } => {
                // Gamepad events could be mapped to keyboard/mouse actions
                // This is a placeholder - real implementation would depend on use case
                debug!("Gamepad event: button={}, value={}, pressed={}", button, value, is_pressed);
                
                // Example: Map some gamepad buttons to keyboard keys
                match button {
                    0 => { // A button
                        if is_pressed {
                            self.enigo.key_down(Key::Layout(' '));
                        } else {
                            self.enigo.key_up(Key::Layout(' '));
                        }
                    },
                    1 => { // B button
                        if is_pressed {
                            self.enigo.key_down(Key::Escape);
                        } else {
                            self.enigo.key_up(Key::Escape);
                        }
                    },
                    // Map more buttons as needed
                    _ => {}
                }
            }
            
            InputEvent::HotKey { combination } => {
                // Handle special hotkey combinations
                debug!("HotKey: {:?}", combination);
                
                // Press all keys in the combination
                let mut keys = Vec::new();
                for key_name in &combination {
                    if let Ok(key) = self.map_key(key_name) {
                        self.enigo.key_down(key);
                        keys.push(key);
                    }
                }
                
                // Small delay
                thread::sleep(Duration::from_millis(50));
                
                // Release all keys in reverse order
                for key in keys.into_iter().rev() {
                    self.enigo.key_up(key);
                }
            },
        }
        
        Ok(())
    }
    
    fn translate_coordinates(&self, x: i32, y: i32, monitor_id: Option<String>) -> Result<(i32, i32), String> {
        if self.monitors.is_empty() {
            // No monitor configuration, use coordinates as-is
            return Ok((x, y));
        }
        
        // Find the monitor by ID or use the active monitor
        let monitor_idx = if let Some(id) = monitor_id {
            self.monitors.iter().position(|(m_id, _, _, _, _)| m_id == &id)
                .unwrap_or(self.active_monitor)
        } else {
            self.active_monitor
        };
        
        if monitor_idx >= self.monitors.len() {
            return Err(format!("Invalid monitor index: {}", monitor_idx));
        }
        
        let (_, offset_x, offset_y, _, _) = &self.monitors[monitor_idx];
        
        // Translate local coordinates to global screen space
        Ok((x + offset_x, y + offset_y))
    }

    fn map_mouse_button(&self, button: &str) -> Result<MouseButton, String> {
        match button {
            "left" => Ok(MouseButton::Left),
            "right" => Ok(MouseButton::Right),
            "middle" => Ok(MouseButton::Middle),
            _ => Err(format!("Unsupported mouse button: {}", button)),
        }
    }
    
    // Improved key mapping with better international keyboard support
    fn map_key(&self, key: &str) -> Result<Key, String> {
        // Check if it's a special key in our map
        if let Some(mapped_key) = KEY_MAP.get(key) {
            return Ok(*mapped_key);
        }
        
        // Handle single character keys with better Unicode support
        if key.chars().count() == 1 {
            let ch = key.chars().next().unwrap();
            return Ok(Key::Layout(ch));
        }
        
        // Handle numeric keypad keys
        if key.starts_with("Numpad") && key.len() > 6 {
            let ch = key.chars().nth(6).unwrap();
            if ch.is_digit(10) {
                return Ok(Key::Layout(ch));
            }
            
            // Handle numpad operators
            match &key[6..] {
                "Add" => return Ok(Key::Layout('+')),
                "Subtract" => return Ok(Key::Layout('-')),
                "Multiply" => return Ok(Key::Layout('*')),
                "Divide" => return Ok(Key::Layout('/')),
                "Decimal" => return Ok(Key::Layout('.')),
                _ => {}
            }
        }
        
        // Handle Dead keys for international keyboards
        if key.starts_with("Dead") {
            match &key[4..] {
                "Acute" => return Ok(Key::Layout('´')),
                "Grave" => return Ok(Key::Layout('`')),
                "Circumflex" => return Ok(Key::Layout('^')),
                "Tilde" => return Ok(Key::Layout('~')),
                "Diaeresis" => return Ok(Key::Layout('¨')),
                _ => {}
            }
        }
        
        // Fall back for unknown keys
        Err(format!("Unsupported key: {}", key))
    }
    
    fn map_key_code(&self, code: &str) -> Result<Key, String> {
        // Map from KeyboardEvent.code to Enigo Key
        // See: https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code
        
        // Key codes for letters (KeyA through KeyZ)
        if code.len() == 4 && code.starts_with("Key") {
            let ch = code.chars().nth(3).unwrap();
            if ch.is_ascii_alphabetic() {
                return Ok(Key::Layout(ch.to_ascii_lowercase()));
            }
        }
        
        // Digit keys (Digit0 through Digit9)
        if code.len() == 6 && code.starts_with("Digit") {
            let ch = code.chars().nth(5).unwrap();
            if ch.is_digit(10) {
                return Ok(Key::Layout(ch));
            }
        }
        
        // Function keys
        match code {
            "F1" => Ok(Key::F1),
            "F2" => Ok(Key::F2),
            "F3" => Ok(Key::F3),
            "F4" => Ok(Key::F4),
            "F5" => Ok(Key::F5),
            "F6" => Ok(Key::F6),
            "F7" => Ok(Key::F7),
            "F8" => Ok(Key::F8),
            "F9" => Ok(Key::F9),
            "F10" => Ok(Key::F10),
            "F11" => Ok(Key::F11),
            "F12" => Ok(Key::F12),
            // Navigation
            "Home" => Ok(Key::Home),
            "End" => Ok(Key::End),
            "PageUp" => Ok(Key::PageUp),
            "PageDown" => Ok(Key::PageDown),
            // "Insert" => Ok(Key::Insert), // Not available in this enigo version
            "Insert" => Err(format!("Insert key not supported in this enigo version")),
            "Delete" => Ok(Key::Delete),
            "ArrowUp" => Ok(Key::UpArrow),
            "ArrowDown" => Ok(Key::DownArrow),
            "ArrowLeft" => Ok(Key::LeftArrow),
            "ArrowRight" => Ok(Key::RightArrow),
            // Common keys
            "Backspace" => Ok(Key::Backspace),
            "Tab" => Ok(Key::Tab),
            "Enter" => Ok(Key::Return),
            "Escape" => Ok(Key::Escape),
            "Space" => Ok(Key::Space),
            "CapsLock" => Ok(Key::CapsLock),
            // Modifiers
            "ControlLeft" | "ControlRight" => Ok(Key::Control),
            "AltLeft" | "AltRight" => Ok(Key::Alt),
            "ShiftLeft" | "ShiftRight" => Ok(Key::Shift),
            "MetaLeft" | "MetaRight" => Ok(Key::Meta),
            // Other common mappings
            "Backquote" => Ok(Key::Layout('`')),
            "Minus" => Ok(Key::Layout('-')),
            "Equal" => Ok(Key::Layout('=')),
            "BracketLeft" => Ok(Key::Layout('[')),
            "BracketRight" => Ok(Key::Layout(']')),
            "Backslash" => Ok(Key::Layout('\\')),
            "Semicolon" => Ok(Key::Layout(';')),
            "Quote" => Ok(Key::Layout('\'')),
            "Comma" => Ok(Key::Layout(',')),
            "Period" => Ok(Key::Layout('.')),
            "Slash" => Ok(Key::Layout('/')),
            // Numpad
            "Numpad0" => Ok(Key::Layout('0')),
            "Numpad1" => Ok(Key::Layout('1')),
            "Numpad2" => Ok(Key::Layout('2')),
            "Numpad3" => Ok(Key::Layout('3')),
            "Numpad4" => Ok(Key::Layout('4')),
            "Numpad5" => Ok(Key::Layout('5')),
            "Numpad6" => Ok(Key::Layout('6')),
            "Numpad7" => Ok(Key::Layout('7')),
            "Numpad8" => Ok(Key::Layout('8')),
            "Numpad9" => Ok(Key::Layout('9')),
            "NumpadAdd" => Ok(Key::Layout('+')),
            "NumpadSubtract" => Ok(Key::Layout('-')),
            "NumpadMultiply" => Ok(Key::Layout('*')),
            "NumpadDivide" => Ok(Key::Layout('/')),
            "NumpadDecimal" => Ok(Key::Layout('.')),
            "NumpadEnter" => Ok(Key::Return),
            // Keys not supported in this enigo version
            "NumLock" => Err(format!("NumLock key not supported in this enigo version")),
            "ScrollLock" => Err(format!("ScrollLock key not supported in this enigo version")),
            "PrintScreen" => Err(format!("PrintScreen key not supported in this enigo version")),
            // Add more as needed
            _ => Err(format!("Unsupported key code: {}", code)),
        }
    }

    fn handle_modifiers(&mut self, modifiers: &[String], is_down: bool) -> Result<(), String> {
        for modifier in modifiers {
            let key = match modifier.as_str() {
                "Control" => Key::Control,
                "Alt" => Key::Alt,
                "Shift" => Key::Shift,
                "Meta" | "Command" | "Windows" => Key::Meta,
                _ => return Err(format!("Unsupported modifier: {}", modifier)),
            };
            
            if is_down {
                self.enigo.key_down(key);
            } else {
                self.enigo.key_up(key);
            }
        }
        
        Ok(())
    }
    
    pub fn set_active_monitor(&mut self, monitor_id: &str) -> Result<(), String> {
        if let Some(idx) = self.monitors.iter().position(|(id, _, _, _, _)| id == monitor_id) {
            self.active_monitor = idx;
            info!("Set active monitor to {} (index {})", monitor_id, idx);
            Ok(())
        } else {
            Err(format!("Monitor with ID {} not found", monitor_id))
        }
    }
}
