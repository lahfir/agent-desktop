pub fn ax_role_to_str(ax_role: &str) -> &'static str {
    match ax_role {
        "AXApplication" => "application",
        "AXButton" => "button",
        "AXMenuButton" => "menubutton",
        "AXTextField" | "AXTextArea" | "AXSearchField" => "textfield",
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

pub fn is_interactive_role(role: &str) -> bool {
    matches!(
        role,
        "button"
            | "menubutton"
            | "textfield"
            | "checkbox"
            | "switch"
            | "link"
            | "menuitem"
            | "tab"
            | "slider"
            | "combobox"
            | "treeitem"
            | "cell"
            | "radiobutton"
            | "incrementor"
            | "colorwell"
            | "dockitem"
    )
}
