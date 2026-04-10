use crate::{config::RuntimeConfig, settings::DesktopSettings};
use std::{fs, io};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu},
};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::WindowId,
};

pub struct TrayState {
    settings: DesktopSettings,
    config: RuntimeConfig,
    browser_url: String,
    server_handle: Option<crate::server::ServerHandle>,
}

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
}

struct TrayApp {
    settings: DesktopSettings,
    config: RuntimeConfig,
    browser_url: String,
    server_handle: Option<crate::server::ServerHandle>,
    tray_icon: Option<TrayIcon>,
    startup_item: Option<CheckMenuItem>,
    browser_item: Option<CheckMenuItem>,
    open_id: Option<MenuId>,
    startup_id: Option<MenuId>,
    browser_id: Option<MenuId>,
    quit_id: Option<MenuId>,
}

impl TrayState {
    pub fn new(
        settings: DesktopSettings,
        config: RuntimeConfig,
        browser_url: String,
        server_handle: crate::server::ServerHandle,
    ) -> Self {
        Self {
            settings,
            config,
            browser_url,
            server_handle: Some(server_handle),
        }
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;

        let proxy = event_loop.create_proxy();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::Menu(event));
        }));

        let proxy = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = proxy.send_event(UserEvent::Tray(event));
        }));

        let mut app = TrayApp {
            settings: self.settings,
            config: self.config,
            browser_url: self.browser_url,
            server_handle: self.server_handle,
            tray_icon: None,
            startup_item: None,
            browser_item: None,
            open_id: None,
            startup_id: None,
            browser_id: None,
            quit_id: None,
        };

        let run_result = event_loop.run_app(&mut app);
        MenuEvent::set_event_handler::<fn(MenuEvent)>(None);
        TrayIconEvent::set_event_handler::<fn(TrayIconEvent)>(None);

        run_result?;
        Ok(())
    }
}

impl TrayApp {
    fn create_tray_once(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.tray_icon.is_some() {
            return Ok(());
        }

        let icon = Icon::from_rgba(desktop_icon_rgba(), 64, 64)?;

        let open_item = MenuItem::new("打开测速页", true, None);
        let quit_item = MenuItem::new("退出", true, None);
        let startup_item = CheckMenuItem::new("开机启动", true, self.settings.autostart, None);
        let browser_item = CheckMenuItem::new(
            "启动时打开浏览器",
            true,
            self.settings.open_browser_on_start,
            None,
        );

        let settings_submenu = Submenu::with_items("设置", true, &[&startup_item, &browser_item])?;

        let menu = Menu::new();
        menu.append_items(&[
            &open_item,
            &settings_submenu,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;

        let tray_icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip(if self.config.locale.starts_with("zh") {
                "测速网页"
            } else {
                "Speed test"
            })
            .build()?;

        self.open_id = Some(open_item.id().clone());
        self.startup_id = Some(startup_item.id().clone());
        self.browser_id = Some(browser_item.id().clone());
        self.quit_id = Some(quit_item.id().clone());
        self.startup_item = Some(startup_item);
        self.browser_item = Some(browser_item);
        self.tray_icon = Some(tray_icon);
        Ok(())
    }

    fn on_menu_event(&mut self, event_loop: &ActiveEventLoop, event: MenuEvent) {
        if self.open_id.as_ref().is_some_and(|id| *id == event.id) {
            let _ = webbrowser::open(&self.browser_url);
            return;
        }

        if self.quit_id.as_ref().is_some_and(|id| *id == event.id) {
            if let Some(handle) = self.server_handle.take() {
                handle.stop();
            }
            event_loop.exit();
            return;
        }

        if self.startup_id.as_ref().is_some_and(|id| *id == event.id) {
            self.settings.autostart = !self.settings.autostart;
            let _ = self.settings.save();
            if let Some(item) = &self.startup_item {
                item.set_checked(self.settings.autostart);
            }

            let _ = if self.settings.autostart {
                register_autostart(&self.settings)
            } else {
                unregister_autostart()
            };
            return;
        }

        if self.browser_id.as_ref().is_some_and(|id| *id == event.id) {
            self.settings.open_browser_on_start = !self.settings.open_browser_on_start;
            let _ = self.settings.save();
            if let Some(item) = &self.browser_item {
                item.set_checked(self.settings.open_browser_on_start);
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        let _ = self.create_tray_once();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(menu_event) => self.on_menu_event(event_loop, menu_event),
            UserEvent::Tray(_tray_event) => {
                // Keep this branch to make tray events observable and easy to extend.
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(handle) = self.server_handle.take() {
            handle.stop();
        }
    }
}

pub fn register_autostart(settings: &DesktopSettings) -> io::Result<()> {
    if !settings.autostart {
        return unregister_autostart();
    }

    let path = crate::settings::startup_entry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let exe = std::env::current_exe()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let exe_string = exe.to_string_lossy().replace('"', "\"");

    if cfg!(target_os = "windows") {
        let content = format!(
            "@echo off\r\nstart \"\" \"{}\" --mode desktop\r\n",
            exe_string
        );
        fs::write(path, content)
    } else if cfg!(target_os = "macos") {
        let content = format!(
            r#"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
  <key>Label</key>
  <string>com.speed-test.autostart</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--mode</string>
    <string>desktop</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#,
            exe_string
        );
        fs::write(path, content)
    } else {
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Speed Test\nExec=\"{}\" --mode desktop\nX-GNOME-Autostart-enabled=true\nNoDisplay=false\n",
            exe_string
        );
        fs::write(path, content)
    }
}

pub fn unregister_autostart() -> io::Result<()> {
    let path = crate::settings::startup_entry_path();
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn desktop_icon_rgba() -> Vec<u8> {
    let size = 64usize;
    let mut pixels = vec![0u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            let in_circle = {
                let dx = x as i32 - 32;
                let dy = y as i32 - 32;
                dx * dx + dy * dy <= 30 * 30
            };

            if in_circle {
                pixels[idx] = 35;
                pixels[idx + 1] = 196;
                pixels[idx + 2] = 183;
                pixels[idx + 3] = 255;
            } else {
                pixels[idx + 3] = 0;
            }
        }
    }

    pixels
}
