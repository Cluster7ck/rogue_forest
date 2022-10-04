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

const WIDTH: usize = 6;
const HEIGHT: usize = 6;

enum State {
    Choosing(ChoosingState),
    Placing(PlacingState),
}

#[derive(Debug, Clone, Copy)]
struct ChoosingState {
    index: usize,
    choice: [Plant; 3],
}

impl ChoosingState {
    fn index(self, index: usize) -> ChoosingState {
        ChoosingState { index, ..self }
    }
}

impl Default for ChoosingState {
    fn default() -> Self {
        Self { index: Default::default(), choice: START_PLANTS }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlacingState {
    x: usize,
    y: usize,
}

impl PlacingState {
    fn onUp(&mut self) {
        self.y = (self.y + 1).clamp(0, HEIGHT - 1);
    }

    fn onDown(&mut self) {
        self.y = (self.y - 1).clamp(0, HEIGHT - 1);
    }

    fn onRight(&mut self) {
        self.x = (self.x + 1).clamp(0, WIDTH - 1);
    }

    fn onLeft(&mut self) {
        self.x = (self.x - 1).clamp(0, WIDTH - 1);
    }

    fn onSpace(self, game: &mut Game) {
        if let Tile::Thing(plant) = game.next {
            game.place_plant(self.x as usize, self.y as usize, plant);
            game.update_game((self.x as usize, self.y as usize));
            game.state = State::Choosing(ChoosingState::default());
        }
    }
}

impl Default for PlacingState {
    fn default() -> Self {
        Self { x: (WIDTH as f64 / 2.0).round() as usize, y: (HEIGHT as f64 / 2.0).round() as usize }
    }
}

struct Game {
    state: State,
    tile: Vec<Tile>,
    available: Vec<Plant>,
    points: u32,
    round: u32,
    next: Tile,
}

impl Game {
    fn empty() -> Game {
        Game {
            state: State::Choosing(ChoosingState::default()),
            tile: (0..(WIDTH * HEIGHT))
                .map(|_| {
                    Tile::Empty
                })
                .collect::<Vec<Tile>>(),
            available: START_PLANTS.into_iter().collect(),
            points: 0,
            round: 0,
            next: Tile::Thing(START_PLANTS[0]),
        }
    }

    fn place_plant(&mut self, x: usize, y: usize, plant: Plant) {
        self.tile[xy_idx(x, y)] = Tile::Thing(plant);
    }

    fn update_game(&mut self, (new_x, new_y): (usize, usize)) {
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
        if self.round == 10 {
            self.available.push(Plant {
                max_age: 10,
                age: 0,
                points: 10,
                display: 'o',
            });
        } else if self.round == 15 {
            self.available.push(Plant {
                max_age: 20,
                age: 0,
                points: 40,
                display: 'T',
            });
        }
        self.round += 1;
    }
}

struct App {
    game: Game,
    x: f64,
    y: f64,
}

impl App {
    fn new() -> App {
        App {
            game: Game::empty(),
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

const START_PLANTS: [Plant; 3] = [
    Plant {
        max_age: 1,
        age: 0,
        points: 0,
        display: 'w',
    },
    Plant {
        max_age: 4,
        age: 0,
        points: 2,
        display: 'F',
    },
    Plant {
        max_age: 7,
        age: 0,
        points: 4,
        display: 'Y',
    },
];

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.game.state {
                State::Choosing(state) => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Right => app.game.state = State::Choosing(state.index((state.index + 1).clamp(0, 2))),
                    KeyCode::Left => {
                        app.game.state = State::Choosing(state.index((state.index as i32 - 1).clamp(0, 2) as usize))
                    }
                    KeyCode::Char(' ') => {
                        app.game.next = Tile::Thing(START_PLANTS[state.index]);
                        app.game.state = State::Placing(PlacingState::default());
                    }
                    _ => {}
                },
                State::Placing(mut state) => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up => state.onUp(),
                    KeyCode::Down => state.onDown(),
                    KeyCode::Right => state.onRight(),
                    KeyCode::Left => state.onLeft(),
                    KeyCode::Char(' ') => {
                        state.onSpace(&mut app.game)
                    }
                    _ => {}
                },
            }
        }
    }
}
fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    match app.game.state {
        State::Choosing(state) => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(f.size());
            let titles = START_PLANTS
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
                .select(state.index)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD)
                        .bg(Color::Black),
                );

            f.render_widget(tabs, chunks[0]);
        }
        State::Placing(state) => {
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .margin(1)
                .split(f.size());

            let title = format!(
                " game // SCORE: {} // ROUND: {} ",
                app.game.points, app.game.round
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

                            let tile = &app.game.tile[idx];
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
