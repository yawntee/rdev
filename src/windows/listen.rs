use std::os::raw::c_int;
use std::ptr::null_mut;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::SystemTime;

use once_cell::sync::OnceCell;
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::um::winuser::{CallNextHookEx, GetMessageA, HC_ACTION};

use crate::rdev::{Event, EventType, ListenError};
use crate::windows::common::{convert, HOOK, HookError, KEYBOARD, set_key_hook, set_mouse_hook};

static TRANSFER: OnceCell<Sender<(WPARAM, LPARAM)>> = OnceCell::new();

impl From<HookError> for ListenError {
    fn from(error: HookError) -> Self {
        match error {
            HookError::Mouse(code) => ListenError::MouseHookError(code),
            HookError::Key(code) => ListenError::KeyHookError(code),
        }
    }
}

unsafe extern "system" fn raw_callback(code: c_int, param: WPARAM, lpdata: LPARAM) -> LRESULT {
    if code == HC_ACTION {
        if let Some(transfer) = TRANSFER.get() {
            let _ = transfer.send((param, lpdata));
        }
    }
    CallNextHookEx(HOOK, code, param, lpdata)
}

pub fn listen(callback: fn(Event)) -> Result<(), ListenError>
{
    unsafe {
        let (tx, rx) = channel();

        let _ = TRANSFER.get_or_init(move || tx);

        thread::spawn(move || {
            while let Ok((param, lpdata)) = rx.recv() {
                let opt = convert(param, lpdata);
                if let Some(event_type) = opt {
                    let name = match &event_type {
                        EventType::KeyPress(_key) => match (*KEYBOARD).lock() {
                            Ok(mut keyboard) => keyboard.get_name(lpdata),
                            Err(_) => None,
                        },
                        _ => None,
                    };
                    let event = Event {
                        event_type,
                        time: SystemTime::now(),
                        name,
                    };
                    callback(event)
                }
            }
        });

        set_key_hook(raw_callback)?;
        set_mouse_hook(raw_callback)?;

        GetMessageA(null_mut(), null_mut(), 0, 0);
    }
    Ok(())
}
