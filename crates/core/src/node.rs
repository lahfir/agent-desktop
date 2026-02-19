use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,

    pub role: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub states: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<AccessibilityNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn bounds_hash(&self) -> u64 {
        use rustc_hash::FxHasher;
        use std::hash::{Hash, Hasher};
        let mut h = FxHasher::default();
        let x = (self.x * 100.0) as i64;
        let y = (self.y * 100.0) as i64;
        let w = (self.width * 100.0) as i64;
        let hh = (self.height * 100.0) as i64;
        x.hash(&mut h);
        y.hash(&mut h);
        w.hash(&mut h);
        hh.hash(&mut h);
        h.finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    #[serde(rename = "app_name")]
    pub app: String,
    pub pid: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
    pub is_focused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub pid: i32,
    pub bundle_id: Option<String>,
}
