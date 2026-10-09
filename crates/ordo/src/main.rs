use gpui::App;
use gpui_platform::application;
use ordo::open_native_runtime;
use ordo::performance::StartupClock;

fn main() {
    // Must be the first statement: startup time is measured from here.
    StartupClock::capture_process_start();
    application().run(|cx: &mut App| {
        open_native_runtime(cx).expect("the native runtime should open");
        cx.activate(true);
    });
}
