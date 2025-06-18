use std::collections::{HashMap, VecDeque};

use egui_graphs::{
    DefaultEdgeShape, Graph, Metadata, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::{
    csr::DefaultIx,
    graph::{EdgeIndex, NodeIndex},
    prelude::StableGraph,
    Undirected,
};
use wg_2024::network::NodeId;

use crate::{
    colored_data::{self, ColoredNode, NodeData},
    packet_animation::{AnimationType, PacketAnimation},
};

pub struct NetworkGraph {
    graph: Graph<NodeData, String, Undirected, DefaultIx, ColoredNode, DefaultEdgeShape>,
    node_indexes: HashMap<NodeId, NodeIndex>,
    edge_indexes: HashMap<(NodeId, NodeId), EdgeIndex>,
    packet_animations: HashMap<(u64, u64), VecDeque<PacketAnimation>>,
    active_animations: HashMap<(u64, u64), PacketAnimation>,
}

impl NetworkGraph {
    pub fn new(
        drones: Vec<NodeId>,
        clients: Vec<NodeId>,
        servers: Vec<NodeId>,
        edges: Vec<(NodeId, NodeId)>,
    ) -> Self {
        let mut g = StableGraph::<NodeData, String, Undirected>::default();

        let mut node_indexes = HashMap::new();
        let mut edge_indexes = HashMap::new();

        for drone_id in drones {
            let drone = NodeData {
                label: drone_id.to_string(),
                node_type: colored_data::NodeType::Drone,
            };
            let a = g.add_node(drone);
            node_indexes.insert(drone_id, a);
        }
        for client_id in clients {
            let client = NodeData {
                label: client_id.to_string(),
                node_type: colored_data::NodeType::Client,
            };
            let b = g.add_node(client);
            node_indexes.insert(client_id, b);
        }
        for server_id in servers {
            let server = NodeData {
                label: server_id.to_string(),
                node_type: colored_data::NodeType::Server,
            };
            let c = g.add_node(server);
            node_indexes.insert(server_id, c);
        }
        for (a, b) in edges {
            let e = g.add_edge(node_indexes[&a], node_indexes[&b], 0.to_string());
            edge_indexes.insert((a, b), e);
        }

        let graph = Graph::from(&g);

        Self {
            graph,
            node_indexes,
            edge_indexes,
            packet_animations: HashMap::new(),
            active_animations: HashMap::new(),
        }
    }

    pub fn add_packet_animation(
        &mut self,
        packet_id: (u64, u64),
        start: NodeId,
        end: NodeId,
        animation_type: AnimationType,
    ) {
        let packet_animation_queue = self.packet_animations.entry(packet_id).or_default();
        packet_animation_queue.push_back(PacketAnimation {
            source: start,
            dest: end,
            start_time: 0.0,
            anim_type: animation_type,
        });
    }

    pub fn crash_drone(&mut self, drone_id: NodeId) {
        let drone_index = self.node_indexes[&drone_id];
        self.graph.remove_node(drone_index);
        self.node_indexes.remove(&drone_id);
    }

    pub fn add_connection(&mut self, drone_id: NodeId, neighbour_id: NodeId) {
        let drone_index = self.node_indexes[&drone_id];
        let neighbour_index = self.node_indexes[&neighbour_id];

        let edge = self
            .graph
            .add_edge(drone_index, neighbour_index, 0.to_string());
        self.edge_indexes.insert((drone_id, neighbour_id), edge);
    }

    pub fn remove_connection(&mut self, drone_id: NodeId, neighbour_id: NodeId) {
        let min = drone_id.min(neighbour_id);
        let max = drone_id.max(neighbour_id);

        let edge_index = self.edge_indexes[&(min, max)];

        self.graph.remove_edge(edge_index);
        self.edge_indexes.remove(&(min, max));
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui, now: f64) {
        let interaction_settings = &SettingsInteraction::new().with_dragging_enabled(true);

        self.active_animations.retain(|_, animation| {
            if let (Some(&source_idx), Some(&dest_idx)) = (
                self.node_indexes.get(&animation.source),
                self.node_indexes.get(&animation.dest),
            ) {
                if let (Some(source_node), Some(dest_node)) =
                    (self.graph.node(source_idx), self.graph.node(dest_idx))
                {
                    let source_pos = source_node.location();
                    let dest_pos = dest_node.location();
                    let distance = source_pos.distance(dest_pos) / 100.0;
                    return now - animation.start_time < distance.into();
                }
            }
            return false;
        });

        for (key, animation_queue) in self.packet_animations.iter_mut() {
            if !self.active_animations.contains_key(&key) {
                let next_animation = animation_queue.pop_front();
                if let Some(mut animation) = next_animation {
                    animation.start_time = now;
                    self.active_animations.insert(*key, animation);
                }
            }
        }

        let style_settings = &SettingsStyle::new().with_labels_always(false);
        let navigation_settings = &SettingsNavigation::new()
            .with_fit_to_screen_enabled(false)
            .with_zoom_and_pan_enabled(true);

        let graph_view = &mut egui_graphs::GraphView::<
            NodeData,
            String,
            Undirected,
            _,
            ColoredNode,
            DefaultEdgeShape,
        >::new(&mut self.graph)
        .with_styles(style_settings)
        .with_interactions(interaction_settings)
        .with_navigations(navigation_settings);

        ui.add(graph_view);

        let painter = ui.painter();

        let meta = Metadata::load(ui);

        let to_screen =
            |canvas_pos: egui::Pos2| (canvas_pos.to_vec2() * meta.zoom + meta.pan).to_pos2();

        for anim in self.active_animations.values() {
            if let (Some(&source_idx), Some(&dest_idx)) = (
                self.node_indexes.get(&anim.source),
                self.node_indexes.get(&anim.dest),
            ) {
                if let (Some(source_node), Some(dest_node)) =
                    (self.graph.node(source_idx), self.graph.node(dest_idx))
                {
                    let source_pos = source_node.location();
                    let dest_pos = dest_node.location();

                    let progress = ((now - anim.start_time)
                        / ((source_pos.distance(dest_pos) as f64) / 100.0))
                        as f32;
                    let source_screen_pos = to_screen(source_pos);
                    let dest_screen_pos = to_screen(dest_pos);

                    let packet_pos = source_screen_pos.lerp(dest_screen_pos, progress);

                    // NEW: Select color based on the animation type
                    let color = match anim.anim_type {
                        AnimationType::Fragment => egui::Color32::YELLOW,
                        AnimationType::Ack => egui::Color32::GREEN,
                        AnimationType::Nack => egui::Color32::RED,
                    };

                    // Draw the packet with the selected color
                    painter.circle_filled(packet_pos, 5.0 * meta.zoom, color);
                }
            }
        }
    }
}
