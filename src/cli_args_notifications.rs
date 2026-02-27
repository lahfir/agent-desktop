use clap::Parser;

#[derive(Parser, Debug)]
pub struct ListNotificationsCliArgs {
    #[arg(long, help = "Filter to notifications from this app")]
    pub app: Option<String>,
    #[arg(long, help = "Filter to notifications containing this text")]
    pub text: Option<String>,
    #[arg(long, help = "Maximum number of notifications to return")]
    pub limit: Option<usize>,
}

#[derive(Parser, Debug)]
pub struct DismissNotificationCliArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications",
          value_parser = clap::value_parser!(u64).range(1..))]
    pub index: u64,
    #[arg(long, help = "Filter notifications by app before selecting index")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct DismissAllNotificationsCliArgs {
    #[arg(long, help = "Only dismiss notifications from this app")]
    pub app: Option<String>,
}

#[derive(Parser, Debug)]
pub struct NotificationActionCliArgs {
    #[arg(value_name = "INDEX", help = "1-based notification index from list-notifications",
          value_parser = clap::value_parser!(u64).range(1..))]
    pub index: u64,
    #[arg(
        value_name = "ACTION",
        help = "Name of the action button to click (e.g., Reply, Open)"
    )]
    pub action: String,
}
