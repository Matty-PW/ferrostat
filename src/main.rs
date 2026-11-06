mod app;
mod ui;
mod history;

use std::io;

use app::App;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
