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
    use std::collections::{BTreeSet, HashMap, HashSet};
    use std::ops::Mul;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use once_cell::sync::Lazy;
    use rand::prelude::SliceRandom;
    use ratatui::widgets::GraphType;
    use wg_2024::network::NodeId;
    use wg_2024::packet::NodeType;
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
        pub chat_for_client: Arc<Mutex<HashMap<BTreeSet<NodeId>,Vec<(NodeId,String)>>>>,      //I had to use this BTree because when I used .key on the hashmap rust gave me problem
        pub list_of_files: Arc<Mutex<HashMap<String,usize>>>,
        pub graph : Arc<Mutex<HashMap<NodeId, (HashSet<NodeId>, NodeType)>>>,
        pub selected_index: Arc<AtomicUsize>,   // <── cambiato!
        pub selected_chat_index: Arc<AtomicUsize>,

    }

    /* ────────────────────────────────────────────────────────── */
    enum Mode {
        Chat,
        FileList,
        Graph
    }

    // useful alias (non )
    type Graph = Arc<Mutex<HashMap<NodeId, (HashSet<NodeId>, NodeType)>>>;
    type Messages = Arc<Mutex<Vec<(String, Color, String, Color)>>>;
    type Chatlist = Arc<Mutex<HashMap<BTreeSet<NodeId>,Vec<(NodeId,String)>>>>;

    type FileList = Arc<Mutex<HashMap<String,usize>>>;

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

        if let Ok(mut locked) = messages.lock() {
            locked.push((name.to_string(), name_color, msg.to_string(), msg_color));

        }
        /*
        let mut locked = messages.lock().unwrap();
        locked.push((name.to_string(), name_color, msg.to_string(), msg_color));*/
    }

    pub fn refresh(list : &FileList, files : Vec<String>)      //Some file could be added when the program is working
    {
        /*let mut locked = list.lock().unwrap();

        for name in files {
            if !locked.contains_key(&name) {
                locked.insert(name.clone(), 0);
            }
        }*/

        if let Ok(mut locked) =list.lock() {

            for name in files {
                if !locked.contains_key(&name) {
                    locked.insert(name.clone(), 0);
                }
            }

        }
        
        
    }

    pub fn increese_file_nrequest(list : &FileList, name : String)
    {
        /*
        let mut locked = list.lock().unwrap();
        *locked.entry(name).or_insert(0) += 1;

        */
        
        if let  Ok(mut locked) = list.lock() {
            *locked.entry(name).or_insert(0) += 1;
        }
        
    }


    pub fn add_message_for_chat(        // message exchange between client (communication server)
                                        chat: &Chatlist,
                                        source_id: NodeId,
                                        destination_id: NodeId,
                                        message : String)
    {
/*
        let mut locked = chat.lock().unwrap();
        let key: BTreeSet<NodeId> = [source_id, destination_id].into_iter().collect();
        locked.entry(key)
            .or_insert_with(Vec::new)
            .push((source_id, message));   */ 
    
        
        if let Ok (mut locked) = chat.lock()
        {
            let key: BTreeSet<NodeId> = [source_id, destination_id].into_iter().collect();
            locked.entry(key)
                .or_insert_with(Vec::new)
                .push((source_id, message));
        }
    
    
    }


    pub fn recive_flood_interface(
        grafo: &Graph,
        path_trace: Vec<(NodeId, NodeType)>,
    ) {
       /*
        let mut grafo = grafo.lock().unwrap();

        for window in path_trace.windows(2) {
            if let [(a_id, a_type), (b_id, b_type)] = window {
                // Nodo A
                grafo
                    .entry(*a_id)
                    .and_modify(|(vicini, tipo)| {
                        vicini.insert(*b_id);
                        *tipo = *a_type;
                    })
                    .or_insert_with(|| {
                        let mut vicini = HashSet::new();
                        vicini.insert(*b_id);
                        (vicini, *a_type)
                    });

                // Nodo B
                grafo
                    .entry(*b_id)
                    .and_modify(|(vicini, tipo)| {
                        vicini.insert(*a_id);
                        *tipo = *b_type;
                    })
                    .or_insert_with(|| {
                        let mut vicini = HashSet::new();
                        vicini.insert(*a_id);
                        (vicini, *b_type)
                    });
            }
        }*/
        
        
        if let Ok(mut grafo) = grafo.lock()
        {

            for window in path_trace.windows(2) {
                if let [(a_id, a_type), (b_id, b_type)] = window {
                    // Nodo A
                    grafo
                        .entry(*a_id)
                        .and_modify(|(vicini, tipo)| {
                            vicini.insert(*b_id);
                            *tipo = *a_type;
                        })
                        .or_insert_with(|| {
                            let mut vicini = HashSet::new();
                            vicini.insert(*b_id);
                            (vicini, *a_type)
                        });

                    // Nodo B
                    grafo
                        .entry(*b_id)
                        .and_modify(|(vicini, tipo)| {
                            vicini.insert(*a_id);
                            *tipo = *b_type;
                        })
                        .or_insert_with(|| {
                            let mut vicini = HashSet::new();
                            vicini.insert(*a_id);
                            (vicini, *b_type)
                        });
                }
            }
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
            enable_raw_mode().expect("impossible to open raw mode");
            let mut stdout = stdout();
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture).expect("impossible to capture impute");
            let backend = CrosstermBackend::new(stdout);
            let mut terminal = Terminal::new(backend).expect("impossible to get terminal");

            let _ = run_app(&mut terminal, all_servers);

            disable_raw_mode().expect("impossible to close raw mode");
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )
                .expect("impossible to leave alternate screen");
            terminal.show_cursor().expect("impossible to show cursor");
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
                let servers_guard = all_servers.lock().expect("impossible to lock all_servers");
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



                            //generate immage



                            let grafo = srv.graph.lock().expect("impossible to lock graph");

                            let mut dot = String::from("graph G {\n");
                            dot.push_str("    node [fontname=Helvetica];\n");

                            // 1. NODI personalizzati
                            for (node_id, (_, node_type)) in grafo.iter() {
                                let (label, fillcolor, shape, penwidth, fontcolor) = match node_type {
                                    NodeType::Drone => (
                                        format!("Drone {}", node_id),
                                        "deepskyblue",
                                        "ellipse",
                                        "1",
                                        "black",
                                    ),
                                    NodeType::Client => (
                                        format!("Client {}", node_id),
                                        "lightgreen",
                                        "oval",
                                        "1",
                                        "black",
                                    ),
                                    NodeType::Server => {
                                        let label = format!("Server {}", node_id);
                                        if *node_id as usize == srv.id  {
                                            // server corrente
                                            (
                                                format!("curr: Server {}", node_id),
                                                "red",
                                                "box",
                                                "3",
                                                "white",
                                            )
                                        } else {
                                            // altro server
                                            (
                                                label,
                                                "darkorange",
                                                "cylinder",
                                                "1",
                                                "black",
                                            )
                                        }
                                    }
                                };

                                dot.push_str(&format!(
                                    "    {} [label=\"{}\", fillcolor={}, shape={}, style=filled, penwidth={}, fontcolor={}];\n",
                                    node_id, label, fillcolor, shape, penwidth, fontcolor
                                ));
                            }

                            // 2. ARCHI
                            let mut seen = HashSet::new();
                            for (a, (vicini, _)) in grafo.iter() {
                                for b in vicini {
                                    let (min, max) = if a < b { (a, b) } else { (b, a) };
                                    if seen.insert((min, max)) {
                                        dot.push_str(&format!("    {} -- {};\n", min, max));
                                    }
                                }
                            }

                            dot.push_str("}\n");






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
                        }

                        else
                        {

                            //generate text


                            let grafo = &srv.graph.lock().expect("impossible to lock graph");



                            // Header
                            info.push_str(&format!("Current id: {}\n", srv.id));

                            // Neighbours
                            let neighbours: Vec<_> = grafo
                                .get(&(srv.id as NodeId))
                                .map(|(ns, _)| {
                                    let mut v: Vec<_> = ns.iter().cloned().collect();
                                    v.sort();
                                    v
                                })
                                .unwrap_or_default();
                            info.push_str("Current neighbours: ");
                            if neighbours.is_empty() {
                                info.push_str("-\n");
                            } else {
                                info.push_str(&neighbours.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "));
                                info.push('\n');
                            }

                            // Liste client/server
                            let mut clienti = vec![];
                            let mut server = vec![];

                            for (id, (_, tipo)) in grafo.iter() {
                                match tipo {
                                    NodeType::Client => clienti.push(*id),
                                    NodeType::Server => {
                                        if *id as usize == srv.id {
                                            server.push(format!("{} (io stesso)", id));
                                        } else {
                                            server.push(id.to_string());
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            clienti.sort();
                            server.sort();

                            info.push_str(&format!(
                                "Clienti trovati: {}\n",
                                if clienti.is_empty() {
                                    "-".into()
                                } else {
                                    clienti
                                        .iter()
                                        .map(|x| x.to_string())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                }
                            ));

                            info.push_str(&format!(
                                "Server trovati: {}\n",
                                if server.is_empty() {
                                    "-".into()
                                } else {
                                    server.join(", ")
                                }
                            ));

                            // Topologia
                            info.push_str("\nGraph topology:\n");
                            let mut ids: Vec<_> = grafo.keys().cloned().collect();
                            ids.sort();

                            for id in ids {
                                let mut vicini: Vec<_> = grafo.get(&id).expect("can access topology graph").0.iter().cloned().collect();
                                vicini.sort();
                                info.push_str(&format!(
                                    "{} → {}\n",
                                    id,
                                    if vicini.is_empty() {
                                        "-".into()
                                    } else {
                                        vicini
                                            .iter()
                                            .map(|x| x.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    }
                                ));
                            }

                        }



                        let paragraph = Paragraph::new(info)
                            .block(Block::default().title("Grafo").borders(Borders::ALL))
                            .style(Style::default().fg(Color::LightCyan));

                        f.render_widget(paragraph, layout[1]);
                    }













                    Mode::Chat => {
                        let mut items: Vec<ListItem> = {
                            let locked = srv.messages.lock().expect("can't load the messages");
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




                        if srv.path == "."




                        /* ─────────────────────────  CHAT VIEW con sidebar  ───────────────────────── */
                        {
                            // 1. prendo le chat presenti e le ordino
                            let chat_map_guard = srv.chat_for_client.lock().expect("can't get chat map for mod 2");
                            let mut chat_keys: Vec<_> = chat_map_guard
                                .keys()
                                .filter(|k| k.len() == 2)          // solo coppie vere
                                .cloned()
                                .collect();
                            chat_keys.sort();

                            /*── layout interno: 30 col per la lista, resto per i messaggi ──*/
                            let chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([Constraint::Length(30), Constraint::Min(0)].as_ref())
                                .split(layout[1]);

                            /*──────────────── 1A. Sidebar con le chat ────────────────*/
                            let items: Vec<ListItem> = chat_keys
                                .iter()
                                .map(|key| {
                                    let mut it = key.iter();
                                    let a = it.next().unwrap_or(&99);
                                    let b = it.next().unwrap_or(&99);
                                    ListItem::new(Line::from(format!("{} ⇄ {}", a, b)))
                                })
                                .collect();

                            // indice selezionato (clamp sicuro)
                            let list_len = chat_keys.len();
                            let mut idx = srv.selected_chat_index.load(Ordering::Relaxed);
                            if idx >= list_len {
                                idx = idx.saturating_sub(1);
                                srv.selected_chat_index.store(idx, Ordering::Relaxed);
                            }

                            let mut sidebar_state = ListState::default();
                            if list_len > 0 {
                                sidebar_state.select(Some(idx));
                            }

                            let sidebar = List::new(items)
                                .block(Block::default().title("Chat").borders(Borders::ALL))
                                .highlight_style(Style::default().fg(Color::Green));

                            f.render_stateful_widget(sidebar, chunks[0], &mut sidebar_state);

                            /*──────────────── 1B. Pannello messaggi ────────────────*/
                            let content: Vec<Line> = if list_len == 0 {
                                vec![Line::from("📭 Nessuna conversazione.")]
                            } else {
                                let key = &chat_keys[idx];
                                let mut it = key.iter();
                                let a = it.next().unwrap_or(&99);
                                let b = it.next().unwrap_or(&99);

                                let msgs = chat_map_guard.get(key).expect("problem loading conversation");   // esiste di sicuro

                                let mut lines = vec![
                                    Line::from(format!("💬 Chat tra {} ⇄ {}", a, b)),
                                    Line::from("-------------------------------"),
                                ];

                                for (sender, text) in msgs {
                                    if *sender == *a {
                                        lines.push(Line::from(vec![
                                            Span::styled(format!("{}: ", sender), Style::default().fg(Color::Yellow)),
                                            Span::raw(text),
                                        ]));
                                    } else {
                                        let pad = " ".repeat(40usize.saturating_sub(text.len().min(40)));
                                        lines.push(Line::from(vec![
                                            Span::raw(pad),
                                            Span::styled(format!("{} :{}", text, sender), Style::default().fg(Color::Cyan)),
                                        ]));
                                    }
                                }
                                lines
                            };

                            let paragraph = Paragraph::new(content)
                                .block(Block::default().borders(Borders::ALL))
                                .style(Style::default().fg(Color::White));

                            f.render_widget(paragraph, chunks[1]);
                        }
                        /* ─────────────────────────────────────────────────────────────────────────── */









                        else {
                            // ───────────────────────────── FILE-LIST VIEW ─────────────────────────────
                            let files = fs::read_dir(&srv.path)
                                .unwrap_or_else(|_| fs::read_dir(".").expect("Can't acces  . dir "))
                                .filter_map(|e| e.ok())
                                .map(|e| e.file_name().to_string_lossy().to_string())
                                .collect::<Vec<_>>();

                            refresh(&srv.list_of_files, files.clone());

                            let file_map = srv.list_of_files.lock().expect("can't access list_of_files");
                            let mut files: Vec<_> = file_map.iter().collect();
                            files.sort_by(|a, b| b.1.cmp(a.1));                 // per occorrenze decrescente

                            let mut lines = vec![
                                Line::from(vec![
                                    Span::styled("📁 Nome File", Style::default().fg(Color::Yellow)),
                                    Span::raw("  |  "),
                                    Span::styled("📊 Occorrenze", Style::default().fg(Color::Yellow)),
                                ]),
                                Line::from("------------------------------"),
                            ];

                            for (name, count) in files {
                                lines.push(Line::from(format!("{:<20} |  {}", name, count)));
                            }

                            let paragraph = Paragraph::new(lines)
                                .block(Block::default().title("File disponibili").borders(Borders::ALL))
                                .style(Style::default().fg(Color::White));

                            f.render_widget(paragraph, layout[1]);
                        }








                    }
                }

                /* ---- Footer ---- */

                let footer_text = {
                    let server_name = &srv.name;
                    if server_name.starts_with("Communication") {
                        "←/→ cambia server  |  1 Messages - 2 Chat - 3 Graph  |  ↑↓ scroll  |  q quit"
                    } else {
                        "←/→ cambia server  |  1 Messages - 2 File - 3 Graph  |  ↑↓ scroll  |  q quit"
                    }
                };

                let footer = Paragraph::new(

                    footer_text

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
                            let len = all_servers.lock().expect("can't access the server while using the arrow <-  ->").len();
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
                                let srv = &all_servers.lock().expect("can't access server lock  UP")[current_srv];
                                let cur = srv.selected_index.load(Ordering::Relaxed);
                                if cur > 0 {
                                    srv.selected_index.store(cur - 1, Ordering::Relaxed);
                                }
                                auto_scroll = false;
                            }

                            if matches!(global_mode, Mode::FileList) {
                                let srv = &all_servers.lock().expect("can't access server lock  UP")[current_srv];
                                let cur = srv.selected_chat_index.load(Ordering::Relaxed);

                                if (srv.path==".")
                                {
                                    if cur > 0 {
                                        srv.selected_chat_index.store(cur - 1, Ordering::Relaxed);
                                    }
                                }

                            }


                        }
                        KeyCode::Down => {
                            if matches!(global_mode, Mode::Chat) {
                                let srv = &all_servers.lock().expect("can't access server lock  DOWN")[current_srv];
                                let len = srv.messages.lock().expect("can't access server message  UP").len();
                                let cur = srv.selected_index.load(Ordering::Relaxed);
                                if cur + 1 < len {
                                    srv.selected_index.store(cur + 1, Ordering::Relaxed);
                                    auto_scroll = false;
                                } else {
                                    auto_scroll = true;
                                }
                            }


                            if matches!(global_mode, Mode::FileList) {
                                let srv = &all_servers.lock().expect("can't access server lock  DOWN")[current_srv];
                                let len = srv.chat_for_client.lock().expect("can't access chat client DOWN").len();
                                let cur = srv.selected_chat_index.load(Ordering::Relaxed);

                                if (srv.path==".")
                                {
                                    if cur + 1 < len{
                                        srv.selected_chat_index.store(cur + 1, Ordering::Relaxed);
                                    }
                                }

                            }
                        }

                        /* ---- Uscita ---- */
                        KeyCode::Char('q') =>
                            {
                                //std::process::exit(0);
                                return Ok(())
                            }
                        _ => {

                        }
                    }
                }
            }
        }
    }
}