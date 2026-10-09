use gpui::App;
use gpui_platform::application;
use ordo::open_native_runtime;

fn main() {
    application().run(|cx: &mut App| {
        open_native_runtime(cx).expect("the native runtime should open");
        cx.activate(true);
    });
}
