//! Physical monitor coordinates; conversion to egui points happens per viewport.
#[derive(Clone, Debug)]
pub struct Display {
    pub id: String,
    pub position: [i32; 2],
    pub size: [i32; 2],
    pub scale: f32,
    pub primary: bool,
}
impl Display {
    pub fn label(&self) -> String {
        format!(
            "{} · {} × {}{}",
            self.id.trim_start_matches("\\\\.\\"),
            self.size[0],
            self.size[1],
            if self.primary { " · primary" } else { "" }
        )
    }
    pub fn window_origin(&self, pixels_per_point: f32) -> [f32; 2] {
        [
            (self.position[0] + 32) as f32 / pixels_per_point,
            (self.position[1] + 32) as f32 / pixels_per_point,
        ]
    }
}

#[cfg(windows)]
pub fn list() -> Vec<Display> {
    use windows_sys::Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW},
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    };
    unsafe extern "system" fn visit(monitor: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) } != 0 {
            let end = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let rect = info.monitorInfo.rcMonitor;
            let (mut dx, mut dy) = (96, 96);
            unsafe {
                GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            }
            // EnumDisplayMonitors calls synchronously; data points at the local Vec.
            unsafe { &mut *(data as *mut Vec<Display>) }.push(Display {
                id: String::from_utf16_lossy(&info.szDevice[..end]),
                position: [rect.left, rect.top],
                size: [rect.right - rect.left, rect.bottom - rect.top],
                scale: dx as f32 / 96.,
                primary: info.monitorInfo.dwFlags & 1 != 0,
            });
        }
        1
    }
    let mut displays: Vec<Display> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(visit),
            &mut displays as *mut Vec<Display> as isize,
        );
    }
    displays.sort_by(|a, b| a.id.cmp(&b.id));
    displays
}
#[cfg(not(windows))]
pub fn list() -> Vec<Display> {
    Vec::new()
}
