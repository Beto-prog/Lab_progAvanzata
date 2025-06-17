use std::collections::HashMap;

use egui_graphs::{
    DefaultEdgeShape, Graph, SettingsInteraction, SettingsNavigation, SettingsStyle,
};
use petgraph::{csr::DefaultIx, prelude::StableGraph, Undirected};
use wg_2024::network::NodeId;

use crate::colored_data::{self, ColoredNode, NodeData};

pub struct NetworkGraph {
    graph: Graph<NodeData, String, Undirected, DefaultIx, ColoredNode, DefaultEdgeShape>,
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
            g.add_edge(node_indexes[&a], node_indexes[&b], 0.to_string());
        }

        let graph = Graph::from(&g);

        Self { graph }
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui) {
        let interaction_settings = &SettingsInteraction::new().with_dragging_enabled(true);

        let style_settings = &SettingsStyle::new().with_labels_always(false);
        let navigation_settings = &SettingsNavigation::new()
            .with_fit_to_screen_enabled(false)
            .with_zoom_and_pan_enabled(true);
        ui.add(
            &mut egui_graphs::GraphView::<
                NodeData,
                String,
                Undirected,
                _,
                ColoredNode,
                DefaultEdgeShape,
            >::new(&mut self.graph)
            .with_styles(style_settings)
            .with_interactions(interaction_settings)
            .with_navigations(navigation_settings),
        );
    }
}
