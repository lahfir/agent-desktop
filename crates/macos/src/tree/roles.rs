pub fn ax_role_to_str(ax_role: &str) -> &'static str {
    match ax_role {
        "AXApplication" => "application",
        "AXButton" => "button",
        "AXMenuButton" => "menubutton",
        "AXTextField" | "AXTextArea" | "AXSearchField" | "AXSecureTextField" => "textfield",
        "AXCheckBox" => "checkbox",
        "AXSwitch" | "AXToggle" => "switch",
        "AXLink" => "link",
        "AXMenuItem" | "AXMenuBarItem" => "menuitem",
        "AXRadioButton" => "radiobutton",
        "AXTab" | "AXTabGroup" => "tab",
        "AXSlider" | "AXValueIndicator" => "slider",
        "AXComboBox" | "AXPopUpButton" => "combobox",
        "AXOutlineRow" | "AXRow" => "treeitem",
        "AXCell" => "cell",
        "AXColumn" => "column",
        "AXWindow" => "window",
        "AXSheet" => "sheet",
        "AXDialog" => "dialog",
        "AXGroup" | "AXGenericElement" => "group",
        "AXToolbar" => "toolbar",
        "AXStaticText" => "statictext",
        "AXImage" => "image",
        "AXTable" => "table",
        "AXList" => "list",
        "AXOutline" => "outline",
        "AXScrollArea" | "AXScrollBar" => "scrollarea",
        "AXSplitter" | "AXSplitGroup" => "splitter",
        "AXMenu" | "AXMenuBar" => "menu",
        "AXIncrementor" | "AXStepper" => "incrementor",
        "AXDisclosureTriangle" => "disclosure",
        "AXProgressIndicator" | "AXBusyIndicator" => "progressbar",
        "AXColorWell" => "colorwell",
        "AXWebArea" => "webarea",
        "AXBrowser" => "browser",
        "AXGrid" => "grid",
        "AXHandle" => "handle",
        "AXPopover" => "popover",
        "AXDockItem" => "dockitem",
        "AXRuler" => "ruler",
        "AXRulerMarker" => "rulermarker",
        "AXTimeField" => "timefield",
        "AXDateField" => "datefield",
        "AXHelpTag" => "helptag",
        "AXMatte" => "matte",
        "AXDrawer" => "drawer",
        "AXLayoutArea" | "AXLayoutItem" => "layoutitem",
        "AXLevelIndicator" => "levelindicator",
        "AXRelevanceIndicator" => "relevanceindicator",
        _ => "unknown",
    }
}

pub fn normalized_role_and_label(
    el: &crate::tree::AXElement,
    ax_role: Option<&str>,
) -> (String, Option<String>) {
    let promoted_label = promoted_item_label(ax_role, el);
    let role = if promoted_label.is_some() {
        "cell"
    } else {
        ax_role.map(ax_role_to_str).unwrap_or("unknown")
    };
    (role.to_string(), promoted_label)
}

pub fn promoted_item_label(ax_role: Option<&str>, el: &crate::tree::AXElement) -> Option<String> {
    if ax_role != Some("AXGroup") {
        return None;
    }
    let children = crate::tree::element::child_attributes(ax_role)
        .iter()
        .find_map(|attr| {
            crate::tree::copy_ax_array(el, attr).filter(|children| !children.is_empty())
        })
        .unwrap_or_default();
    let has_icon = children
        .iter()
        .any(|child| crate::tree::copy_string_attr(child, "AXRole").as_deref() == Some("AXImage"));
    if !has_icon {
        return None;
    }
    children.iter().find_map(|child| {
        if crate::tree::copy_string_attr(child, "AXRole").as_deref() == Some("AXTextField") {
            crate::tree::copy_string_attr(child, "AXValue").filter(|value| !value.is_empty())
        } else {
            None
        }
    })
}

pub use agent_desktop_core::roles::is_toggleable_role;
