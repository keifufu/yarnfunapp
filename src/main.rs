use gtk4::{Application, ApplicationWindow, Picture, Box, Orientation};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::sync::mpsc::{self, Receiver, Sender};
use evdev::{Device, EventType, KeyCode};
use std::time::{Duration, Instant};
use std::cell::RefCell;
use gtk4::prelude::*;
use std::rc::Rc;
use std::thread;
use gtk4::gdk;
use std::env;
use std::io;

const IDLE_IMAGE: &[u8] = include_bytes!("../images/idle.png");
const LEFT_IMAGE: &[u8] = include_bytes!("../images/left.png");
const RIGHT_IMAGE: &[u8] = include_bytes!("../images/right.png");

#[derive(Debug)]
enum InputMessage {
  KeyPress(KeyCode),
}

#[derive(Clone)]
struct Config {
  output: Option<String>,
  x: i32,
  y: i32,
  idle_ms: u64,
  switch_debounce_ms: u64,
  size: i32,
  flip: bool,
  bounce: bool,
  osu: bool,
}

fn parse_env<T>(name: &str, default: T) -> T
where
  T: std::str::FromStr,
{
  env::var(name)
    .ok()
    .and_then(|value| value.parse().ok())
    .unwrap_or(default)
}

impl Config {
  fn from_env() -> Self {
    Self {
      output: env::var("YARN_OUTPUT").ok(),
      x: parse_env("YARN_X", 20),
      y: parse_env("YARN_Y", 20),
      idle_ms: parse_env("YARN_IDLE_MS", 800),
      switch_debounce_ms: parse_env("YARN_SWITCH_DEBOUNCE_MS", 25),
      size: parse_env("YARN_SIZE", 256),
      flip: env::var("YARN_FLIP")
        .map(|value| {
          matches!(
            value.to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
          )
        })
        .unwrap_or(false),
      bounce: env::var("YARN_BOUNCE")
        .map(|value| {
          matches!(
            value.to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
          )
        })
        .unwrap_or(true),
      osu: env::var("YARN_OSU")
        .map(|value| {
          matches!(
            value.to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
          )
        })
        .unwrap_or(false),
    }
  }
}

fn texture_from_bytes(bytes: &'static [u8], name: &str) -> gdk::Texture {
  let bytes = gtk4::glib::Bytes::from_static(bytes);

  gdk::Texture::from_bytes(&bytes)
    .unwrap_or_else(|error| panic!("failed to load embedded image {name}: {error}"))
}

fn find_output(
  display: &gtk4::gdk::Display,
  requested: &str,
) -> Option<gtk4::gdk::Monitor> {
  let monitors = display.monitors();

  for index in 0..monitors.n_items() {
    let object = monitors.item(index)?;
    let monitor = object.downcast::<gtk4::gdk::Monitor>().ok()?;

    if monitor.connector().as_deref() == Some(requested) {
      return Some(monitor);
    }
  }

  None
}

fn is_ignored_key(code: u16) -> bool {
  code == KeyCode::KEY_LEFTCTRL.0
  || code == KeyCode::KEY_RIGHTCTRL.0
  || code == KeyCode::KEY_LEFTSHIFT.0
  || code == KeyCode::KEY_RIGHTSHIFT.0
  || code == KeyCode::KEY_LEFTALT.0
  || code == KeyCode::KEY_RIGHTALT.0
  || code == KeyCode::KEY_LEFTMETA.0
  || code == KeyCode::KEY_RIGHTMETA.0
  || code == KeyCode::KEY_CAPSLOCK.0
  || code == KeyCode::KEY_NUMLOCK.0
  || code == KeyCode::KEY_SCROLLLOCK.0
  // || (KeyCode::KEY_F1.0..=KeyCode::KEY_F12.0).contains(&code)
}

fn input_thread(tx: Sender<InputMessage>) -> io::Result<()> {
  let mut devices = Vec::new();

  for entry in std::fs::read_dir("/dev/input")?.flatten() {
    let path = entry.path();

    let is_event_device = path
      .file_name()
      .is_some_and(|name| name.to_string_lossy().starts_with("event"));

    if !is_event_device {
      continue;
    }

    match Device::open(&path) {
      Ok(device) => {
        let Some(keys) = device.supported_keys() else {
          continue;
        };

        let is_keyboard = keys.contains(KeyCode::KEY_A)
          && keys.contains(KeyCode::KEY_Z)
          && keys.contains(KeyCode::KEY_ENTER)
          && keys.contains(KeyCode::KEY_SPACE);

        if !is_keyboard {
          continue;
        }

        if let Err(error) = device.set_nonblocking(true) {
          eprintln!("cannot make {} nonblocking: {error}", path.display());
          continue;
        }

        eprintln!(
          "listening to {} ({})",
          path.display(),
          device.name().unwrap_or("unnamed")
        );

        devices.push(device);
      }
      Err(error) => {
        eprintln!("cannot open {}: {error}", path.display());
      }
    }
  }

  if devices.is_empty() {
    return Err(io::Error::new(
      io::ErrorKind::NotFound,
      "no input devices could be opened. check /dev/input permissions",
    ));
  }

  loop {
    for device in &mut devices {
      match device.fetch_events() {
        Ok(events) => {
          for event in events {
            let is_key_down = event.value() == 1;

            if event.event_type() == EventType::KEY
              && event.code() < 0x100
              && is_key_down
              && !is_ignored_key(event.code())
            {
              let _ = tx.send(InputMessage::KeyPress(KeyCode(event.code())));
            }
          }
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
        Err(error) => {
          eprintln!("input read error: {error}");
        }
      }
    }

    thread::sleep(Duration::from_millis(2));
  }
}

fn build_ui(app: &Application, rx: Receiver<InputMessage>, config: Config) {
  let window = ApplicationWindow::builder()
    .application(app)
    .default_width(config.size)
    .default_height(config.size)
    .build();

  let provider = gtk4::CssProvider::new();provider.load_from_data(
    r#"
      window,
      .background,
      picture,
      box {
        background: transparent;
        background-color: rgba(0, 0, 0, 0);
        box-shadow: none;
        border: none;
      }

      picture.flipped {
        transform: scaleX(-1);
      }

      box.bounce-a {
        animation-name: yarn-bounce-a;
        animation-duration: 180ms;
        animation-timing-function: ease-out;
        animation-iteration-count: 1;
      }

      box.bounce-b {
        animation-name: yarn-bounce-b;
        animation-duration: 180ms;
        animation-timing-function: ease-out;
        animation-iteration-count: 1;
      }

      @keyframes yarn-bounce-a {
        0% {
          transform: translateY(0) scale(1, 1);
        }
        30% {
          transform: translateY(2px) scale(0.96, 0.90);
        }
        60% {
          transform: translateY(-2px) scale(0.98, 1.06);
        }
        100% {
          transform: translateY(0) scale(1, 1);
        }
      }

      @keyframes yarn-bounce-b {
        0% {
          transform: translateY(0) scale(1, 1);
        }
        30% {
          transform: translateY(2px) scale(0.96, 0.90);
        }
        60% {
          transform: translateY(-2px) scale(0.98, 1.06);
        }
        100% {
          transform: translateY(0) scale(1, 1);
        }
      }
    "#,
  );

  gtk4::style_context_add_provider_for_display(
    &gtk4::prelude::RootExt::display(&window),
    &provider,
    gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
  );

  window.set_decorated(false);
  window.init_layer_shell();
  window.set_namespace(Some("yarnfunapp"));
  window.set_layer(Layer::Overlay);
  window.set_keyboard_mode(KeyboardMode::None);
  window.set_can_focus(false);
  window.set_focusable(false);
  window.set_exclusive_zone(0);
  window.set_anchor(Edge::Top, true);
  window.set_anchor(Edge::Left, true);
  window.set_margin(Edge::Top, config.y);
  window.set_margin(Edge::Left, config.x);

  if let Some(output_name) = config.output.as_deref() {
    match find_output(&gtk4::prelude::RootExt::display(&window), output_name) {
      Some(monitor) => window.set_monitor(Some(&monitor)),
      None => eprintln!("could not find output {output_name}. using default output instead"),
    }
  }

  let textures = [
    texture_from_bytes(IDLE_IMAGE, "idle.png"),
    texture_from_bytes(LEFT_IMAGE, "left.png"),
    texture_from_bytes(RIGHT_IMAGE, "right.png"),
  ];

  let picture = Picture::new();

  if config.flip {
    picture.add_css_class("flipped");
  }

  picture.set_can_shrink(false);
  picture.set_size_request(config.size, config.size);
  picture.set_keep_aspect_ratio(true);
  picture.set_halign(gtk4::Align::Start);
  picture.set_valign(gtk4::Align::Start);
  picture.set_paintable(Some(&textures[0]));

  let container = Box::new(Orientation::Horizontal, 0);
  container.set_size_request(config.size, config.size);
  container.append(&picture);

  window.set_child(Some(&container));
  window.present();

  if let Some(surface) = window.surface() {
    surface.set_input_region(Some(&gtk4::cairo::Region::create()));
  }

  let mut last_input = Instant::now()
    .checked_sub(Duration::from_millis(config.idle_ms))
    .unwrap_or_else(Instant::now);

  let mut last_switch = Instant::now()
    .checked_sub(Duration::from_millis(config.switch_debounce_ms))
    .unwrap_or_else(Instant::now);

  let mut frame = 1_u8;
  let mut last_displayed_frame = 0_u8;
  let mut bounce_phase = false;

  gtk4::glib::timeout_add_local(Duration::from_millis(16), move || {
    let mut should_bounce = false;

    while let Ok(InputMessage::KeyPress(key)) = rx.try_recv() {
      last_input = Instant::now();
      should_bounce = true;

      match key {
        key if config.osu && (key == KeyCode::KEY_Z || key == KeyCode::KEY_Y) => frame = 1,
        key if config.osu && key == KeyCode::KEY_X => frame = 2,
        _ => {
          let now = Instant::now();

          if now.duration_since(last_switch)
            >= Duration::from_millis(config.switch_debounce_ms)
          {
            frame = if frame == 1 { 2 } else { 1 };
            last_switch = now;
          }
        }
      }
    }

    let idle = last_input.elapsed() >= Duration::from_millis(config.idle_ms);
    let desired_frame = if idle { 0 } else { frame };
    if desired_frame != last_displayed_frame {
      picture.set_paintable(Some(&textures[desired_frame as usize]));
      last_displayed_frame = desired_frame;
    }

    if should_bounce && config.bounce {
      bounce_phase = !bounce_phase;

      container.remove_css_class("bounce-a");
      container.remove_css_class("bounce-b");

      if bounce_phase {
        container.add_css_class("bounce-a");
      } else {
        container.add_css_class("bounce-b");
      }
    }

    gtk4::glib::ControlFlow::Continue
  });
}

fn main() {
  let config = Config::from_env();
  let (tx, rx) = mpsc::channel();

  thread::spawn(move || {
    if let Err(error) = input_thread(tx) {
      eprintln!("input thread stopped: {error:#}");
    }
  });

  let app = Application::builder().build();
  let receiver = Rc::new(RefCell::new(Some(rx)));
  let receiver_for_activate = Rc::clone(&receiver);

  app.connect_activate(move |app| {
    let Some(rx) = receiver_for_activate.borrow_mut().take() else {
      return;
    };

    build_ui(app, rx, config.clone());
  });

  app.run();
}
