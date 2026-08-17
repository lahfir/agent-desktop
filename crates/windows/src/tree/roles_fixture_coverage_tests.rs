use super::super::{FIXTURE_COVERED_ROLES, FIXTURE_UNCOVERED_ROLES};
use super::{ControlType, TreeProperty, flag, role_of};

/// Every role [`super::super::control_type_role`] can produce, enumerated by
/// driving each `ControlType` arm through the same [`role_of`] helper the
/// vocabulary tests use, with every refinement-gate flag combination that
/// changes the answer. This is the map's actual producible set, computed
/// rather than declared, so [`fixture_covered_and_uncovered_roles_union_to_the_map_producible_set`]
/// below is checking the pin against reality rather than against another
/// hand-written list that could drift the same way.
fn all_producible_roles() -> Vec<String> {
    vec![
        role_of(ControlType::Button, Vec::new()),
        role_of(
            ControlType::Button,
            vec![flag(TreeProperty::ToggleAvailable, true)],
        ),
        role_of(
            ControlType::Button,
            vec![flag(TreeProperty::ExpandCollapseAvailable, true)],
        ),
        role_of(ControlType::Calendar, Vec::new()),
        role_of(ControlType::CheckBox, Vec::new()),
        role_of(ControlType::ComboBox, Vec::new()),
        role_of(ControlType::Edit, Vec::new()),
        role_of(ControlType::Hyperlink, Vec::new()),
        role_of(ControlType::Image, Vec::new()),
        role_of(ControlType::ListItem, Vec::new()),
        role_of(ControlType::List, Vec::new()),
        role_of(
            ControlType::List,
            vec![flag(TreeProperty::SelectionAvailable, true)],
        ),
        role_of(ControlType::Menu, Vec::new()),
        role_of(ControlType::MenuBar, Vec::new()),
        role_of(ControlType::MenuItem, Vec::new()),
        role_of(ControlType::ProgressBar, Vec::new()),
        role_of(ControlType::RadioButton, Vec::new()),
        role_of(ControlType::ScrollBar, Vec::new()),
        role_of(ControlType::Slider, Vec::new()),
        role_of(ControlType::Spinner, Vec::new()),
        role_of(ControlType::StatusBar, Vec::new()),
        role_of(ControlType::Tab, Vec::new()),
        role_of(ControlType::TabItem, Vec::new()),
        role_of(ControlType::Text, Vec::new()),
        role_of(ControlType::ToolBar, Vec::new()),
        role_of(ControlType::ToolTip, Vec::new()),
        role_of(ControlType::Tree, Vec::new()),
        role_of(ControlType::TreeItem, Vec::new()),
        role_of(ControlType::Custom, Vec::new()),
        role_of(
            ControlType::Custom,
            vec![flag(TreeProperty::GridItemAvailable, true)],
        ),
        role_of(
            ControlType::Custom,
            vec![flag(TreeProperty::TableItemAvailable, true)],
        ),
        role_of(ControlType::Group, Vec::new()),
        role_of(ControlType::Thumb, Vec::new()),
        role_of(ControlType::DataGrid, Vec::new()),
        role_of(ControlType::DataItem, Vec::new()),
        role_of(
            ControlType::DataItem,
            vec![flag(TreeProperty::GridItemAvailable, true)],
        ),
        role_of(
            ControlType::DataItem,
            vec![flag(TreeProperty::TableItemAvailable, true)],
        ),
        role_of(ControlType::Document, Vec::new()),
        role_of(
            ControlType::Document,
            vec![
                flag(TreeProperty::ValueAvailable, true),
                flag(TreeProperty::ValueIsReadOnly, false),
            ],
        ),
        role_of(ControlType::SplitButton, Vec::new()),
        role_of(ControlType::Window, Vec::new()),
        role_of(ControlType::Pane, Vec::new()),
        role_of(
            ControlType::Pane,
            vec![flag(TreeProperty::WindowAvailable, true)],
        ),
        role_of(ControlType::Pane, vec![flag(TreeProperty::IsDialog, true)]),
        role_of(ControlType::Header, Vec::new()),
        role_of(ControlType::HeaderItem, Vec::new()),
        role_of(ControlType::Table, Vec::new()),
        role_of(ControlType::TitleBar, Vec::new()),
        role_of(ControlType::Separator, Vec::new()),
        role_of(ControlType::SemanticZoom, Vec::new()),
        role_of(ControlType::AppBar, Vec::new()),
    ]
}

/// The fixture-coverage decision: every role the map can produce must be
/// classified as either required-of-the-fixture or explicitly out of scope,
/// with nothing left uncategorized. A `ControlType` arm added later that
/// yields a role in neither set fails this test rather than silently
/// escaping both the fixture's assertions and this pin.
#[test]
fn fixture_covered_and_uncovered_roles_union_to_the_map_producible_set() {
    let mut produced = all_producible_roles();
    produced.sort();
    produced.dedup();

    let mut declared: Vec<String> = FIXTURE_COVERED_ROLES
        .iter()
        .chain(FIXTURE_UNCOVERED_ROLES.iter())
        .map(|role| role.to_string())
        .collect();
    declared.sort();
    declared.dedup();

    assert_eq!(
        declared, produced,
        "FIXTURE_COVERED_ROLES union FIXTURE_UNCOVERED_ROLES must equal exactly the roles control_type_role can produce"
    );
}

#[test]
fn fixture_covered_roles_are_sorted_unique_and_disjoint_from_uncovered() {
    let mut sorted = FIXTURE_COVERED_ROLES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), FIXTURE_COVERED_ROLES);
    for role in FIXTURE_COVERED_ROLES {
        assert!(
            !FIXTURE_UNCOVERED_ROLES.contains(role),
            "{role} is declared in both FIXTURE_COVERED_ROLES and FIXTURE_UNCOVERED_ROLES"
        );
    }
}

#[test]
fn fixture_uncovered_roles_are_sorted_and_unique() {
    let mut sorted = FIXTURE_UNCOVERED_ROLES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.as_slice(), FIXTURE_UNCOVERED_ROLES);
}
