use crate::{
    error::{AdapterError, ErrorCode},
    native_handle::NativeHandle,
    node::AccessibilityNode,
    roles, search_text,
    state::{self, STATE_VOCABULARY},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatePredicate {
    pub token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocatorQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_text: Option<String>,
    #[serde(default)]
    pub exact: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<StatePredicate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has: Option<Box<LocatorQuery>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_not: Option<Box<LocatorQuery>>,
}

impl LocatorQuery {
    pub fn is_empty(&self) -> bool {
        self.role.is_none()
            && self.name.is_none()
            && self.description.is_none()
            && self.native_id.is_none()
            && self.value.is_none()
            && self.has_text.is_none()
            && self.states.is_empty()
            && self.has.is_none()
            && self.has_not.is_none()
    }

    pub fn validate_states(&self) -> Result<(), AdapterError> {
        for predicate in &self.states {
            if !STATE_VOCABULARY.contains(&predicate.token.as_str()) {
                return Err(AdapterError::new(
                    ErrorCode::InvalidArgs,
                    format!("Unknown state token '{}'", predicate.token),
                )
                .with_suggestion(format!("Use one of: {}", STATE_VOCABULARY.join(", "))));
            }
        }
        if let Some(has) = &self.has {
            has.validate_states()?;
        }
        if let Some(has_not) = &self.has_not {
            has_not.validate_states()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryClassification {
    NotFound,
    Single(usize),
    Ambiguous { candidate_indices: Vec<usize> },
}

pub fn classify_query_result(handles: &[NativeHandle]) -> QueryClassification {
    match handles.len() {
        0 => QueryClassification::NotFound,
        1 => QueryClassification::Single(0),
        count if count > 1 => QueryClassification::Ambiguous {
            candidate_indices: (0..count).collect(),
        },
        _ => QueryClassification::NotFound,
    }
}

pub fn ambiguous_candidate_summaries(
    handles: &[NativeHandle],
    summaries: &[QueryCandidateSummary],
) -> serde_json::Value {
    let capped: Vec<_> = summaries.iter().take(10).cloned().collect();
    serde_json::json!({
        "candidate_count": handles.len(),
        "candidates": capped,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryCandidateSummary {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<String>,
}

pub struct NodeMatchContext<'a> {
    pub role: &'a str,
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub native_id: Option<&'a str>,
    pub value: Option<&'a str>,
    pub states: &'a [String],
    pub children: &'a [AccessibilityNode],
}

pub fn node_context(node: &AccessibilityNode) -> NodeMatchContext<'_> {
    NodeMatchContext {
        role: &node.role,
        name: node.name.as_deref(),
        description: node.description.as_deref(),
        native_id: node.native_id.as_deref(),
        value: node.value.as_deref(),
        states: &node.states,
        children: &node.children,
    }
}

pub fn node_matches(query: &LocatorQuery, ctx: NodeMatchContext<'_>) -> bool {
    if !role_matches(query, ctx.role) {
        return false;
    }
    if !text_field_matches(query.name.as_deref(), ctx.name, query.exact) {
        return false;
    }
    if !text_field_matches(query.description.as_deref(), ctx.description, query.exact) {
        return false;
    }
    if !native_id_matches(query.native_id.as_deref(), ctx.native_id) {
        return false;
    }
    if !text_field_matches(query.value.as_deref(), ctx.value, query.exact) {
        return false;
    }
    if !has_text_matches(query.has_text.as_deref(), &ctx) {
        return false;
    }
    if !state_predicates_match(&query.states, ctx.states) {
        return false;
    }
    if let Some(has) = &query.has {
        if !subtree_contains(has, ctx.children) {
            return false;
        }
    }
    if let Some(has_not) = &query.has_not {
        if subtree_contains(has_not, ctx.children) {
            return false;
        }
    }
    true
}

pub fn accessibility_node_matches(node: &AccessibilityNode, query: &LocatorQuery) -> bool {
    node_matches(query, node_context(node))
}

fn role_matches(query: &LocatorQuery, role: &str) -> bool {
    query
        .role
        .as_deref()
        .is_none_or(|expected| roles::normalize_role_query(expected) == role)
}

fn text_field_matches(expected: Option<&str>, actual: Option<&str>, exact: bool) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    let Some(actual) = actual else {
        return false;
    };
    if exact {
        search_text::normalize(actual) == expected
    } else {
        search_text::contains(actual, expected)
    }
}

fn native_id_matches(expected: Option<&str>, actual: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    actual == Some(expected)
}

fn has_text_matches(expected: Option<&str>, ctx: &NodeMatchContext<'_>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    [ctx.name, ctx.description, ctx.value]
        .into_iter()
        .flatten()
        .any(|text| search_text::contains(text, expected))
}

fn state_predicates_match(predicates: &[StatePredicate], states: &[String]) -> bool {
    predicates.iter().all(|predicate| {
        let present = state::has_state(states, &predicate.token);
        predicate.expected.unwrap_or(true) == present
    })
}

fn subtree_contains(query: &LocatorQuery, children: &[AccessibilityNode]) -> bool {
    children
        .iter()
        .any(|child| accessibility_node_matches(child, query))
}

#[cfg(test)]
#[path = "locator_tests.rs"]
mod tests;
