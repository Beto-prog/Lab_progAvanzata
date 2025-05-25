pub mod interface {
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
        widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
        Terminal,
    };
    use std::{
        fs,
        io::{stdout, Result},
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use once_cell::sync::Lazy;
    use rand::prelude::SliceRandom;
    use ratatui::widgets::GraphType;
    use wg_2024::network::NodeId;

    /* ──────────────────────────────────────────────────────────
       ───────  DATI CONDIVISI TRA TUTTI I THREAD / SERVER  ─────
       ────────────────────────────────────────────────────────── */
    pub type AllServersUi = Arc<Mutex<Vec<ServerUiState>>>;

    #[derive(Clone)]
    pub struct ServerUiState {
        pub id: usize,
        pub name: String,
        pub path: String,
        pub messages: Arc<Mutex<Vec<(String, Color, String, Color)>>>,
        pub clients: Arc<Mutex<Vec<(NodeId, bool)>>>,
        pub selected_index: Arc<AtomicUsize>,   // <── cambiato!
    }

    /* ────────────────────────────────────────────────────────── */
    enum Mode {
        Chat,
        FileList,
        Graph
    }

    // alias utili (non cambiano)
    type Messages = Arc<Mutex<Vec<(String, Color, String, Color)>>>;
    type UserList = Arc<Mutex<Vec<(NodeId, bool)>>>;

    /* ──────────────────────────────────────────────────────────
       ───────────── FUNZIONI CHIAMATE DAI SERVER ───────────────
       ────────────────────────────────────────────────────────── */
    pub fn add_message(
        messages: &Messages,
        name: &str,
        msg: &str,
        name_color: Color,
        msg_color: Color,
    ) {
        let mut locked = messages.lock().unwrap();
        locked.push((name.to_string(), name_color, msg.to_string(), msg_color));
    }

    pub fn add_client_to_interface(users: &UserList, client_id: NodeId) {
        let mut locked = users.lock().unwrap();
        if !locked.iter().any(|(id, _)| *id == client_id) {
            locked.push((client_id, true));
        }
    }

    pub fn unreachable(users: &UserList, client_id: NodeId) {
        let mut locked = users.lock().unwrap();
        if let Some((_, reachable)) = locked.iter_mut().find(|(id, _)| *id == client_id) {
            *reachable = false;
        }
    }

    /* ──────────────────────────────────────────────────────────
       ────────────────────  AVVIO DELLA UI  ────────────────────
       ────────────────────────────────────────────────────────── */
    ///
    /// Avvia la TUI in un thread separato.
    /// - `all_servers` dev'essere lo stesso `Arc<Mutex<_>>`
    ///   passato a **tutti** i `Server::new`.
    ///
    pub fn start_ui(all_servers: AllServersUi) {
        thread::spawn(move || {
            enable_raw_mode().unwrap();
            let mut stdout = stdout();
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture).unwrap();
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend).unwrap();

            let _ = run_app(&mut terminal, all_servers);

            disable_raw_mode().unwrap();
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )
                .unwrap();
            terminal.show_cursor().unwrap();
        });
    }

    /* ──────────────────────────────────────────────────────────
       ────────────────  LOOP PRINCIPALE DELLA UI  ──────────────
       ────────────────────────────────────────────────────────── */
    fn run_app(
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
        all_servers: AllServersUi,
    ) -> Result<()> {
        let mut global_mode = Mode::Chat;          // Chat/File per il server selezionato
        let mut current_srv = 0usize;              // indice del tab/server attivo
        let mut auto_scroll = true;                // vale per il server corrente
        let mut  ALREADY_OPENED: bool = false;


        loop {
            /* ─────── Disegno ─────── */
            terminal.draw(|f| {
                let size = f.size();
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),    // header con tab
                        Constraint::Min(5),       // corpo (chat o files)
                        Constraint::Length(3),    // footer
                    ])
                    .split(size);

                /* ---- Header: Tabs con i nomi dei server ---- */
                let servers_guard = all_servers.lock().unwrap();
                if servers_guard.is_empty() {
                    let empty = Paragraph::new("Nessun server attivo")
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(empty, layout[1]);
                    return;
                }
                let titles: Vec<Span> = servers_guard
                    .iter()
                    .map(|s| Span::styled(s.name.clone(), Style::default().fg(Color::Yellow)))
                    .collect();
                let tabs = Tabs::new(titles)
                    .select(current_srv)
                    .block(Block::default().borders(Borders::ALL).title("Server"))
                    .highlight_style(Style::default().bg(Color::Blue));
                f.render_widget(tabs, layout[0]);

                /* ---- Corpo: Chat o FileList per il server attivo ---- */
                let srv = &servers_guard[current_srv];

                match global_mode {




                    Mode::Graph => {
                        use std::fs::write;
                        use std::process::{Command, Stdio};
                        use std::sync::atomic::{AtomicBool, Ordering};
                        use once_cell::sync::Lazy;
                        use ratatui::widgets::{Paragraph, Block, Borders};
                        use ratatui::style::{Style, Color};

                        // Flag globale condiviso

                        let mut info = String::new();

                        if !ALREADY_OPENED {
                            // genera DOT e PNG
                            let dot = r#"

graph Topologia {
    node [style=filled, fontname=Helvetica];

    A [label="API Gateway", shape=box, fillcolor=lightblue, color=blue, penwidth=2];
    B [label="Auth Service", shape=ellipse, fillcolor=lightyellow, color=orange, penwidth=2];
    C [label="DB Master", shape=cylinder, fillcolor=mistyrose, color=red];
    D [label="DB Replica", shape=cylinder, fillcolor=lemonchiffon, color=green];
    E [label="Cache", shape=box, fillcolor=khaki, color=darkgoldenrod];
    F [label="Worker 1", shape=box, fillcolor=lightgreen, color=green];
    G [label="Worker 2", shape=box, fillcolor=lightgreen, color=green];
    H [label="Logger", shape=ellipse, fillcolor=lavender, color=purple];
    I [label="Metrics", shape=hexagon, fillcolor=lightcyan, color=deepskyblue];
    J [label="External API", shape=parallelogram, fillcolor=white, color=black];

    // Connessioni principali
    A -- B;
    A -- E;
    B -- C;
    B -- D;
    B -- E;
    E -- F;
    E -- G;
    F -- H;
    G -- H;
    H -- I;
    A -- J;

    // Connessioni extra per densità
    F -- C;
    G -- D;
    I -- J;
    C -- D;
}
"#;
                            let _ = write("grafo.dot", dot);
                            let _ = Command::new("dot")
                                .args(["-Tpng", "grafo.dot", "-o", "grafo.png"])
                                .status();
                            let _ = Command::new("yad")
                                .args([
                                    "--window-icon=dialog-information",
                                    "--title=Topologia di Rete",
                                    "--image=grafo.png",
                                    "--image-on-top",
                                    "--no-buttons",
                                    "--width=500",
                                    "--height=500",
                                ])
                                .stdout(Stdio::null())
                                .stderr(Stdio::null())
                                .spawn();

                            ALREADY_OPENED = true;
                            info = "🖼️ Finestra YAD aperta.".into();
                        } else {
                            info = "✅ YAD già aperto. Cambia modalità per riaprirla.".into();
                        }

                        let paragraph = Paragraph::new(info)
                            .block(Block::default().title("Grafo").borders(Borders::ALL))
                            .style(Style::default().fg(Color::LightCyan));

                        f.render_widget(paragraph, layout[1]);
                    }













                    Mode::Chat => {
                        let mut items: Vec<ListItem> = {
                            let locked = srv.messages.lock().unwrap();
                            locked
                                .iter()
                                .map(|(name, name_color, msg, msg_color)| {
                                    let line = Line::from(vec![
                                        Span::styled(
                                            format!("{}: ", name),
                                            Style::default().fg(*name_color),
                                        ),
                                        Span::styled(msg.clone(), Style::default().fg(*msg_color)),
                                    ]);
                                    ListItem::new(line)
                                })
                                .collect()
                        };

                        // auto‑scroll / bounds check
                        let list_len = items.len();
                        let mut new_sel = srv.selected_index.load(Ordering::Relaxed);

                        if auto_scroll {
                            new_sel = list_len.saturating_sub(1);
                        } else if new_sel >= list_len {
                            new_sel = list_len.saturating_sub(1);
                        }

                        srv.selected_index.store(new_sel, Ordering::Relaxed);

                        let mut state = ListState::default();
                        if list_len > 0 {
                            state.select(Some(new_sel));
                        }

                        let list = List::new(items)
                            .block(Block::default().borders(Borders::ALL).title("Messaggi"))
                            .highlight_style(Style::default().bg(Color::Blue));
                        f.render_stateful_widget(list, layout[1], &mut state);
                    }
                    Mode::FileList => {
                        let files = fs::read_dir(&srv.path)
                            .unwrap_or_else(|_| fs::read_dir(".").unwrap())
                            .filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .collect::<Vec<_>>();
                        let items: Vec<_> = files.iter().map(|f| ListItem::new(f.as_str())).collect();
                        let list = List::new(items)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title("File nella cartella corrente"),
                            );
                        f.render_widget(list, layout[1]);
                    }
                }

                /* ---- Footer ---- */
                let footer = Paragraph::new(
                    "←/→ cambia server  |  1 Chat - 2 File - 3 Graph  |  ↑↓ scroll  |  q quit",
                )
                    .style(Style::default().fg(Color::Green))
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(footer, layout[2]);
            })?;

            /* ─────── Input ─────── */
            if event::poll(Duration::from_millis(30))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        /* ---- Cambio server ---- */
                        KeyCode::Left => {
                            if current_srv > 0 {
                                current_srv -= 1;
                                auto_scroll = true;
                            }
                        }
                        KeyCode::Right => {
                            let len = all_servers.lock().unwrap().len();
                            if current_srv + 1 < len {
                                current_srv += 1;
                                auto_scroll = true;
                            }
                        }

                        /* ---- Cambio modalità ---- */
                        KeyCode::Char('1') => {
                            
                            ALREADY_OPENED= false; // resetta!
                            global_mode = Mode::Chat;
                        },
                        KeyCode::Char('2') => {
                            ALREADY_OPENED= false; // resetta!
                            global_mode = Mode::FileList;
                        },


                        KeyCode::Char('3') => global_mode = Mode::Graph,

                        /* ---- Scroll messaggi (solo modalità Chat) ---- */
                        KeyCode::Up => {
                            if matches!(global_mode, Mode::Chat) {
                                let srv = &all_servers.lock().unwrap()[current_srv];
                                let cur = srv.selected_index.load(Ordering::Relaxed);
                                if cur > 0 {
                                    srv.selected_index.store(cur - 1, Ordering::Relaxed);
                                }
                                auto_scroll = false;
                            }
                        }
                        KeyCode::Down => {
                            if matches!(global_mode, Mode::Chat) {
                                let srv = &all_servers.lock().unwrap()[current_srv];
                                let len = srv.messages.lock().unwrap().len();
                                let cur = srv.selected_index.load(Ordering::Relaxed);
                                if cur + 1 < len {
                                    srv.selected_index.store(cur + 1, Ordering::Relaxed);
                                    auto_scroll = false;
                                } else {
                                    auto_scroll = true;
                                }
                            }
                        }

                        /* ---- Uscita ---- */
                        KeyCode::Char('q') => return Ok(()),

                        _ => {}
                    }
                }
            }
        }
    }
}
