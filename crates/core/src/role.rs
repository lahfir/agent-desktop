#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Button,
    Cell,
    Checkbox,
    Colorwell,
    Combobox,
    Disclosure,
    Dockitem,
    Group,
    Image,
    Incrementor,
    Link,
    List,
    Menubutton,
    Menuitem,
    Radiobutton,
    Scrollarea,
    Slider,
    Statictext,
    Switch,
    Tab,
    Table,
    Textfield,
    Treeitem,
    Unknown,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Cell => "cell",
            Self::Checkbox => "checkbox",
            Self::Colorwell => "colorwell",
            Self::Combobox => "combobox",
            Self::Disclosure => "disclosure",
            Self::Dockitem => "dockitem",
            Self::Group => "group",
            Self::Image => "image",
            Self::Incrementor => "incrementor",
            Self::Link => "link",
            Self::List => "list",
            Self::Menubutton => "menubutton",
            Self::Menuitem => "menuitem",
            Self::Radiobutton => "radiobutton",
            Self::Scrollarea => "scrollarea",
            Self::Slider => "slider",
            Self::Statictext => "statictext",
            Self::Switch => "switch",
            Self::Tab => "tab",
            Self::Table => "table",
            Self::Textfield => "textfield",
            Self::Treeitem => "treeitem",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(role: &str) -> Self {
        role.parse().unwrap_or(Self::Unknown)
    }
}

impl std::str::FromStr for Role {
    type Err = std::convert::Infallible;

    fn from_str(role: &str) -> Result<Self, Self::Err> {
        Ok(match role.trim().to_ascii_lowercase().as_str() {
            "button" => Self::Button,
            "cell" => Self::Cell,
            "checkbox" => Self::Checkbox,
            "colorwell" => Self::Colorwell,
            "combobox" => Self::Combobox,
            "disclosure" => Self::Disclosure,
            "dockitem" => Self::Dockitem,
            "group" => Self::Group,
            "image" => Self::Image,
            "incrementor" => Self::Incrementor,
            "link" => Self::Link,
            "list" => Self::List,
            "menubutton" => Self::Menubutton,
            "menuitem" => Self::Menuitem,
            "radiobutton" => Self::Radiobutton,
            "scrollarea" => Self::Scrollarea,
            "slider" => Self::Slider,
            "statictext" => Self::Statictext,
            "switch" => Self::Switch,
            "tab" => Self::Tab,
            "table" => Self::Table,
            "textfield" => Self::Textfield,
            "treeitem" => Self::Treeitem,
            _ => Self::Unknown,
        })
    }
}

impl Role {
    pub fn is_interactive(self) -> bool {
        matches!(
            self,
            Self::Button
                | Self::Cell
                | Self::Checkbox
                | Self::Colorwell
                | Self::Combobox
                | Self::Dockitem
                | Self::Incrementor
                | Self::Link
                | Self::Menubutton
                | Self::Menuitem
                | Self::Radiobutton
                | Self::Slider
                | Self::Switch
                | Self::Tab
                | Self::Textfield
                | Self::Treeitem
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_unknown_for_bogus_role() {
        assert_eq!(Role::parse("bogus"), Role::Unknown);
    }

    #[test]
    fn is_interactive_matches_interactive_roles_list() {
        for role in crate::roles::INTERACTIVE_ROLES {
            assert!(
                Role::parse(role).is_interactive(),
                "{role} should be interactive"
            );
        }
    }
}
