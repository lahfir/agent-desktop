use serde::{Deserialize, Deserializer, Serialize};

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub states: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<AccessibilityNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rect {
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub x: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub y: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub width: f64,
    #[serde(default, deserialize_with = "f64_or_zero")]
    pub height: f64,
}

fn f64_or_zero<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Option::<f64>::deserialize(deserializer).map(|opt| opt.unwrap_or(0.0))
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceInfo {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_count: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_null_fields_deserialize() {
        let json = r#"{"x": null, "y": null, "width": 0.0, "height": 0.0}"#;
        let rect: Rect = serde_json::from_str(json).unwrap();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
    }

    #[test]
    fn test_rect_missing_fields_deserialize() {
        let json = r#"{"width": 100.0, "height": 50.0}"#;
        let rect: Rect = serde_json::from_str(json).unwrap();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 100.0);
    }

    #[test]
    fn test_rect_normal_roundtrip() {
        let rect = Rect {
            x: 10.5,
            y: 20.3,
            width: 100.0,
            height: 50.0,
        };
        let json = serde_json::to_string(&rect).unwrap();
        let back: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(back.x, 10.5);
        assert_eq!(back.width, 100.0);
    }
}
