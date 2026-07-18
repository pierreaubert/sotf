//! Android dynamic library entry point for SOTF.
//!
//! Android's `NativeActivity` loads this `.so` and the `android-activity`
//! glue calls `android_main` with the process `AndroidApp` handle.

#[cfg(target_os = "android")]
mod imp {
    use gpui::{
        App, AppContext, Application, Context, IntoElement, ParentElement, Render, Styled, Window,
        WindowOptions, div,
    };
    use gpui_android::android::jni::{init_platform, shared_platform};
    use gpui_ui_kit::i18n::I18nState;
    use gpui_ui_kit::theme::{ThemeState, ThemeVariant};

    struct PlaceholderView;

    impl Render for PlaceholderView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(gpui_ui_kit::Text::new("SOTF on Android"))
        }
    }

    #[unsafe(no_mangle)]
    pub fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Info)
                .with_tag("sotf-android"),
        );

        std::panic::set_hook(Box::new(|info| {
            log::error!("SOTF Android panic: {info}");
        }));

        let _platform = init_platform(&app);
        let Some(shared_platform) = shared_platform() else {
            log::error!("android_main: shared_platform() returned None");
            return;
        };

        Application::with_platform(shared_platform.into_rc()).run(|cx: &mut App| {
            cx.set_global(ThemeState::with_variant(ThemeVariant::Dark));
            cx.set_global(I18nState::new());

            log::info!("android_main: opening window");
            match cx.open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| PlaceholderView)
            }) {
                Ok(_) => cx.activate(true),
                Err(error) => log::error!("failed to open Android window: {error}"),
            }
        });

        log::info!("android_main: Application::run returned");
    }
}
