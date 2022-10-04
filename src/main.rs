use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use std::{
    error::Error,
    fmt::{Debug, Display, Write},
    io::{self},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, Borders, Tabs,
    },
    Frame, Terminal,
};

const WIDTH: usize = 8;
const HEIGHT: usize = 8;

enum State {
    Choosing(usize),
    Placing,
}

struct Board {
    tile: Vec<Tile>,
    points: u32,
    round: u32,
    next: Tile,
}

impl Board {
    fn empty() -> Board {
        Board {
            tile: (0..(WIDTH * HEIGHT))
                .map(|_| {
                    Tile::Empty
                })
                .collect::<Vec<Tile>>(),
            points: 0,
            round: 0,
            next: Tile::Thing(PLANTS[0]),
        }
    }

    fn place_plant(&mut self, x: usize, y: usize, plant: Plant) {
        self.tile[xy_idx(x, y)] = Tile::Thing(plant);
    }

    fn update_board(&mut self, (new_x, new_y): (usize, usize)) {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if x == new_x && y == new_y {
                    continue;
                }
                let idx = xy_idx(x, y);
                if let Tile::Thing(p) = &mut self.tile[idx] {
                    p.age += 1;
                    if p.age >= p.max_age {
                        self.points += p.points;
                        self.tile[idx] = Tile::Empty;
                    }
                }
            }
        }
        self.round += 1;
    }
}

struct App {
    board: Board,
    state: State,
    x: f64,
    y: f64,
}

impl App {
    fn new() -> App {
        App {
            board: Board::empty(),
            state: State::Choosing(0),
            x: WIDTH as f64 / 2.0,
            y: HEIGHT as f64 / 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Plant {
    max_age: u32,
    age: u32,
    points: u32,
    display: char,
}

impl Display for Plant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tile_info = format!("{}: {}/{}", self.display, self.age, self.max_age);
        f.write_str(&tile_info)
    }
}

enum Tile {
    Empty,
    Thing(Plant),
}

impl Display for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tile::Empty => f.write_char(' '),
            Tile::Thing(x) => f.write_str(&x.to_string()),
        }
    }
}

fn xy_idx(x: usize, y: usize) -> usize {
    y * WIDTH + x
}

const PLANTS: [Plant; 3] = [
    Plant {
        max_age: 4,
        age: 0,
        points: 2,
        display: 'F',
    },
    Plant {
        max_age: 10,
        age: 0,
        points: 10,
        display: 'o',
    },
    Plant {
        max_age: 20,
        age: 0,
        points: 40,
        display: 'T',
    },
];

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.state {
                State::Choosing(index) => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Right => app.state = State::Choosing((index + 1).clamp(0, 2)),
                    KeyCode::Left => {
                        app.state = State::Choosing((index as i32 - 1).clamp(0, 2) as usize)
                    }
                    KeyCode::Char(' ') => {
                        app.board.next = Tile::Thing(PLANTS[index]);
                        app.state = State::Placing;
                    }
                    _ => {}
                },
                State::Placing => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up => app.y = (app.y + 1.0).clamp(0.0, HEIGHT as f64 - 1.0),
                    KeyCode::Down => app.y = (app.y - 1.0).clamp(0.0, HEIGHT as f64 - 1.0),
                    KeyCode::Right => app.x = (app.x + 1.0).clamp(0.0, WIDTH as f64 - 1.0),
                    KeyCode::Left => app.x = (app.x - 1.0).clamp(0.0, WIDTH as f64 - 1.0),
                    KeyCode::Char(' ') => {
                        if let Tile::Thing(plant) = app.board.next {
                            app.board.place_plant(app.x as usize, app.y as usize, plant);
                            app.board.update_board((app.x as usize, app.y as usize));
                            app.state = State::Choosing(0);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
}
fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    match app.state {
        State::Choosing(index) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());
            let titles = PLANTS
                .iter()
                .map(|t| {
                    Spans::from(vec![Span::styled(
                        t.display.to_string(),
                        Style::default().fg(Color::Green),
                    )])
                })
                .collect();
            let tabs = Tabs::new(titles)
                .block(Block::default().borders(Borders::ALL).title("Plants"))
                .select(index)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Black),
                );

            f.render_widget(tabs, chunks[0]);
        }
        State::Placing => {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .margin(1)
                .split(f.size());

            let title = format!(
                " BOARD // SCORE: {} // ROUND: {} ",
                app.board.points, app.board.round
            );
            let canvas = Canvas::default()
                .block(Block::default().borders(Borders::ALL).title(title))
                .paint(|ctx| {
                    let r_width = 0.7;
                    let r_height = 0.7;
                    for x in 0..WIDTH {
                        for y in 0..HEIGHT {
                            let color = match (x, y) {
                                (a, b) if a as f64 == app.x && b as f64 == app.y => Color::Green,
                                (_, _) => Color::White,
                            };

                            let idx = xy_idx(x, y);
                            let y_off = y as f64 + (1.0 - r_height) / 2.0;
                            let x_off = x as f64 + (1.0 - r_width) / 2.0;
                            let rect = Rectangle {
                                x: x_off,
                                y: y_off,
                                width: r_width,
                                height: r_height,
                                color,
                            };

                            let tile = &app.board.tile[idx];
                            let t_c = if let Tile::Thing(p) = tile {
                                if p.max_age - p.age < 3 {
                                    Color::Magenta
                                } else {
                                    Color::White
                                }
                            } else {
                                Color::White
                            };
                            let _debug = format!("({},{}): {}", x, y, tile,);
                            let s = Span::styled(tile.to_string(), Style::default().fg(t_c));
                            ctx.layer();
                            ctx.print(x_off + r_width / 4.0, y_off + r_height / 2.0, s);
                            ctx.draw(&rect);
                        }
                    }
                })
                .x_bounds([0.0, WIDTH as f64])
                .y_bounds([0.0, HEIGHT as f64]);
            f.render_widget(canvas, rects[0]);
        }
    };
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }
    Ok(())
}
