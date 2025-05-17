pub mod interface
{
    use crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{
        backend::CrosstermBackend,
        layout::{Constraint, Direction, Layout},
        style::{Color, Style},
        text::{Span, Line},
        widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
        Terminal,
    };
    use std::{
        fs,
        io::{stdout, Result},
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };
    use wg_2024::network::NodeId;

    enum Mode {
        Chat,
        FileList,
    }

    // (nome, colore_nome, messaggio, colore_messaggio)
    type Messages = Arc<Mutex<Vec<(String, Color, String, Color)>>>;
    type UserList = Arc<Mutex<Vec<(NodeId, bool)>>>;

    pub fn start_ui(server_name: String, messages: Messages, path : Option<String>) {
        thread::spawn(move || {
            enable_raw_mode().unwrap();
            let mut stdout = stdout();
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend).unwrap();

            let _ = run_app(&mut terminal, server_name, messages,path.unwrap_or(".".to_string()));

            disable_raw_mode().unwrap();
            execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture).unwrap();
            terminal.show_cursor().unwrap();
        });
    }

    pub fn add_message(messages: &Messages, name: &str, msg: &str, name_color: Color, msg_color: Color) {
        let mut locked = messages.lock().unwrap();
        locked.push((
            name.to_string(),
            name_color,
            msg.to_string(),
            msg_color,
        ));
    }
    
    pub fn add_client_to_interface (users  : &UserList ,clientId : NodeId )
    {
        let mut locked = users.lock().unwrap();
        
        if !locked.contains(&(clientId,true))
        {
            locked.push((
                clientId, true
            ));
        }

    }
    
    pub fn unreachable  (users  : &UserList, client_id: NodeId )     //function that set a client not reachable 
    {
        let mut locked = users.lock().unwrap();
        for i in locked.iter_mut()
            {
                if i.0 == client_id {
                    i.1 = false;
                    break;
                }
                
            }
            
    }
    
    

    fn run_app(
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        server_name: String,
        messages: Messages,
        path : String,
    ) -> Result<()> {
        let mut mode = Mode::Chat;
        let mut selected_index = 0;
        let mut auto_scroll = true;

        loop {
            terminal.draw(|f| {
                let size = f.size();

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(5),
                        Constraint::Length(3),
                    ])
                    .split(size);

                let header = Paragraph::new(server_name.as_str())
                    .style(Style::default().fg(Color::Yellow))
                    .block(Block::default().borders(Borders::ALL).title("Server"));
                f.render_widget(header, chunks[0]);

                match mode {
                    Mode::Chat => {
                        let locked = messages.lock().unwrap();
                        let items: Vec<ListItem> = locked
                            .iter()
                            .map(|(name, name_color, msg, msg_color)| {
                                let line = Line::from(vec![
                                    Span::styled(format!("{}: ", name), Style::default().fg(*name_color)),
                                    Span::styled(msg.clone(), Style::default().fg(*msg_color)),
                                ]);
                                ListItem::new(line)
                            })
                            .collect();

                        if auto_scroll {
                            selected_index = items.len().saturating_sub(1);
                        } else if selected_index >= items.len() {
                            selected_index = items.len().saturating_sub(1);
                        }

                        let mut state = ListState::default();
                        if !items.is_empty() {
                            state.select(Some(selected_index));
                        }

                        let list = List::new(items)
                            .block(Block::default().borders(Borders::ALL).title("Messaggi"))
                            .highlight_style(Style::default().bg(Color::Blue));
                        f.render_stateful_widget(list, chunks[1], &mut state);
                    }
                    Mode::FileList => {
                        let copy  = path.clone();
                        let files = fs::read_dir(copy)
                            .unwrap()
                            .filter_map(|entry| entry.ok())
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .collect::<Vec<_>>();
                        let items: Vec<ListItem> = files.iter().map(|f| ListItem::new(f.as_str())).collect();
                        let list = List::new(items)
                            .block(Block::default().borders(Borders::ALL).title("File nella cartella corrente"));
                        f.render_widget(list, chunks[1]);
                    }
                }

                let footer = Paragraph::new("1 - Messaggi  |  2 - File  |  ↑↓ per scorrere  |  q - Esci")
                    .style(Style::default().fg(Color::Green))
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(footer, chunks[2]);
            })?;

            if event::poll(Duration::from_millis(30))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('1') => mode = Mode::Chat,
                        KeyCode::Char('2') => mode = Mode::FileList,
                        KeyCode::Up => {
                            if selected_index > 0 {
                                selected_index -= 1;
                            }
                            auto_scroll = false;
                        }
                        KeyCode::Down => {
                            let len = messages.lock().unwrap().len();
                            if selected_index + 1 < len {
                                selected_index += 1;
                                auto_scroll = false;
                            } else {
                                auto_scroll = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}