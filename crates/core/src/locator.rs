use crate::{
    error::{AdapterError, ErrorCode},
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

/// Fields that identify a candidate element: role plus its text-bearing
/// evidence. Grouped out of [`LocatorQuery`] so the query stays under the
/// repo's field-count limit; flattened back onto the wire so JSON callers
/// still see a single flat object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct IdentityPredicate {
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
}

/// Subtree-containment predicates: recursive sub-queries a matching element's
/// descendants must (or must not) satisfy. Flattened back onto the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ContainmentPredicate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has: Option<Box<LocatorQuery>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_not: Option<Box<LocatorQuery>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocatorQuery {
    #[serde(flatten)]
    pub identity: IdentityPredicate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_text: Option<String>,
    #[serde(default)]
    pub exact: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<StatePredicate>,
    #[serde(flatten)]
    pub containment: ContainmentPredicate,
}

impl LocatorQuery {
    pub fn is_empty(&self) -> bool {
        self.identity.role.is_none()
            && self.identity.name.is_none()
            && self.identity.description.is_none()
            && self.identity.native_id.is_none()
            && self.identity.value.is_none()
            && self.has_text.is_none()
            && self.states.is_empty()
            && self.containment.has.is_none()
            && self.containment.has_not.is_none()
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
        if let Some(has) = &self.containment.has {
            has.validate_states()?;
        }
        if let Some(has_not) = &self.containment.has_not {
            has_not.validate_states()?;
        }
        Ok(())
    }
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
    if !text_field_matches(query.identity.name.as_deref(), ctx.name, query.exact) {
        return false;
    }
    if !text_field_matches(
        query.identity.description.as_deref(),
        ctx.description,
        query.exact,
    ) {
        return false;
    }
    if !native_id_matches(query.identity.native_id.as_deref(), ctx.native_id) {
        return false;
    }
    if !text_field_matches(query.identity.value.as_deref(), ctx.value, query.exact) {
        return false;
    }
    if !has_text_matches(query.has_text.as_deref(), &ctx) {
        return false;
    }
    if !state_predicates_match(&query.states, ctx.states) {
        return false;
    }
    if let Some(has) = &query.containment.has {
        if !subtree_contains(has, ctx.children) {
            return false;
        }
    }
    if let Some(has_not) = &query.containment.has_not {
        if subtree_contains(has_not, ctx.children) {
            return false;
        }
    }
    true
}

pub fn accessibility_node_matches(node: &AccessibilityNode, query: &LocatorQuery) -> bool {
    node_matches(query, node_context(node))
}

/// Cheap standalone role predicate, factored out of [`node_matches`] so
/// callers building a full match context (states, children, name/value
/// text) can short-circuit before paying for that work when the role alone
/// already rules a candidate out.
pub fn role_matches(query: &LocatorQuery, role: &str) -> bool {
    query
        .identity
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
