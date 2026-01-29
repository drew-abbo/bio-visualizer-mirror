use std::borrow::Cow;

use crate::view::View;
use egui::Color32;
use egui_node_editor::*;

#[derive(Clone, Debug, Default)]
struct UserState;

#[derive(Clone, Debug)]
struct NodeData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataType {
    Flow,
}

impl DataTypeTrait<UserState> for DataType {
    fn data_type_color(&self, _user_state: &mut UserState) -> Color32 {
        match self {
            DataType::Flow => Color32::from_rgb(120, 200, 255),
        }
    }

    fn name(&self) -> Cow<'_, str> {
        match self {
            DataType::Flow => Cow::Borrowed("Flow"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ValueType {
    Flow,
}

impl Default for ValueType {
    fn default() -> Self {
        ValueType::Flow
    }
}

#[derive(Clone, Debug)]
struct MyResponse;

impl UserResponseTrait for MyResponse {}

impl WidgetValueTrait for ValueType {
    type Response = MyResponse;
    type UserState = UserState;
    type NodeData = NodeData;

    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: NodeId,
        ui: &mut egui::Ui,
        _user_state: &mut UserState,
        _node_data: &NodeData,
    ) -> Vec<MyResponse> {
        ui.label(param_name);
        Vec::new()
    }
}

impl NodeDataTrait for NodeData {
    type Response = MyResponse;
    type UserState = UserState;
    type DataType = DataType;
    type ValueType = ValueType;

    fn bottom_ui(
        &self,
        _ui: &mut egui::Ui,
        _node_id: NodeId,
        _graph: &Graph<NodeData, DataType, ValueType>,
        _user_state: &mut UserState,
    ) -> Vec<NodeResponse<MyResponse, NodeData>>
    where
        MyResponse: UserResponseTrait,
    {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeTemplate {
    BasicNode,
}

impl NodeTemplateTrait for NodeTemplate {
    type NodeData = NodeData;
    type DataType = DataType;
    type ValueType = ValueType;
    type UserState = UserState;
    type CategoryType = &'static str;

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        match self {
            NodeTemplate::BasicNode => Cow::Borrowed("Node"),
        }
    }

    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        vec!["Basic"]
    }

    fn node_graph_label(&self, _user_state: &mut Self::UserState) -> String {
        match self {
            NodeTemplate::BasicNode => "Node".to_string(),
        }
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        NodeData
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
        node_id: NodeId,
    ) {
        match self {
            NodeTemplate::BasicNode => {
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    DataType::Flow,
                    ValueType::Flow,
                    InputParamKind::ConnectionOrConstant,
                    true,
                );
                graph.add_output_param(node_id, "Out".to_string(), DataType::Flow);
            }
        }
    }
}

struct AllNodeTemplates;

impl NodeTemplateIter for AllNodeTemplates {
    type Item = NodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        vec![NodeTemplate::BasicNode]
    }
}

type GraphState = GraphEditorState<NodeData, DataType, ValueType, NodeTemplate, UserState>;

pub struct NodeGraphView {
    state: GraphState,
    user_state: UserState,
}

impl NodeGraphView {
    pub fn new() -> Self {
        Self {
            state: GraphState::default(),
            user_state: UserState::default(),
        }
    }
}

impl View for NodeGraphView {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let _graph_response = self.state.draw_graph_editor(
            ui,
            AllNodeTemplates,
            &mut self.user_state,
            Vec::default(),
        );
    }
}
