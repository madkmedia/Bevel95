//! About Bevel95: the bar's Bevel95 > About Bevel95 and Quick Search's "About Bevel95" open it
//! (`bevel_core::about::AboutWindow`, a window of the control kit), in the middle of the bar's
//! screen, as the bar's other windows open. OK, Esc and the title bar's X close it.

use bevel_core::about::{AboutWindow, Part};

use crate::bar::with_app;
use crate::kit::{self, Hosted};
use crate::log::log;
use crate::util::take_focus;

pub struct AboutHost {
    form: AboutWindow,
}

impl Hosted for AboutHost {
    type F = AboutWindow;
    fn form(&self) -> &AboutWindow {
        &self.form
    }
    fn form_mut(&mut self) -> &mut AboutWindow {
        &mut self.form
    }
    fn activate(part: Part) {
        match part {
            Part::Ok | Part::Close => close(),
        }
    }
}

/// Opens the window (if it is open already, it comes forward).
pub fn open() {
    if let Some(hwnd) = kit::hwnd::<AboutHost>() {
        take_focus(hwnd);
        return;
    }
    let Some((era, m)) = with_app(|app| (app.era, app.monitor)) else {
        return;
    };
    let form = AboutWindow::new(era, bevel_winkit::BUILD);
    let place = |w: i32, h: i32| (m.left + (m.right - m.left - w) / 2, m.top + (m.bottom - m.top - h) / 2);
    if kit::open(AboutHost { form }, place).is_some() {
        log("opened About Bevel95");
    }
}

pub fn close() {
    kit::close::<AboutHost>();
}

/// The bar's look, colors or size changed: an open window closes (it belongs to the old look).
pub fn look_changed() {
    if kit::hwnd::<AboutHost>().is_some() {
        close();
    }
}
