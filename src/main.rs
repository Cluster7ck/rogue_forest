use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use std::{
    borrow::{Cow, Borrow},
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display, Write},
    fs,
    io::{self},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        canvas::{Canvas, Rectangle},
        Block, Borders, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame, Terminal,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 6)]
    dim: usize,
}

#[derive(Debug)]
pub struct GlobalSetting {
    width: usize,
    height: usize,
}

static INSTANCE: OnceCell<GlobalSetting> = OnceCell::new();

impl GlobalSetting {
    pub fn global() -> &'static GlobalSetting {
        INSTANCE.get().expect("logger is not initialized")
    }

    fn load() -> Result<GlobalSetting, std::io::Error> {
        let args = Args::parse();
        Ok(GlobalSetting {
            width: args.dim,
            height: args.dim,
        })
    }
}

fn height() -> usize {
    INSTANCE.get().unwrap().height
}

fn width() -> usize {
    INSTANCE.get().unwrap().width
}

enum State {
    Choosing,
    Placing,
}

#[derive(Debug, Clone)]
struct ChoosingState {
    index: Option<usize>,
    choice: Option<Plant>,
}

impl ChoosingState {
    fn on_down(&mut self, len: usize) {
        if len == 0 {
            return
        }
        match self.index {
            Some(index) => {
                self.index = Some((index + 1).rem_euclid(len));
            }
            None => self.index = Some(0),
        }
    }

    fn on_up(&mut self, len: usize) {
        if len == 0 {
            return
        }
        match self.index {
            Some(index) => {
                self.index = Some((index as i32 - 1).rem_euclid(len as i32) as usize);
            }
            None => self.index = Some(0),
        }
    }

    fn _on_space(&mut self, game: &mut Game) {}
}

impl Default for ChoosingState {
    fn default() -> Self {
        Self {
            index: Some(0),
            choice: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PlacingState {
    x: usize,
    y: usize,
}

impl PlacingState {
    fn on_up(&mut self) {
        self.y = (self.y + 1).clamp(0, height() - 1);
    }

    fn on_down(&mut self) {
        self.y = (self.y - 1).clamp(0, height() - 1);
    }

    fn on_right(&mut self) {
        self.x = (self.x + 1).clamp(0, width() - 1);
    }

    fn on_left(&mut self) {
        self.x = (self.x - 1).clamp(0, width() - 1);
    }

    fn _on_space(self) {}
}

impl Default for PlacingState {
    fn default() -> Self {
        Self {
            x: (width() as f64 / 2.0).round() as usize,
            y: (height() as f64 / 2.0).round() as usize,
        }
    }
}

struct Game {
    state: State,
    tile: Vec<Tile>,
    hand: Vec<Plant>,
    all_plants: Vec<Plant>,
    name_to_plant: HashMap<String, Plant>,
    points: u32,
    round: u32,
    placing: PlacingState,
    choosing: ChoosingState,
}

impl Game {
    fn empty() -> Game {
        let all_plants = [
            Plant {
                max_age: 2,
                age: 0,
                points: 0,
                name: Cow::Borrowed("Grass"),
                short_display: 'w',
                drops: vec![
                    Drop {
                        chance: 5.0,
                        plants: vec!["Grass".into(), "Grass".into()],
                    },
                    Drop {
                        chance: 1.0,
                        plants: vec!["Grass".into(), "Tall Grass".into()],
                    },
                ],
            },
            Plant {
                max_age: 4,
                age: 0,
                points: 2,
                name: Cow::Borrowed("Tall Grass"),
                short_display: 'W',
                drops: vec![
                    Drop {
                        chance: 5.0,
                        plants: vec!["Tall Grass".into(), "Tall Grass".into()],
                    },
                    Drop {
                        chance: 1.0,
                        plants: vec!["Tall".into(), "Shrub".into()],
                    },
                ],
            },
            Plant {
                max_age: 7,
                age: 0,
                points: 4,
                name: Cow::Borrowed("Shrub"),
                short_display: 'Y',
                drops: vec![Drop {
                    chance: 5.0,
                    plants: vec!["Shrub".into(), "Shrub".into()],
                }],
            },
        ];

        let all_plants: Vec<Plant> = all_plants.into_iter().collect::<Vec<Plant>>();
        let name_to_plant: HashMap<String, Plant> = all_plants
            .iter()
            .map(|p| (<Cow<'_, str> as Borrow<str>>::borrow(&p.name).to_string(), p.clone()))
            .collect::<HashMap<String, Plant>>();
        let hand_plant = all_plants[0].clone();
        let hand = vec![hand_plant.clone(), hand_plant];

        Game {
            state: State::Choosing,
            tile: (0..(width() * height()))
                .map(|_| Tile::Empty)
                .collect::<Vec<Tile>>(),
            hand,
            all_plants,
            name_to_plant,
            points: 0,
            round: 0,
            placing: PlacingState::default(),
            choosing: ChoosingState::default(),
        }
    }

    fn selected_plant(&self) -> Option<Plant> {
        if self.hand.len() == 0 {
            None
        } else {
            self.choosing.index.map(|idx| self.hand[idx].clone())
        }
    }

    fn on_space(&mut self) {
        if self.hand.len() == 0 {
            self.update_game((100, 100));
            return;
        }

        match self.state {
            State::Choosing => {
                self.choosing.choice = self.choosing.index.map(|idx| self.hand[idx].clone());
                self.state = State::Placing;
            }
            State::Placing => {
                if let Some(plant) = self.choosing.choice.take() {
                    self.place_plant(self.placing.x as usize, self.placing.y as usize, &plant);
                    if let Some(idx) = self.choosing.index.take() {
                        self.hand.remove(idx);
                        self.update_game((self.placing.x as usize, self.placing.y as usize));
                        self.choosing.index = if idx > 0 { Some(idx - 1) } else { Some(idx) };
                        self.state = State::Choosing;
                    }
                } else {
                    // TODO what is this case even? maybe when we switch back to the board during choosing?
                }
            }
        }
    }

    fn place_plant(&mut self, x: usize, y: usize, plant: &Plant) {
        self.tile[xy_idx(x, y)] = Tile::Thing(plant.clone());
    }

    fn update_game(&mut self, (new_x, new_y): (usize, usize)) {
        for y in 0..height() {
            for x in 0..width() {
                if x == new_x && y == new_y {
                    continue;
                }
                let idx = xy_idx(x, y);
                if let Tile::Thing(p) = &mut self.tile[idx] {
                    p.age += 1;
                    if p.age >= p.max_age {
                        self.points += p.points;
                        if let Some(drop) = p.drops.get(0) {
                            for plant_name in drop.plants.iter() {
                                if let Some(dropped_plant) = self.name_to_plant.get(plant_name) {
                                    self.hand.push(dropped_plant.clone())
                                }
                            }
                        }
                        self.tile[idx] = Tile::Empty;
                    }
                }
            }
        }
        self.round += 1;
    }
}

struct App {
    game: Game,
    list_state: ListState,
}

impl App {
    fn new() -> App {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            game: Game::empty(),
            list_state: list_state,
        }
    }

    fn select(&mut self, index: Option<usize>) {
        self.list_state.select(index);
    }

    fn unselect(&mut self) {
        self.list_state.select(None);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Plant {
    max_age: u32,
    age: u32,
    points: u32,
    name: Cow<'static, str>,
    short_display: char,
    drops: Vec<Drop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Drop {
    chance: f32,
    plants: Vec<String>,
}

impl Display for Plant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tile_info = format!(
            "{}: {}/{}",
            self.short_display.to_string(),
            self.age,
            self.max_age
        );
        f.write_str(&tile_info)
    }
}

fn load_plants() -> Vec<Plant> {
    let contents = fs::read_to_string("assets/plants.json").unwrap();
    let plants: Vec<Plant> = serde_json::from_str(&contents).unwrap();

    plants
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
    y * width() + x
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.game.state {
                State::Choosing => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down => {
                        app.game.choosing.on_down(app.game.hand.len());
                        app.select(app.game.choosing.index);
                    }
                    KeyCode::Up => {
                        app.game.choosing.on_up(app.game.hand.len());
                        app.select(app.game.choosing.index);
                    }
                    KeyCode::Char(' ') => {
                        app.unselect();
                        app.game.on_space();
                    }
                    _ => {}
                },
                State::Placing => match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Up => app.game.placing.on_up(),
                    KeyCode::Down => app.game.placing.on_down(),
                    KeyCode::Right => app.game.placing.on_right(),
                    KeyCode::Left => app.game.placing.on_left(),
                    KeyCode::Char(' ') => {
                        app.game.on_space();
                        app.select(app.game.choosing.index);
                    }
                    _ => {}
                },
            }
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .margin(1)
        .split(f.size());

    draw_game_board(f, app, chunks[0]);
    draw_side(f, app, chunks[1]);
}

fn draw_game_board<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let title = format!(
        " Forest // Score: {} // Round: {} ",
        app.game.points, app.game.round
    );
    let canvas = Canvas::default()
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .paint(|ctx| {
            let r_width = 0.7;
            let r_height = 0.7;
            for x in 0..width() {
                for y in 0..height() {
                    let color = match app.game.state {
                        State::Choosing => Color::White,
                        State::Placing => match (x, y) {
                            (x_, y_) if x_ == app.game.placing.x && y_ == app.game.placing.y => {
                                Color::Green
                            }
                            (_, _) => Color::White,
                        },
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
        .x_bounds([0.0, width() as f64])
        .y_bounds([0.0, height() as f64]);
    f.render_widget(canvas, area)
}

fn draw_side<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let chunks = Layout::default()
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(area);
    draw_card_chooser(f, app, chunks[0]);
    draw_card_info(f, app, chunks[1]);
}

fn draw_card_chooser<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let items: Vec<ListItem> = app
        .game
        .hand
        .iter()
        .map(|i| {
            let lines = vec![Spans::from(i.name.as_ref())];

            ListItem::new(lines).style(Style::default())
        })
        .collect();
    let items = List::new(items)
        .block(
            Block::default().borders(Borders::ALL).title(Span::styled(
                "Plants",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
        )
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">>  ");

    // We can now render the item list
    f.render_stateful_widget(items, area, &mut app.list_state);
}

fn draw_card_info<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let plant_opt = match app.game.state {
        State::Choosing => app.game.selected_plant(),
        State::Placing => {
            let idx = xy_idx(app.game.placing.x, app.game.placing.y);
            let tile = &app.game.tile[idx];
            match tile {
                Tile::Empty => None,
                Tile::Thing(plant) => Some(plant.clone()),
            }
        }
    };

    let content = match plant_opt {
        Some(ref plant) => {
            vec![
                Spans::from("Plant:"),
                Spans::from(""),
                Spans::from(vec![
                    Span::styled("Age: ", Style::default().fg(Color::Cyan)),
                    Span::raw(plant.max_age.to_string()),
                ]),
            ]
        }
        None => {
            vec![Spans::from("Empty")]
        }
    };

    let title = match plant_opt {
        Some(ref plant) => format!(" {} ", plant.name),
        None => "".into(),
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        title,
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn main() -> Result<(), Box<dyn Error>> {
    {
        let settings = GlobalSetting::load().unwrap();
        INSTANCE.set(settings).unwrap();
    }

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
