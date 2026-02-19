pub fn ax_role_to_str(ax_role: &str) -> &'static str {
    match ax_role {
        "AXApplication" => "application",
        "AXButton" => "button",
        "AXTextField" | "AXTextArea" | "AXSearchField" => "textfield",
        "AXCheckBox" => "checkbox",
        "AXLink" => "link",
        "AXMenuItem" | "AXMenuBarItem" => "menuitem",
        "AXRadioButton" => "radiobutton",
        "AXTab" | "AXTabGroup" => "tab",
        "AXSlider" | "AXValueIndicator" => "slider",
        "AXComboBox" | "AXPopUpButton" => "combobox",
        "AXOutlineRow" | "AXRow" => "treeitem",
        "AXCell" => "cell",
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
        _ => "unknown",
    }
}

pub fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "button"
            | "textfield"
            | "checkbox"
            | "link"
            | "menuitem"
            | "tab"
            | "slider"
            | "combobox"
            | "treeitem"
            | "cell"
            | "radiobutton"
            | "incrementor"
    )
}
