use std::sync::Arc;
use std::time::Instant;
use crate::core::Memory;

pub struct DexNode {
    pub address: u64,
    pub name: String,
    pub class_name: String,
    pub value_display: Option<String>,
    pub children: Vec<DexNode>,
    pub children_loaded: bool,
    pub expanded: bool,
    pub child_count: usize,
    pub depth: usize,
    pub children_fully_displayed: bool,
}

impl DexNode {
    pub fn new(address: u64, name: String, class_name: String, depth: usize) -> Self {
        Self {
            address,
            name,
            class_name,
            value_display: None,
            children: Vec::new(),
            children_loaded: false,
            expanded: false,
            child_count: 0,
            depth,
            children_fully_displayed: false,
        }
    }

    pub fn is_value_type(&self) -> bool { false }
    pub fn icon(&self) -> &'static str { "📦" }
}

pub struct SearchHit {
    pub path: String,
    pub name: String,
    pub class_name: String,
    pub address: u64,
}

pub struct DexExplorer {
    pub root_nodes: Vec<DexNode>,
    pub place_id: u64,
    pub game_id: u64,
    pub creator_id: u64,
    pub datamodel_address: u64,
    pub total_nodes_loaded: usize,
    pub last_refresh: Instant,
    pub full_scan_in_progress: bool,
    pub full_scan_progress: String,
    #[allow(dead_code)]
    pub search_query: String,
    pub search_results: Vec<SearchHit>,
    pub status: String,
    pub is_initialized: bool,
    pub tree_dirty: bool,
}

impl DexExplorer {
    pub fn new(_memory: Arc<Memory>) -> Self {
        Self {
            root_nodes: Vec::new(),
            place_id: 0,
            game_id: 0,
            creator_id: 0,
            datamodel_address: 0,
            total_nodes_loaded: 0,
            last_refresh: Instant::now(),
            full_scan_in_progress: false,
            full_scan_progress: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            status: String::from("Premium feature - join Discord for access"),
            is_initialized: false,
            tree_dirty: false,
        }
    }

    pub fn initialize(&mut self) {}
    pub fn expand_node(&mut self, _path: &[usize]) {}
    pub fn load_more_children(&mut self, _path: &[usize]) {}
    pub fn collapse_node(&mut self, _path: &[usize]) {}
    pub fn collapse_all(&mut self) {}
    pub fn full_scan(&mut self) {}
    pub fn refresh_values(&mut self) {}
    pub fn search(&mut self, _query: &str) {}
    pub fn deep_search(&mut self, _query: &str) {}
    pub fn get_node(&self, _path: &[usize]) -> Option<&DexNode> { None }
}
