#![cfg(target_os = "nuttx")]

extern crate alloc;

#[allow(dead_code)]
mod color_probe;
#[allow(dead_code)]
mod lite;
mod widgets;

use mirui::app::App;
use mirui::surface::nuttx::{NuttxConfig, NuttxFbSurface};

#[unsafe(no_mangle)]
pub extern "C" fn mirui_main(_argc: i32, _argv: *const *const u8) -> i32 {
    let backend = match NuttxFbSurface::open(NuttxConfig::default()) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("mirui-nuttx-demo: open backend failed: {err}");
            return 1;
        }
    };
    let mut app = App::new(backend);
    app.with_default_widgets().with_default_systems();
    let root = widgets::build(&mut app);
    app.set_root(root);
    app.run();
    0
}
