use gpui::{App, AppContext, Bounds, Focusable, WindowBounds, WindowOptions, px, size};
use gpui_platform::application;
use ui::lab::LabView;

/// `ui-lab [scenario]` opens straight onto a scenario by name, which is what a
/// review of one control wants; `F2` still cycles from wherever it lands.
fn main() {
    let requested = std::env::args().nth(1);
    if let Some(name) = requested.as_deref()
        && !ui::lab::scenarios().iter().any(|s| s.name == name)
    {
        eprintln!("ui-lab: no scenario named {name}. Known scenarios:");
        for scenario in ui::lab::scenarios() {
            eprintln!("  {}", scenario.name);
        }
        std::process::exit(2);
    }
    application().run(move |cx: &mut App| {
        ui::lab::init(cx).expect("the UI environment should initialize");
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let requested = requested.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    let mut lab = LabView::new(window, cx);
                    if let Some(name) = requested.as_deref() {
                        lab.select_scenario(name, cx);
                    }
                    window.focus(&lab.focus_handle(cx), cx);
                    lab
                })
            },
        )
        .expect("the UI laboratory window should open");
        cx.activate(true);
    });
}
