use egui::{
    epaint::{CircleShape, TextShape},
    FontFamily, FontId, Pos2, Shape, Vec2,
};
use egui_graphs::{DisplayNode, NodeProps};
use petgraph::{csr::IndexType, EdgeType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Drone,
    Client,
    Server,
}

#[derive(Clone, Debug)]
pub struct NodeData {
    pub label: String,
    pub node_type: NodeType,
}

trait ColoredData {
    fn color(&self) -> egui::Color32;
    fn label(&self) -> &str;
}

impl ColoredData for NodeData {
    fn color(&self) -> egui::Color32 {
        match self.node_type {
            NodeType::Drone => egui::Color32::from_rgb(0, 0, 255),
            NodeType::Client => egui::Color32::from_rgb(255, 0, 0),
            NodeType::Server => egui::Color32::from_rgb(0, 255, 0),
        }
    }

    fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug)]
pub struct ColoredNode {
    label: String,
    color: egui::Color32,
    size: f32,
    loc: Pos2,
}

impl<N: Clone + ColoredData> From<NodeProps<N>> for ColoredNode {
    fn from(node_props: NodeProps<N>) -> Self {
        Self {
            label: node_props.payload.label().to_string(),
            color: node_props.payload.color(),
            loc: node_props.location(),
            size: 10.0,
        }
    }
}

impl<N: Clone + ColoredData, E: Clone, Ty: EdgeType, Ix: IndexType> DisplayNode<N, E, Ty, Ix>
    for ColoredNode
{
    fn is_inside(&self, pos: Pos2) -> bool {
        let vector_to_point = pos - self.loc;

        let dist_sq = vector_to_point.length_sq();

        let radius_sq = self.size * self.size;

        dist_sq <= radius_sq
    }

    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        let normalized_dir = dir.normalized();

        let offset_from_center = normalized_dir * self.size;

        self.loc + offset_from_center
    }

    fn shapes(&mut self, ctx: &egui_graphs::DrawContext) -> Vec<egui::Shape> {
        let center = ctx.meta.canvas_to_screen_pos(self.loc);
        let size = ctx.meta.canvas_to_screen_size(self.size);
        let color = self.color;
        let shape = Shape::Circle(CircleShape::filled(center, size, color));

        let color = egui::Color32::from_rgb(255, 255, 255);
        let galley = ctx.ctx.fonts(|f| {
            f.layout_no_wrap(
                self.label.clone(),
                FontId::new(ctx.meta.canvas_to_screen_size(10.), FontFamily::Monospace),
                color,
            )
        });

        let offset = Vec2::new(-galley.size().x / 2., -galley.size().y / 2.);

        let shape_label = TextShape::new(center + offset, galley, color);

        vec![shape, shape_label.into()]
    }

    fn update(&mut self, state: &NodeProps<N>) {
        self.loc = state.location();
    }
}
