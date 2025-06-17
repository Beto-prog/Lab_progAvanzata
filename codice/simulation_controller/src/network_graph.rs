use egui::{Color32, Label, Shape};
use egui_graphs::{
    DefaultEdgeShape, DefaultNodeShape, Graph, Node, SettingsInteraction, SettingsNavigation,
    SettingsStyle,
};
use petgraph::{prelude::StableGraph, Undirected};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeType {
    Drone,
    Client,
    Server,
}

#[derive(Clone, Debug)]
struct NodeData {
    label: String,
    node_type: NodeType,
}
pub struct NetworkGraph {
    graph: Graph<NodeData, String, Undirected>,
}

impl NetworkGraph {
    pub fn new() -> Self {
        let mut g = StableGraph::<NodeData, String, Undirected>::default();
        let drone = NodeData {
            label: "Drone".to_string(),
            node_type: NodeType::Drone,
        };
        let a = g.add_node(drone);
        let client = NodeData {
            label: "Client".to_string(),
            node_type: NodeType::Client,
        };
        let b = g.add_node(client);
        let server = NodeData {
            label: "Server".to_string(),
            node_type: NodeType::Server,
        };
        let c = g.add_node(server);

        g.add_edge(a, b, "Client".to_string());
        g.add_edge(b, c, "Server".to_string());

        Self {
            graph: Graph::from(&g),
        }
    }

    pub fn show_ui(&mut self, ui: &mut egui::Ui) {
        let interaction_settings = &SettingsInteraction::new().with_dragging_enabled(true);

        let style_settings = &SettingsStyle::new().with_labels_always(false);
        let navigation_settings = &SettingsNavigation::new()
            .with_fit_to_screen_enabled(false)
            .with_zoom_and_pan_enabled(true);
        ui.add(
            &mut egui_graphs::GraphView::<NodeData, String, Undirected>::new(&mut self.graph)
                .with_styles(style_settings)
                .with_interactions(interaction_settings)
                .with_navigations(navigation_settings),
        );
    }
}
