use anyhow::Result;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    cli::Args, state::{EachFrameImpl, ShouldRender, State}, tui::{Event, Tui}, ui::ui, widget::AsWeatherWidget
};

#[derive(Copy, Clone)]
pub struct AppRuntimeInfo {
    pub fps: usize,
}

pub struct App<T> {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    tui: Tui,
    state: State<T>,
    should_quit: bool,
    should_render: ShouldRender,
    args: Args,
    frame_in_second: usize,
    runtime_info: AppRuntimeInfo,
}

impl<T> App<T>
where
    T: EachFrameImpl + AsWeatherWidget,
{
    pub fn new(args: Args, weather: T) -> Result<Self> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        let state = State::new(terminal.size()?, weather, args);

        Ok(Self {
            terminal,
            state,
            args,
            tui: Tui::new(args.fps as f64, args.tps as f64)?,
            should_quit: false,
            should_render: ShouldRender::Render,
            frame_in_second: 0,
            runtime_info: AppRuntimeInfo { fps: 0 },
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        use Event::*;
        self.tui.run();

        loop {
            if let Some(event) = self.tui.next().await {
                match event {
                    Init => (),
                    Quit | Error => self.should_quit = true,
                    Render => self.on_render()?,
                    Key(key) => self.handle_keyboard(key),
                    Tick => self.on_tick(),
                    Timer => self.on_timer(),
                    Resize(columns, rows) => self.on_resize(columns, rows),
                };
            };

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_keyboard(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            _ => {}
        }
    }

    fn on_resize(&mut self, columns: u16, rows: u16) {
        self.state.on_resize(columns, rows);
        self.should_render = ShouldRender::Render;
    }

    fn on_tick(&mut self) {
        self.should_render = self.should_render.or(self.state.tick());
        self.frame_in_second = self.frame_in_second.saturating_add(1);
    }

    fn on_render(&mut self) -> anyhow::Result<()> {
        if self.args.fps == self.args.tps {
            self.on_tick()
        }

        if self.should_render.is_render() {
            self.should_render = ShouldRender::Skip;
            self.terminal.draw(|f| ui(f, &mut self.state, self.args, self.runtime_info))?;
        }

        Ok(())
    }

    fn on_timer(&mut self) {
        self.state.tick_timer();
        self.runtime_info.fps = self.frame_in_second;
        self.frame_in_second = 0;
        self.should_render = ShouldRender::Render;
    }
}

impl<T> Drop for App<T> {
    fn drop(&mut self) {
        // restore terminal
        if crossterm::terminal::is_raw_mode_enabled().unwrap() {
            let _ = disable_raw_mode();
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                cursor::Show
            );
        }
    }
}
